//! What the game's own screens share. Every screen but the meter (which has a look of its own,
//! after the Recount addon) fits the stock 1.12 interface: its frames, borders, buttons, fonts
//! and colours, read from the player's own data. Here: the fonts, the stock art's paths, the
//! stock text colours, class and rarity colours, the order screens stack in, the game's content
//! (names, icons and texts for what the server names by id), and when a screen may show. Every
//! screen's root node carries `benilla_app::OverlayWindow`, so the clicks over it are its own.

use std::sync::Arc;

use benilla_app::Objects;
use bevy::prelude::*;
use esurv_content::Content;

/// The stock interface's fonts: Friz Quadrata (every `GameFont*`: headings, labels, tooltips),
/// Arial Narrow (`NumberFont*`: counts and numbers) and Morpheus (`QuestTitleFont`: big titles).
pub const TITLE_FONT: &str = "mpq://Fonts/FRIZQT__.ttf";
pub const BODY_FONT: &str = "mpq://Fonts/ARIALN.ttf";
pub const MORPHEUS_FONT: &str = "mpq://Fonts/MORPHEUS.ttf";

/// The fonts, loaded once: `title` is Friz Quadrata, `body` Arial Narrow, `morpheus` Morpheus.
#[derive(Resource, Clone)]
pub struct UiFonts {
    pub title: Handle<Font>,
    pub body: Handle<Font>,
    pub morpheus: Handle<Font>,
}

/// The stock art the screens are made of (paths in the player's data, loaded in the stock kit's
/// form by [`crate::stock::load_stock_art`]).
pub mod art {
    /// The dialog box (`StaticPopup`, `GameMenuFrame`): its tiled background, its edge file
    /// (eight squares in a row: left, right, top, bottom, then the four corners) and its header.
    pub const DIALOG_BACKGROUND: &str = r"Interface\DialogFrame\UI-DialogBox-Background";
    pub const DIALOG_BORDER: &str = r"Interface\DialogFrame\UI-DialogBox-Border";
    pub const DIALOG_HEADER: &str = r"Interface\DialogFrame\UI-DialogBox-Header";
    /// The tooltip's background and edge (`GameTooltip`).
    pub const TOOLTIP_BACKGROUND: &str = r"Interface\Tooltips\UI-Tooltip-Background";
    pub const TOOLTIP_BORDER: &str = r"Interface\Tooltips\UI-Tooltip-Border";
    /// The red panel button (`UIPanelButtonTemplate`) up, down, highlighted and disabled.
    pub const BUTTON_UP: &str = r"Interface\Buttons\UI-Panel-Button-Up";
    pub const BUTTON_DOWN: &str = r"Interface\Buttons\UI-Panel-Button-Down";
    pub const BUTTON_HIGHLIGHT: &str = r"Interface\Buttons\UI-Panel-Button-Highlight";
    pub const BUTTON_DISABLED: &str = r"Interface\Buttons\UI-Panel-Button-Disabled";
    /// An action-button style icon frame and its highlight.
    pub const ICON_BORDER: &str = r"Interface\Buttons\UI-Quickslot2";
    pub const ICON_HIGHLIGHT: &str = r"Interface\Buttons\ButtonHilight-Square";
    /// The stock status bar fill (`UI-StatusBar`), as unit frames and cast bars use.
    pub const STATUS_BAR: &str = r"Interface\TargetingFrame\UI-StatusBar";
    /// A window's tab (`CharacterFrameTabButtonTemplate`): the chosen tab, the others, and the
    /// glow under the pointer.
    pub const TAB_ACTIVE: &str = r"Interface\PaperDollInfoFrame\UI-Character-ActiveTab";
    pub const TAB_INACTIVE: &str = r"Interface\PaperDollInfoFrame\UI-Character-InActiveTab";
    pub const TAB_HIGHLIGHT: &str = r"Interface\PaperDollInfoFrame\UI-Character-Tab-Highlight";
    /// A window's close button (`UIPanelCloseButton`).
    pub const CLOSE_UP: &str = r"Interface\Buttons\UI-Panel-MinimizeButton-Up";
    pub const CLOSE_DOWN: &str = r"Interface\Buttons\UI-Panel-MinimizeButton-Down";
    pub const CLOSE_DISABLED: &str = r"Interface\Buttons\UI-Panel-MinimizeButton-Disabled";
    pub const CLOSE_HIGHLIGHT: &str = r"Interface\Buttons\UI-Panel-MinimizeButton-Highlight";
    /// The spellbook's page arrows, and the round glow over them.
    pub const PREVIOUS_PAGE_UP: &str = r"Interface\Buttons\UI-SpellbookIcon-PrevPage-Up";
    pub const PREVIOUS_PAGE_DOWN: &str = r"Interface\Buttons\UI-SpellbookIcon-PrevPage-Down";
    pub const PREVIOUS_PAGE_DISABLED: &str =
        r"Interface\Buttons\UI-SpellbookIcon-PrevPage-Disabled";
    pub const NEXT_PAGE_UP: &str = r"Interface\Buttons\UI-SpellbookIcon-NextPage-Up";
    pub const NEXT_PAGE_DOWN: &str = r"Interface\Buttons\UI-SpellbookIcon-NextPage-Down";
    pub const NEXT_PAGE_DISABLED: &str = r"Interface\Buttons\UI-SpellbookIcon-NextPage-Disabled";
    pub const PAGE_HIGHLIGHT: &str = r"Interface\Buttons\UI-Common-MouseHilight";
}

/// The stock interface's text colours: `NORMAL_FONT_COLOR` (gold), `HIGHLIGHT_FONT_COLOR`
/// (white), `GRAY_FONT_COLOR`, `GREEN_FONT_COLOR`, `RED_FONT_COLOR`.
pub const GOLD_TEXT: Color = Color::srgb(1.0, 0.82, 0.0);
pub const WHITE_TEXT: Color = Color::WHITE;
pub const GRAY_TEXT: Color = Color::srgb(0.5, 0.5, 0.5);
pub const GREEN_TEXT: Color = Color::srgb(0.1, 1.0, 0.1);
pub const RED_TEXT: Color = Color::srgb(1.0, 0.1, 0.1);
/// The shadow under text, as the stock fonts have.
pub const TEXT_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.95);

/// The stock class colours of the 1.12 interface (`RAID_CLASS_COLORS`), by class id.
pub const CLASS_COLOURS: [(&str, [f32; 3]); 9] = [
    ("warrior", [0.78, 0.61, 0.43]),
    ("paladin", [0.96, 0.55, 0.73]),
    ("hunter", [0.67, 0.83, 0.45]),
    ("rogue", [1.0, 0.96, 0.41]),
    ("priest", [1.0, 1.0, 1.0]),
    ("shaman", [0.96, 0.55, 0.73]),
    ("mage", [0.41, 0.8, 0.94]),
    ("warlock", [0.58, 0.51, 0.79]),
    ("druid", [1.0, 0.49, 0.04]),
];

/// A class's colour by its id.
pub fn class_colour(class: &str) -> Option<Color> {
    CLASS_COLOURS
        .iter()
        .find(|(id, _)| *id == class)
        .map(|(_, [r, g, b])| Color::srgb(*r, *g, *b))
}

/// A card's or item's rarity colour, 0 common to 4 legendary: the 1.12 item quality colours
/// (`ITEM_QUALITY_COLORS`: white, green, blue, purple, orange).
pub fn rarity_colour(rarity: u8) -> Color {
    match rarity {
        0 => Color::srgb(1.0, 1.0, 1.0),
        1 => Color::srgb(0.12, 1.0, 0.0),
        2 => Color::srgb(0.0, 0.44, 0.87),
        3 => Color::srgb(0.64, 0.21, 0.93),
        _ => Color::srgb(1.0, 0.5, 0.0),
    }
}

/// The order the game's screens stack in, all over the game view and the stock UI and under the
/// loading cover (1000), the screen fade (1050) and the cinematic (900).
pub mod layer {
    /// The meter and the HUD's pieces.
    pub const METER: i32 = 100;
    pub const HUD: i32 = 110;
    /// A choice screen covers the HUD.
    pub const CARDS: i32 = 200;
    /// The end of a run covers everything else of the run.
    pub const RUN_END: i32 = 300;
    /// The run's Escape menu covers the end panel too.
    pub const GAME_MENU: i32 = 310;
    /// A tooltip shows over every window.
    pub const TOOLTIP: i32 = 800;
    /// The hero picker's parts, over the stock select screen (1100); they go before the loading
    /// cover rises.
    pub const PICKER: i32 = 1200;
}

/// The game's content (`content\*.ron`): what a screen shows for an id the server names.
#[derive(Resource, Clone)]
pub struct GameContent(pub Arc<Content>);

/// Whether the hero is in the world (the screens of a run show only then).
pub fn in_world(objects: &Objects) -> bool {
    objects.player().is_some()
}

/// Loads the fonts and keeps the content.
pub(crate) fn register(app: &mut App, content: Option<Arc<Content>>) {
    let assets = app.world().resource::<AssetServer>().clone();
    app.insert_resource(UiFonts {
        title: assets.load(TITLE_FONT),
        body: assets.load(BODY_FONT),
        morpheus: assets.load(MORPHEUS_FONT),
    });
    if let Some(content) = content {
        app.insert_resource(GameContent(content));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_and_rarity_colours() {
        assert!(class_colour("mage").is_some() && class_colour("ogre").is_none());
        assert_eq!(
            rarity_colour(9),
            rarity_colour(4),
            "past legendary stays legendary"
        );
    }
}
