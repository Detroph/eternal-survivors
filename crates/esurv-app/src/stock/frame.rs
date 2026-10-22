//! A backdrop's edge, as `SetBackdrop` draws it: the stock edge file is eight squares in a row
//! (left, right, top, bottom, then the corners top-left, top-right, bottom-left, bottom-right),
//! the top and bottom stored a quarter turn from how they show. Once the file loads its squares
//! are laid out as a 3x3 frame image (the middle clear), which a node draws nine-sliced: the
//! corners as they are, the sides tiled along the edge.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// One square of a stock edge file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EdgePiece {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl EdgePiece {
    /// In the file's order.
    pub(crate) const ALL: [EdgePiece; 8] = [
        EdgePiece::Left,
        EdgePiece::Right,
        EdgePiece::Top,
        EdgePiece::Bottom,
        EdgePiece::TopLeft,
        EdgePiece::TopRight,
        EdgePiece::BottomLeft,
        EdgePiece::BottomRight,
    ];

    /// Its place in the file, counted in squares from the left.
    pub(crate) fn index(self) -> u32 {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0) as u32
    }

    /// Its cell in the 3x3 frame: column, row.
    pub(crate) fn cell(self) -> (u32, u32) {
        match self {
            EdgePiece::TopLeft => (0, 0),
            EdgePiece::Top => (1, 0),
            EdgePiece::TopRight => (2, 0),
            EdgePiece::Left => (0, 1),
            EdgePiece::Right => (2, 1),
            EdgePiece::BottomLeft => (0, 2),
            EdgePiece::Bottom => (1, 2),
            EdgePiece::BottomRight => (2, 2),
        }
    }

    /// The top and bottom squares are stored with their outer side at the left and right: they
    /// show turned a quarter turn clockwise.
    pub(crate) fn turned(self) -> bool {
        matches!(self, EdgePiece::Top | EdgePiece::Bottom)
    }
}

/// The 3x3 frame image's pixels (RGBA, `3 * square` a side, the middle clear) from an edge file's
/// pixels, `width` wide with squares `square` texels a side.
pub(crate) fn compose_frame(edge: &[u8], width: u32, square: u32) -> Vec<u8> {
    let side = 3 * square;
    let mut out = vec![0u8; (side * side * 4) as usize];
    for piece in EdgePiece::ALL {
        let (column, row) = piece.cell();
        let first = piece.index() * square;
        for y in 0..square {
            for x in 0..square {
                // a quarter turn clockwise: the stored left column becomes the top row
                let (sx, sy) = if piece.turned() {
                    (y, square - 1 - x)
                } else {
                    (x, y)
                };
                let from = ((sy * width + first + sx) * 4) as usize;
                let to = (((row * square + y) * side + column * square + x) * 4) as usize;
                if let (Some(src), Some(dst)) = (edge.get(from..from + 4), out.get_mut(to..to + 4))
                {
                    dst.copy_from_slice(src);
                }
            }
        }
    }
    out
}

/// An image of `side` texels a side from RGBA pixels: sRGB, as the stock art is drawn, sampled
/// smoothly and clamped.
pub(crate) fn frame_image(side: u32, pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A backdrop's frame: the edge file as loaded (its pixels kept on the CPU), the frame image the
/// nodes draw (clear until the file loads), and the squares' size in texels.
#[derive(Clone, Debug)]
pub(crate) struct EdgeFrame {
    pub(crate) source: Handle<Image>,
    pub(crate) image: Handle<Image>,
    pub(crate) square: u32,
    pub(crate) ready: bool,
}

impl EdgeFrame {
    /// A frame waiting for `source`, its image clear for now.
    pub(crate) fn new(source: Handle<Image>, square: u32, images: &mut Assets<Image>) -> Self {
        let side = 3 * square;
        let image = images.add(frame_image(side, vec![0; (side * side * 4) as usize]));
        Self {
            source,
            image,
            square,
            ready: false,
        }
    }

    /// Lays the edge file out into the frame image once it has loaded; true when done.
    pub(crate) fn compose(&mut self, images: &mut Assets<Image>) -> bool {
        if self.ready {
            return true;
        }
        let Some(edge) = images.get(&self.source) else {
            return false;
        };
        let Some(pixels) = edge.data.as_ref() else {
            return false;
        };
        let (width, height) = (edge.width(), edge.height());
        if height != self.square || width < 8 * self.square {
            warn!(
                "stock: an edge file is {width}x{height}, not eight squares of {}",
                self.square
            );
        }
        let frame = compose_frame(pixels, width, self.square);
        if let Some(image) = images.get_mut(&self.image) {
            *image = frame_image(3 * self.square, frame);
        }
        self.ready = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An edge file of 2-texel squares, each texel naming its square, column and row.
    fn edge_file() -> Vec<u8> {
        let (square, width) = (2u32, 16u32);
        let mut px = Vec::new();
        for y in 0..square {
            for x in 0..width {
                px.extend_from_slice(&[(x / square) as u8, (x % square) as u8, y as u8, 255]);
            }
        }
        px
    }

    fn texel(frame: &[u8], side: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * side + x) * 4) as usize;
        frame[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn the_pieces_take_their_cells() {
        let frame = compose_frame(&edge_file(), 16, 2);
        // corners and sides as stored
        assert_eq!(texel(&frame, 6, 0, 0), [4, 0, 0, 255], "top-left corner");
        assert_eq!(texel(&frame, 6, 5, 0), [5, 1, 0, 255], "top-right corner");
        assert_eq!(texel(&frame, 6, 0, 5), [6, 0, 1, 255], "bottom-left corner");
        assert_eq!(
            texel(&frame, 6, 5, 5),
            [7, 1, 1, 255],
            "bottom-right corner"
        );
        assert_eq!(texel(&frame, 6, 0, 2), [0, 0, 0, 255], "left side");
        assert_eq!(texel(&frame, 6, 5, 3), [1, 1, 1, 255], "right side");
        // the middle is clear
        assert_eq!(texel(&frame, 6, 2, 2)[3], 0);
        assert_eq!(texel(&frame, 6, 3, 3)[3], 0);
    }

    #[test]
    fn the_top_and_bottom_turn_a_quarter_clockwise() {
        let frame = compose_frame(&edge_file(), 16, 2);
        // the stored left column (the outer side) becomes the top row, read bottom to top
        assert_eq!(texel(&frame, 6, 2, 0), [2, 0, 1, 255]);
        assert_eq!(texel(&frame, 6, 3, 0), [2, 0, 0, 255]);
        // the stored right column becomes the bottom row: the bottom's outer side is down
        assert_eq!(texel(&frame, 6, 2, 5), [3, 1, 1, 255]);
        assert_eq!(texel(&frame, 6, 3, 5), [3, 1, 0, 255]);
        assert_eq!(texel(&frame, 6, 2, 4), [3, 0, 1, 255]);
    }

    #[test]
    fn pieces_count_from_the_left() {
        assert_eq!(EdgePiece::Left.index(), 0);
        assert_eq!(EdgePiece::BottomRight.index(), 7);
        assert!(EdgePiece::Top.turned() && !EdgePiece::TopLeft.turned());
    }
}
