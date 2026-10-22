# Game docs

Design and reference docs for Eternal Survivors. Each reference has a plain-language twin in
[guides/](guides).

| Doc | Guide | What |
|---|---|---|
| [design.md](design.md) | | Architecture, ground rules, and the build steps |
| [AUTHORING.md](AUTHORING.md) | | How to add or change content with data alone: a worked example for a zone, enemy kind, boss, class, spell, talent, gear, ring or trinket, amulet and race passive; the shared-number files; the rules authors must know; what is still code. Templates: [templates/](templates) |
| [zones.md](zones.md) | [guides/zones.txt](guides/zones.txt) | Zones, enemies, bosses, rares, and how difficulty rises between zones; how to add one (a file per zone; copy [templates/zone.ron](templates/zone.ron)) |
| [enemy-weapons.md](enemy-weapons.md) | | What each creature enemy and boss holds (vmangos's equipment), and how the client shows and swings it |
| [gear.md](gear.md) | [guides/gear.txt](guides/gear.txt) | Class gear, drops, caches, jewelry, amulets and pickups; how to add or change gear |
| [bosses.md](bosses.md) | [guides/bosses.txt](guides/bosses.txt) | How to make a boss and add, change or remove its abilities; copy [templates/boss.ron](templates/boss.ron) |
| [spells.md](spells.md) | [guides/spells.txt](guides/spells.txt) | How to make a spell, field by field; copy [templates/spell.ron](templates/spell.ron); spell categories and changing a whole kind of spell at once |
| [classes.md](classes.md) | [guides/classes.txt](guides/classes.txt) | Classes, races, hero stats, spells and their delivery kinds, cards, talents; how to add them |
| [server.md](server.md) | [guides/server.txt](guides/server.txt) | The game's own server: launch, zone list, heroes and the save, what the client is told, switches for testing |
| [screens.md](screens.md) | [guides/screens.txt](guides/screens.txt) | The game's own windows over WoW's interface: the stock look kit, the level-up and cache cards, the damage meter and its feed, the end panel, the HUD and its Escape menu |
| [menus.md](menus.md) | | The menus before a run: class picker, hero sheet with its Character, Collection and Talents tabs, the warband select with its zone selector, and their art style |
| [roadmap.md](roadmap.md) | [guides/roadmap.txt](guides/roadmap.txt) | Changes planned after version 1, and what the build prepares for them |

## Content

`content/*.ron` holds every enemy kind, class, race, spell, talent, gear set, ring, trinket,
amulet and run option; `content/zones/` holds one file per zone (`<id>.ron`: its place, roster
and bosses), and the zones are ordered by their difficulty, so a zone is added with its file
alone (zones.md, section 5). `content/spell_categories.ron` changes every spell of a category at
once (all 1.0 as shipped). `cargo test -p esurv-content -p esurv-sim` checks it loads, validates and
plays by the rules. `cargo run -p esurv-sim --example measure_runs` plays one run per zone
and class with a fixed hero and seed and prints how long each boss took to kill against what the
hero's spells say they deal, the damage the hero took per minute, and a check of every spawn's
health and damage against the scaling formulas.

### Editing content while the game runs (dev builds)

A dev build (`cargo run -p esurv`, the default `dev` feature) watches the content folder,
subfolders included, and loads it again half a second after the last `.ron` file is saved:

1. Edit a file under `content/` (a number in `classes.ron`, a zone in `content/zones/`, a new
   zone file) and save it.
2. The game says **Content reloaded** over the menus (in a run, as a HUD line), and the log says
   it too. If the folder does not load (a typo, a broken reference), the game keeps the content
   it had and says **Content did not reload: ...**; the log has the whole error. Fix the file and
   save again.
3. Start a new run. The menus before a run (warband screen and its zone list, hero sheet, codex)
   show the new content at once; a zone file added appears in the zone list. A run already going
   on keeps the content it started with to its end, so nothing changes mid-fight.

Read once at launch, so they need a restart: the races `races.ron` adds to the create screen and
their looks, and the ground data opened for them. A zone added, removed or moved while a run is
going on can file that run's gear drops under the wrong zone; change the zone list between runs.
A player build (`--no-default-features`) has no watcher: no thread, and the `notify` crate is
not built in.

## Status

| Step | State |
|---|---|
| 0 Foundation | done |
| 1 Content model | done |
| 2 Simulation rules | done: a headless run plays from the first step to a win or a loss (spawning, bosses, rares, spells, pets, milestones, pickups, choice screens, gear drops, the end tally); open details are marked in the spec for the play check |
| 3 Embedded server | done: the launcher starts the server, the client logs in by itself into the game's one realm, heroes are created and kept in a save, and entering a zone starts a run whose enemies are created, walked along the real ground and removed |
| 4 Combat presentation | done: spell looks, damage and heal numbers, swings, enemy casts, deaths and corpses, auras and debuffs, totems, looks lying on the ground, pets, boss abilities, milestone procs, pickups on the field, shouts and centre-screen lines, the action bar with cooldowns, and the character-pane numbers line; the boss bar comes with the HUD (step 6) |
| 5 Progression and rewards | done: the save keeps what each class earns (talent experience and ranks, zones won, gear worn and its look) and each hero's records; a run pays out whether it is won, lost or left (gear joins the collection the moment it drops); a class enters only the zones it opened; a new run brings the class's talents and gear; a won Elwynn opens Duskwood and everything is kept across a restart (tested) |
| 6 Screens | done: the menus before a run in a look of their own (class picker, hero sheet with Character, Collection and Talents, the warband camps with the codex, zone selector, run options and options with save export and import); the run's screens (choice cards in the same look with text from the content, the HUD and its Escape menu with options, the end panel, the damage meter); the stock frames of a run (character pane with the run's numbers, worn gear and jewelry; one-tab spellbook; read-only talents; the rest hidden); heroes wear their gear in the world. A whole session plays without a stock menu ([screens.md](screens.md), [menus.md](menus.md)); gamepad and touch moved to the roadmap (R7) |
| 7 Play check | done: every rule checked against its design, mismatches fixed with tests; enemy and boss numbers follow their formulas exactly (headless measurements) |
| 7b Tuning | numbers done: one enemy formula (balance.ron: base 240/8, type multipliers, clocks, zone D), toughness, spawn budget, live cap, hero growth per level, experience; cards and hero base stats; every class spell (roles, cooldowns, damage over time, burns, impacts, repeating novas) and the spell tags; talent experience and point curve; plain talent values; gear by zone with its drops (`balance.ron` `zone_scaling`); ring and trinket values; cache prices and run options; boss ability and enemy roster standards; class, race and pet base stats. Kept as designed: the class spell lists, spell ranges and radii, zone rosters and bosses. Left for later reworks: milestone talents, rings, trinkets and amulets, weapon swings |
| 8 Extensibility | started: the authoring guide ([AUTHORING.md](AUTHORING.md)) with tested templates; enemy kinds and boss ability numbers defined once (`enemy_kinds.ron`); zones are one file each in `content/zones/`, ordered by difficulty, and open by the hardest zone won; every zone list follows that order and scrolls when long; a zone added with data alone loads, is listed, opens in its place and plays (tested); dev builds reload the content as its files change (new runs and the menus take it, a run going on keeps its own; see above). Left: behaviour kinds as one module each |
| 9 | started: the golden run (`examples/golden_run.rs`); next: the inventory, WoW client text as pointers, our own names for numbered kinds, the installer with its patch MPQ |
| 10 Release | done: tagged release archives, v0.1.0 to v0.3.0 |
