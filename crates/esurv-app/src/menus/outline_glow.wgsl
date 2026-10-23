// The picked hero's golden outline on the warband screen: the hero alone, drawn by benilla into a
// window-sized mask (`SceneFigureMask`), is read round each pixel outside it; within `edge` pixels
// of the silhouette a thin gold edge, further out a soft glow fading over `glow` pixels, both added
// to what is under them in gamma bytes. Nothing is drawn over the hero itself.

#import bevy_ui::ui_vertex_output::UiVertexOutput

// The colour, in gamma bytes, and the strength of the glow (alpha): the hero's gold, or an
// enemy's purple or silver (`enemy_glow`), each from its own mask.
@group(1) @binding(0) var<uniform> colour: vec4<f32>;
// x: the edge's width, y: the glow's reach (both in pixels at 1080 rows), z: the pulse's depth,
// w: the time, seconds.
@group(1) @binding(1) var<uniform> params: vec4<f32>;
@group(1) @binding(2) var mask: texture_2d<f32>;

const DIRECTIONS: i32 = 12;
const TAU: f32 = 6.2831853;

fn covered(p: vec2<f32>, dims: vec2<f32>) -> bool {
    let q = clamp(p, vec2<f32>(0.0), dims - vec2<f32>(1.0));
    return textureLoad(mask, vec2<i32>(q), 0).a > 0.5;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(mask));
    let p = in.uv * dims;
    if covered(p, dims) {
        return vec4<f32>(0.0);
    }
    // the sizes follow the window's height, as the stock interface does
    let scale = dims.y / 1080.0;
    let edge = max(1.0, params.x * scale);
    let reach = max(2.0, params.y * scale);
    // how far the silhouette is: the first ring out from here that touches it
    var near = reach + 1.0;
    var r = 1.0;
    loop {
        if r > reach || near <= reach {
            break;
        }
        for (var k = 0; k < DIRECTIONS; k = k + 1) {
            let a = TAU * f32(k) / f32(DIRECTIONS);
            if covered(p + vec2<f32>(cos(a), sin(a)) * r, dims) {
                near = r;
                break;
            }
        }
        r = r + select(1.0, 2.0, r >= 3.0);
    }
    if near > reach {
        return vec4<f32>(0.0);
    }
    let pulse = 1.0 - params.z * (0.5 + 0.5 * sin(params.w * 1.7));
    let rim = select(0.0, 1.0, near <= edge);
    let glow = exp(-2.5 * (near - 1.0) / reach) * colour.a;
    let strength = clamp(max(rim * 0.9, glow) * pulse, 0.0, 1.0);
    return vec4<f32>(colour.rgb, strength);
}
