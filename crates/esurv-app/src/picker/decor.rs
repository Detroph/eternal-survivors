//! The camp's decorations (docs/game/achievements.md, "Camp decorations"; screens.md, "The
//! camps"): the camp fire as progress grows it (or the bought fire set on it), what the banner
//! post and the extra spots hold (`ToClient::Achievements` camps), the camp theme's pieces (the
//! one bought set it shows), the trophy corner, the camp sign, the pennant line along the back (a
//! pennant for each zone the warband cleared) and the skull spear (a skull more at its foot for
//! each Slayer step). Each is a game-object figure of the select scene ([`SceneFigure::Prop`]), its look the
//! camp's faction's, facing the fire. They stand anew whenever what they show changes.

use benilla_app::SceneFigure;
use bevy::prelude::*;
use esurv_content::{Content, DecorLook, Decoration, Faction, ProgressLook, SpotKind};
use esurv_protocol::Achievements;

use super::{faction_scene, PickerState};
use crate::achievements::cosmetics::{camp_word, with_holdings, CAMP_DECORATIONS};
use crate::ui::GameContent;

/// One prop to stand: its look, where (WoW coordinates), its facing and its size.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CampProp {
    pub display: u32,
    pub at: [f32; 3],
    pub facing: f32,
    pub scale: f32,
}

/// The decorated camp's props.
#[derive(Resource, Default)]
pub(crate) struct CampProps {
    standing: Vec<Entity>,
    stood_for: Option<u64>,
}

fn earned(told: &Achievements, a: &str) -> bool {
    told.earned.iter().any(|(e, _)| e == a)
}

fn progress_on(told: &Achievements, l: &ProgressLook) -> bool {
    l.achievement.as_deref().is_none_or(|a| earned(told, a))
}

/// What the camp of `faction` shows: while camp decorations are parked for the hub
/// ([`CAMP_DECORATIONS`] off), only its plain camp fire, as before them; else
/// [`decorated_camp_props`].
pub(crate) fn camp_props(
    content: &Content,
    told: Option<&Achievements>,
    faction: Faction,
) -> Vec<CampProp> {
    if CAMP_DECORATIONS {
        return decorated_camp_props(content, told, faction);
    }
    let place = faction_scene(faction);
    vec![CampProp {
        display: place.campfire_display,
        at: place.campfire,
        facing: 0.0,
        scale: 1.0,
    }]
}

/// Everything the decorated camp of `faction` shows: the fire first, then the spots, the theme,
/// the trophy corner, the camp sign, the pennant line and the skull spear.
pub(crate) fn decorated_camp_props(
    content: &Content,
    told: Option<&Achievements>,
    faction: Faction,
) -> Vec<CampProp> {
    let place = faction_scene(faction);
    let deco = &content.decorations;
    let camp = camp_word(faction);
    let empty = Achievements::default();
    let told = told.unwrap_or(&empty);
    let mut out = Vec::new();
    let stand = |look: DecorLook, at: [f32; 3], turn: f32, scale: f32| CampProp {
        display: look.display_for(faction),
        at,
        facing: place.facing_the_fire(at) + turn.to_radians(),
        scale: scale * look.scale_for(faction),
    };
    let owns = |d: &Decoration| with_holdings(told, |h| content.owns_decoration(&d.id, h));
    // a piece as the camp shows it, at (right, beyond)
    let piece = |d: &Decoration, (right, beyond): (f32, f32)| {
        let (display, scale) = with_holdings(told, |h| content.decoration_look(d, faction, h));
        let at = place.point(right, beyond);
        CampProp {
            display,
            at,
            facing: place.facing_the_fire(at) + d.turn.to_radians(),
            scale,
        }
    };
    // the fire: a bought one set on it, else as progress grows it
    let fire_spot = deco.spots.iter().find(|s| s.kind == SpotKind::Campfire);
    let bought_fire = fire_spot
        .and_then(|s| told.on_spot(camp, &s.id))
        .and_then(|p| content.decoration(p))
        .filter(|d| with_holdings(told, |h| content.owns_decoration(&d.id, h)));
    let fire_look = match bought_fire {
        Some(d) => Some((d.look, d.scale)),
        None => with_holdings(told, |h| {
            content.grown_campfire(h).map(|l| (l.look, l.scale))
        }),
    };
    if let Some((look, scale)) = fire_look {
        out.push(CampProp {
            display: look.display_for(faction),
            at: place.campfire,
            facing: 0.0,
            scale: scale * look.scale_for(faction),
        });
    }
    // the banner post and the extra spots: what stands on each
    let mut on_spots = Vec::new();
    for s in deco
        .spots
        .iter()
        .filter(|s| matches!(s.kind, SpotKind::BannerPost | SpotKind::Ground))
    {
        let Some(d) = told
            .on_spot(camp, &s.id)
            .and_then(|p| content.decoration(p))
        else {
            continue;
        };
        if with_holdings(told, |h| content.may_set_out(&d.id, &s.id, h)) {
            on_spots.push(d.id.as_str());
            out.push(piece(d, s.at));
        }
    }
    // the camp's theme: its set's pieces the warband owns, each in its place (one on an extra
    // spot stands there instead)
    let theme = deco
        .spots
        .iter()
        .find(|s| s.kind == SpotKind::Theme)
        .and_then(|s| told.on_spot(camp, &s.id));
    if let Some(set) = theme {
        for d in content
            .set_pieces(set)
            .filter(|d| d.spot == SpotKind::Ground)
        {
            if let (Some(at), true) = (d.at, owns(d) && !on_spots.contains(&d.id.as_str())) {
                out.push(piece(d, at));
            }
        }
    }
    // the trophy corner: every trophy earned, each in its place
    for d in deco.pieces.iter().filter(|d| d.trophy()) {
        if let (Some(at), true) = (d.at, owns(d)) {
            out.push(piece(d, at));
        }
    }
    // the one camp sign, once a set's signpost is earned
    if let Some(d) = deco
        .pieces
        .iter()
        .find(|d| d.spot == SpotKind::Sign && owns(d))
    {
        out.push(piece(d, deco.camp_sign));
    }
    // the pennant line: its rope, then a pennant for each zone cleared, in its place on it
    let line = &deco.banner_line;
    for r in &line.rope {
        let mut at = place.point(line.from.0 + r.at.0, line.from.1 + r.at.1);
        at[2] += r.lift;
        out.push(stand(r.look, at, r.turn, r.scale));
    }
    for (k, b) in line.banners.iter().enumerate() {
        if told.zones_cleared.iter().any(|z| *z == b.zone) {
            let at = place.point(line.from.0 + line.step * k as f32, line.from.1);
            out.push(stand(b.look, at, 0.0, b.scale));
        }
    }
    // the skull spear, its skulls piling up at its foot
    let spear = &deco.skull_spear;
    if progress_on(told, &spear.spear) {
        let foot = place.point(spear.at.0, spear.at.1);
        out.push(stand(
            spear.spear.look,
            foot,
            spear.spear.turn,
            spear.spear.scale,
        ));
        for s in spear.skulls.iter().filter(|s| progress_on(told, s)) {
            let mut at = place.point(spear.at.0 + s.at.0, spear.at.1 + s.at.1);
            at[2] += s.lift;
            out.push(stand(s.look, at, s.turn, s.scale));
        }
    }
    out
}

/// Stands the camp's props anew when the camp, the content or what the warband owns changed.
pub(super) fn stand_decorations(
    mut commands: Commands,
    screen: Res<benilla_app::SelectScreen>,
    content: Option<Res<GameContent>>,
    facts: Res<crate::menus::PreRunFacts>,
    state: Res<PickerState>,
    mut props: ResMut<CampProps>,
) {
    let wanted = match (&content, screen.open) {
        (Some(c), true) => camp_props(&c.0, facts.achievements(), state.faction),
        _ => Vec::new(),
    };
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for p in &wanted {
            (
                p.display,
                p.at.map(f32::to_bits),
                p.facing.to_bits(),
                p.scale.to_bits(),
            )
                .hash(&mut h);
        }
        h.finish()
    };
    if props.stood_for == Some(key) {
        return;
    }
    props.stood_for = Some(key);
    for e in props.standing.drain(..) {
        commands.entity(e).despawn();
    }
    for p in wanted {
        let e = commands
            .spawn((
                Name::new("camp decoration"),
                SceneFigure::Prop {
                    display_id: p.display,
                    scale: p.scale,
                },
                SceneFigure::place(p.at, p.facing),
            ))
            .id();
        props.standing.push(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esurv_protocol::CampLayout;

    fn content() -> Content {
        Content::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content")).unwrap()
    }

    #[test]
    fn while_decorations_are_parked_the_camp_shows_its_plain_fire() {
        let c = content();
        let told = Achievements {
            zones_cleared: vec!["elwynn".into()],
            earned: vec![("slayer_1".into(), 1), ("bossbane".into(), 2)],
            ..Default::default()
        };
        if CAMP_DECORATIONS {
            return;
        }
        let plain = camp_props(&c, Some(&told), Faction::Horde);
        assert_eq!(plain.len(), 1);
        assert_eq!(
            (plain[0].display, plain[0].scale),
            (192, 1.0),
            "the camp fire as before"
        );
    }

    #[test]
    fn a_bare_camp_has_its_fire_and_a_decorated_one_shows_its_progress_and_pieces() {
        let c = content();
        let bare: Vec<u32> = decorated_camp_props(&c, None, Faction::Horde)
            .iter()
            .map(|p| p.display)
            .collect();
        assert_eq!(
            bare,
            [192, 6769],
            "the camp fire and the pennant line's rope"
        );
        let told = Achievements {
            earned: vec![
                ("twice_as_hard".into(), 1),
                ("slayer_1".into(), 2),
                ("slayer_2".into(), 3),
                ("bossbane".into(), 4),
            ],
            zones_cleared: vec!["elwynn".into(), "wsg".into()],
            bought: vec![
                "decoration:battle_banner".into(),
                "decoration:midsummer_bonfire".into(),
                "decoration:weapon_rack".into(),
                "decoration:powder_kegs".into(),
            ],
            camps: vec![
                CampLayout {
                    camp: "horde".into(),
                    pieces: vec![
                        ("banner".into(), "battle_banner".into()),
                        ("theme".into(), "war_camp".into()),
                    ],
                },
                CampLayout {
                    camp: "alliance".into(),
                    pieces: vec![("fire".into(), "midsummer_bonfire".into())],
                },
            ],
            ..Default::default()
        };
        let horde: Vec<u32> = decorated_camp_props(&c, Some(&told), Faction::Horde)
            .iter()
            .map(|p| p.display)
            .collect();
        // the blue fire, the Horde's banner, the War Camp's two pieces owned, Onyxia on the
        // Horde's post, the throne and the Gulch's sword in a skull in the trophy corner, the rope with the Horde's pennants for
        // Elwynn and the Gulch, the spear and three skulls
        assert_eq!(
            horde,
            [5611, 5773, 130, 33, 5742, 2810, 4174, 6769, 4313, 6707, 2067, 448, 4173, 226]
        );
        let alliance: Vec<u32> = decorated_camp_props(&c, Some(&told), Faction::Alliance)
            .iter()
            .map(|p| p.display)
            .collect();
        assert_eq!(alliance[0], 6754, "the bought fire in its place");
        assert!(
            alliance.contains(&624) && alliance.contains(&6390),
            "the Alliance's own pennants"
        );
        assert!(
            !alliance.contains(&5773) && !alliance.contains(&130),
            "the Horde camp's layout stays there"
        );
        assert!(alliance.contains(&5745), "the trophies stand in both camps");
    }

    #[test]
    fn a_piece_on_an_extra_spot_stands_there_and_not_also_in_its_set() {
        let c = content();
        let told = Achievements {
            bought: vec!["decoration:weapon_rack".into(), "camp_spot:extra_1".into()],
            camps: vec![CampLayout {
                camp: "horde".into(),
                pieces: vec![
                    ("theme".into(), "war_camp".into()),
                    ("extra_1".into(), "weapon_rack".into()),
                ],
            }],
            ..Default::default()
        };
        let racks: Vec<_> = decorated_camp_props(&c, Some(&told), Faction::Horde)
            .into_iter()
            .filter(|p| p.display == 130)
            .collect();
        assert_eq!(racks.len(), 1);
        let place = faction_scene(Faction::Horde);
        let spot = c.camp_spot("extra_1").unwrap().at;
        assert_eq!(racks[0].at, place.point(spot.0, spot.1));
    }
}
