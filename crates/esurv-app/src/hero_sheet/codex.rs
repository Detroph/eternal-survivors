//! The codex: every ring, trinket and amulet taken during play, by any
//! hero of the save, kept for good. Its lists, from the game's content: every ring, every
//! trinket, every amulet, and every remade legendary amulet (one per class spell whose delivery
//! kind has a recipe), each with its icon, name, text and where it is found; how many of each the
//! save has collected; the lists cut into pages (the legendary amulets a class a page); and
//! [`CodexState`], what the server says has been found (`Codex`) with the list and page shown.

use std::collections::BTreeSet;

use bevy::prelude::Resource;
use esurv_content::{Content, JewelrySlot};
use esurv_protocol::ToClient;


/// How many pieces a page of a list holds (two columns of six).
pub const PER_PAGE: usize = 12;

/// The codex's lists, left to right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodexList {
    #[default]
    Rings,
    Trinkets,
    Amulets,
    Legendary,
}

impl CodexList {
    pub const ALL: [CodexList; 4] = [
        CodexList::Rings,
        CodexList::Trinkets,
        CodexList::Amulets,
        CodexList::Legendary,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CodexList::Rings => "Rings",
            CodexList::Trinkets => "Trinkets",
            CodexList::Amulets => "Amulets",
            CodexList::Legendary => "Legendary",
        }
    }

    /// Where its pieces are found, for one not found yet.
    pub fn found_in(self) -> &'static str {
        match self {
            CodexList::Rings | CodexList::Trinkets | CodexList::Amulets => {
                "Not found yet: caches hold them, bought or a rare's."
            }
            CodexList::Legendary => {
                "Not found yet: a cache's legendary amulet, for a spell the hero knows."
            }
        }
    }
}

/// One piece of a list.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexEntry {
    /// Its id, as the codex keeps it.
    pub id: String,
    pub name: String,
    pub icon: String,
    pub text: String,
    /// Its quality colour index (an amulet's rarity; rings and trinkets are found at any).
    pub rarity: Option<u8>,
    /// A legendary amulet's class (its id).
    pub class: Option<String>,
}

/// Every piece of `list`, in the content's order (legendary amulets class by class).
pub fn entries(content: &Content, list: CodexList) -> Vec<CodexEntry> {
    match list {
        CodexList::Rings | CodexList::Trinkets => {
            let slot = if list == CodexList::Rings {
                JewelrySlot::Ring
            } else {
                JewelrySlot::Trinket
            };
            content
                .jewelry
                .iter()
                .filter(|j| j.slot == slot)
                .map(|j| CodexEntry {
                    id: j.id.clone(),
                    name: j.name.clone(),
                    icon: j.icon.clone(),
                    text: j.text_at(j.value),
                    rarity: None,
                    class: None,
                })
                .collect()
        }
        CodexList::Amulets => content
            .amulets
            .iter()
            .map(|a| CodexEntry {
                id: a.id.clone(),
                name: a.name.clone(),
                icon: a.icon.clone(),
                text: a.text.clone(),
                rarity: Some(a.rarity.index()),
                class: None,
            })
            .collect(),
        CodexList::Legendary => content
            .classes
            .iter()
            .flat_map(|class| {
                // the starting options' spells first, then every card the class can be offered
                let mut ids: Vec<&String> = Vec::new();
                let options = class.starting_options.iter();
                let all = options.clone().flat_map(|o| &o.spells).chain(options.flat_map(|o| o.card_pool.iter().flatten()));
                for id in all.chain(&class.abilities) {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
                // one legendary per spell (a strike and its finisher share one: the strike's)
                ids.into_iter().filter_map(move |id| {
                    let l = content.legendary_amulets.iter().find(|l| l.main_spell() == id.as_str())?;
                    Some(CodexEntry {
                        id: l.id(),
                        name: l.name.clone(),
                        icon: l.icon.clone(),
                        text: l.text.clone(),
                        rarity: Some(l.rarity.index()),
                        class: Some(class.id.clone()),
                    })
                })
            })
            .collect(),
    }
}

/// How many of `list` are found, of how many.
pub fn found_of(content: &Content, list: CodexList, found: &BTreeSet<String>) -> (usize, usize) {
    let all = entries(content, list);
    let have = all.iter().filter(|e| found.contains(&e.id)).count();
    (have, all.len())
}

/// The classes that have a legendary amulet, in the content's order.
pub fn legendary_classes(content: &Content) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in entries(content, CodexList::Legendary) {
        if let Some(c) = e.class {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

/// The pages of a list: for the legendary amulets one per class, for the others as many as
/// their pieces fill.
pub fn pages(content: &Content, list: CodexList) -> usize {
    match list {
        CodexList::Legendary => legendary_classes(content).len().max(1),
        _ => entries(content, list).len().div_ceil(PER_PAGE).max(1),
    }
}

/// The pieces on page `page` of `list`, and the class it is for (legendary amulets).
pub fn page_entries(
    content: &Content,
    list: CodexList,
    page: usize,
) -> (Vec<CodexEntry>, Option<String>) {
    let all = entries(content, list);
    if list == CodexList::Legendary {
        let class = legendary_classes(content).get(page).cloned();
        let entries = all.into_iter().filter(|e| e.class == class).collect();
        return (entries, class);
    }
    (
        all.into_iter()
            .skip(page * PER_PAGE)
            .take(PER_PAGE)
            .collect(),
        None,
    )
}

/// How much of every list is found.
pub fn summary(content: &Content, found: &BTreeSet<String>) -> String {
    let (mut have, mut all) = (0, 0);
    for list in CodexList::ALL {
        let (h, a) = found_of(content, list, found);
        have += h;
        all += a;
    }
    format!("{have} of {all} pieces found in all.")
}

/// The page the legendary amulets open on: the class named `class_name`'s (the hero's own), else
/// the first.
pub fn class_page(content: &Content, class_name: Option<&str>) -> usize {
    class_name
        .and_then(|name| {
            legendary_classes(content)
                .iter()
                .position(|id| content.class(id).is_some_and(|c| c.name == name))
        })
        .unwrap_or(0)
}

/// What the codex knows and shows: what the server says the save has found, the hero's class
/// (whose legendary amulets show first), and the list and page shown.
#[derive(Resource, Debug, Default)]
pub struct CodexState {
    /// Every piece found, by id; `None` until the server said.
    found: Option<BTreeSet<String>>,
    /// The hero's class (its name), whose legendary amulets show first.
    class_name: Option<String>,
    list: CodexList,
    /// The page of the list shown (for the legendary amulets: the class).
    page: usize,
    version: u32,
}

impl CodexState {
    pub fn found(&self) -> Option<&BTreeSet<String>> {
        self.found.as_ref()
    }

    pub fn list(&self) -> CodexList {
        self.list
    }

    pub fn page(&self) -> usize {
        self.page
    }

    pub fn class_name(&self) -> Option<&str> {
        self.class_name.as_deref()
    }

    /// Changes whenever what a codex page shows changes.
    pub fn version(&self) -> u32 {
        self.version
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Takes in a server message; `true` when what it shows changed.
    pub fn take(&mut self, m: &ToClient) -> bool {
        match m {
            ToClient::Codex { found } => {
                self.found = Some(found.iter().cloned().collect());
                self.changed();
                true
            }
            ToClient::Ready(r) => {
                self.class_name = Some(r.class.clone());
                self.changed();
                true
            }
            _ => false,
        }
    }

    /// Shows `list`, from its first page (the hero's class first, for the legendary amulets).
    pub fn choose(&mut self, list: CodexList, first_page: usize) {
        if self.list != list {
            self.list = list;
            self.page = first_page;
            self.changed();
        }
    }

    pub fn turn_to(&mut self, page: usize) {
        if self.page != page {
            self.page = page;
            self.changed();
        }
    }

    /// Forgets what the server said (it is told again on the next hello).
    pub fn forget(&mut self) {
        self.found = None;
        self.class_name = None;
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero_sheet::gear::tests::content;

    #[test]
    fn a_list_is_paged_and_the_legendary_amulets_go_by_class() {
        let c = content();
        let rings = entries(c, CodexList::Rings).len();
        assert_eq!(pages(c, CodexList::Rings), rings.div_ceil(PER_PAGE));
        let (first, class) = page_entries(c, CodexList::Rings, 0);
        assert_eq!(first.len(), rings.min(PER_PAGE));
        assert_eq!(class, None);
        assert_eq!(pages(c, CodexList::Legendary), c.classes.len());
        let mage_page = legendary_classes(c)
            .iter()
            .position(|x| x == "mage")
            .unwrap();
        let (mage, class) = page_entries(c, CodexList::Legendary, mage_page);
        assert_eq!(class.as_deref(), Some("mage"));
        assert!(mage.iter().any(|e| e.id == "legendary:fireball"));
        // with the starting options a class can have more than twelve: the page lists them all
        assert!(!mage.is_empty());
        assert_eq!(class_page(c, Some("Class 8")), mage_page);
        assert_eq!(class_page(c, None), 0);
        let found = ["accuria".to_owned()].into_iter().collect();
        assert!(summary(c, &found).starts_with("1 of "));
    }

    #[test]
    fn the_codex_is_what_the_server_said_last() {
        let mut s = CodexState::default();
        assert!(s.found().is_none());
        let v = s.version();
        assert!(s.take(&ToClient::Codex {
            found: vec![
                "accuria".into(),
                "accuria".into(),
                "legendary:fireball".into()
            ]
        }));
        assert_eq!(s.found().unwrap().len(), 2);
        assert!(s.version() != v);
        assert!(!s.take(&ToClient::Toast("hi".into())));
        s.choose(CodexList::Legendary, 6);
        assert_eq!((s.list(), s.page()), (CodexList::Legendary, 6));
        s.turn_to(2);
        s.choose(CodexList::Legendary, 6);
        assert_eq!(s.page(), 2, "the same list keeps its page");
        s.choose(CodexList::Rings, 0);
        assert_eq!(s.page(), 0);
        s.forget();
        assert!(s.found().is_none());
    }

    #[test]
    fn every_list_comes_from_the_content() {
        let c = content();
        let rings = entries(c, CodexList::Rings);
        let trinkets = entries(c, CodexList::Trinkets);
        assert_eq!(rings.len() + trinkets.len(), c.jewelry.len());
        let accuria = rings.iter().find(|e| e.id == "accuria").unwrap();
        assert_eq!(accuria.text, "+15% damage");
        assert_eq!(entries(c, CodexList::Amulets).len(), c.amulets.len());
        let legendary = entries(c, CodexList::Legendary);
        let fireball = legendary
            .iter()
            .find(|e| e.id == "legendary:fireball")
            .unwrap();
        assert_eq!(fireball.class.as_deref(), Some("mage"));
        assert_eq!(fireball.name, "Phoenix");
        assert_eq!(legendary.len(), c.legendary_amulets.len(), "one entry per legendary");
        assert!(legendary
            .iter()
            .all(|e| !e.text.contains('{') && e.rarity == Some(4)));
        assert_eq!(legendary_classes(c).len(), c.classes.len());
        let mut ids: Vec<&str> = legendary.iter().map(|e| e.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "one per spell");
    }

    #[test]
    fn found_pieces_are_counted_per_list() {
        let c = content();
        let found: BTreeSet<String> = ["accuria", "legendary:fireball", "no_such_piece"]
            .into_iter()
            .map(String::from)
            .collect();
        let (have, all) = found_of(c, CodexList::Rings, &found);
        assert_eq!(have, 1);
        assert!(all > 1);
        assert_eq!(found_of(c, CodexList::Legendary, &found).0, 1);
        assert_eq!(found_of(c, CodexList::Amulets, &found).0, 0);
    }
}
