//! Which zones the save has discovered: every zone slot is listed, so
//! the player sees how many there are, but a zone no class of the save has opened yet shows only
//! [`UNDISCOVERED`] and a lock, with no name, art, enemies, bosses or rewards. Once any class opens
//! it, every hero sees it in full; each class still opens it the normal way to play it.

use esurv_content::Content;
use esurv_protocol::Progress;

/// What an undiscovered zone shows in place of its name and everything else.
pub const UNDISCOVERED: &str = "???";

/// The mark beside an undiscovered zone: a key, the client having no padlock of its own.
pub const LOCK_ICON: &str = r"Interface\Icons\INV_Misc_Key_03";

/// Whether zone `index` (in the zones' difficulty order) is discovered, for a class with `open`
/// zones open and the save's `discovered` zone ids: the first zone always is, so is a zone open
/// to the class.
pub fn is_discovered(content: &Content, discovered: &[String], open: usize, index: usize) -> bool {
    index == 0
        || index < open
        || content
            .zones
            .get(index)
            .is_some_and(|z| discovered.iter().any(|d| *d == z.id))
}

/// [`is_discovered`] from a class's progress (none known: only the first zone).
pub fn discovered_in(content: &Content, progress: Option<&Progress>, index: usize) -> bool {
    match progress {
        Some(p) => is_discovered(content, &p.zones_discovered, p.zones_open as usize, index),
        None => index == 0,
    }
}

/// Zone `index`'s name as it may be shown: [`UNDISCOVERED`] until discovered ("" for no zone).
pub fn shown_name<'c>(content: &'c Content, progress: Option<&Progress>, index: usize) -> &'c str {
    match content.zones.get(index) {
        Some(_) if !discovered_in(content, progress, index) => UNDISCOVERED,
        Some(z) => z.name.as_str(),
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    fn progress(open: u32, discovered: &[&str]) -> Progress {
        Progress {
            class: "mage".into(),
            talent_experience: 0,
            points: 0,
            spent: 0,
            into_next: 0,
            next_cost: 0,
            zones_open: open,
            zones_discovered: discovered.iter().map(|s| s.to_string()).collect(),
            talents: vec![],
            worn: vec![],
            looks: vec![],
            collection: vec![],
            prestige: Default::default(),
            loadout: String::new(),
            new_gear: false,
            new_points: false,
        }
    }

    #[test]
    fn a_zone_shows_once_any_class_has_opened_it() {
        let c = content();
        // one open to this class, a second opened by another class
        let p = progress(1, &["elwynn", "duskwood"]);
        assert_eq!(shown_name(&c, Some(&p), 0), c.zones[0].name);
        assert_eq!(shown_name(&c, Some(&p), 1), c.zones[1].name, "discovered, though locked here");
        assert_eq!(shown_name(&c, Some(&p), 2), UNDISCOVERED);
        // open to the class counts even before the server's list says so
        assert!(discovered_in(&c, Some(&progress(3, &[])), 2));
        assert!(!discovered_in(&c, Some(&progress(3, &[])), 3));
        assert!(discovered_in(&c, None, 0) && !discovered_in(&c, None, 1));
        assert_eq!(shown_name(&c, None, 99), "");
    }
}
