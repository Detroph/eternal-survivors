//! A run starts with a teleport: the mage's teleport, the sound Teleport: Stormwind's impact
//! makes (`SoundEntries` "Teleport", `Sound\Spells\Teleport.wav`; `Spell` 3561, `SpellVisual` 263,
//! its kit 3394), plays once as the run is set off, and on through the world's leave and the
//! loading screen ([`SoundThroughLoading`]).
//!
//! Every way of starting a run makes it once: the warband screen's Start run as it enters the world
//! ([`crate::menus`]), and from the camp (the zones' Start run, the F1 menu's, Chromie's "Send me
//! through the portal") the server's word to leave the camp as the same hero
//! ([`ToClient::CampLeave`] naming the hero walking it; another hero named is a take-over, no run).

use benilla_app::SoundThroughLoading;
use bevy::prelude::*;
use esurv_protocol::ToClient;

use crate::camp::CampVisit;
use crate::protocol::ServerEvent;

/// The teleport a run starts with.
pub const RUN_START_SOUND: SoundThroughLoading = SoundThroughLoading("Teleport");

/// Whether the camp left for `enter` is a run setting off: the hero walking the camp goes back in
/// as itself, into its run.
pub fn sets_off_on_a_run(walking: Option<u64>, enter: u64) -> bool {
    walking == Some(enter)
}

pub(crate) fn register(app: &mut App) {
    app.add_message::<SoundThroughLoading>()
        .add_systems(Update, teleport_out_of_the_camp);
}

/// The camp left for a run: the teleport.
fn teleport_out_of_the_camp(
    mut events: MessageReader<ServerEvent>,
    visit: Res<CampVisit>,
    mut sounds: MessageWriter<SoundThroughLoading>,
) {
    for ServerEvent(m) in events.read() {
        if let ToClient::CampLeave { enter } = m {
            if sets_off_on_a_run(visit.hero, *enter) {
                sounds.write(RUN_START_SOUND);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_hero_walking_the_camp_going_back_in_is_a_run() {
        assert!(sets_off_on_a_run(Some(7), 7), "off to its run");
        assert!(!sets_off_on_a_run(Some(7), 9), "another hero taken over");
        assert!(!sets_off_on_a_run(None, 7));
    }

    /// The camp's leave for a run asks for the teleport once; a take-over asks for none.
    #[test]
    fn the_camp_left_for_a_run_asks_for_the_teleport() {
        let mut app = App::new();
        app.add_message::<ServerEvent>().init_resource::<CampVisit>();
        register(&mut app);
        app.world_mut().resource_mut::<CampVisit>().hero = Some(7);
        let asked = |app: &mut App| -> Vec<SoundThroughLoading> {
            app.world_mut().resource_mut::<Messages<SoundThroughLoading>>().drain().collect()
        };
        app.world_mut().write_message(ServerEvent(ToClient::CampLeave { enter: 9 }));
        app.update();
        assert!(asked(&mut app).is_empty(), "a take-over");
        app.world_mut().write_message(ServerEvent(ToClient::CampLeave { enter: 7 }));
        app.update();
        assert_eq!(asked(&mut app), vec![RUN_START_SOUND]);
    }
}
