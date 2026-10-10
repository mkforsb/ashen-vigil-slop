//! GLSL ES 3.00 shaders emulating the PSX GPU/GTE look.

pub const MAX_LIGHTS: usize = 8;

pub const WORLD_VS: &str = r#"#version 300 es
precision highp float;
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec2 a_uv;
layout(location=3) in vec4 a_color;
layout(location=4) in vec4 a_tex;

uniform mat4 u_model;
uniform mat4 u_view;
uniform mat4 u_proj;
uniform vec4 u_tint;
uniform vec2 u_res;
uniform float u_snap;
uniform vec3 u_ambient;
uniform vec3 u_sun_dir;
uniform vec3 u_sun_color;
uniform vec4 u_lpos[8];
uniform vec4 u_lcol[8];
uniform int u_nlights;
uniform vec2 u_fog;
uniform vec2 u_uv_offset;

out vec2 v_uv;
out vec3 v_uv_aff;
out vec4 v_color;
out float v_fog;
flat out float v_layer;

void main() {
    vec4 world = u_model * vec4(a_pos, 1.0);
    vec3 n = normalize(mat3(u_model) * a_normal);
    vec4 viewp = u_view * world;
    vec4 clip = u_proj * viewp;
    // GTE-style vertex snapping to the low-res pixel grid.
    if (u_snap > 0.0 && clip.w > 0.0) {
        vec2 grid = u_res * 0.5 / u_snap;
        clip.xy = floor(clip.xy / clip.w * grid + 0.5) / grid * clip.w;
    }
    gl_Position = clip;

    bool unlit = mod(a_tex.y, 2.0) >= 1.0;
    bool nofog = mod(floor(a_tex.y / 2.0), 2.0) >= 1.0;
    vec3 light;
    if (unlit) {
        light = vec3(1.0);
    } else {
        light = u_ambient + u_sun_color * max(dot(n, -u_sun_dir), 0.0);
        for (int i = 0; i < 8; i++) {
            if (i >= u_nlights) break;
            vec3 d = u_lpos[i].xyz - world.xyz;
            float dist = length(d);
            float att = clamp(1.0 - dist / u_lpos[i].w, 0.0, 1.0);
            att *= att;
            float ndl = dot(n, d / max(dist, 0.001)) * 0.6 + 0.4;
            light += u_lcol[i].rgb * u_lcol[i].a * att * max(ndl, 0.0);
        }
    }
    v_color = vec4(a_color.rgb / 255.0 * light, a_color.a / 255.0) * u_tint;
    float depth = -viewp.z;
    v_fog = nofog ? 0.0 : clamp((depth - u_fog.x) / max(u_fog.y - u_fog.x, 0.001), 0.0, 1.0);
    vec2 uv = a_uv + u_uv_offset;
    v_uv = uv;
    v_uv_aff = vec3(uv * clip.w, clip.w);
    v_layer = a_tex.x;
}
"#;

pub const WORLD_FS: &str = r#"#version 300 es
precision highp float;
precision highp sampler2DArray;

in vec2 v_uv;
in vec3 v_uv_aff;
in vec4 v_color;
in float v_fog;
flat in float v_layer;

uniform sampler2DArray u_tex;
uniform float u_affine;
uniform float u_dither;
uniform vec3 u_fog_color;
uniform vec4 u_flash;
uniform float u_dissolve;
uniform float u_additive;

out vec4 o_color;

float bayer4(ivec2 p) {
    int x = p.x & 3;
    int y = p.y & 3;
    int m[16] = int[16](0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5);
    return float(m[y * 4 + x]) / 16.0;
}

void main() {
    vec2 uv = mix(v_uv, v_uv_aff.xy / v_uv_aff.z, u_affine);
    vec4 t = texture(u_tex, vec3(uv, v_layer));
    if (t.a < 0.5) discard;
    float b = bayer4(ivec2(gl_FragCoord.xy));
    if (u_dissolve > 0.0 && b * 0.94 + 0.03 < u_dissolve) discard;
    vec3 c = t.rgb * v_color.rgb;
    c = mix(c, u_flash.rgb, u_flash.a);
    if (u_additive > 0.5) {
        c *= (1.0 - v_fog);
    } else {
        c = mix(c, u_fog_color, v_fog);
    }
    // 15-bit colour with ordered dithering.
    if (u_dither > 0.0) {
        c += (b - 0.5) * (1.0 / 31.0) * u_dither * 1.5;
        c = floor(clamp(c, 0.0, 1.0) * 31.0 + 0.5) / 31.0;
    }
    o_color = vec4(c, v_color.a);
}
"#;

pub const BLIT_VS: &str = r#"#version 300 es
precision highp float;
out vec2 v_uv;
void main() {
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const BLIT_FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_src;
uniform vec4 u_fade;
uniform vec4 u_rect;
out vec4 o_color;
void main() {
    vec2 uv = (v_uv - u_rect.xy) / u_rect.zw;
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
        o_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    vec3 c = texture(u_src, uv).rgb;
    c = mix(c, u_fade.rgb, u_fade.a);
    o_color = vec4(c, 1.0);
}
"#;
