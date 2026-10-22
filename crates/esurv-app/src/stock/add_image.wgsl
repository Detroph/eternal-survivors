// The stock interface's additive textures (`alphaMode="ADD"`: button highlights, glows): the
// texel times its alpha is added to what is under it, in gamma bytes, as the 1.12 client blends
// (`dst + texel * a`). The UI lane holds gamma bytes, so the texel (decoded by its sRGB sampler)
// is encoded back before the add.

#import bevy_ui::ui_vertex_output::UiVertexOutput

// The tint, in gamma bytes, alpha last.
@group(1) @binding(0) var<uniform> tint: vec4<f32>;
// The part of the texture drawn: its least and greatest uv.
@group(1) @binding(1) var<uniform> uv_rect: vec4<f32>;
@group(1) @binding(2) var add_texture: texture_2d<f32>;
@group(1) @binding(3) var add_sampler: sampler;

// Linear to sRGB (IEC 61966-2-1), the inverse of the sampler's decode.
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let higher = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    let lower = c * 12.92;
    return select(higher, lower, c <= vec3<f32>(0.0031308));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = mix(uv_rect.xy, uv_rect.zw, in.uv);
    let texel = textureSample(add_texture, add_sampler, uv);
    return vec4<f32>(linear_to_srgb(texel.rgb) * tint.rgb, texel.a * tint.a);
}
