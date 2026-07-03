// Centered 1:1 blit of the grid-sized text texture onto the surface.
// Unlike the upstream default (which stretches the texture to the surface,
// blurring glyphs between cell multiples), this keeps every glyph pixel-
// exact and paints the leftover margin in the theme background color -
// which is also how the window gets its padding.

struct VertexOutput {
    @builtin(position) gl_Position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) Index: u32) -> VertexOutput {
    let vertex = vec2(f32((Index << 1u) & 2u), f32(Index & 2u));
    return VertexOutput(vec4(vertex * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0));
}

struct FragmentOutput {
    @location(0) FragColor: vec4<f32>,
}

@group(0) @binding(0)
var Texture: texture_2d<f32>;
@group(0) @binding(1)
var Sampler: sampler;

struct Uniforms {
    screen_size: vec2<f32>,
    use_srgb: u32,
    _pad: u32,
    bg: vec4<f32>,
}

@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

@fragment
fn fs_main(@builtin(position) gl_Position: vec4<f32>) -> FragmentOutput {
    let tex = vec2<f32>(textureDimensions(Texture));
    let offset = floor((uniforms.screen_size - tex) * 0.5);
    let uv = (gl_Position.xy - offset) / tex;
    let factor = select(1.0, 2.2, uniforms.use_srgb == 1u);

    let sampled = pow(
        textureSample(Texture, Sampler, clamp(uv, vec2(0.0), vec2(1.0))),
        vec4(vec3(factor), 1.0),
    );
    let bg = pow(uniforms.bg, vec4(vec3(factor), 1.0));
    let outside = uv.x < 0.0 || uv.y < 0.0 || uv.x >= 1.0 || uv.y >= 1.0;

    return FragmentOutput(select(sampled, bg, outside));
}
