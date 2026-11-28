"""Freeze the bundled variable font at the UI's weights and optical sizes.

With a second argument, that variable font (Apple's SF Pro, kept untracked in
local/fonts) is first cut to the text Lounge shows: Latin and Latin Extended,
at its regular width, without the symbol glyphs it also carries.
"""
import sys
import json
from pathlib import Path
from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

directory = Path(sys.argv[1])
if len(sys.argv) > 2:
    source = TTFont(sys.argv[2])
    keep = set(range(0x20, 0x250)) | set(range(0x250, 0x2B0)) | set(range(0x2B0, 0x370))
    keep |= set(range(0x1E00, 0x1F00)) | set(range(0x2000, 0x2070)) | set(range(0x20A0, 0x20D0))
    keep |= set(range(0x2100, 0x2200)) | set(range(0x2190, 0x2200)) | {0x2022, 0x25CF, 0xFEFF}
    options = subset.Options()
    options.layout_features = ["calt", "ccmp", "dnom", "frac", "kern", "locl", "mark", "mkmk", "numr", "pnum", "tnum"]
    options.name_IDs = ["*"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=keep & set(source.getBestCmap()))
    subsetter.subset(source)
    if "fvar" in source and "wdth" in {axis.axisTag for axis in source["fvar"].axes}:
        source = instantiateVariableFont(source, {"wdth": 100})
    source.save(directory / "font.ttf")
font = TTFont(directory / "font.ttf")
axes = {axis.axisTag for axis in font["fvar"].axes} if "fvar" in font else set()
metrics = {}
for weight in (350, 400, 450, 500, 550, 600, 650, 800):
    for display in (False, True):
        values = {"wght": weight} if "wght" in axes else {}
        if "opsz" in axes:
            # Text at the axis's small end, display (titles) at its large end.
            opsz = next(axis for axis in font["fvar"].axes if axis.axisTag == "opsz")
            values["opsz"] = opsz.maxValue if display else opsz.minValue
        instance = instantiateVariableFont(font, values, inplace=False) if axes else font
        instance.save(directory / f"font-{'display-' if display else ''}{weight}.ttf")
        units = instance["head"].unitsPerEm
        advances = instance["hmtx"].metrics
        metrics[f"{'display-' if display else ''}{weight}"] = {
            chr(codepoint): advances[glyph][0] / units
            for codepoint, glyph in instance.getBestCmap().items()
        }
# The big clock's figures (src/view/clock.rs): tall and narrow, as the Lock Screen's, cut from
# the condensed end of SF Pro's width axis; with Inter (no width axis) its display weight.
if len(sys.argv) > 2:
    clock = TTFont(sys.argv[2])
    options = subset.Options()
    options.layout_features = ["tnum", "kern"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text="0123456789: ")
    subsetter.subset(clock)
    clock = instantiateVariableFont(clock, {"wdth": 46, "wght": 620, "opsz": 28})
    # A family of its own: the screen saver's Qt picks fonts by family name, and under SF Pro's
    # it took the regular width (tv/screensaver.qml).
    for record in list(clock["name"].names):
        if record.nameID in (1, 4, 6, 16, 17):
            clock["name"].removeNames(nameID=record.nameID)
    for name_id, value in ((1, "Lounge Clock"), (4, "Lounge Clock"), (6, "LoungeClock")):
        clock["name"].setName(value, name_id, 3, 1, 0x409)
        clock["name"].setName(value, name_id, 1, 0, 0)
    clock.save(directory / "font-clock.ttf")
else:
    (directory / "font-clock.ttf").write_bytes((directory / "font-display-500.ttf").read_bytes())
# The Contour screen saver's figures (tv/screensaver.qml): six cuts of the digits from narrow
# and light to wide and heavy, the hour at the hand in the widest, as the watch's Contour
# figures swell. SF Pro varies width and weight; Inter, weight only. Each is a family of its
# own, as Qt picks fonts by family name.
for level in range(6):
    t = level / 5
    if len(sys.argv) > 2:
        cut = TTFont(sys.argv[2])
        values = {"wdth": 76 + 54 * t, "wght": 480 + 380 * t, "opsz": 28}
    else:
        cut = TTFont(directory / "font.ttf")
        cut_axes = {axis.axisTag for axis in cut["fvar"].axes} if "fvar" in cut else set()
        values = {"wght": 380 + 420 * t} if "wght" in cut_axes else {}
        if "opsz" in cut_axes:
            values["opsz"] = next(a for a in cut["fvar"].axes if a.axisTag == "opsz").maxValue
    if values:
        cut = instantiateVariableFont(cut, values)
    options = subset.Options()
    options.layout_features = ["kern"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text="0123456789")
    subsetter.subset(cut)
    family = f"Lounge Contour {level}"
    for record in list(cut["name"].names):
        if record.nameID in (1, 2, 4, 6, 16, 17):
            cut["name"].removeNames(nameID=record.nameID)
    for name_id, value in ((1, family), (2, "Regular"), (4, family), (6, family.replace(" ", ""))):
        cut["name"].setName(value, name_id, 3, 1, 0x409)
        cut["name"].setName(value, name_id, 1, 0, 0)
    cut.save(directory / f"font-contour-{level}.ttf")
# Scripts SF Pro and Inter do not draw, shaped with rustybuzz (src/renderer/text.rs): Google's
# Noto Sans (OFL), one semibold cut each, as the interface's titles are.
fonts = Path(__file__).resolve().parent.parent / "assets" / "fonts"
for script in ("telugu", "devanagari", "tamil", "kannada", "malayalam", "bengali"):
    noto = TTFont(fonts / f"noto-sans-{script}.ttf")
    noto = instantiateVariableFont(noto, {"wght": 600, "wdth": 100})
    noto.save(directory / f"font-{script}.ttf")
(directory / "font-metrics.json").write_text(json.dumps(metrics, ensure_ascii=False), encoding="utf8")
