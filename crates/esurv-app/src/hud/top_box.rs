//! What the run's readouts sit on at the top of the screen: a slim ribbon across the top,
//! its ends slanting in to its foot, for the kills and the gold, and over its middle a crest hung
//! from the screen's edge, its foot coming to a point, for the clock and the next boss. Both are
//! the key art's night sky ([`NIGHT`]), the crest a shade brighter; each in a bronze edge with a gold hairline
//! inside it, lit at the top and shaded at the foot.
//!
//! Drawn once per scale into a texture at the screen's own pixels, from each shape's distance
//! field, so the edges stay smooth and both halves the same.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::menus::look::palette;
use crate::stock::StockScale;

// The shapes, in stock units: x from the middle, y down from the screen's top.
/// The ribbon's half width at the top, its height, and how far its ends slant in by its foot.
pub(crate) const RIBBON_HALF: f32 = 170.0;
pub(super) const RIBBON_HEIGHT: f32 = 34.0;
const RIBBON_SLANT: f32 = 10.0;
/// The crest's half width, how far down its sides run straight, and where its point is.
pub(super) const CREST_HALF: f32 = 50.0;
const CREST_SIDE: f32 = 44.0;
const CREST_POINT: f32 = 66.0;

/// The whole piece's size in units.
pub(super) const SIZE: Vec2 = Vec2::new(RIBBON_HALF * 2.0, CREST_POINT);
/// A small shadow round the outline:
/// how far it spreads in units, how far it drops, and how dark it is under the shape; the texture
/// is this much larger at the sides and the foot.
pub(super) const SHADOW: f32 = 6.0;
const SHADOW_DROP: f32 = 1.5;
const SHADOW_DARK: f32 = 0.55;
/// The texture's size in units, the shadow's margin round the piece.
pub(super) const IMAGE_SIZE: Vec2 = Vec2::new(SIZE.x + 2.0 * SHADOW, SIZE.y + SHADOW);

/// The key art's night sky, the blue middle of the band under its logo (1030 by 200 from 300, 740
/// in the 4096 wide art), as wide to tall as the piece: laid over its whole size.
const NIGHT: &[u8] = include_bytes!("../../assets/top_night.jpg");
/// How much of the sky shows over the panel's fill, on the ribbon and the crest; and how much
/// brighter the crest's sky is.
const RIBBON_SKY: f32 = 0.8;
const CREST_SKY: f32 = 0.9;
const CREST_BRIGHT: f32 = 1.3;

/// The night sky's colour at `p` (units), or nothing when it did not decode.
fn sky(night: Option<&image::RgbImage>, p: Vec2) -> Option<Vec3> {
    let night = night?;
    let (w, h) = (night.width() as f32, night.height() as f32);
    let x = ((p.x + SIZE.x / 2.0) / SIZE.x * w - 0.5).clamp(0.0, w - 1.0);
    let y = (p.y / SIZE.y * h - 0.5).clamp(0.0, h - 1.0);
    let at = |x: u32, y: u32| {
        let [r, g, b] = night.get_pixel(x.min(night.width() - 1), y.min(night.height() - 1)).0;
        // the JPEG's sRGB bytes, as the fill's colours are
        Vec3::new(r as f32, g as f32, b as f32) / 255.0
    };
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (fx, fy) = (x.fract(), y.fract());
    let top = at(x0, y0).lerp(at(x0 + 1, y0), fx);
    let foot = at(x0, y0 + 1).lerp(at(x0 + 1, y0 + 1), fx);
    Some(top.lerp(foot, fy))
}

/// The distance from `p` (units) to the ribbon's edge: negative inside. Its top runs off the
/// screen's edge.
fn ribbon(p: Vec2) -> f32 {
    let x = p.x.abs();
    // the slanting end, as a line from the top corner to the foot's
    let end = Vec2::new(RIBBON_HEIGHT, RIBBON_SLANT).normalize();
    let side = (Vec2::new(x, p.y) - Vec2::new(RIBBON_HALF, 0.0)).dot(end);
    side.max(p.y - RIBBON_HEIGHT)
}

/// The distance from `p` (units) to the crest's edge: negative inside.
fn crest(p: Vec2) -> f32 {
    let x = p.x.abs();
    // the foot's slope, from the side's end to the point
    let foot = Vec2::new(CREST_POINT - CREST_SIDE, CREST_HALF).normalize();
    let slope = (Vec2::new(x, p.y) - Vec2::new(CREST_HALF, CREST_SIDE)).dot(foot);
    (x - CREST_HALF).max(slope)
}

/// One shape's colour and cover at a pixel: `d` its distance in pixels, `depth` how far down the
/// shape (0 to 1), `lines` the edge's lines inward from it (colour, from, to in pixels, strength).
/// `fill` is the colour under the edge (the sky over the panel's fill).
fn paint(d: f32, depth: f32, fill: Vec3, alpha: f32, lines: &[(Srgba, f32, f32, f32)]) -> Option<(Vec3, f32)> {
    let cover = (0.5 - d).clamp(0.0, 1.0);
    if cover <= 0.0 {
        return None;
    }
    let lit = (1.0 - depth / 0.45).max(0.0) * 0.08;
    let shade = ((depth - 0.45) / 0.55).max(0.0) * 0.22;
    let mut c = fill;
    c = c.lerp(Vec3::new(1.0, 0.95, 0.85), lit).lerp(Vec3::ZERO, shade);
    let mut a = alpha;
    let inside = -d;
    for &(colour, from, to, strength) in lines {
        let k = (inside - from + 0.5).clamp(0.0, 1.0) * (to - inside + 0.5).clamp(0.0, 1.0) * strength;
        if k > 0.0 {
            c = c.lerp(Vec3::new(colour.red, colour.green, colour.blue), k);
            a += (1.0 - a) * k;
        }
    }
    Some((c, a * cover))
}

/// `top` over `under`, each a colour and its cover.
fn over_of(top: (Vec3, f32), under: (Vec3, f32)) -> (Vec3, f32) {
    let ((tc, ta), (uc, ua)) = (top, under);
    let a = ta + ua * (1.0 - ta);
    ((tc * ta + uc * ua * (1.0 - ta)) / a.max(1e-6), a)
}

/// The shadow's cover at `p` (units): dark under the shapes, dropped a little, fading out over
/// [`SHADOW`] past their outline.
fn shadow(p: Vec2) -> f32 {
    let q = p - Vec2::new(0.0, SHADOW_DROP);
    let d = ribbon(q).min(crest(q)).max(0.0);
    let k = (1.0 - d / SHADOW).max(0.0);
    SHADOW_DARK * k * k
}

/// The piece's texture at `scale`, a texel to each of the screen's pixels.
pub(super) fn image(scale: StockScale) -> Image {
    let texel = scale.unit * scale.device;
    let w = (IMAGE_SIZE.x * texel).ceil().max(1.0) as u32;
    let h = (IMAGE_SIZE.y * texel).ceil().max(1.0) as u32;
    let s = |c: Color| c.to_srgba();
    let panel = s(palette::PANEL);
    let panel_rgb = Vec3::new(panel.red, panel.green, panel.blue);
    let night = image::load_from_memory_with_format(NIGHT, image::ImageFormat::Jpeg)
        .ok()
        .map(|art| art.to_rgb8());
    // the sky over the panel's fill; the crest's brighter, so it reads as its own piece
    let fill = |p: Vec2, sky_share: f32, bright: f32| match sky(night.as_ref(), p) {
        Some(sky) => panel_rgb.lerp(sky * bright, sky_share),
        None => panel_rgb,
    };
    // inward from the edge, in pixels: a dark line, bronze, then a gold hairline in from it; the
    // ribbon's thin, the crest's bronze twice as broad
    let stroke = texel.max(1.0);
    let edge = |bronze: f32| {
        [
            (s(palette::CARVED_DARK), 0.0, 1.0, 1.0),
            (s(palette::BRONZE), 1.0, 1.0 + bronze, 1.0),
            (s(palette::GOLD), 2.0 + bronze, 3.0 + bronze, 0.7),
        ]
    };
    let ribbon_lines = edge(stroke);
    let crest_lines = edge(stroke * 2.0);
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for py in 0..h {
        for px in 0..w {
            let p = Vec2::new((px as f32 + 0.5) / texel - IMAGE_SIZE.x / 2.0, (py as f32 + 0.5) / texel);
            let (dr, dc) = (ribbon(p) * texel, crest(p) * texel);
            let under = (dr < 0.5)
                .then(|| paint(dr, p.y / RIBBON_HEIGHT, fill(p, RIBBON_SKY, 1.0), panel.alpha, &ribbon_lines))
                .flatten();
            let over = (dc < 0.5)
                .then(|| paint(dc, p.y / CREST_POINT, fill(p, CREST_SKY, CREST_BRIGHT), panel.alpha, &crest_lines))
                .flatten();
            // the crest over the ribbon
            let (c, a) = match (under, over) {
                (None, None) => (Vec3::ZERO, 0.0),
                (Some(u), None) => u,
                (None, Some(o)) => o,
                (Some(u), Some(o)) => over_of(o, u),
            };
            // both over their shadow
            let (c, a) = over_of((c, a), (Vec3::ZERO, shadow(p)));
            let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            pixels.extend_from_slice(&[byte(c.x), byte(c.y), byte(c.z), byte(a)]);
        }
    }
    let mut image = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ribbon_slants_in_and_the_crest_hangs_to_a_point() {
        // the ribbon: full at the top, its ends slanting in by its foot, nothing under it
        assert!(ribbon(Vec2::new(RIBBON_HALF - 1.0, 1.0)) < 0.0);
        assert!(ribbon(Vec2::new(RIBBON_HALF - 3.0, RIBBON_HEIGHT - 1.0)) > 0.0);
        assert!(ribbon(Vec2::new(0.0, RIBBON_HEIGHT + 1.0)) > 0.0);
        // the crest: straight sides, then a point at the middle of its foot
        assert!(crest(Vec2::new(CREST_HALF - 1.0, CREST_SIDE - 1.0)) < 0.0);
        assert!(crest(Vec2::new(0.0, CREST_POINT - 1.0)) < 0.0);
        assert!(crest(Vec2::new(0.0, CREST_POINT + 1.0)) > 0.0);
        assert!(crest(Vec2::new(CREST_HALF - 5.0, CREST_POINT - 5.0)) > 0.0);
        for (x, y) in [(37.0, 12.0), (120.0, 30.0), (45.0, 50.0)] {
            assert_eq!(ribbon(Vec2::new(x, y)), ribbon(Vec2::new(-x, y)));
            assert_eq!(crest(Vec2::new(x, y)), crest(Vec2::new(-x, y)));
        }
    }
}
