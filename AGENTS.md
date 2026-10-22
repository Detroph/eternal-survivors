# Eternal Survivors

A survivors-style roguelite set in the World of Warcraft 1.12.1 world: the benilla client
(`crates/benilla-*`) with the game's own embedded server and rules (`crates/esurv-*`), driven by
data in `content/`.

Read, in this order:

1. `docs/game/README.md`: the game's systems, where each lives, and their docs.
2. `docs/game/AUTHORING.md`: how zones, enemies, bosses, spells, talents and gear are written.
3. `docs/CONTRIBUTING.md`: building it, the checks, and feedback.

For work inside the client crates, benilla's own notes apply: `docs/METHOD.md` and
`docs/MAP.md`.

The rest of the root: `crates/` the workspace, `content/` the game's data, `docs/` the docs,
`reference/` the 1.12 name catalogues the client's tests check against, `scripts/` the checks and
tools, `third_party/` vendored code.
