//! Smart damage numbers (screens.md, "Damage numbers"): the floating
//! numbers of the hero's damage on enemies (its pets' and procs' too, logged as the hero's) hide
//! the small hits, so the big ones read in a busy fight.
//!
//! Every [`WINDOW_SECS`] the game keeps only two running totals of the hero's counted hits, no
//! list: their sum and the sum of their squares. Every floating damage number is one hit: each
//! target of a nova or other many-target spell is its own, and so is each damage-over-time tick.
//! At each window's end the threshold is set to [`SHARE`] of that window's average weighted by
//! damage (`sum of squares / sum`: each hit counts as much as it deals, so a flood of small ticks
//! and procs cannot pull it down), the totals start again, and the threshold holds the whole next
//! window; while the option is Smart, a number under it does not float. The first window starts
//! with the first hit, and every number floats until it is over; a window with no counted hit
//! clears the threshold.
//!
//! A crit is held to the threshold too, but crits are left out of the sum and count, unless the
//! hero's crit chance is over [`CRITS_COUNT_OVER`] (50 %), when they are its usual hits and count.
//! The crit chance is the one the server tells the client with the spellbook's tooltips
//! (`SpellTips`: the hero's crit chance, on each spell that can crit); none told is 0. The damage,
//! the meter and the combat log are untouched; numbers over the hero (damage taken, heals) are not
//! the hero's damage and never hide.
//!
//! The option is our own CVar, [`CVAR`] (`"1"` Smart, the default; `"0"` All), on the options
//! panel's Interface tab, and kept with the game's other settings.

use benilla_app::{DamageNumberFloor, DamageNumberSpawned};
use benilla_ui::script::UiScript;
use bevy::prelude::*;

/// The option's CVar: `"1"` Smart, `"0"` All.
pub const CVAR: &str = "esurvSmartDamageNumbers";
/// The option's value when nothing was saved: Smart.
pub const DEFAULT: &str = "1";
/// How long each average's window is, and how long its threshold holds (seconds).
pub const WINDOW_SECS: f64 = 30.0;
/// The threshold's share of the damage-weighted average.
pub const SHARE: f64 = 0.6;
/// Over this crit chance (a share) the hero's crits count in the average.
pub const CRITS_COUNT_OVER: f32 = 0.5;
/// How often the option is read from its CVar (seconds).
const OPTION_EVERY: f32 = 1.0;

/// Registers our CVar with its default, as an addon's is, so it reads and is kept.
pub fn register_cvar(script: &UiScript) {
    let _ = script.eval::<()>(&format!("RegisterCVar(\"{CVAR}\", \"{DEFAULT}\")"));
}

/// Whether a CVar value means Smart: anything but `"0"` (an unknown value is the default).
pub fn smart_from_value(v: Option<&str>) -> bool {
    v.is_none_or(|v| v.trim() != "0")
}

/// The current window's running totals and the threshold the last window set.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SmartNumbers {
    /// The counted hits of the current window: their sum and the sum of their squares.
    sum: f64,
    sum_of_squares: f64,
    /// The hero's crit chance (a share), as the server last told it.
    pub crit_chance: f32,
    /// When the current window ends; `None` until the first hit.
    window_ends: Option<f64>,
    /// The least number that floats, while Smart; `None` floats every one.
    threshold: Option<u32>,
    /// The option: Smart (filter) or All.
    pub smart: bool,
}

impl SmartNumbers {
    /// A record with the option at its default, Smart.
    pub fn new() -> Self {
        Self {
            smart: true,
            ..default()
        }
    }

    /// One damage number the hero floated, at `now` (seconds); a crit counts only while the
    /// hero's crit chance is over [`CRITS_COUNT_OVER`].
    pub fn record(&mut self, now: f64, amount: u32, crit: bool) {
        if self.window_ends.is_none() {
            self.window_ends = Some(now + WINDOW_SECS);
        }
        if crit && self.crit_chance <= CRITS_COUNT_OVER {
            return;
        }
        let amount = f64::from(amount);
        self.sum += amount;
        self.sum_of_squares += amount * amount;
    }

    /// At a window's end: the threshold from its average, and the totals start again.
    pub fn tick(&mut self, now: f64) {
        let Some(ends) = self.window_ends else {
            return;
        };
        if now < ends {
            return;
        }
        self.threshold = threshold_of(self.sum, self.sum_of_squares);
        debug!("smart numbers: {} damage counted, threshold {:?}", self.sum, self.threshold);
        self.sum = 0.0;
        self.sum_of_squares = 0.0;
        self.window_ends = Some(now + WINDOW_SECS);
    }

    /// The threshold as last set, whatever the option.
    pub fn threshold(&self) -> Option<u32> {
        self.threshold
    }

    /// The floor the floating numbers are held to now: the threshold while Smart, none while All.
    pub fn floor(&self) -> Option<u32> {
        self.threshold.filter(|_| self.smart)
    }

    /// Whether a number floats now.
    pub fn shows(&self, amount: u32) -> bool {
        self.floor().is_none_or(|f| amount >= f)
    }
}

/// [`SHARE`] of the damage-weighted average `sum_of_squares / sum`, rounded up, so a whole number
/// at or over the exact share floats; `None` for no hits.
pub fn threshold_of(sum: f64, sum_of_squares: f64) -> Option<u32> {
    (sum > 0.0).then(|| (SHARE * sum_of_squares / sum - 1e-9).ceil().max(0.0) as u32)
}

pub(crate) fn register(app: &mut App) {
    app.insert_resource(SmartNumbers::new()).add_systems(
        Update,
        (read_option, read_crit_chance, keep_the_record).chain(),
    );
}

/// The hero's crit chance, from the spellbook's tooltips (each client hears only its own hero's):
/// the highest a spell's tip tells, 0 when none can crit.
pub fn crit_chance_of(spells: &[esurv_protocol::SpellTip]) -> f32 {
    spells.iter().filter_map(|t| t.crit.map(|(chance, _)| chance)).fold(0.0, f32::max)
}

fn read_crit_chance(mut events: MessageReader<crate::protocol::ServerEvent>, mut smart: ResMut<SmartNumbers>) {
    for crate::protocol::ServerEvent(m) in events.read() {
        if let esurv_protocol::ToClient::SpellTips { spells, .. } = m {
            let chance = crit_chance_of(spells);
            if smart.crit_chance != chance {
                smart.crit_chance = chance;
            }
        }
    }
}

/// The hits heard this frame into the record, the threshold refreshed, the floor handed to the
/// client.
fn keep_the_record(
    mut heard: MessageReader<DamageNumberSpawned>,
    mut smart: ResMut<SmartNumbers>,
    floor: Option<ResMut<DamageNumberFloor>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    for n in heard.read() {
        smart.record(now, n.amount, n.crit);
    }
    smart.tick(now);
    if let Some(mut floor) = floor {
        floor.set_if_neq(DamageNumberFloor(smart.floor()));
    }
}

/// The option, read from its CVar now and then.
fn read_option(
    script: Option<NonSend<UiScript>>,
    mut smart: ResMut<SmartNumbers>,
    time: Res<Time>,
    mut next: Local<f32>,
    mut registered: Local<bool>,
) {
    let Some(script) = script else { return };
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + OPTION_EVERY;
    if !*registered {
        register_cvar(&script);
        *registered = true;
    }
    let v = script.eval::<Option<String>>(&format!("return GetCVar(\"{CVAR}\")")).ok().flatten();
    let on = smart_from_value(v.as_deref());
    if smart.smart != on {
        smart.smart = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The totals of a list of hits, as the record keeps them.
    fn totals(hits: &[u32]) -> (f64, f64) {
        hits.iter().fold((0.0, 0.0), |(s, q), &h| (s + f64::from(h), q + f64::from(h) * f64::from(h)))
    }

    #[test]
    fn the_threshold_is_sixty_percent_of_the_damage_weighted_average_rounded_up() {
        let (s, q) = totals(&[200, 200, 200]);
        assert_eq!(threshold_of(s, q), Some(120), "equal hits: 0.6 x 200");
        let (s, q) = totals(&[10]);
        assert_eq!(threshold_of(s, q), Some(6));
        assert_eq!(threshold_of(0.0, 0.0), None);
        // a flood of small ticks barely moves it: 0.6 x (1000^2 + 50 x 10^2) / (1000 + 500)
        let mut hits = vec![1000];
        hits.extend([10; 50]);
        let (s, q) = totals(&hits);
        assert_eq!(threshold_of(s, q), Some(402), "the plain average would give 0.6 x 29 = 18");
    }

    /// The first threshold comes once the first window is over (every number floats until then),
    /// then once a window, each from that window's hits only, and holds the whole next window.
    #[test]
    fn the_threshold_is_set_once_a_window_from_the_last_window() {
        let mut s = SmartNumbers::new();
        s.record(0.0, 100, false); // the first hit: the first window starts
        s.tick(0.0);
        assert_eq!(s.threshold(), None, "every number floats in the first window");
        s.record(10.0, 100, false);
        s.record(20.0, 100, false);
        s.tick(WINDOW_SECS - 0.1);
        assert_eq!(s.threshold(), None);
        s.tick(WINDOW_SECS);
        assert_eq!(s.threshold(), Some(60), "0.6 x 100 at 30 s");
        // the whole next window the threshold holds, whatever comes in
        s.record(40.0, 300, false);
        s.tick(2.0 * WINDOW_SECS - 0.1);
        assert_eq!(s.threshold(), Some(60), "held until 60 s");
        s.tick(2.0 * WINDOW_SECS);
        // the window to 60 s counted only its own hit: the totals started again at 30 s
        assert_eq!(s.threshold(), Some(180), "0.6 x 300");
        // a window with no hit clears it
        s.tick(3.0 * WINDOW_SECS + 1.0);
        assert_eq!(s.threshold(), None);
        assert!(s.shows(1));
    }

    #[test]
    fn smart_hides_what_is_under_the_threshold_and_all_shows_every_number() {
        let mut s = SmartNumbers::new();
        assert!(s.smart, "Smart is the default");
        s.record(0.0, 200, false);
        s.record(1.0, 200, false);
        s.tick(WINDOW_SECS);
        assert_eq!(s.threshold(), Some(120));
        assert_eq!(s.floor(), Some(120));
        assert!(!s.shows(119) && s.shows(120) && s.shows(500));
        s.smart = false;
        assert_eq!(s.floor(), None, "All: no floor");
        assert!(s.shows(1));
        assert_eq!(s.threshold(), Some(120), "the totals go on under All");
    }

    /// Crits stay out of the average, unless the hero crits more often than not; either way a
    /// crit floats only at or over the threshold.
    #[test]
    fn crits_count_in_the_average_only_over_half_crit_chance() {
        let run = |chance: f32| {
            let mut s = SmartNumbers::new();
            s.crit_chance = chance;
            s.record(0.0, 100, false);
            s.record(1.0, 100, false);
            s.record(2.0, 400, true);
            s.tick(WINDOW_SECS);
            s
        };
        let low = run(0.2);
        assert_eq!(low.threshold(), Some(60), "0.6 x 100: the 400 crit is left out");
        let half = run(0.5);
        assert_eq!(half.threshold(), Some(60), "at 50 % crits are still left out");
        let high = run(0.55);
        assert_eq!(high.threshold(), Some(180), "over 50 %: 0.6 x (100^2 + 100^2 + 400^2) / 600");
        assert!(!high.shows(179) && high.shows(400), "a crit is held to the threshold");
        // only crits kept, crits left out: no threshold, every number floats
        let mut s = SmartNumbers::new();
        s.record(0.0, 300, true);
        s.tick(WINDOW_SECS);
        assert_eq!(s.threshold(), None);
    }

    #[test]
    fn the_crit_chance_is_the_spellbook_s() {
        let tip = |crit| esurv_protocol::SpellTip {
            look: 1,
            kind: esurv_protocol::TipKind::Learned,
            id: "x".into(),
            rank: 1,
            numbers: Vec::new(),
            upgrades: Vec::new(),
            bonuses: Vec::new(),
            crit,
        };
        assert_eq!(crit_chance_of(&[tip(None), tip(Some((0.62, 2.0)))]), 0.62);
        assert_eq!(crit_chance_of(&[tip(None)]), 0.0, "no spell crits: 0");
    }

    #[test]
    fn the_option_defaults_to_smart_and_reads_its_cvar() {
        assert_eq!(DEFAULT, "1");
        assert!(smart_from_value(None));
        assert!(smart_from_value(Some("1")));
        assert!(!smart_from_value(Some("0")));
        assert!(!smart_from_value(Some(" 0 ")));
    }

    /// The system: hits heard into the record, the floor handed to the client while Smart.
    #[test]
    fn the_floor_goes_to_the_client() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .add_message::<DamageNumberSpawned>()
            .init_resource::<DamageNumberFloor>()
            .insert_resource(SmartNumbers::new())
            .add_systems(Update, keep_the_record);
        app.world_mut().write_message(DamageNumberSpawned { amount: 100, crit: false });
        app.world_mut().write_message(DamageNumberSpawned { amount: 200, crit: true });
        app.update();
        // a refresh due now: the first one
        app.world_mut().resource_mut::<SmartNumbers>().window_ends = Some(0.0);
        app.update();
        assert_eq!(app.world().resource::<DamageNumberFloor>().0, Some(60), "the crit is left out: 0.6 x 100");
        app.world_mut().resource_mut::<SmartNumbers>().smart = false;
        app.update();
        assert_eq!(app.world().resource::<DamageNumberFloor>().0, None);
    }
}
