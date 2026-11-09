# Game docs

Design and reference docs for Eternal Survivors. Each reference has a plain-language twin in
[guides/](guides).

| Doc | Guide | What |
|---|---|---|
| [design.md](design.md) | | Architecture, ground rules, tuning, clear names and release builds |
| [AUTHORING.md](AUTHORING.md) | | How to add or change content with data alone: a worked example for a zone, enemy kind, boss, class, spell, talent, gear, ring or trinket, amulet, race passive, player character with its assist and achievement; the shared-number files; the rules authors must know; what is still code. Templates: [templates/](templates) |
| [zones.md](zones.md) | [guides/zones.txt](guides/zones.txt) | Zones, enemies, bosses, rares, and how difficulty rises between zones; how to add one (a file per zone; copy [templates/zone.ron](templates/zone.ron)) |
| [enemy-weapons.md](enemy-weapons.md) | | What each creature enemy and boss holds (vmangos's equipment), and how the client shows and swings it |
| [gear.md](gear.md) | [guides/gear.txt](guides/gear.txt) | Class gear, drops, caches, jewelry, amulets and pickups; how to add or change gear |
| [assists.md](assists.md) | | The battle standard pickup and the assists it summons: the hand-made player characters (one list for the Warsong bosses and the assists), their scripts and the class defaults |
| [bosses.md](bosses.md) | [guides/bosses.txt](guides/bosses.txt) | How to make a boss and add, change or remove its abilities; copy [templates/boss.ron](templates/boss.ron) |
| [spells.md](spells.md) | [guides/spells.txt](guides/spells.txt) | How to make a spell, field by field; copy [templates/spell.ron](templates/spell.ron); spell categories and changing a whole kind of spell at once |
| [classes.md](classes.md) | [guides/classes.txt](guides/classes.txt) | Classes, races, hero stats, spells and their delivery kinds, cards, talents; how to add them |
| [server.md](server.md) | [guides/server.txt](guides/server.txt) | The game's own server: launch, zone list, heroes and the save, what the client is told, the update check and hand-off to the setup, switches for testing |
| [screens.md](screens.md) | [guides/screens.txt](guides/screens.txt) | The game's own windows over WoW's interface: the stock look kit, the level-up and cache cards, the damage meter and its feed, the end panel, the HUD and its Escape menu |
| [menus.md](menus.md) | | The menus before a run: class picker, hero sheet with its Character, Collection and Talents tabs, the warband select with its zone selector, and their art style; the walkable camp at the Caverns of Time that takes the warband select's place (`ESURV_HUB=0` brings the select back) |
| [achievements.md](achievements.md) | | The warband's achievements: their rules, every criteria kind, what a run counts and when, what the save keeps, what an older save is granted, and what the screens are told; copy [templates/achievement.ron](templates/achievement.ron) |

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
3. Start a new run. The menus before a run (the camp, its zone list, hero sheet, codex)
   show the new content at once; a zone file added appears in the zone list. A run already going
   on keeps the content it started with to its end, so nothing changes mid-fight.

Read once at launch, so they need a restart: the races `races.ron` adds to the create screen and
their looks, and the ground data opened for them. A zone added, removed or moved while a run is
going on can file that run's gear drops under the wrong zone; change the zone list between runs.
A player build (`--no-default-features`) has no watcher: no thread, and the `notify` crate is
not built in.
