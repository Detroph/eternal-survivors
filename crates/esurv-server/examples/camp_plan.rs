//! Draws a camp plan over its overhead photo (benilla's `camp-*-overhead-wide` capture): each
//! tent to its true size, turned as it faces (the basic tent's canvas, its guy ropes and door, the
//! largest upgrade's box, the plot's circle), the hero's spot before it, and the fire.
//!
//! `cargo run -p esurv-server --example camp_plan -- <photo.png> <plan.txt> <out.png>`
//!
//! The photo: straight down from 150 yards with the 45-degree view, north up, the anchor spot
//! in the middle. The plan: one line per thing, `|`-separated:
//! `TENT|P1|east|north|door_bearing|size|hero_east|hero_north|plot_radius` and
//! `FIRE|east|north` and `BOX|name|east|north|facing|back|front|half` (any other thing: yards
//! behind and ahead of its point along its facing, and half its width), yards from the anchor
//! spot, bearings in degrees from north; `SPOT|A|east|north|radius` rings a spot to choose in
//! yellow, lettered.

use image::{Rgba, RgbaImage};

/// Pixels per yard at the overhead's 150 yards, 900 pixels tall with a 45-degree view.
const PIXELS_PER_YARD: f32 = 7.24;
/// The drawing is this many times the photo's size.
const ZOOM: f32 = 2.0;
/// How many yards are shown round the plan's farthest things.
const MARGIN: f32 = 14.0;

/// The basic tent (HumanTentMedium at 0.7, its origin 2.3 yards ahead of the plot's middle) and the
/// largest upgrade (HumanTentLarge at 0.8): yards back and forward of the plot's middle along its
/// facing, and half its width (guy ropes included); the basic tent's canvas half width.
const BASIC: (f32, f32, f32) = (1.3, 4.86, 4.76);
const BASIC_CANVAS: f32 = 2.1;
const LARGEST: (f32, f32, f32) = (4.77, 4.94, 5.3);

struct Canvas {
    img: RgbaImage,
    /// The yards shown: east from, east to, north from, north to.
    view: (f32, f32, f32, f32),
}

impl Canvas {
    /// The pixel of a point `east`, `north` yards from the anchor spot.
    fn pixel(&self, east: f32, north: f32) -> (f32, f32) {
        let s = PIXELS_PER_YARD * ZOOM;
        ((east - self.view.0) * s, (self.view.3 - north) * s)
    }

    fn dot(&mut self, (x, y): (f32, f32), r: f32, c: Rgba<u8>) {
        let (w, h) = self.img.dimensions();
        for py in (y - r).max(0.0) as u32..((y + r).max(0.0) as u32).min(h) {
            for px in (x - r).max(0.0) as u32..((x + r).max(0.0) as u32).min(w) {
                if (px as f32 - x).hypot(py as f32 - y) <= r {
                    self.img.put_pixel(px, py, c);
                }
            }
        }
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, c: Rgba<u8>) {
        let steps = ((b.0 - a.0).hypot(b.1 - a.1) * 2.0).max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f32 / steps as f32;
            self.dot((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), width / 2.0, c);
        }
    }

    /// A box `back` behind and `front` ahead of a point along `bearing`, `half` to each side.
    fn turned_box(&mut self, east: f32, north: f32, bearing: f32, (back, front, half): (f32, f32, f32), width: f32, c: Rgba<u8>) {
        let (s, co) = bearing.to_radians().sin_cos();
        // ahead along the bearing, and across it to its right
        let at = |along: f32, across: f32| self.pixel(east + along * s + across * co, north + along * co - across * s);
        let corners = [at(front, -half), at(front, half), at(-back, half), at(-back, -half)];
        for k in 0..4 {
            self.line(corners[k], corners[(k + 1) % 4], width, c);
        }
    }

    /// `text` (digits and capital letters of [`glyph`]) in blocks `size` pixels square, centred on
    /// `at`, on a dark plate so it reads over the snow.
    fn label(&mut self, at: (f32, f32), text: &str, size: f32, c: Rgba<u8>) {
        let width = text.len() as f32 * 4.0 * size - size;
        let (x0, y0) = (at.0 - width / 2.0, at.1 - 2.5 * size);
        for py in (y0 - size) as i32..(y0 + 6.0 * size) as i32 {
            for px in (x0 - size) as i32..(x0 + width + size) as i32 {
                if px >= 0 && py >= 0 {
                    self.blend(px as u32, py as u32, Rgba([0, 0, 0, 170]));
                }
            }
        }
        for (k, ch) in text.chars().enumerate() {
            let rows = glyph(ch);
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..3 {
                    if bits & (0b100 >> col) != 0 {
                        let (bx, by) = (x0 + (k as f32 * 4.0 + col as f32) * size, y0 + row as f32 * size);
                        for py in by as i32..(by + size) as i32 {
                            for px in bx as i32..(bx + size) as i32 {
                                if px >= 0 && py >= 0 {
                                    self.blend(px as u32, py as u32, c);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn circle(&mut self, centre: (f32, f32), r: f32, c: Rgba<u8>) {
        let steps = (r * 8.0) as usize;
        for k in 0..steps {
            let a = k as f32 / steps as f32 * std::f32::consts::TAU;
            self.dot((centre.0 + r * a.cos(), centre.1 + r * a.sin()), 1.0, c);
        }
    }

    /// `c` laid over the pixel at `x`, `y` by its alpha.
    fn blend(&mut self, x: u32, y: u32, c: Rgba<u8>) {
        if x >= self.img.width() || y >= self.img.height() {
            return;
        }
        let p = self.img.get_pixel_mut(x, y);
        let a = f32::from(c[3]) / 255.0;
        for k in 0..3 {
            p[k] = (f32::from(p[k]) * (1.0 - a) + f32::from(c[k]) * a) as u8;
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [photo, plan, out] = args.as_slice() else {
        anyhow::bail!("usage: camp_plan <photo.png> <plan.txt> <out.png>");
    };
    let photo = image::open(photo)?.to_rgba8();
    let (cx, cy) = (photo.width() as f32 / 2.0, photo.height() as f32 / 2.0);
    let plan = std::fs::read_to_string(plan)?;
    let view = view_of(&plan, (cx / PIXELS_PER_YARD, cy / PIXELS_PER_YARD));
    let (x0, y0) = (cx + view.0 * PIXELS_PER_YARD, cy - view.3 * PIXELS_PER_YARD);
    let (w, h) = ((view.1 - view.0) * PIXELS_PER_YARD, (view.3 - view.2) * PIXELS_PER_YARD);
    let crop = image::imageops::crop_imm(&photo, x0 as u32, y0 as u32, w as u32, h as u32).to_image();
    let img = image::imageops::resize(&crop, (w * ZOOM) as u32, (h * ZOOM) as u32, image::imageops::FilterType::CatmullRom);
    let mut canvas = Canvas { img, view };
    let s = PIXELS_PER_YARD * ZOOM;
    // a faint 5-yard grid
    let grid = Rgba([255, 255, 255, 70]);
    let (width, height) = canvas.img.dimensions();
    for e in (view.0 as i32..=view.1 as i32).filter(|e| e % 5 == 0) {
        let x = ((e as f32 - view.0) * s) as u32;
        for y in 0..height {
            canvas.blend(x, y, grid);
        }
    }
    for n in (view.2 as i32..=view.3 as i32).filter(|n| n % 5 == 0) {
        let y = ((view.3 - n as f32) * s) as u32;
        for x in 0..width {
            canvas.blend(x, y, grid);
        }
    }
    let orange = Rgba([255, 165, 0, 255]);
    let brown = Rgba([110, 60, 20, 255]);
    let wheat = Rgba([245, 222, 179, 255]);
    let red = Rgba([230, 20, 20, 255]);
    let lime = Rgba([60, 255, 60, 255]);
    let fire = Rgba([255, 69, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let cyan = Rgba([0, 230, 255, 255]);
    for line in plan.lines() {
        let p: Vec<&str> = line.trim().split('|').collect();
        let num = |i: usize| p.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        match p.first() {
            Some(&"TENT") => {
                let (e, n, bearing) = (num(2), num(3), num(4));
                let middle = canvas.pixel(e, n);
                canvas.circle(middle, num(8) * s, white);
                canvas.turned_box(e, n, bearing, LARGEST, 3.0, orange);
                canvas.turned_box(e, n, bearing, BASIC, 2.0, brown);
                canvas.turned_box(e, n, bearing, (BASIC.0, BASIC.1, BASIC_CANVAS), 4.0, wheat);
                // the door: the middle of the canvas's front edge
                let (sn, co) = bearing.to_radians().sin_cos();
                let a = canvas.pixel(e + BASIC.1 * sn - 1.5 * co, n + BASIC.1 * co + 1.5 * sn);
                let b = canvas.pixel(e + BASIC.1 * sn + 1.5 * co, n + BASIC.1 * co - 1.5 * sn);
                canvas.line(a, b, 5.0, red);
                let hero = canvas.pixel(num(6), num(7));
                canvas.dot(hero, 6.0, lime);
                // its number (`P3` is 3), on the canvas's middle
                let number = p.get(1).map_or("", |n| n.trim_start_matches('P'));
                let canvas_middle = canvas.pixel(e + 1.8 * sn, n + 1.8 * co);
                canvas.label(canvas_middle, number, 5.0, Rgba([255, 255, 0, 255]));
            }
            Some(&"SPOT") => {
                // a spot to choose: a ring of its size, its letter (if it has one) above it
                let at = canvas.pixel(num(2), num(3));
                let yellow = Rgba([255, 230, 0, 255]);
                canvas.circle(at, num(4) * s, yellow);
                if let Some(letter) = p.get(1).filter(|l| !l.is_empty()) {
                    canvas.label((at.0, at.1 - num(4) * s - 22.0), letter, 4.0, yellow);
                }
            }
            Some(&"FIRE") => {
                let f = canvas.pixel(num(1), num(2));
                canvas.dot(f, 9.0, fire);
            }
            Some(&"BOX") => {
                // a thing's true footprint, turned as it faces, a tick on its front
                let (e, n, bearing) = (num(2), num(3), num(4));
                canvas.turned_box(e, n, bearing, (num(5), num(6), num(7)), 3.0, cyan);
                let (sn, co) = bearing.to_radians().sin_cos();
                let front = canvas.pixel(e + num(6) * sn, n + num(6) * co);
                canvas.line(canvas.pixel(e, n), front, 3.0, cyan);
                canvas.dot(canvas.pixel(e, n), 3.0, cyan);
            }
            _ => {}
        }
    }
    // the anchor spot
    let o = canvas.pixel(0.0, 0.0);
    canvas.circle(o, 9.0, red);
    canvas.img.save(out)?;
    Ok(())
}

/// A character of the labels, five rows of three blocks (a digit, or a letter used in the plans'
/// names); anything else is blank.
fn glyph(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        _ => [0; 5],
    }
}

/// The yards to show for `plan`: its things and the anchor spot, with [`MARGIN`] round them, kept
/// within the photo's `half` (east, north) reach.
fn view_of(plan: &str, half: (f32, f32)) -> (f32, f32, f32, f32) {
    let mut points = vec![(0.0_f32, 0.0_f32)];
    for line in plan.lines() {
        let p: Vec<&str> = line.trim().split('|').collect();
        let num = |i: usize| p.get(i).and_then(|v| v.parse::<f32>().ok());
        let at = match p.first() {
            Some(&"TENT") | Some(&"BOX") => num(2).zip(num(3)),
            Some(&"FIRE") => num(1).zip(num(2)),
            _ => None,
        };
        points.extend(at);
    }
    let (mut e0, mut e1, mut n0, mut n1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for (e, n) in points {
        (e0, e1, n0, n1) = (e0.min(e), e1.max(e), n0.min(n), n1.max(n));
    }
    let edge = 1.0;
    (
        (e0 - MARGIN).max(-half.0 + edge),
        (e1 + MARGIN).min(half.0 - edge),
        (n0 - MARGIN).max(-half.1 + edge),
        (n1 + MARGIN).min(half.1 - edge),
    )
}
