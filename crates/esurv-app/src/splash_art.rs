//! The key art ([`crate::title_screen::SPLASH_SCREEN`]), decoded once for the title screen and
//! the menus: the menus lay cuts of it under their panels for texture (the ui rework). A cut is a [`SplashCut`] node filling its parent; it shows once the art is decoded,
//! cropped from its part of the art to the node's shape.

use std::thread::JoinHandle;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

use crate::title_screen::{cover_size, SPLASH_SCREEN, SPLASH_SIZE};

/// The decoded art, once it is.
#[derive(Resource, Default)]
pub struct SplashArt {
    pub image: Option<Handle<Image>>,
    /// The decoded texture's size in pixels.
    pub size: Vec2,
    decoding: Option<JoinHandle<Option<Image>>>,
}

/// A part of the art a cut is taken from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// The golden portal's swirl (the zone selector: where the run goes).
    Portal,
    /// The starry night-blue sky under the logo (every panel's fill).
    Night,
    /// The bordeaux ribbon at the left.
    Wine,
}

impl Cut {
    /// Its part of the art, as shares of the art's width and height (min, max).
    pub fn region(self) -> Rect {
        match self {
            Cut::Portal => Rect::new(0.52, 0.12, 0.90, 0.80),
            Cut::Night => Rect::new(0.02, 0.28, 0.42, 0.46),
            Cut::Wine => Rect::new(0.00, 0.52, 0.26, 0.82),
        }
    }

    /// The crop of the region a node `aspect` wide to tall shows: the largest of that shape in
    /// the region, centred, in pixels of art `size`.
    pub fn crop(self, size: Vec2, aspect: f32) -> Rect {
        let r = self.region();
        let (min, max) = (r.min * size, r.max * size);
        let (w, h) = (max.x - min.x, max.y - min.y);
        let (cw, ch) = if aspect > w / h { (w, w / aspect) } else { (h * aspect, h) };
        let centre = (min + max) / 2.0;
        Rect::from_center_size(centre, Vec2::new(cw, ch))
    }
}

/// A node showing a cut of the art at `alpha`, over its parent's fill and under its content.
#[derive(Component, Clone, Copy, Debug)]
pub struct SplashCut {
    pub cut: Cut,
    pub alpha: f32,
}

/// The cut's node: it fills its parent's padding box and keeps no pointer.
pub fn cut(cut: Cut, alpha: f32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::ZERO,
            right: Val::ZERO,
            top: Val::ZERO,
            bottom: Val::ZERO,
            ..default()
        },
        ImageNode {
            color: Color::NONE,
            ..default()
        },
        SplashCut { cut, alpha },
        ZIndex(-1),
        Pickable::IGNORE,
    )
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SplashArt>()
        .add_systems(Startup, decode_the_art)
        .add_systems(Update, (take_the_art, fit_the_cuts).chain());
}

/// Decodes the embedded art at `want` pixels (no larger than it is), as a texture.
pub(crate) fn decode(want: UVec2) -> Option<Image> {
    let art = image::load_from_memory_with_format(SPLASH_SCREEN, image::ImageFormat::Jpeg).ok()?;
    let want = want
        .min(UVec2::new(art.width(), art.height()))
        .max(UVec2::ONE);
    let art = if want.x < art.width() {
        art.resize_exact(want.x, want.y, image::imageops::FilterType::Triangle)
    } else {
        art
    };
    let pixels = art.to_rgba8();
    let mut texture = Image::new(
        Extent3d {
            width: pixels.width(),
            height: pixels.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    texture.sampler = ImageSampler::linear();
    Some(texture)
}

/// Starts decoding the art at the size it covers the window with, on its own thread: the 4096
/// pixel JPEG costs a frame or two of work the launch does not wait for.
fn decode_the_art(window: Query<&Window, With<PrimaryWindow>>, mut art: ResMut<SplashArt>) {
    let physical = window
        .single()
        .map(|w| Vec2::new(w.physical_width() as f32, w.physical_height() as f32))
        .unwrap_or(SPLASH_SIZE.as_vec2());
    let want = cover_size(physical, SPLASH_SIZE.as_vec2())
        .ceil()
        .as_uvec2();
    art.decoding = Some(std::thread::spawn(move || decode(want)));
}

/// Keeps the art once it is decoded.
fn take_the_art(mut art: ResMut<SplashArt>, mut images: ResMut<Assets<Image>>) {
    if !art.decoding.as_ref().is_some_and(|d| d.is_finished()) {
        return;
    }
    match art.decoding.take().and_then(|d| d.join().ok()).flatten() {
        Some(texture) => {
            art.size = texture.size().as_vec2();
            art.image = Some(images.add(texture));
        }
        None => warn!("splash art: the key art did not decode; the menus show without it"),
    }
}

/// Each cut shows its part of the art in its node's shape.
fn fit_the_cuts(art: Res<SplashArt>, mut cuts: Query<(&SplashCut, &ComputedNode, &mut ImageNode)>) {
    let Some(image) = art.image.as_ref() else {
        return;
    };
    for (cut, node, mut shown) in &mut cuts {
        let size = node.size();
        if size.x < 1.0 || size.y < 1.0 {
            continue;
        }
        let rect = Some(cut.cut.crop(art.size, size.x / size.y));
        let colour = Color::WHITE.with_alpha(cut.alpha);
        if shown.image != *image || shown.rect != rect || shown.color != colour {
            shown.image = image.clone();
            shown.rect = rect;
            shown.color = colour;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_keeps_the_nodes_shape_inside_its_region() {
        let size = SPLASH_SIZE.as_vec2();
        for cut in [Cut::Portal, Cut::Night, Cut::Wine] {
            let region = cut.region();
            let region = Rect::from_corners(region.min * size, region.max * size).inflate(0.01);
            for aspect in [0.3, 1.0, 4.0] {
                let crop = cut.crop(size, aspect);
                assert!((crop.width() / crop.height() - aspect).abs() < 1e-3);
                assert!(region.contains(crop.min) && region.contains(crop.max), "{cut:?} {aspect}");
            }
        }
    }
}
