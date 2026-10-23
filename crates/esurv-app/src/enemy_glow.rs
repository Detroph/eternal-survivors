//! The enemies' glows in a run (screens.md, "Glows round elites and rares"): an elite wears a
//! purple outline glow, a rare and the Rare Cache it drops a silver one; bosses and the others
//! none. The look is the warband screen's picked-hero outline (`menus::outline`): a thin edge along
//! the silhouette, a soft glow fading outward and a gentle pulse, from the same shader in another
//! colour. benilla draws each glowing figure alone into its colour's mask
//! (`benilla_app::SceneFigureMask`, [`ELITE_MASK`] or [`RARE_MASK`]); a node over the window per
//! colour draws round it.
//!
//! An elite or a rare is told by the rank the creature query gives it (`benilla_app::CreatureRanks`,
//! the portrait's gold or silver dragon), a Rare Cache by its game-object entry. A dead one stops
//! glowing. Every one in sight glows: 40 elites glowing cost about 0.4 ms of the main thread
//! and 0.5 ms of the GPU a frame and kept the frame at the screen's refresh (screens.md), so there is no limit.
//!
//! The glows follow one option, "Glow around elites and rares" ([`crate::glow_option`]): off,
//! they all hide at once (the masks are taken off, so their cameras stop too).
//!
//! `ESURV_GLOW_CHECK=<s>`, a development aid: logs the frame time every 2 s from the hero's
//! entering the world (mean and worst, with how many glow), turns the option on then and off
//! that many seconds after it, so one run measures the same scene with the glows on and off; it
//! turns benilla's FPS journal on too (`fpsJournal`: the CPU and GPU time of each second).

use benilla_app::{creature_rank, CreatureRanks, FigureMaskImage, Objects, SceneFigureMask};
use benilla_ui::script::UiScript;
use bevy::prelude::*;

use crate::glow_option;
use crate::menus::outline::{OutlineGlow, EDGE, GLOW, PULSE};

/// The elites' mask and the rares' (the picked hero's is `outline::HERO_MASK`).
pub const ELITE_MASK: usize = 1;
pub const RARE_MASK: usize = 2;
/// The colours, in gamma bytes; the alpha is the glow's strength at the edge.
const PURPLE: Vec4 = Vec4::new(0.74, 0.36, 1.0, 0.6);
const SILVER: Vec4 = Vec4::new(0.92, 0.95, 1.0, 0.7);
/// The Rare Cache's game-object entry (the server's `entries::RARE_CACHE`).
pub const RARE_CACHE_ENTRY: u32 = esurv_content::entries::CREATURES_END + 108;
/// How often the option is read from its CVar (seconds).
const OPTION_EVERY: f32 = 1.0;

/// Which glow a figure wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    /// Purple.
    Elite,
    /// Silver: a rare, or a Rare Cache.
    Rare,
}

impl Glow {
    pub fn mask(self) -> usize {
        match self {
            Glow::Elite => ELITE_MASK,
            Glow::Rare => RARE_MASK,
        }
    }
}

/// The glow of a living unit of `rank` (dead units have none).
pub fn glow_of_unit(rank: u32, alive: bool) -> Option<Glow> {
    if !alive {
        return None;
    }
    match rank {
        creature_rank::ELITE => Some(Glow::Elite),
        creature_rank::RARE => Some(Glow::Rare),
        _ => None,
    }
}

/// The glow of a game object of `entry`: the Rare Cache's silver.
pub fn glow_of_object(entry: Option<u32>) -> Option<Glow> {
    (entry == Some(RARE_CACHE_ENTRY)).then_some(Glow::Rare)
}

/// The option, and how many figures glow in each colour this frame.
#[derive(Resource, Debug)]
struct EnemyGlow {
    on: bool,
    glowing: [usize; 2],
}

/// The colour nodes, once made: by mask, the node and its material.
#[derive(Resource)]
struct GlowNodes([(usize, Entity, Handle<OutlineGlow>); 2]);

pub(crate) fn register(app: &mut App) {
    app.insert_resource(EnemyGlow {
        on: true,
        glowing: [0; 2],
    })
    .add_systems(Update, (read_the_option, mask_the_glowing, draw_the_glows).chain());
    if let Some(check) = GlowCheck::from_env() {
        app.insert_resource(check).add_systems(Update, check_the_glows);
    }
}

/// The option, read from its CVar now and then.
fn read_the_option(
    script: Option<NonSend<UiScript>>,
    mut glow: ResMut<EnemyGlow>,
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
        glow_option::register_cvar(&script);
        *registered = true;
    }
    let v = script
        .eval::<Option<String>>(&format!("return GetCVar(\"{}\")", glow_option::CVAR))
        .ok()
        .flatten();
    let on = glow_option::glow_on(v.as_deref());
    if glow.on != on {
        glow.on = on;
    }
}

/// Puts each glowing figure in its colour's mask and takes the mask off the others.
fn mask_the_glowing(
    mut commands: Commands,
    mut glow: ResMut<EnemyGlow>,
    objects: Objects,
    ranks: CreatureRanks,
    masked: Query<(Entity, &SceneFigureMask)>,
) {
    let want: Vec<(Entity, Glow)> = if glow.on && objects.player().is_some() {
        objects
            .iter()
            .filter_map(|(guid, e, f)| {
                let g = if benilla_protocol::guid::is_gameobject(guid) {
                    glow_of_object(f.object_entry())
                } else if benilla_protocol::guid::is_creature_or_pet(guid) {
                    glow_of_unit(ranks.rank(e), f.unit_health().is_none_or(|h| h > 0))
                } else {
                    None
                }?;
                Some((e, g))
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut glowing = [0; 2];
    for &(_, g) in &want {
        glowing[(g == Glow::Rare) as usize] += 1;
    }
    if glow.glowing != glowing {
        glow.glowing = glowing;
    }
    for (e, mask) in &masked {
        let ours = mask.0 == ELITE_MASK || mask.0 == RARE_MASK;
        if ours && !want.iter().any(|&(w, g)| w == e && g.mask() == mask.0) {
            commands.entity(e).remove::<SceneFigureMask>();
        }
    }
    for (e, g) in want {
        if masked.get(e).map(|(_, m)| m.0) != Ok(g.mask()) {
            commands.entity(e).insert(SceneFigureMask(g.mask()));
        }
    }
}

/// The node over the window for each colour, shown while a figure wears it, its pulse moving.
fn draw_the_glows(
    mut commands: Commands,
    glow: Res<EnemyGlow>,
    image: Res<FigureMaskImage>,
    time: Res<Time>,
    mut materials: ResMut<Assets<OutlineGlow>>,
    nodes: Option<Res<GlowNodes>>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(nodes) = nodes else {
        // made once the first glowing figure's mask has its picture
        return make_the_nodes_when_ready(&mut commands, &image, &mut materials);
    };
    for &(mask, node, ref material) in &nodes.0 {
        let shown = glow.glowing[(mask == RARE_MASK) as usize] > 0;
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Ok(mut v) = visibility.get_mut(node) {
            v.set_if_neq(want);
        }
        if shown {
            if let (Some(m), Some(picture)) = (materials.get_mut(material), image.0[mask].as_ref()) {
                m.params.w = time.elapsed_secs();
                if m.mask != *picture {
                    m.mask = picture.clone();
                }
            }
        }
    }
}

/// Makes both colour nodes once either mask has a picture; a mask with none yet borrows the
/// other's until its own comes (its node stays hidden until a figure wears its colour).
fn make_the_nodes_when_ready(
    commands: &mut Commands,
    image: &FigureMaskImage,
    materials: &mut Assets<OutlineGlow>,
) {
    let Some(any) = image.0[ELITE_MASK].clone().or_else(|| image.0[RARE_MASK].clone()) else {
        return;
    };
    let mut made = |mask: usize, colour: Vec4, name: &'static str| {
        let material = materials.add(OutlineGlow {
            colour,
            params: Vec4::new(EDGE, GLOW, PULSE, 0.0),
            mask: image.0[mask].clone().unwrap_or_else(|| any.clone()),
        });
        let node = commands
            .spawn((
                Name::new(name),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                MaterialNode(material.clone()),
                // over the world, under the run's own windows
                GlobalZIndex(crate::ui::layer::METER - 10),
                Pickable::IGNORE,
                Visibility::Hidden,
            ))
            .id();
        (mask, node, material)
    };
    let nodes = [
        made(ELITE_MASK, PURPLE, "elite glow"),
        made(RARE_MASK, SILVER, "rare glow"),
    ];
    commands.insert_resource(GlowNodes(nodes));
}

/// `ESURV_GLOW_CHECK`'s state: when the option goes off, and the frame times gathered.
#[derive(Resource)]
struct GlowCheck {
    off_after: f32,
    entered_at: Option<f32>,
    turned_off: bool,
    window_from: f32,
    frames: Vec<f32>,
}

impl GlowCheck {
    fn from_env() -> Option<Self> {
        let off_after = std::env::var("ESURV_GLOW_CHECK").ok()?.parse().ok()?;
        Some(Self {
            off_after,
            entered_at: None,
            turned_off: false,
            window_from: 0.0,
            frames: Vec::new(),
        })
    }
}

/// How long each frame-time line covers (seconds).
const CHECK_WINDOW: f32 = 2.0;

fn check_the_glows(
    mut check: ResMut<GlowCheck>,
    glow: Res<EnemyGlow>,
    script: Option<NonSend<UiScript>>,
    objects: Objects,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let Some(entered) = check.entered_at else {
        if let (Some(_), Some(script)) = (objects.player(), script) {
            // on to begin with, whatever the last run left, and benilla's FPS journal on (the
            // CPU and GPU time of each second, in the state folder's Diagnostics)
            let _ = script.eval::<()>(&format!("SetCVar(\"{}\", \"1\")", glow_option::CVAR));
            let _ = script.eval::<()>("SetCVar(\"fpsJournal\", \"1\")");
            check.entered_at = Some(now);
            check.window_from = now;
        }
        return;
    };
    check.frames.push(time.delta_secs() * 1000.0);
    if now - check.window_from >= CHECK_WINDOW {
        let n = check.frames.len().max(1) as f32;
        let mean = check.frames.iter().sum::<f32>() / n;
        let worst = check.frames.iter().copied().fold(0.0, f32::max);
        info!(
            "enemy glow check: {:.0}-{:.0} s in: {} frames, mean {mean:.2} ms, worst {worst:.2} ms; option {}, {} elites and {} rares glowing",
            check.window_from - entered,
            now - entered,
            check.frames.len(),
            if glow.on { "on" } else { "off" },
            glow.glowing[0],
            glow.glowing[1],
        );
        check.frames.clear();
        check.window_from = now;
    }
    if !check.turned_off && now - entered >= check.off_after {
        if let Some(script) = script {
            let _ = script.eval::<()>(&format!("SetCVar(\"{}\", \"0\")", glow_option::CVAR));
            info!("enemy glow check: the option turned off");
            check.turned_off = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elites_glow_purple_rares_and_their_caches_silver_the_rest_none() {
        assert_eq!(glow_of_unit(creature_rank::ELITE, true), Some(Glow::Elite));
        assert_eq!(glow_of_unit(creature_rank::RARE, true), Some(Glow::Rare));
        assert_eq!(glow_of_unit(creature_rank::BOSS, true), None);
        assert_eq!(glow_of_unit(creature_rank::NORMAL, true), None);
        assert_eq!(glow_of_unit(creature_rank::ELITE, false), None, "a dead elite stops");
        assert_eq!(glow_of_object(Some(RARE_CACHE_ENTRY)), Some(Glow::Rare));
        assert_eq!(glow_of_object(Some(RARE_CACHE_ENTRY - 7)), None, "a battleground cache");
        assert_eq!(glow_of_object(None), None);
        assert_eq!(Glow::Elite.mask(), ELITE_MASK);
        assert_eq!(Glow::Rare.mask(), RARE_MASK);
        assert_ne!(ELITE_MASK, crate::menus::outline::HERO_MASK);
        assert!(RARE_MASK < benilla_app::FIGURE_MASKS);
    }
}
