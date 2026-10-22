//! The meter filled from the server ([`ToClient::Meter`], the run clock from [`ToClient::Hud`]):
//! damage done by the hero's spells, one row each, with its pets, `thorns`, melee and the rest
//! each gathered on one row, and procs under what gave them
//! (a talent, ring, trinket, amulet, legendary amulet or weapon loadout
//! by its name); damage taken by enemy, and "Area" for what no enemy dealt. The server sends run
//! totals, so a reset keeps the totals of that moment and shows what came after.

use std::collections::BTreeMap;

use bevy::prelude::*;
use esurv_content::Content;
use esurv_protocol::{MeterEntry as ServerEntry, ProcOrigin, Source, ToClient};

use super::{MeterFeed, MeterMode, MeterReset};
use crate::protocol::ServerEvent;
use crate::ui::GameContent;

/// `thorns`' `Spell.dbc` id: a row of its own among the procs.
const THORNS_SPELL: u32 = 467;

/// Totals by row.
type Totals = BTreeMap<String, u64>;

/// The run's totals as the server last sent them, those at the last reset, and the run clocks.
#[derive(Resource, Default)]
pub(super) struct ServerMeter {
    done: Totals,
    taken: Totals,
    reset_done: Totals,
    reset_taken: Totals,
    clock: u32,
    reset_clock: u32,
}

/// The row a source of damage done counts under. A proc counts under
/// what gave it, named as the player knows it from `content` (a talent's, ring's, trinket's,
/// amulet's, legendary amulet's or weapon loadout's name; a spell's as its own row); rows of one
/// name are one row. Thorns keeps its row; a proc of no known origin counts as "Other".
pub fn done_row(source: &Source, content: Option<&Content>) -> String {
    match source {
        Source::Spell(id) => spell_name(id),
        Source::Swing => "Melee".into(),
        Source::Pet(_) => "Pets".into(),
        Source::Proc { spell, origin } => match origin_name(origin, content) {
            Some(name) => name,
            None if *spell == THORNS_SPELL => spell_name("thorns"),
            None => "Other".into(),
        },
        Source::Other | Source::Enemy(_) | Source::Area => "Other".into(),
    }
}

/// The name of what gave a proc, from the content (its id as words without it); `None` for no
/// known origin.
fn origin_name(origin: &ProcOrigin, content: Option<&Content>) -> Option<String> {
    let named = |id: &str, name: Option<&String>| name.cloned().unwrap_or_else(|| spell_name(id));
    Some(match origin {
        ProcOrigin::Talent(id) => {
            let talent = content.and_then(|c| c.talent_trees.iter().flat_map(|t| &t.talents).find(|t| &t.id == id));
            named(id, talent.map(|t| &t.name))
        }
        ProcOrigin::Jewelry(id) => named(id, content.and_then(|c| c.jewelry.iter().find(|j| &j.id == id)).map(|j| &j.name)),
        ProcOrigin::Amulet(id) => named(id, content.and_then(|c| c.amulets.iter().find(|a| &a.id == id)).map(|a| &a.name)),
        ProcOrigin::Legendary(id) => named(id, content.and_then(|c| c.legendary_amulets.iter().find(|l| &l.id() == id)).map(|l| &l.name)),
        ProcOrigin::Loadout(id) => {
            let loadout = content.and_then(|c| c.loadouts.iter().flat_map(|l| &l.loadouts).find(|l| &l.id == id));
            named(id, loadout.map(|l| &l.name))
        }
        ProcOrigin::Spell(id) => spell_name(id),
        ProcOrigin::Other => return None,
    })
}

/// The row a source of damage taken counts under.
pub fn taken_row(source: &Source, _content: Option<&Content>) -> String {
    match source {
        Source::Enemy(name) => name.clone(),
        Source::Area => "Area".into(),
        _ => "Other".into(),
    }
}

/// A spell's content id as words, each capitalised: `shadow_bolt` reads as two words.
pub fn spell_name(id: &str) -> String {
    id.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The server's entries summed by row.
fn grouped(entries: &[ServerEntry], content: Option<&Content>, row: fn(&Source, Option<&Content>) -> String) -> Totals {
    let mut totals = Totals::new();
    for e in entries {
        *totals.entry(row(&e.source, content)).or_default() += e.amount;
    }
    totals
}

impl ServerMeter {
    /// What the window shows: the totals since the last reset, over the seconds since then.
    fn feed(&self) -> MeterFeed {
        let mut feed = MeterFeed {
            elapsed: self.clock.saturating_sub(self.reset_clock) as f32,
            ..Default::default()
        };
        for (mode, totals, at_reset) in [
            (MeterMode::DamageDone, &self.done, &self.reset_done),
            (MeterMode::DamageTaken, &self.taken, &self.reset_taken),
        ] {
            for (name, amount) in totals {
                let since = amount.saturating_sub(at_reset.get(name).copied().unwrap_or(0));
                if since > 0 {
                    feed.add(mode, name, since);
                }
            }
        }
        feed
    }

    /// The player reset the meter: from now on it counts from these totals.
    fn reset(&mut self) {
        self.reset_done = self.done.clone();
        self.reset_taken = self.taken.clone();
        self.reset_clock = self.clock;
    }
}

/// Reads the server's meter and clock, and the window's reset, into the feed.
pub(super) fn fill_from_server(
    mut events: MessageReader<ServerEvent>,
    mut resets: MessageReader<MeterReset>,
    mut meter: ResMut<ServerMeter>,
    mut feed: ResMut<MeterFeed>,
    content: Option<Res<GameContent>>,
) {
    let content = content.as_ref().map(|c| c.0.as_ref());
    let mut changed = false;
    for ServerEvent(m) in events.read() {
        match m {
            // a new run starts from nothing
            ToClient::Ready(_) => *meter = ServerMeter::default(),
            ToClient::Hud(h) if h.clock != meter.clock => meter.clock = h.clock,
            ToClient::Meter(m) => {
                meter.done = grouped(&m.done, content, done_row);
                meter.taken = grouped(&m.taken, content, taken_row);
            }
            _ => continue,
        }
        changed = true;
    }
    if resets.read().count() > 0 {
        meter.reset();
        changed = true;
    }
    if changed {
        let next = meter.feed();
        if *feed != next {
            *feed = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: Source, amount: u64) -> ServerEntry {
        ServerEntry { source, amount }
    }

    #[test]
    fn sources_gather_on_the_meters_rows() {
        assert_eq!(
            done_row(&Source::Spell("my_new_spell".into()), None),
            "My New Spell"
        );
        assert_eq!(done_row(&Source::Pet("Imp".into()), None), "Pets");
        let proc = |spell, origin| Source::Proc { spell, origin };
        assert_eq!(done_row(&proc(467, ProcOrigin::Other), None), spell_name("thorns"));
        assert_eq!(done_row(&proc(13812, ProcOrigin::Other), None), "Other");
        assert_eq!(done_row(&Source::Swing, None), "Melee");
        assert_eq!(taken_row(&Source::Area, None), "Area");
        assert_eq!(taken_row(&Source::Enemy("Hogger".into()), None), "Hogger");
        let done = grouped(
            &[
                entry(Source::Pet("Imp".into()), 10),
                entry(Source::Pet("Wolf".into()), 5),
            ],
            None,
            done_row,
        );
        assert_eq!(done.get("Pets"), Some(&15), "every pet on one row");
    }

    /// A proc counts under what gave it, by the name the content gives
    /// it; procs of one name share a row, an aura striking back joins its spell's row.
    #[test]
    fn procs_count_under_what_gave_them() {
        let c = crate::hero_sheet::gear::tests::content();
        let talent = &c.talent_trees[0].talents[0];
        let ring = c.jewelry.iter().find(|j| j.id == "soulstone").unwrap();
        let amulet = &c.amulets[0];
        let legendary = c.legendary_amulets.iter().find(|l| l.name == "Light's Blessing").unwrap();
        let loadout = &c.loadouts[0].loadouts[0];
        let row = |origin| done_row(&Source::Proc { spell: 133, origin }, Some(c));
        assert_eq!(row(ProcOrigin::Talent(talent.id.clone())), talent.name);
        assert_eq!(row(ProcOrigin::Jewelry(ring.id.clone())), ring.name);
        assert_eq!(row(ProcOrigin::Amulet(amulet.id.clone())), amulet.name);
        assert_eq!(row(ProcOrigin::Legendary(legendary.id())), "Light's Blessing");
        assert_eq!(row(ProcOrigin::Loadout(loadout.id.clone())), loadout.name);
        assert_eq!(row(ProcOrigin::Spell("lightning_shield".into())), spell_name("lightning_shield"));
        assert_eq!(row(ProcOrigin::Other), "Other");
        // thorns keeps its row whatever the content
        assert_eq!(done_row(&Source::Proc { spell: 467, origin: ProcOrigin::Other }, Some(c)), spell_name("thorns"));
        let done = grouped(
            &[
                entry(Source::Proc { spell: 133, origin: ProcOrigin::Talent(talent.id.clone()) }, 10),
                entry(Source::Proc { spell: 2120, origin: ProcOrigin::Talent(talent.id.clone()) }, 5),
                entry(Source::Proc { spell: 324, origin: ProcOrigin::Spell("lightning_shield".into()) }, 7),
                entry(Source::Spell("lightning_shield".into()), 3),
            ],
            Some(c),
            done_row,
        );
        assert_eq!(done.get(&talent.name), Some(&15), "one talent, one row");
        assert_eq!(done.get(&spell_name("lightning_shield")), Some(&10), "the aura on its spell's row");
    }

    #[test]
    fn a_reset_counts_from_that_moment() {
        let mut meter = ServerMeter {
            done: Totals::from([("Spell 133".into(), 500)]),
            taken: Totals::from([("Area".into(), 40)]),
            clock: 50,
            ..Default::default()
        };
        assert_eq!(meter.feed().elapsed, 50.0);
        meter.reset();
        assert!(meter.feed().damage_done.is_empty(), "nothing since");
        meter.done.insert("Spell 133".into(), 620);
        meter.done.insert("Spell 116".into(), 30);
        meter.clock = 60;
        let feed = meter.feed();
        assert_eq!(feed.elapsed, 10.0);
        let row = |name: &str| {
            feed.damage_done
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.amount)
        };
        assert_eq!((row("Spell 133"), row("Spell 116")), (Some(120), Some(30)));
    }
}
