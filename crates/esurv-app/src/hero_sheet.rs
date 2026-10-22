//! What the hero sheet will show before a run (the menu rework; menus.md, "The flow"): a class's
//! progress between runs and the codex. The armory and codex are not in the run (the run's
//! Escape menu has neither, and N and L open nothing of ours); the menu rework draws them in
//! its own look.
//!
//! * [`progress`]: the class's progress as the server says it (`Progress`, `Ready`, the run
//!   options' bonus), what the player picked, and the changes it asks (`learn`, `unlearn`, `wear`,
//!   `takeoff`, `look`) one at a time, with the refusals in our words.
//! * [`talents`]: the class's trees under the server's own rules: ranks, what can be learned or
//!   taken back, how each talent shows, its tooltip.
//! * [`gear`]: the class's pieces by slot and zone, what is worn and shown, what worn gear adds,
//!   the drop tables (which boss drops what, at what chance), a piece's tooltip.
//! * [`item_icons`]: a piece's icon from the player's own item displays.
//! * [`codex`]: the codex's lists and pages, and what the save has found (`Codex`).

pub mod codex;
pub mod gear;
pub mod item_icons;
pub mod prestige;
pub mod progress;
pub mod racials;
pub mod talents;

pub use codex::{CodexEntry, CodexList, CodexState};
pub use progress::{HeroSheetState, HeroSheetTab, ProgressChange};
