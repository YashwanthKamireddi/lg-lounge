varying PAINT_PRECISION vec4 v_color;
varying PAINT_PRECISION float v_coverage;
varying PAINT_PRECISION float v_logical_y;
uniform vec4 u_edge_fade;
#ifdef GLASS_TEXT
// Liquid Glass lettering: the pre-blurred backdrop behind each glyph, lit by the tint (above 1
// brightens) and frosted toward white, so the figures read as glass over the picture.
uniform sampler2D u_texture;
varying PAINT_PRECISION float v_logical_x;
#endif
void main() {
    float edge = 1.0;
    if (u_edge_fade.y > u_edge_fade.x) {
        edge = smoothstep(u_edge_fade.x, u_edge_fade.y, v_logical_y) *
            (1.0 - smoothstep(u_edge_fade.z, u_edge_fade.w, v_logical_y));
    }
#ifdef GLASS_TEXT
    vec3 behind = texture2D(u_texture, vec2(v_logical_x / 1920.0, v_logical_y / 1080.0)).rgb;
    vec3 glass = mix(min(behind * v_color.rgb, vec3(1.0)), vec3(1.0), 0.55);
    gl_FragColor = vec4(glass, v_color.a * clamp(v_coverage, 0.0, 1.0) * edge);
#else
    gl_FragColor = vec4(v_color.rgb, v_color.a * clamp(v_coverage, 0.0, 1.0) * edge);
#endif
}
