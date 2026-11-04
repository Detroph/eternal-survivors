//! What each hero wears besides its gear and companion (a title, a tabard, a weapon glow) and how
//! each camp is decorated, as the save keeps them (docs/game/achievements.md, step 3). Only what
//! the warband owns may be worn or set out; a piece no longer owned (or no longer in the content)
//! is left out when told, never removed from the save.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use esurv_content::{glow_item, tabard_item, Content, Decoration, Earned, Faction, Holdings, SpotKind};
use esurv_protocol::{self as protocol, WearKind};

use super::achievements::{unix_now, SavedAchievements};
use super::{SavedHero, Saves};

/// A hero's title (the id of the achievement giving it), tabard and weapon glow (their ids).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wears {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<String>,
}

impl Wears {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.tabard.is_none() && self.glow.is_none()
    }

    pub fn get(&self, kind: WearKind) -> Option<&str> {
        match kind {
            WearKind::Title => self.title.as_deref(),
            WearKind::Tabard => self.tabard.as_deref(),
            WearKind::Glow => self.glow.as_deref(),
        }
    }

    fn slot(&mut self, kind: WearKind) -> &mut Option<String> {
        match kind {
            WearKind::Title => &mut self.title,
            WearKind::Tabard => &mut self.tabard,
            WearKind::Glow => &mut self.glow,
        }
    }
}

/// The camps decorated apart, by the word the asks name them with.
pub const CAMPS: [(&str, Faction); 2] =
    [("horde", Faction::Horde), ("alliance", Faction::Alliance)];

/// The faction of camp `camp` (`horde`, `alliance`).
pub fn camp_faction(camp: &str) -> Option<Faction> {
    CAMPS.iter().find(|(w, _)| *w == camp).map(|(_, f)| *f)
}

/// The faction of hero `h`, by its race.
pub fn hero_faction(content: &Content, h: &SavedHero) -> Option<Faction> {
    content
        .races
        .iter()
        .find(|r| r.id == h.race)
        .map(|r| r.faction)
}

/// The tent piece free to class `class`.
fn class_piece<'c>(content: &'c Content, class: &str) -> Option<&'c Decoration> {
    content.decorations.pieces.iter().find(|d| d.spot == SpotKind::Tent && d.earned == Some(Earned::Class(class.to_owned())))
}

/// Whether hero `h` may have `piece` at its tent: a tent piece the warband owns, or its class's.
fn tent_may_have(content: &Content, a: &SavedAchievements, h: &SavedHero, piece: &str) -> bool {
    content.decoration(piece).is_some_and(|d| d.spot == SpotKind::Tent)
        && a.with_holdings(|holdings| content.hero_owns_decoration(piece, &h.class, holdings))
}

/// What a hero shows in a run, from what it wears: its name with its title, its tabard's look
/// (`ItemDisplayInfo.dbc`) and its weapons' enchant (whose glow they show).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunWears {
    pub name: String,
    pub tabard: Option<u32>,
    pub enchant: Option<u32>,
}

impl SavedAchievements {
    /// Runs `f` with the warband's holdings as the content's owning rules read them.
    pub fn with_holdings<R>(&self, f: impl FnOnce(&Holdings) -> R) -> R {
        let bought = |i: &str| self.bought.iter().any(|b| b == i);
        let earned = |a: &str| self.earned.contains_key(a);
        let cleared = |z: &str| self.zones_cleared.contains(z);
        f(&Holdings {
            bought: &bought,
            earned: &earned,
            cleared: &cleared,
        })
    }

    /// Whether the warband may wear `id` of `kind`: a title earned, a tabard or glow bought.
    pub fn owns_wear(&self, content: &Content, kind: WearKind, id: &str) -> bool {
        match kind {
            WearKind::Title => content.title_of(id).is_some() && self.earned.contains_key(id),
            WearKind::Tabard => {
                content.tabard(id).is_some() && self.bought.contains(&tabard_item(id))
            }
            WearKind::Glow => {
                content.weapon_glow(id).is_some() && self.bought.contains(&glow_item(id))
            }
        }
    }

    /// Whether decoration `piece` may stand on spot `spot` (on the theme spot, set `piece` be
    /// the camp's theme): the warband owns it (a piece of the set), it fits, the spot is open.
    fn may_stand(&self, content: &Content, spot: &str, piece: &str) -> bool {
        self.with_holdings(|h| content.may_set_out(piece, spot, h))
    }

    /// Each camp's decorations as the screens are told them: those that may still stand.
    pub fn camps_told(&self, content: &Content) -> Vec<protocol::CampLayout> {
        self.camps
            .iter()
            .map(|(camp, spots)| protocol::CampLayout {
                camp: camp.clone(),
                pieces: spots
                    .iter()
                    .filter(|(s, p)| self.may_stand(content, s, p))
                    .map(|(s, p)| (s.clone(), p.clone()))
                    .collect(),
            })
            .filter(|c| !c.pieces.is_empty())
            .collect()
    }

    /// Whether `piece` may stand on spot `spot` of a walkable camp: a shared spot by the fire
    /// (`fire_<n>`) takes any ground piece the warband owns (plans/hub-decoration-spots.md; the
    /// tents' own spots are to come).
    fn may_stand_in_hub(&self, content: &Content, spot: &str, piece: &str) -> bool {
        spot.starts_with("fire_")
            && content.decoration(piece).is_some_and(|d| d.spot == SpotKind::Ground)
            && self.with_holdings(|h| content.owns_decoration(piece, h))
    }

    /// Walkable camp `camp`'s decorations as shown: those that may still stand, by spot.
    pub fn hub_told(&self, content: &Content, camp: &str) -> BTreeMap<String, String> {
        self.hub
            .get(camp)
            .into_iter()
            .flatten()
            .filter(|(s, p)| self.may_stand_in_hub(content, s, p))
            .map(|(s, p)| (s.clone(), p.clone()))
            .collect()
    }

    /// The pieces a walkable camp's spot by the fire may take: every ground piece owned.
    pub fn hub_choices(&self, content: &Content) -> Vec<String> {
        content.decorations.pieces.iter().filter(|d| self.may_stand_in_hub(content, "fire_1", &d.id)).map(|d| d.id.clone()).collect()
    }

    /// Walkable camp `camp` shows `piece` on `spot` (`None`: nothing), moving it off another spot
    /// of the camp: refused (false) for an unknown camp, or a piece that may not stand there.
    pub fn decorate_hub(&mut self, content: &Content, camp: &str, spot: &str, piece: Option<&str>) -> bool {
        if camp_faction(camp).is_none() || piece.is_some_and(|p| !self.may_stand_in_hub(content, spot, p)) {
            return false;
        }
        let layout = self.hub.entry(camp.to_owned()).or_default();
        layout.remove(spot);
        if let Some(p) = piece {
            // a piece stands on one spot of a camp: set out elsewhere, it moves
            layout.retain(|_, q| q != p);
            layout.insert(spot.to_owned(), p.to_owned());
        }
        if layout.is_empty() {
            self.hub.remove(camp);
        }
        true
    }

    /// Camp `camp` shows `piece` on `spot` (`None`: nothing), moving it off another spot of the
    /// camp: refused (false) for an unknown camp or spot, a spot not open, or a piece not owned
    /// or not fitting.
    pub fn decorate(
        &mut self,
        content: &Content,
        camp: &str,
        spot: &str,
        piece: Option<&str>,
    ) -> bool {
        if camp_faction(camp).is_none() || content.camp_spot(spot).is_none() {
            return false;
        }
        if let Some(p) = piece {
            if !self.may_stand(content, spot, p) {
                return false;
            }
        }
        let layout = self.camps.entry(camp.to_owned()).or_default();
        layout.remove(spot);
        if let Some(p) = piece {
            // a piece stands on one spot of a camp: set out elsewhere, it moves
            layout.retain(|_, q| q != p);
            layout.insert(spot.to_owned(), p.to_owned());
        }
        if layout.is_empty() {
            self.camps.remove(camp);
        }
        true
    }
}

impl Saves {
    /// Hero `guid` wears `id` of `kind` (`None`: none), one the warband owns and, for a
    /// tabard, one its faction may wear; written when it changed. False when refused.
    pub fn dress(&self, content: &Content, guid: u32, kind: WearKind, id: Option<&str>) -> bool {
        let mut file = self.file.lock().unwrap();
        if let Some(id) = id {
            let section = file.achievements_section(content, unix_now());
            if !section.owns_wear(content, kind, id) {
                return false;
            }
        }
        let Some(hero) = file.heroes.iter_mut().find(|h| h.guid == guid) else {
            return false;
        };
        if let (WearKind::Tabard, Some(id)) = (kind, id) {
            let faction = hero_faction(content, hero);
            let fits = content
                .tabard(id)
                .zip(faction)
                .is_some_and(|(t, f)| t.look.display_for(f).is_some());
            if !fits {
                return false;
            }
        }
        let slot = hero.wears.slot(kind);
        if slot.as_deref() != id {
            *slot = id.map(str::to_owned);
            let _ = self.write(&file);
        }
        true
    }

    /// Camp `camp` shows `piece` on `spot` ([`SavedAchievements::decorate`]); written when it
    /// changed. False when refused.
    pub fn decorate(&self, content: &Content, camp: &str, spot: &str, piece: Option<&str>) -> bool {
        let mut file = self.file.lock().unwrap();
        let section = file.achievements_section(content, unix_now());
        let before = section.camps.clone();
        let ok = section.decorate(content, camp, spot, piece);
        if section.camps != before {
            let _ = self.write(&file);
        }
        ok
    }

    /// Walkable camp `camp` shows `piece` on `spot` ([`SavedAchievements::decorate_hub`]);
    /// written when it changed. False when refused.
    pub fn decorate_hub(&self, content: &Content, camp: &str, spot: &str, piece: Option<&str>) -> bool {
        let mut file = self.file.lock().unwrap();
        let section = file.achievements_section(content, unix_now());
        let before = section.hub.clone();
        let ok = section.decorate_hub(content, camp, spot, piece);
        if section.hub != before {
            let _ = self.write(&file);
        }
        ok
    }

    /// What stands at hero `h`'s tent in a walkable camp, by spot (`1` to `3`): those it may still
    /// have there; never decorated, its class's free piece beside the tent (spot `3`).
    pub fn plot_told(&self, content: &Content, h: &SavedHero) -> BTreeMap<String, String> {
        let Some(plot) = &h.plot else {
            return class_piece(content, &h.class).map(|d| ("3".to_string(), d.id.clone())).into_iter().collect();
        };
        let a = self.achievements(content);
        plot.iter().filter(|(_, p)| tent_may_have(content, &a, h, p)).map(|(s, p)| (s.clone(), p.clone())).collect()
    }

    /// The pieces hero `h` may set out at its tent: the warband's tent pieces and its class's.
    pub fn plot_choices(&self, content: &Content, h: &SavedHero) -> Vec<String> {
        let a = self.achievements(content);
        content.decorations.pieces.iter().filter(|d| tent_may_have(content, &a, h, &d.id)).map(|d| d.id.clone()).collect()
    }

    /// Hero `guid`'s tent shows `piece` on spot `spot` (`1` to `3`; `None`: nothing), moving it off
    /// its other spot; written when it changed. False for no such hero or spot, or a piece it may
    /// not have there.
    pub fn decorate_plot(&self, content: &Content, guid: u32, spot: &str, piece: Option<&str>) -> bool {
        if !["1", "2", "3"].contains(&spot) {
            return false;
        }
        let a = self.achievements(content);
        let mut file = self.file.lock().unwrap();
        let Some(hero) = file.heroes.iter_mut().find(|h| h.guid == guid) else {
            return false;
        };
        if piece.is_some_and(|p| !tent_may_have(content, &a, hero, p)) {
            return false;
        }
        // first decorated: the class piece standing by default is kept where it stood
        let mut plot = hero.plot.clone().unwrap_or_else(|| {
            class_piece(content, &hero.class).map(|d| ("3".to_string(), d.id.clone())).into_iter().collect()
        });
        plot.remove(spot);
        if let Some(p) = piece {
            plot.retain(|_, q| q != p);
            plot.insert(spot.to_owned(), p.to_owned());
        }
        if hero.plot.as_ref() != Some(&plot) {
            hero.plot = Some(plot);
            let _ = self.write(&file);
        }
        true
    }

    /// What hero `h` wears as the screens are told it: (title, tabard, glow), each empty when
    /// none, or when the warband no longer owns it (a tabard also when its faction may not wear
    /// it, after a change of race).
    pub fn wears_told(&self, content: &Content, h: &SavedHero) -> [String; 3] {
        let a = self.achievements(content);
        WearKind::ALL.map(|k| {
            let id = h.wears.get(k).filter(|id| a.owns_wear(content, k, id));
            let id = match (k, id) {
                (WearKind::Tabard, Some(t)) => {
                    let f = hero_faction(content, h);
                    let ok = content
                        .tabard(t)
                        .zip(f)
                        .is_some_and(|(x, f)| x.look.display_for(f).is_some());
                    ok.then_some(t)
                }
                (_, id) => id,
            };
            id.unwrap_or_default().to_owned()
        })
    }

    /// What hero `h` shows in a run ([`RunWears`]).
    pub fn run_wears(&self, content: &Content, h: &SavedHero) -> RunWears {
        let [title, tabard, glow] = self.wears_told(content, h);
        let faction = hero_faction(content, h);
        RunWears {
            name: content.titled_name(&h.name, Some(title.as_str()).filter(|t| !t.is_empty())),
            tabard: content
                .tabard(&tabard)
                .zip(faction)
                .and_then(|(t, f)| t.look.display_for(f)),
            enchant: content.weapon_glow(&glow).map(|g| g.enchant),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_content::decoration_item;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    /// A walkable camp's fire spot takes a ground piece the warband owns, and only one: set out
    /// elsewhere it moves; a piece not owned, not a ground piece, or a tent spot is refused.
    #[test]
    fn a_fire_spot_takes_an_owned_ground_piece() {
        let content = content();
        let mut a = SavedAchievements::default();
        assert!(!a.decorate_hub(&content, "alliance", "fire_1", Some("weapon_rack")), "not owned");
        a.bought.push(decoration_item("weapon_rack"));
        a.bought.push(decoration_item("battle_banner"));
        assert!(!a.decorate_hub(&content, "alliance", "fire_1", Some("battle_banner")), "a banner post's");
        assert!(!a.decorate_hub(&content, "alliance", "tent_1_1", Some("weapon_rack")), "the tents' are to come");
        assert!(a.decorate_hub(&content, "alliance", "fire_1", Some("weapon_rack")));
        assert!(a.decorate_hub(&content, "alliance", "fire_2", Some("weapon_rack")));
        let told = a.hub_told(&content, "alliance");
        assert_eq!(told.into_iter().collect::<Vec<_>>(), [("fire_2".to_string(), "weapon_rack".to_string())]);
        assert!(a.decorate_hub(&content, "alliance", "fire_2", None));
        assert!(a.hub.is_empty());
    }
}
