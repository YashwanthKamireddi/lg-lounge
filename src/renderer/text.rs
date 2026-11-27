//! Text: glyph meshes, runs and their caches.
use super::*;

/// Stem emboldening for small text, in physical pixels per edge: thin stems
/// get an opaque core instead of only grey edge pixels, which the TV's 2x
/// scale-up to 4K softens further. Full strength up to 16 physical pixels,
/// none from 28 (upstream Lounge 3359618).
pub(super) fn embolden(physical_size: f32) -> f32 {
    0.25 * ((28. - physical_size) / 12.).clamp(0., 1.)
}

/// The big clock's own figures (tools/font-weights.py), tall and narrow.
pub(super) const CLOCK_FONT: u16 = 3000;
/// Scripts the interface's font does not draw, each in its Noto cut, shaped (rustybuzz).
const TELUGU_FONT: u16 = 3001;
const DEVANAGARI_FONT: u16 = 3002;
/// Shaped glyphs are cached by id under these private-use characters.
const SHAPED_BASE: u32 = 0xF0000;

/// The font a character's script needs, where the interface's own cannot draw it.
fn script_font(c: char) -> Option<u16> {
    match c as u32 {
        0x0C00..=0x0C7F => Some(TELUGU_FONT),
        0x0900..=0x097F | 0xA8E0..=0xA8FF => Some(DEVANAGARI_FONT),
        _ => None,
    }
}

/// A line split where its script changes: each run with the font it needs (None: the
/// interface's). Joiners and spaces stay with the run they are in, so shaping sees whole words.
fn script_runs(line: &str) -> Vec<(Option<u16>, String)> {
    let mut runs: Vec<(Option<u16>, String)> = Vec::new();
    for c in line.chars() {
        let font = match script_font(c) {
            Some(font) => Some(font),
            // Spaces, joiners and marks continue the run they are in.
            None if c.is_whitespace() || matches!(c as u32, 0x200C | 0x200D) => {
                runs.last().and_then(|(font, _)| *font)
            }
            None => None,
        };
        match runs.last_mut() {
            Some((last, text)) if *last == font => text.push(c),
            _ => runs.push((font, c.to_string())),
        }
    }
    runs
}

pub(super) fn load_font(directory: &Path, id: u16) -> Result<Font, String> {
    let name = if id == CLOCK_FONT {
        "font-clock.ttf".to_owned()
    } else if id == TELUGU_FONT {
        "font-telugu.ttf".to_owned()
    } else if id == DEVANAGARI_FONT {
        "font-devanagari.ttf".to_owned()
    } else if id >= 1000 {
        format!("font-display-{}.ttf", id - 1000)
    } else {
        format!("font-{id}.ttf")
    };
    let path = directory.join(&name);
    let started = Instant::now();
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("Font {}: {e}. Regenerate native assets.", path.display()))?;
    let font = Font::from_bytes(bytes).map_err(|e| format!("Font {}: {e}", path.display()))?;
    let font = if id == CLOCK_FONT { font.fine() } else { font };
    if cfg!(feature = "webos") {
        eprintln!(
            "Lounge font: {name} loaded in {} ms",
            started.elapsed().as_millis()
        );
    }
    Ok(font)
}

impl Renderer {
    pub(super) fn ensure_font(&mut self, id: u16) -> bool {
        if self.fonts.contains_key(&id) {
            return true;
        }
        if self.failed_fonts.contains(&id) {
            return false;
        }
        match load_font(&self.font_directory, id) {
            Ok(font) => {
                self.fonts.insert(id, font);
                true
            }
            Err(error) => {
                eprintln!("{error}");
                self.failed_fonts.insert(id);
                false
            }
        }
    }

    pub(super) fn glyph(
        &mut self,
        c: char,
        font: u16,
    ) -> Result<&[lounge_vector_geometry::Vertex], String> {
        let key = (c, font);
        if !self.glyphs.contains_key(&key) {
            let geometry = match (c as u32).checked_sub(SHAPED_BASE) {
                Some(id) => self.fonts[&font].glyph_geometry(id as u16)?,
                None => self.fonts[&font].geometry(c)?,
            };
            self.glyphs.insert(
                key,
                Glyph {
                    geometry,
                    frame: self.frame,
                },
            );
            self.stats.glyph_tessellations += 1;
        }
        let glyph = self.glyphs.get_mut(&key).unwrap();
        glyph.frame = self.frame;
        Ok(&glyph.geometry)
    }

    /// Prepare not-yet-cached glyphs of the printable ASCII, for up to `budget`,
    /// set in the faces the interface uses, so a screen opened for the first
    /// time never tessellates its text mid-frame. Returns whether any work
    /// remains. Meant for idle ticks only.
    pub fn warm_glyphs(&mut self, budget: std::time::Duration) -> bool {
        let started = std::time::Instant::now();
        // Font ids are the weight, +1000 for display sizes (>= 32pt).
        const FACES: [u16; 6] = [400, 500, 600, 650, 1500, 1650];
        let mut done = 0;
        for font in FACES {
            if !self.ensure_font(font) {
                continue;
            }
            for c in ' '..='~' {
                if self.glyphs.contains_key(&(c, font)) {
                    continue;
                }
                if done > 0 && started.elapsed() >= budget {
                    return true;
                }
                if let Ok(geometry) = self.fonts[&font].geometry(c) {
                    self.glyphs.insert(
                        (c, font),
                        Glyph {
                            geometry,
                            frame: self.frame,
                        },
                    );
                } else {
                    self.glyphs.insert(
                        (c, font),
                        Glyph {
                            geometry: Vec::new(),
                            frame: self.frame,
                        },
                    );
                }
                done += 1;
            }
        }
        false
    }

    pub(super) fn trim_text_geometry(&mut self) {
        let glyph_bytes =
            |glyph: &Glyph| glyph.geometry.len() * size_of::<lounge_vector_geometry::Vertex>();
        let mut bytes: usize = self.glyphs.values().map(glyph_bytes).sum();
        if bytes > GLYPH_GEOMETRY_BUDGET || self.glyphs.len() > 4096 {
            let mut old: Vec<_> = self
                .glyphs
                .iter()
                .filter(|(_, glyph)| glyph.frame != self.frame)
                .map(|(key, glyph)| (*key, glyph.frame))
                .collect();
            old.sort_by_key(|(_, frame)| *frame);
            for (key, _) in old {
                if bytes <= GLYPH_GEOMETRY_BUDGET && self.glyphs.len() <= 4096 {
                    break;
                }
                if let Some(glyph) = self.glyphs.remove(&key) {
                    bytes -= glyph_bytes(&glyph);
                }
            }
        }
        let run_bytes = |run: &TextRun| run.mesh.as_ref().map_or(0, |mesh| mesh.bytes);
        let mut bytes: usize = self.text_runs.values().map(run_bytes).sum();
        if bytes > TEXT_GEOMETRY_BUDGET || self.text_runs.len() > 512 {
            let mut old: Vec<_> = self
                .text_runs
                .iter()
                .filter(|(_, run)| run.frame != self.frame)
                .map(|(key, run)| (key.clone(), run.frame))
                .collect();
            old.sort_by_key(|(_, frame)| *frame);
            for (key, _) in old {
                if bytes <= TEXT_GEOMETRY_BUDGET && self.text_runs.len() <= 512 {
                    break;
                }
                if let Some(run) = self.text_runs.remove(&key) {
                    bytes -= run_bytes(&run);
                }
            }
        }
        self.stats.cached_glyphs = self.glyphs.len();
        self.stats.glyph_geometry_bytes = self.glyphs.values().map(glyph_bytes).sum();
        self.stats.cached_text_runs = self.text_runs.len();
        self.stats.text_geometry_bytes = self.text_runs.values().map(run_bytes).sum();
    }

    pub(super) fn text(
        &mut self,
        text: &str,
        rect: Rect,
        style: TextStyle,
        color: Color,
        centered: bool,
    ) {
        // Layout remains logical. Reuse the same run geometry at every density;
        // its shader snaps insertion origins and computes pixel-sized coverage.
        let size_f = style.size;
        let viewport = self.stats.viewport;
        self.stats.max_logical_font_px = self.stats.max_logical_font_px.max(size_f);
        self.stats.max_physical_font_px = self
            .stats
            .max_physical_font_px
            .max(size_f * style.scale * viewport.text_scale());
        let padded = Rect::new(rect.x, rect.y, rect.w * style.scale, rect.h * style.scale)
            .inset(-size_f * style.scale);
        if padded.x + padded.w < 0.
            || padded.y + padded.h < 0.
            || padded.x > 1920.
            || padded.y > 1080.
            || self.clip.is_some_and(|clip| {
                padded.x + padded.w < clip.x
                    || padded.y + padded.h < clip.y
                    || padded.x > clip.x + clip.w
                    || padded.y > clip.y + clip.h
            })
        {
            return;
        }
        let origin = [rect.x, rect.y];
        let key = TextKey {
            text: text.into(),
            weight: style.weight,
            layout: [
                size_f.to_bits(),
                style.tracking.to_bits(),
                style.line_height.to_bits(),
                rect.w.to_bits(),
                rect.h.to_bits(),
            ],
            flags: u8::from(style.align_right)
                | (u8::from(centered) << 1)
                | (u8::from(style.clock) << 3)
                | (u8::from(self.stats.multisample_samples > 1) << 2),
        };
        if self.text_runs.contains_key(&key) {
            self.submit_text(key, origin, style.scale, color);
            return;
        }
        let rect = Rect::new(0., 0., rect.w, rect.h);
        let font_id = if style.clock {
            CLOCK_FONT
        } else {
            style.weight + if style.size >= 32. { 1000 } else { 0 }
        };
        if !self.ensure_font(font_id) {
            self.stats.glyph_failures += text.chars().count();
            return;
        }
        let mut scripts: Vec<u16> = text.chars().filter_map(script_font).collect();
        scripts.dedup();
        for script in scripts {
            self.ensure_font(script);
        }
        let font = &self.fonts[&font_id];
        let fonts = &self.fonts;
        let line_height = size_f * style.line_height;
        let baseline = font.horizontal_line_metrics(size_f).map_or(size_f, |m| {
            (line_height - m.ascent + m.descent) / 2. + m.ascent
        });
        let tracking = style.tracking * size_f;
        let advance = |c| {
            if c == '\n' {
                0.
            } else if let Some(script) = script_font(c).and_then(|id| fonts.get(&id)) {
                // Near enough to wrap by: shaping only joins letters into fewer glyphs.
                script.metrics(c, size_f).advance_width
            } else {
                font.metrics(c, size_f).advance_width + tracking
            }
        };
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        let mut width = 0.;
        // Word wrapping, with character fallback for a title wider than the tile.
        for word in text.split_inclusive(char::is_whitespace) {
            let word_width: f32 = word.chars().map(advance).sum();
            if width > 0. && width + word_width > rect.w {
                lines.push(line.trim_end().into());
                line.clear();
                width = 0.;
            }
            for c in word.chars() {
                if c == '\n' {
                    lines.push(line.trim_end().into());
                    line.clear();
                    width = 0.;
                    continue;
                }
                let w = advance(c);
                if width > 0. && width + w > rect.w {
                    lines.push(line.trim_end().into());
                    line.clear();
                    width = 0.;
                }
                line.push(c);
                width += w;
            }
        }
        if !line.is_empty() {
            lines.push(line.trim_end().into());
        }
        let mut placed = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let top = rect.y + index as f32 * line_height;
            if index > 0 && top + line_height > rect.y + rect.h + 1. {
                break;
            }
            // Laid out from 0, then moved into place once its width is known.
            let mut line_glyphs = Vec::new();
            let mut x = 0.;
            for (script, run) in script_runs(line) {
                match script.and_then(|id| fonts.get(&id).map(|face| (id, face))) {
                    Some((id, face)) => {
                        // Shaped: conjuncts, vowel signs and reordering as the script needs.
                        let Some(shaper) = rustybuzz::Face::from_slice(face.bytes(), 0) else {
                            continue;
                        };
                        let mut buffer = rustybuzz::UnicodeBuffer::new();
                        buffer.push_str(&run);
                        let output = rustybuzz::shape(&shaper, &[], buffer);
                        let scale = size_f / f32::from(shaper.units_per_em() as u16);
                        for (info, position) in
                            output.glyph_infos().iter().zip(output.glyph_positions())
                        {
                            let key =
                                char::from_u32(SHAPED_BASE + info.glyph_id).unwrap_or('\u{FFFD}');
                            line_glyphs.push((
                                key,
                                id,
                                x + position.x_offset as f32 * scale,
                                -(position.y_offset as f32) * scale,
                            ));
                            x += position.x_advance as f32 * scale;
                        }
                    }
                    None => {
                        let mut previous = None;
                        for c in run.chars() {
                            if let Some(previous) = previous {
                                x += font.horizontal_kern(previous, c, size_f).unwrap_or(0.);
                            }
                            line_glyphs.push((c, font_id, x, 0.));
                            x += advance(c);
                            previous = Some(c);
                        }
                    }
                }
            }
            let width = x;
            let shift = rect.x
                + if style.align_right {
                    rect.w - width
                } else if centered {
                    (rect.w - width) / 2.
                } else {
                    0.
                };
            for (c, id, x, rise) in line_glyphs {
                placed.push((c, id, shift + x, top + baseline + rise));
            }
        }
        let mut vertices = Vec::new();
        let mut missing_glyphs = 0;
        let mut failures = 0;
        let multisampled = self.stats.multisample_samples > 1;
        for (c, glyph_font, x, baseline) in placed {
            if glyph_font == font_id
                && self.fonts[&font_id].lookup_glyph_index(c) == 0
                && !c.is_whitespace()
            {
                missing_glyphs += 1;
            }
            match self.glyph(c, glyph_font) {
                Ok(geometry) => {
                    for triangle in geometry.as_chunks::<3>().0 {
                        // True sample coverage needs only the original fill.
                        // Exclude the boundary fringe, not just its alpha.
                        if multisampled && triangle.iter().any(|vertex| vertex.coverage < 1.) {
                            continue;
                        }
                        for vertex in triangle {
                            vertices.extend_from_slice(&[
                                vertex.position.x * size_f,
                                vertex.position.y * size_f,
                                if multisampled { 0. } else { vertex.offset[0] },
                                if multisampled { 0. } else { vertex.offset[1] },
                                x,
                                baseline,
                                0.,
                                0.,
                                vertex.coverage,
                            ]);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("Lounge glyph {c:?}: {error}");
                    failures += 1;
                }
            }
        }
        let mesh = if vertices.is_empty() {
            None
        } else {
            match vector::Mesh::upload(1., 1., &vertices) {
                Ok(mesh) => Some(mesh),
                Err(error) => {
                    eprintln!("Lounge text: {error}");
                    failures += 1;
                    None
                }
            }
        };
        self.text_runs.insert(
            key.clone(),
            TextRun {
                mesh,
                missing_glyphs,
                failures,
                frame: self.frame,
            },
        );
        self.submit_text(key, origin, style.scale, color);
    }

    pub(super) fn submit_text(&mut self, key: TextKey, origin: [f32; 2], scale: f32, tint: Color) {
        let run = self.text_runs.get_mut(&key).unwrap();
        run.frame = self.frame;
        self.stats.missing_glyphs += run.missing_glyphs;
        self.stats.glyph_failures += run.failures;
        if let Some(mesh) = &run.mesh {
            self.stats.text_triangles += mesh.vertices as usize / 3;
            self.batches.push(Batch {
                kind: BatchKind::Text {
                    key,
                    origin,
                    scale,
                    tint,
                    glass: self.text_glass,
                },
                clip: self.clip,
                fade: self.fade,
                rank: self.rank,
            });
        }
    }
}
