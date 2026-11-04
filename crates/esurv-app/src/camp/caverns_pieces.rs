//! The Caverns of Time entrance taken apart for the neutral camp: its wrecks and
//! ruins are one building, so the game cuts it as it loads it from the player's own WoW folder and
//! sets each piece where it was drawn, in the mountain sides round the camp's low ground
//! (plans/hub-decisions.md, "The Caverns building taken apart"). The leans and heights are a
//! first try: each piece is tilted to the slope under its new spot, at most
//! 20°, the pillars kept upright, and sunk to sit in the ground.
//!
//! Every number is in the building's own frame (`cavernsoftimeentrance.wmo`): x is the camp's
//! north turned about, `x = 5.244 − north`, `y = east − 73.32`, `z = up − 3.33`.

use benilla_formats::{set_wmo_carve, CarveMove, CarvePiece, WmoCarve};

const BUILDING: &str = "world\\wmo\\dungeon\\cavernsoftime\\cavernsoftimeentrance.wmo";

/// The sun the ground's shadows round the building were baked with, WoW frame (north, west, up):
/// measured, not chosen. A ray cast from each ground texel toward it through the building as it
/// stands in the install agrees with the baked MCSH best at 45.5° round from north toward west and
/// 46.5° up (38,531 shaded texels matched, 486 not).
pub(super) const BAKED_SUN: [f32; 3] = [0.4825, 0.4910, 0.7254];

/// One piece: its area's x and y, the textures that pick it out (empty: any), and its move.
type Piece = ([f32; 2], [f32; 2], &'static [&'static str], Option<([f32; 3], [f32; 4], [f32; 3])>);

/// Tried in order; the stone ruins and the cave mouth stay where they are.
const PIECES: [Piece; 10] = [
    // 1 the big shipwreck
    ([17.2, 100.2], [-27.3, 31.7], &["SHIPWRECK"], Some(([58.84, 2.58, -11.96], [0.17132, 0.02836, 0.0, 0.98481], [68.54, 44.88, -9.58]))),
    // 2 the small shipwreck: taken away
    ([35.2, 85.2], [-113.3, -27.3], &["SHIPWRECK"], None),
    // 6 the troll ladder, leaned 30° sideways toward downhill and sunk so half its height lies under the ground at
    // its deepest
    ([-60.8, -33.8], [-93.3, -77.3], &["TROLL"], Some(([-47.26, -85.32, -12.80], [-0.39553, -0.13131, 0.20916, 0.88463], [29.24, -168.32, -10.07]))),
    // 7 the bridge
    ([-54.8, -35.8], [-69.3, -46.3], &["DARKSHOREBRIDGE"], Some(([-45.16, -57.62, -13.42], [0.00244, -0.04734, 0.0, 0.99888], [-29.16, -59.22, -14.07]))),
    // 4 the orc ruin: taken away, a whole hut stands at its spot instead
    ([-50.8, -6.8], [-173.3, -121.3], &[], None),
    // 3 the dwarf stone hall, turned back round to its first turn under the same lean; and set 3 yards further into the
    // mountain, straight away from the anchor spot, at the same height ("embedd it deeper into the
    // wall"; was -61.06, -154.22); then leaned with the mountain (52.75°, was 20°) and lifted 5.8,
    // fit so the part that stood above the old sand is what sticks out of the mountain; then
    // pushed 10 yards uphill into the mountain and set so 60% of its height is under the ground, the
    // most it sticks out anywhere 40% of its 58.4 height
    ([25.2, 89.2], [-79.3, -7.3], &[], Some(([57.54, -42.82, -11.02], [-0.16642, 0.41190, 0.0, 0.89591], [-72.23, -160.29, 16.91]))),
    // 5 the ruin wall
    ([-84.8, -49.8], [-113.3, -29.3], &[], Some(([-67.36, -70.72, -14.17], [0.03875, 0.16927, -0.08840, 0.98083], [-89.66, -22.92, -14.47]))),
    // 8 the pillars and rubble
    ([23.2, 65.2], [-155.3, -115.3], &[], Some(([44.44, -134.72, -11.78], [0.0, 0.0, 0.0, 1.0], [55.54, -136.72, -11.10]))),
    // 9 the snake pillars
    ([40.2, 93.2], [32.7, 70.7], &[], Some(([66.34, 51.68, -11.94], [0.0, 0.0, -0.43051, 0.90259], [115.64, 1.28, -10.84]))),
    // 10 the stone ruins: stay
    ([81.2, 130.2], [-118.3, -33.3], &[], Some(([0.0; 3], [0.0, 0.0, 0.0, 1.0], [0.0; 3]))),
];

/// Cut the Caverns building as it loads, when the hub is on ([`crate::camp::hub_on`]).
pub(crate) fn register() {
    if !crate::camp::hub_on() {
        return;
    }
    let pieces = PIECES
        .iter()
        .map(|&(x, y, textures, to)| CarvePiece {
            area: (x, y),
            textures: textures.iter().map(|t| t.to_string()).collect(),
            to: to.map(|(pivot, turn, to)| CarveMove { pivot, turn, to }),
        })
        .collect();
    set_wmo_carve(BUILDING, WmoCarve { pieces, sun: Some(BAKED_SUN) });
}
