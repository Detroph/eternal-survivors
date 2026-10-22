//! The player's own art the menus show: the stock create screen's class and race icon sheets (a
//! cell each, at the stock create screen's own cells), read at runtime like every stock texture.

use bevy::prelude::*;

/// The class icons, a cell per class.
pub const CLASS_ICONS: &str = r"Interface\Glues\CharacterCreate\UI-CharacterCreate-Classes";
/// The race portraits, a cell per race and sex.
pub const RACE_ICONS: &str = r"Interface\Glues\CharacterCreate\UI-CharacterCreate-Races";

/// A class's cell in [`CLASS_ICONS`] by its 1.12 id, as texture coordinates (left, right, top,
/// bottom), as the stock create screen cuts it.
pub fn class_cell(class: u32) -> Option<[f32; 4]> {
    Some(match class {
        1 => [0.0, 0.25, 0.0, 0.25],
        8 => [0.25, 0.49609375, 0.0, 0.25],
        4 => [0.49609375, 0.7421875, 0.0, 0.25],
        11 => [0.7421875, 0.98828125, 0.0, 0.25],
        3 => [0.0, 0.25, 0.25, 0.5],
        7 => [0.25, 0.49609375, 0.25, 0.5],
        5 => [0.49609375, 0.7421875, 0.25, 0.5],
        9 => [0.7421875, 0.98828125, 0.25, 0.5],
        2 => [0.0, 0.25, 0.5, 0.75],
        _ => return None,
    })
}

/// A race's portrait in [`RACE_ICONS`] by its 1.12 id and sex (0 male, 1 female): a quarter of
/// the sheet each way, the women's two rows under the men's.
pub fn race_cell(race: u8, sex: u8) -> Option<[f32; 4]> {
    let (col, row) = match race {
        1 => (0.0, 0.0),
        3 => (1.0, 0.0),
        7 => (2.0, 0.0),
        4 => (3.0, 0.0),
        6 => (0.0, 1.0),
        5 => (1.0, 1.0),
        8 => (2.0, 1.0),
        2 => (3.0, 1.0),
        _ => return None,
    };
    let row = row + if sex == 1 { 2.0 } else { 0.0 };
    Some([
        col * 0.25,
        (col + 1.0) * 0.25,
        row * 0.25,
        (row + 1.0) * 0.25,
    ])
}

/// A cell of a loaded sheet in its pixels; `None` until the sheet has loaded.
pub fn cell_rect(images: &Assets<Image>, sheet: &Handle<Image>, cell: [f32; 4]) -> Option<Rect> {
    let size = images.get(sheet)?.size().as_vec2();
    Some(Rect::new(
        cell[0] * size.x,
        cell[2] * size.y,
        cell[1] * size.x,
        cell[3] * size.y,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_race_and_class_has_its_cell() {
        for class in [1, 2, 3, 4, 5, 7, 8, 9, 11] {
            assert!(class_cell(class).is_some(), "class {class}");
        }
        assert_eq!(class_cell(6), None, "no 1.12 class 6");
        assert_eq!(race_cell(2, 1), Some([0.75, 1.0, 0.75, 1.0]), "orc woman");
        assert_eq!(race_cell(9, 0), None);
    }
}
