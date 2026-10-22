# Contributing

Eternal Survivors is my own version of a survivors-style roguelite set in the World of Warcraft
1.12.1 world, built on the [benilla](https://github.com/samwhosung/benilla) client. I develop it
myself, so this repository doesn't take pull requests.

## Feedback and bug reports

Feedback, ideas and bug reports are welcome on the [Eternal Survivors Discord](https://discord.gg/X5USKT65Jq). For a bug, say what
you did, what you saw and what you expected, with the game's version, your class, starting spell
and zone, and the log from the game folder (`logs\esurv.log`, and `esurv.prev.log` for the run
before).

## Make your own version

You're welcome to fork it and make it your own, under its license (MIT or Apache-2.0, at your
option). Keep the license files and notices, benilla's included (`docs/BENILLA.md`), and keep
Blizzard's files out: the game reads each player's own 1.12.1 client and ships none of its art,
models, sounds, data or interface code.

## Building it

- **Rust** (the version is pinned in `rust-toolchain.toml`) and a C compiler, because the client's
  Lua is built from source. On Windows, the MSVC build tools.
- **Your own WoW 1.12.1 client** (build 5875, enUS). Point `WOW_DATA` at its `Data` folder, or link
  it as `WoW` at the repository root (`scripts/run-game.ps1` shows how).
- **Run it:** `scripts/run-game.ps1` builds and starts a dev build.
- **Test it:** the game's crates (benilla-app's own tests need Linux or macOS):

  ```
  cargo test --release -p esurv -p esurv-app -p esurv-content -p esurv-protocol -p esurv-server -p esurv-sim -p esurv-client-check -p esurv-setup
  ```

- **The full check:** `scripts/local-check.ps1` runs the tests, the golden run, the client-text scan,
  and the release build and archive with their scans.
- **Content is data:** zones, enemies, bosses, spells, talents and gear are RON files in
  `content/`; `docs/game/AUTHORING.md` explains them, with templates in `docs/game/templates/`.
