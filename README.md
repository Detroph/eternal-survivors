# Eternal Survivors

A survivors-style roguelite set in the World of Warcraft 1.12.1 world. Pick a hero, drop into a
zone and hold out against ever-growing hordes, bosses and rares while your spells fire on
their own. Level-ups offer new spells and upgrades; caches offer rings, trinkets and amulets.
Bosses drop class gear that your hero keeps between runs.

The game runs on [benilla](https://github.com/samwhosung/benilla), an open-source 1.12.1 client
written in Rust and Bevy, with its own server embedded in the same executable.

Inspired by *Azeroth Survivors*. This project is not affiliated with it or its author.

> **Status:** alpha (v0.4.0). Zones from Elwynn Forest to Ahn'Qiraj, nine classes, every boss with
> its own mechanics, and a setup that installs and updates the game. See
> [docs/game/design.md](docs/game/design.md) for the steps.

Downloads are on the [Releases](https://github.com/Detroph/eternal-survivors/releases) page.
Feedback, ideas and bug reports go to the [Eternal Survivors Discord](https://discord.gg/X5USKT65Jq).
This is my own version and takes no pull requests, but you're welcome to fork it and make your own
([docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)).

## Install and update

1. Download `eternal-survivors-<version>.zip` from the
   [Releases](https://github.com/Detroph/eternal-survivors/releases) page and unzip it.
2. Copy `Eternal Survivors Setup.exe` into your WoW 1.12.1 folder (beside `WoW.exe`) and run it
   there. It checks the client and installs the game into `Eternal Survivors\` inside that folder.
3. Start `Eternal Survivors\Eternal Survivors.exe`.

The game checks for a newer version when it starts and offers it on the character screen: Update
downloads it, checks it against the release's `.sha256` and restarts the game into it, keeping
your heroes and settings (Options > Interface > Check for updates turns this off). Players on
v0.4.0 or older update by hand once, running the new setup in the WoW folder as above; later
versions offer their updates in the game. Each release carries the zip and its
`eternal-survivors-<version>.zip.sha256`.

## Requirements

- A legally owned **World of Warcraft 1.12.1** client (build 5875, enUS). The game reads all art,
  models and sound from your own install; this repository contains no Blizzard files.
- Rust (the version is pinned in `rust-toolchain.toml`) and, on Windows, the MSVC build tools.

## Build and run

```powershell
# once: link your client at the repo root (the link is gitignored)
New-Item -ItemType Junction -Path WoW -Target "C:\path\to\WoW 1.12"

# build and run a dev build
.\scripts\run-game.ps1
```

Run the tests:

```bash
cargo test --release -p esurv -p esurv-app -p esurv-content -p esurv-protocol -p esurv-server -p esurv-sim -p esurv-client-check -p esurv-setup
```

## Repository layout

| Path | What |
|---|---|
| `crates/esurv` | The game executable |
| `crates/esurv-content` | Data model and loader for everything in `content/` |
| `crates/esurv-sim` | The game rules as a headless, deterministic simulation |
| `crates/esurv-server` | The embedded logon and world server |
| `crates/esurv-protocol` | The messages between the server and the game's screens |
| `crates/esurv-app` | The game's own screens over the client |
| `crates/benilla-*` | benilla, the client this game runs on |
| `content/` | Game data (RON): zones, enemies, bosses, classes, spells, talents, gear, jewelry |
| `docs/game/` | Design and reference docs; `docs/game/guides/` has plain-language versions |

## Making content

Zones, enemies, bosses, classes, spells, talents and gear are plain data files under `content/`.
The references explain every field and how to add your own:

- [Zones, enemies and bosses](docs/game/zones.md)
- [Gear, jewelry and amulets](docs/game/gear.md)
- [Classes, spells and talents](docs/game/classes.md)
- [Making a boss](docs/game/bosses.md) (copy [the template](docs/game/templates/boss.ron))
- [Making a spell](docs/game/spells.md) (copy [the template](docs/game/templates/spell.ron))
- [Roadmap](docs/game/roadmap.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option, like
benilla, whose README is kept at [docs/BENILLA.md](docs/BENILLA.md).

World of Warcraft is a trademark of Blizzard Entertainment. This project is not affiliated with
or endorsed by Blizzard Entertainment. Inspired by *Azeroth Survivors*, with no affiliation.
