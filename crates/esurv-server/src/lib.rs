//! The game's own 1.12.1 server, run on a thread inside the game process (design step 3):
//! `realmd` (SRP6 logon, one realm), `world` (characters as heroes, the run as the world)
//! and the bridge that turns simulation events into packets.

pub mod client;
pub mod entries;
pub mod game;
pub mod ground;
pub mod looks;
pub mod packets;
pub mod prerun;
pub mod realmd;
pub mod save;
pub mod srp_server;
pub mod update;
pub mod wire;
pub mod walk_graph;
pub mod wmo_floor;
pub mod world;

use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;

use anyhow::{Context, Result};
use esurv_content::LiveContent;

use crate::ground::GameData;
use crate::realmd::{RealmEntry, RealmdConfig, SessionKeys};
use crate::save::Saves;

/// The one realm the realm list shows: the world listener every zone is played from. The zone is
/// picked with the hero on the character screen, not in the realm list.
pub const REALM_NAME: &str = "Eternal Survivors";

/// How to run the server.
pub struct ServerConfig {
    /// The content; each run takes it as it is when the run starts (see [`LiveContent`]).
    pub content: Arc<LiveContent>,
    pub saves: Arc<Saves>,
    /// The player's 1.12.1 data (ground heights, missile speeds); `None` plays on flat ground.
    pub data: Option<Arc<GameData>>,
    /// Every account logs in with this password.
    pub password: String,
    /// The address the listeners bind (`127.0.0.1` to play alone).
    pub bind_host: String,
    /// The address clients reach the world listener at, as the realm list gives it.
    pub public_host: String,
    /// The logon port; 0 takes any free one.
    pub realmd_port: u16,
    /// Runs start as the hero enters, without waiting for a first move (a dev switch).
    pub autostart: bool,
    /// Every hero enters this zone (its index in the zones' order) whatever was chosen for it (a
    /// dev switch, `ESURV_ZONE`); a zone its class has not opened is still refused.
    pub zone: Option<usize>,
    /// What every hero is given from the start of its run (`Game::grant`): rings, trinkets and
    /// amulets by id (`id@rarity` for a ring or trinket at a rarity, 0 common to 4 legendary),
    /// runes by name (`speed_rune:<yards ahead>`): a dev switch (`ESURV_GRANT`).
    pub granted: Vec<String>,
    /// Each hero's moves are logged to this file, a line per move: run seconds, hero, x, y, the
    /// run speed the server holds (a dev switch, `ESURV_MOVE_LOG`).
    pub move_log: Option<std::path::PathBuf>,
}

/// A running server.
pub struct ServerHandle {
    /// Where the client logs on.
    pub realmd: SocketAddr,
    /// The world listener, the one realm.
    pub world: SocketAddr,
}

/// Starts realmd and the world listener, each on its own thread, and returns at once.
pub fn start(config: ServerConfig) -> Result<ServerHandle> {
    let keys = SessionKeys::default();
    let shared = Arc::new(world::Shared {
        content: config.content.clone(),
        saves: config.saves.clone(),
        data: config.data,
        keys: keys.clone(),
        autostart: config.autostart,
        zone: config.zone,
        granted: config.granted,
        move_log: config.move_log,
    });
    let listener =
        TcpListener::bind((config.bind_host.as_str(), 0)).context("opening the world listener")?;
    let world = listener.local_addr()?;
    std::thread::Builder::new()
        .name("world".into())
        .spawn(move || world::serve(listener, shared))?;
    let address = format!("{}:{}", config.public_host, world.port());
    let saves = config.saves.clone();
    let realm_list = move || {
        vec![RealmEntry {
            name: REALM_NAME.into(),
            address: address.clone(),
            characters: saves.heroes().len().min(255) as u8,
        }]
    };
    let listener = TcpListener::bind((config.bind_host.as_str(), config.realmd_port))
        .context("opening the logon listener")?;
    let realmd = listener.local_addr()?;
    let realmd_config = RealmdConfig {
        password: config.password,
        realms: Arc::new(realm_list),
        keys,
    };
    std::thread::Builder::new()
        .name("realmd".into())
        .spawn(move || realmd::serve(listener, realmd_config))?;
    Ok(ServerHandle { realmd, world })
}

#[cfg(test)]
mod tests;
