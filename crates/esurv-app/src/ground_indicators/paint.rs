//! An indicator's look as a picture laid flat on the ground: the shape in the indicator's own
//! frame (yards `along` its facing and `across` it, leftwards), filled see-through orange with a
//! brighter outline, the wind-up's fill swept from its origin (a line's or cone's near end) to its
//! edge, and chevrons the way a moving one goes. The ground decal stretches the picture over
//! [`Layout`]'s rectangle.

use esurv_protocol::IndicatorShape;

/// The orange of the fill, the swept fill, the outline and the chevrons (sRGB, 0-255).
const FILL: [f32; 3] = [255.0, 112.0, 16.0];
const SWEPT: [f32; 3] = [255.0, 128.0, 24.0];
const OUTLINE: [f32; 3] = [255.0, 168.0, 64.0];
const CHEVRON: [f32; 3] = [255.0, 214.0, 120.0];
/// How see-through each is (0 to 1): the shape's fill, the part the wind-up has swept, the
/// sweep's front, the outline (a lasting zone's thinner and fainter), the chevrons.
const FILL_ALPHA: f32 = 0.26;
const SWEPT_ALPHA: f32 = 0.46;
const FRONT_ALPHA: f32 = 0.62;
const OUTLINE_ALPHA: f32 = 0.95;
const EDGE_ALPHA: f32 = 0.85;
const CHEVRON_ALPHA: f32 = 0.9;
/// The outline's width (yards): a mark's grows with it, within these; a lasting zone's is thin.
const OUTLINE_MIN: f32 = 0.22;
const OUTLINE_MAX: f32 = 0.45;
const OUTLINE_SHARE: f32 = 0.025;
const EDGE_WIDTH: f32 = 0.18;
/// The sweep's front, a band this wide (yards) at the edge of the swept part.
const FRONT_WIDTH: f32 = 0.2;
/// The finest a texel is (yards), and the most texels along a side.
const FINEST_TEXEL: f32 = 0.05;
const MOST_TEXELS: f32 = 384.0;
const LEAST_TEXELS: u32 = 16;

/// Where `(along, across)` lies in or out of the shape: the distance to its edge (yards), minus
/// inside. Its sign is the simulation's hit test (`esurv_content::Shape::contains`).
pub fn edge_distance(shape: IndicatorShape, along: f32, across: f32) -> f32 {
    let rho = (along * along + across * across).sqrt();
    match shape {
        IndicatorShape::Circle { radius } => rho - radius,
        IndicatorShape::Ring { inner, outer } => (inner - rho).max(rho - outer),
        IndicatorShape::Cone { length, angle } => {
            let half = angle.to_radians() / 2.0;
            // the hit test takes anything this near the origin as in
            if rho < 0.5 {
                return (rho - length).max(rho - 0.5);
            }
            let off = across.atan2(along).abs();
            let side = if off <= half {
                -rho * (half - off).min(std::f32::consts::FRAC_PI_2).sin()
            } else {
                rho * (off - half).min(std::f32::consts::FRAC_PI_2).sin()
            };
            (rho - length).max(side)
        }
        IndicatorShape::Line { length, width } => (-along).max(along - length).max(across.abs() - width / 2.0),
    }
}

/// How far the wind-up's sweep has to come to reach `(along, across)`: 0 at its origin (a
/// ring's inner edge, a line's near end), 1 at its far edge.
pub fn sweep_reach(shape: IndicatorShape, along: f32, across: f32) -> f32 {
    let rho = (along * along + across * across).sqrt();
    match shape {
        IndicatorShape::Circle { radius } => rho / radius.max(0.01),
        IndicatorShape::Ring { inner, outer } => (rho - inner) / (outer - inner).max(0.01),
        IndicatorShape::Cone { length, .. } => rho / length.max(0.01),
        IndicatorShape::Line { length, .. } => along / length.max(0.01),
    }
}

/// The yards the wind-up's sweep crosses, origin to edge.
fn sweep_span(shape: IndicatorShape) -> f32 {
    match shape {
        IndicatorShape::Ring { inner, outer } => outer - inner,
        _ => reach(shape),
    }
    .max(0.01)
}

/// How far the shape reaches from its origin.
pub fn reach(shape: IndicatorShape) -> f32 {
    match shape {
        IndicatorShape::Circle { radius } => radius,
        IndicatorShape::Ring { outer, .. } => outer,
        IndicatorShape::Cone { length, .. } | IndicatorShape::Line { length, .. } => length,
    }
}

/// The picture's rectangle in the indicator's frame (yards) and its size in texels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub along: (f32, f32),
    pub across: (f32, f32),
    pub width: u32,
    pub height: u32,
}

impl Layout {
    /// The rectangle round `shape`, with room for its outline.
    pub fn of(shape: IndicatorShape) -> Self {
        let m = OUTLINE_MAX + 0.3;
        let (along, across) = match shape {
            IndicatorShape::Circle { radius: r } | IndicatorShape::Ring { outer: r, .. } => ((-r - m, r + m), (-r - m, r + m)),
            IndicatorShape::Cone { length, angle } => {
                let half = (angle.to_radians() / 2.0).min(std::f32::consts::PI);
                let back = (length * half.cos()).min(0.0);
                let side = length * half.min(std::f32::consts::FRAC_PI_2).sin();
                ((back - m, length + m), (-side - m, side + m))
            }
            IndicatorShape::Line { length, width } => ((-m, length + m), (-width / 2.0 - m, width / 2.0 + m)),
        };
        let span = (along.1 - along.0).max(across.1 - across.0);
        let texel = (span / MOST_TEXELS).max(FINEST_TEXEL);
        let texels = |s: f32| ((s / texel).ceil() as u32).max(LEAST_TEXELS);
        Self { along, across, width: texels(along.1 - along.0), height: texels(across.1 - across.0) }
    }

    /// The yards a texel spans (the larger side's).
    pub fn texel(&self) -> f32 {
        ((self.along.1 - self.along.0) / self.width as f32).max((self.across.1 - self.across.0) / self.height as f32)
    }

    /// Texel `(i, j)`'s centre in the frame: `i` runs along, `j` runs from the left edge
    /// (most across) to the right (the decal's `v` grows rightwards).
    pub fn centre(&self, i: u32, j: u32) -> (f32, f32) {
        let u = (i as f32 + 0.5) / self.width as f32;
        let v = (j as f32 + 0.5) / self.height as f32;
        (self.along.0 + u * (self.along.1 - self.along.0), self.across.1 - v * (self.across.1 - self.across.0))
    }

    /// The point `(along, across)` as the picture's `(u, v)`.
    #[cfg(test)]
    pub fn uv(&self, along: f32, across: f32) -> (f32, f32) {
        ((along - self.along.0) / (self.along.1 - self.along.0), (self.across.1 - across) / (self.across.1 - self.across.0))
    }
}

/// A chevron: its middle in the frame, the way it points (a unit vector) and its size (yards).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chevron {
    pub at: (f32, f32),
    pub towards: (f32, f32),
    pub size: f32,
}

/// The chevrons of a moving indicator, pointing `towards` (in its frame): three through a
/// circle's middle; along a line or cone, a row across it every few yards.
pub fn chevrons(shape: IndicatorShape, towards: (f32, f32)) -> Vec<Chevron> {
    match shape {
        IndicatorShape::Circle { radius: r } | IndicatorShape::Ring { outer: r, .. } => {
            let size = r * 0.3;
            [-0.42, 0.0, 0.42].iter().map(|k| Chevron { at: (towards.0 * r * k, towards.1 * r * k), towards, size }).collect()
        }
        IndicatorShape::Line { length, width } => {
            let size = (width * 0.55).clamp(0.6, 3.0);
            let n = ((length / (size * 2.5)).floor() as usize).max(1);
            (0..n).map(|k| Chevron { at: (length * (k as f32 + 0.5) / n as f32, 0.0), towards, size }).collect()
        }
        IndicatorShape::Cone { length, .. } => {
            let size = (length * 0.12).clamp(0.6, 3.0);
            [0.35, 0.6, 0.85].iter().map(|k| Chevron { at: (length * k, 0.0), towards, size }).collect()
        }
    }
}

/// How much of a chevron covers `(a, c)`: a V of two arms meeting at its point.
fn chevron_cover(ch: &Chevron, a: f32, c: f32, aa: f32) -> f32 {
    let (fx, fy) = ch.towards;
    let (dx, dy) = (a - ch.at.0, c - ch.at.1);
    // in the chevron's own frame: x the way it points, y across it
    let (x, y) = (dx * fx + dy * fy, -dx * fy + dy * fx);
    let s = ch.size;
    let tip = (s * 0.35, 0.0);
    let thick = s * 0.16;
    let mut d = f32::MAX;
    for end in [(-s * 0.35, s * 0.5), (-s * 0.35, -s * 0.5)] {
        d = d.min(segment_distance((x, y), tip, end));
    }
    (0.5 - (d - thick / 2.0) / aa).clamp(0.0, 1.0)
}

fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / (abx * abx + aby * aby).max(1e-6)).clamp(0.0, 1.0);
    ((p.0 - a.0 - abx * t).powi(2) + (p.1 - a.1 - aby * t).powi(2)).sqrt()
}

/// One colour over another, both straight (not premultiplied) alpha.
fn over(top: ([f32; 3], f32), under: ([f32; 3], f32)) -> ([f32; 3], f32) {
    let a = top.1 + under.1 * (1.0 - top.1);
    if a <= 0.0 {
        return ([0.0; 3], 0.0);
    }
    let rgb = std::array::from_fn(|k| (top.0[k] * top.1 + under.0[k] * under.1 * (1.0 - top.1)) / a);
    (rgb, a)
}

/// The picture's RGBA texels (sRGB, rows from the left edge): `shape` as a mark winding up,
/// `swept` of the way through (0 to 1; `None` for a lasting zone's outline alone), with
/// `chevrons` over it.
pub fn paint(shape: IndicatorShape, layout: &Layout, swept: Option<f32>, chevrons: &[Chevron]) -> Vec<u8> {
    let aa = layout.texel();
    let outline = match swept {
        Some(_) => (reach(shape) * OUTLINE_SHARE).clamp(OUTLINE_MIN, OUTLINE_MAX),
        None => EDGE_WIDTH,
    }
    .max(aa * 1.5);
    let mut out = Vec::with_capacity((layout.width * layout.height * 4) as usize);
    for j in 0..layout.height {
        for i in 0..layout.width {
            let (a, c) = layout.centre(i, j);
            let d = edge_distance(shape, a, c);
            let inside = (0.5 - d / aa).clamp(0.0, 1.0);
            // the outline lies just inside the edge
            let band = (0.5 - ((d + outline / 2.0).abs() - outline / 2.0) / aa).clamp(0.0, 1.0);
            let mut px = ([0.0; 3], 0.0);
            if let Some(p) = swept {
                let r = sweep_reach(shape, a, c);
                let scale = sweep_span(shape);
                let done = (0.5 + (p - r) * scale / aa).clamp(0.0, 1.0);
                let front = if p > 0.0 && p < 1.0 { (0.5 - (((p - r) * scale).abs() - FRONT_WIDTH / 2.0) / aa).clamp(0.0, 1.0) } else { 0.0 };
                let fill = FILL_ALPHA + (SWEPT_ALPHA - FILL_ALPHA) * done;
                let colour = std::array::from_fn(|k| FILL[k] + (SWEPT[k] - FILL[k]) * done);
                px = over((OUTLINE, FRONT_ALPHA * front), (colour, fill));
                px.1 *= inside;
                px = over((OUTLINE, OUTLINE_ALPHA * band), px);
            } else {
                px = over((OUTLINE, EDGE_ALPHA * band), px);
            }
            let ch = chevrons.iter().map(|ch| chevron_cover(ch, a, c, aa)).fold(0.0, f32::max) * inside.max(band);
            if ch > 0.0 {
                px = over((CHEVRON, CHEVRON_ALPHA * ch), px);
            }
            out.extend(px.0.iter().map(|v| v.round().clamp(0.0, 255.0) as u8));
            out.push((px.1 * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::Shape;

    fn shapes() -> Vec<(IndicatorShape, Shape)> {
        vec![
            (IndicatorShape::Circle { radius: 8.0 }, Shape::Circle { radius: 8.0 }),
            (IndicatorShape::Ring { inner: 6.0, outer: 15.0 }, Shape::Ring { inner: 6.0, outer: 15.0 }),
            (IndicatorShape::Cone { length: 20.0, angle: 60.0 }, Shape::Cone { length: 20.0, angle: 60.0 }),
            (IndicatorShape::Cone { length: 12.0, angle: 200.0 }, Shape::Cone { length: 12.0, angle: 200.0 }),
            (IndicatorShape::Line { length: 40.0, width: 6.0 }, Shape::Line { length: 40.0, width: 6.0 }),
        ]
    }

    /// A tiny xorshift, so the points are the same every run.
    fn points(n: usize, spread: f32) -> Vec<(f32, f32)> {
        let mut s: u32 = 0x9e37_79b9;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s as f32 / u32::MAX as f32 * 2.0 - 1.0
        };
        (0..n).map(|_| (next() * spread, next() * spread)).collect()
    }

    /// The shape drawn is the shape that hits: for every shape, a point is inside the picture's
    /// edge exactly when the simulation's hit test takes it (laid at an origin, facing anywhere),
    /// away from the edge itself.
    #[test]
    fn the_drawn_shape_is_the_hit_test() {
        for (drawn, hit) in shapes() {
            for facing in [0.0f32, 1.0, -2.4] {
                let origin = (-8913.0, -105.5);
                for (dx, dy) in points(4000, 45.0) {
                    let p = (origin.0 + dx, origin.1 + dy);
                    // into the indicator's frame: along its facing, across it leftwards
                    let along = dx * facing.cos() + dy * facing.sin();
                    let across = -dx * facing.sin() + dy * facing.cos();
                    let d = edge_distance(drawn, along, across);
                    if d.abs() < 0.05 {
                        continue;
                    }
                    assert_eq!(d < 0.0, hit.contains(origin, facing, p), "{drawn:?} facing {facing} at {along},{across}: {d}");
                }
            }
        }
    }

    /// The picture fills the shape and nothing outside it; the outline is near opaque, the fill
    /// see-through, the swept part brighter than the rest.
    #[test]
    fn the_picture_is_the_shape_filled_and_outlined() {
        for (drawn, _) in shapes() {
            let layout = Layout::of(drawn);
            assert!(layout.width <= MOST_TEXELS as u32 + 1 && layout.height <= MOST_TEXELS as u32 + 1);
            let px = paint(drawn, &layout, Some(0.5), &[]);
            assert_eq!(px.len(), (layout.width * layout.height * 4) as usize);
            let aa = layout.texel();
            let (mut swept, mut ahead, mut edge) = (0, 0, 0);
            for j in 0..layout.height {
                for i in 0..layout.width {
                    let (a, c) = layout.centre(i, j);
                    let alpha = px[((j * layout.width + i) * 4 + 3) as usize] as f32 / 255.0;
                    let d = edge_distance(drawn, a, c);
                    if d > aa {
                        assert_eq!(alpha, 0.0, "{drawn:?}: nothing outside ({a},{c})");
                    } else if d < -OUTLINE_MAX - 2.0 * aa {
                        let r = sweep_reach(drawn, a, c);
                        if r < 0.45 {
                            assert!((alpha - SWEPT_ALPHA).abs() < 0.02, "{drawn:?} swept {alpha}");
                            swept += 1;
                        } else if r > 0.55 {
                            assert!((alpha - FILL_ALPHA).abs() < 0.02, "{drawn:?} ahead {alpha}");
                            ahead += 1;
                        }
                    } else if (-0.15..-0.05).contains(&d) {
                        assert!(alpha > 0.9, "{drawn:?} outline {alpha}");
                        edge += 1;
                    }
                }
            }
            assert!(swept > 0 && ahead > 0 && edge > 0, "{drawn:?}");
        }
    }

    /// A lasting zone is its outline alone; a moving one's chevrons point its way.
    #[test]
    fn a_lasting_zone_is_an_outline_with_chevrons_its_way() {
        let circle = IndicatorShape::Circle { radius: 6.0 };
        let layout = Layout::of(circle);
        let at = |px: &[u8], a: f32, c: f32| {
            let (u, v) = layout.uv(a, c);
            let (i, j) = ((u * layout.width as f32) as u32, (v * layout.height as f32) as u32);
            px[((j * layout.width + i) * 4 + 3) as usize]
        };
        let plain = paint(circle, &layout, None, &[]);
        assert_eq!(at(&plain, 0.0, 3.0), 0, "no fill");
        assert!(at(&plain, 5.91, 0.0) > 200, "its edge");
        let ch = chevrons(circle, (1.0, 0.0));
        assert_eq!(ch.len(), 3);
        let moving = paint(circle, &layout, None, &ch);
        // the middle chevron's point is ahead of its middle, its arms behind
        assert!(at(&moving, ch[1].size * 0.35 - 0.05, 0.0) > 200);
        assert_eq!(at(&moving, -ch[1].size * 0.35, 0.0), 0, "the V is open behind");
    }
}
