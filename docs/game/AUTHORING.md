# Adding and changing content

Everything a run is built from (zones, enemies, bosses, classes, races, spells, talents, gear,
rings, trinkets, amulets and the shared numbers) is plain text in the `content\` folder. This
guide shows how to add or change each kind of thing with data alone, with a worked example for
each, and says honestly where a change still needs code.

The detailed references stay the place for every field and rule; this guide links to them
instead of repeating them:

| Reference | What it covers |
|---|---|
| [zones.md](zones.md) | zones, rosters, rares, the enemy formula, how difficulty rises |
| [bosses.md](bosses.md) | bosses, their ability kinds and the building blocks (marks, zones, objects, scripts, warnings) |
| [classes.md](classes.md) | classes, races, hero stats, level-up cards, talents and milestones |
| [spells.md](spells.md) | spells field by field, delivery kinds, on-hit effects, categories and tags |
| [gear.md](gear.md) | class gear, drops, caches, rings, trinkets, amulets |
| [menus.md](menus.md) | the menus before a run, where zones, classes and talents show up |
| [assists.md](assists.md) | the battle standard, the hand-made player characters it summons and their assists |

Ready-to-copy blocks are in [templates/](templates):

| Template | Goes in |
|---|---|
| [zone.ron](templates/zone.ron) | a new file `content\zones\<id>.ron` |
| [boss.ron](templates/boss.ron) | a zone file's `bosses` list |
| [spell.ron](templates/spell.ron) | `content\abilities.ron` |
| [class.ron](templates/class.ron) | `content\classes.ron` |
| [gear_set.ron](templates/gear_set.ron) | `content\gear.ron`, its `sets` list |
| [talent.ron](templates/talent.ron) | a tree's `talents` list in `content\talents.ron` |
| [jewelry.ron](templates/jewelry.ron) | `content\jewelry.ron` (a ring or a trinket) |
| [amulet.ron](templates/amulet.ron) | `content\amulets.ron` |
| [player_character.ron](templates/player_character.ron) | `content\player_characters.ron` (a Warsong boss and an assist) |

Every template is checked by `cargo test -p esurv-content`, so a template that stops
loading is caught at once.

## 1. Before you start

### Where the game reads content from

The game loads `content\` once, when it starts: on a development build the project's own
`content\` folder, in a release the one beside the game, or the folder named by
`ESURV_CONTENT` ([server.md](server.md)). **Edit, then restart the game.** Nothing needs
rebuilding, and there is no reload while the game is running yet.

### A few words about RON

The files are RON, a simple text format:

* `// ...` is a comment; it is ignored, so use it freely for notes.
* `( name: value, ... )` is one record; `[ a, b, c ]` is a list; `{ Key: value }` is a table
  (only `enemy_kinds.ron` uses one).
* Text goes in double quotes: `name: "Hogger"`. Names of kinds go without: `school: Fire`,
  `kind: Fodder`.
* Write numbers the way the record around them does: `6.0` where the others have a decimal
  point, `6` where they do not (`entry`, `targets`, `ticks` and counts are whole numbers).
* An optional value is `None` or `Some(...)`: `look_spell: Some(133)`.
* **A name the WoW client already holds is a pointer, not text** (the
  repository holds no client text). A spell's, talent's, talent tree's or race passive's name, a
  zone's, a class's or a race's is written as the client's row id, `name: Spell(12294)`,
  `name: Area(12)`, `name: Class(1)`, `name: Race(1)`, `name: TalentTab(161)`, and the game
  reads the name from the player's own client as it loads. A name of our own, one the client
  does not have word for word, stays text: `name: "Frost Lance"`. Enemy, boss, pet and item
  names, and boss yells, are always text. Inside our own words (a description, a talent's
  `text`), `{Spell(772)}` shows the client's name for that spell. Without client data (a dev
  build with no `WoW` folder) a pointer shows as `Spell 12294`. To find an id, look the name up
  in `Spell.dbc` (any rank of the spell will do; the shipped content uses the lowest rank) or
  `AreaTable.dbc`; `cargo run -p esurv-content --features client --example
  scan_client_names` lists every client name still written as text, with the pointer to write
  for it (a legendary's `text` and a player character's `assist` take `{Spell(...)}` too). One
  that must stay text (a test message, our own word that equals a client name) goes in
  `crates/esurv-content/examples/scan_client_names.allow`, one line with its reason.
* A comma after the last item of a list is fine, and a missing comma between items is the most
  common mistake. The error message names the file, the line and the column.

### The checks

| Check | What it tells you |
|---|---|
| `cargo test -p esurv-content` | Every file loads, every id one record names exists, the zone rules hold, and the templates still load. Run it after every change. |
| `cargo test -p esurv-sim` | The run's rules still hold with your data: for example every class spell hits its role's damage (spells.md, section 5c). |
| `cargo test -p esurv-app` | Slower, as it builds the game's window code: every spell's card text fills in at ranks 1 to 10 (`every_spell_description_fills_at_every_rank`). Run it after adding or rewording a spell. |
| `cargo run -p esurv-sim --example measure_runs -- <zone id>` | Plays one run per class in that zone without the game, and prints how long each boss took to die, the damage the hero took per minute, and a check of every spawn's health and damage against the formula. Leave out the zone to play them all. |
| In the game | `ESURV_ZONE=<zone id>` sends every hero into that zone; `ESURV_GRANT=<id>,...` gives every hero those rings, trinkets or amulets from the start of a run ([server.md](server.md), "Switches"). |

**Some tests guard the shipped numbers on purpose.** `crates\esurv-content\tests\load.rs`
counts the shipped records (zones, enemies, bosses, classes, spells, talent trees and talents,
rings and trinkets, run options, pets, races) and spot-checks a few values (the enemy formula,
the zone difficulties, some spells, the dwarf, the run options, the kind numbers, the category
and tag numbers all 1.0). They are there so a value never changes by accident. When you add a
record or change one of those values on purpose, the test fails and its message shows the old and
the new value: change the number in the test to the new one, in the same commit as the data.
That is the one edit outside `content\` that ordinary content work needs. The other is a new
class spell's role (section 7).

## 2. Rules every author should know

**Zones**

* **One file per zone.** `content\zones\<id>.ron` holds the whole zone, and its file name must
  be its `id`. No list or code names zones.
* **Zones are ordered by their difficulty D, lowest first.** That order is the unlock path and
  the order of every list of zones (the zone selector, the collection log, the class picker's
  count). Two zones may not share a difficulty, and D must be above 0. A zone added in the middle
  takes its place there and leaves the others as they were.
* **Winning a zone opens every zone up to it and the next one.** A class starts with the first
  zone only. A zone added below a class's hardest win is open at once.
* **Health and damage are never set per enemy.** Every enemy, elite, rare and boss takes them
  from one formula in `balance.ron`: health grows by a fixed factor for every step of D, damage by
  0.6 of that factor's exponent, both with the run clock (section 3). A normal enemy only says how
  sturdy it is next to its zone's average (`toughness`, a quarter step: 0.25, 0.5, 0.75, 1, 1.25 ...).
* **Roster numbers come from the enemy's kind** (`enemy_kinds.ron`), boss ability numbers from
  the ability's kind, and every boss moves at one speed. A single enemy or ability may override
  a number for itself.
* **Rares come from the roster's elites**, so a zone needs at least one enemy of kind
  `Elite` to have rares (5 or 6 a run, on the elite timer). A boss's `Summon` calls the roster's first two
  enemies, so a zone with summoning bosses needs at least two roster enemies.
* **Exactly one final boss per zone, and it comes last.** Its death wins the run.

**Rewards follow D, so a new zone needs no reward numbers of its own** (all in `balance.ron`
`zone_scaling`, section 3)

* **Gear:** a piece from a zone of difficulty D has its Elwynn stats × the gear factor of D
  (D^0.55 up to D 9, then ×1.3 a step), rounded up to whole numbers. Its name and look are its
  zone's own (`gear.ron` `looks`, the weapons' in `loadouts.ron`; a zone
  may add rows of looks, `row: 1`, each optionally named with `row_name` as Stratholme's
  "Scholomance sets"); its
  colour follows the zone's gear tier, which also comes from D: tier 1 from D 2, tier 2 from 4,
  tier 3 from 7, tier 4 from 9.
* **Every zone drops its whole set:** its non-final bosses share the eight armor slots, its final
  boss drops the class's weapons.
* **Drop chance:** each piece rolls at the chance of its D's step (100 % at D 1, 80 % from 2, 70 % from 4,
  60 % from 6, 40 % from 8 on), × the run
  option bonus.
* **Talent experience:** a run's talent experience is × √D (×1 in Elwynn, ×3.16 in Ahn'Qiraj),
  then × the run option bonus.

**Spells**

* Every spell needs a `description` (its card text, in our own words) and at least one category.
* Every spell has **tags** for the tag coefficients in `balance.ron`: exactly one damage tag
  if it deals damage (`Level1`, `Nuke`, `Cleave`, `Dot`, `Nova`, `Area` or `Pet`, by its shape),
  plus `Sustain`, `Buffs` or `Defensive` for what its on-hit effect gives. The starting options'
  spells (and what their passives cast), and only those, are `Level1`. The tests check all of this (spells.md, section 8a).
* **Race spells are outside the spell tags:** they have no tags, take no tag coefficient and no
  class `spell_damage_scale`, and play exactly as their numbers say. They are tuned one by one.
* **Every class spell has a role** (filler, nuke, few targets, over time ...) that sets its
  damage at level 1 (spells.md, section 5c), checked by a test.
* Other rules of ours a spell can use: a heal waiting for a lower health (`heal_below`), a hit
  that leaves a burn (`burn`), a ground area landing with an impact (`impact`), double damage on
  enemies below 30 % (`double_on_low_health`), a nova that repeats (`ticks` on a `Nova`), and
  healing charges (`HealingCharges`). They are in spells.md, sections 2 and 5c.

**Classes, races and talents**

* Each class has one damage knob, `spell_damage_scale` (1 for all today), which never reaches
  cooldowns, durations or the race spell.
* A race may be more classes than the stock create screen offers (`added_classes`).
* Talents without a milestone use one standard value per rank for what they change (classes.md,
  section 4, "Talent values").

## 3. The shared-number files

These files hold numbers that many records share. Change a number here and everything that uses
it follows.

### `balance.ron`

| Part | What it controls |
|---|---|
| `base_health`, `base_damage` | What every enemy starts from: 240 health and 8 damage. |
| `zone_scaling` | Everything that follows D: enemy health and damage growth, gear, drops, talent experience, gear tiers (below). |
| `normal`, `elite`, `rare`, `boss`, `final_boss` | Each kind's multipliers on health and damage (an elite ×10 health and ×3 damage ...). |
| `health_clock`, `damage_clock` | How much sturdier and harder-hitting enemies are by the run minute they appear: points `(minute, multiplier)`, straight lines between them, and past the last point the last step's pace goes on. |
| `spawn_budget` | How much enemy arrives each second at 100 % density, by minute (each enemy costs its kind's `spawn_cost`). |
| `live_cap` | Most normal enemies alive at once: at 100 % density, and at the highest density. |
| `battle_standard` | The pickup that summons an assist (section 13a, [assists.md](assists.md)): how many a run lays (`per_run`, 1 to 2), one more every `endless_every` seconds in Endless (600), the zones it never lies in (`never_in`: `wsg`), its looks per faction, the ally's size (`ally_scale`, 2), how long an ally stays (`assist_seconds`, 15) and what every assist deals a second (`assist_damage_per_second`: `normal` 0.05, `elite_rare_boss` 0.01 of the struck enemy's max health). |
| `experience` | What kills drop (normal, elite, boss), what each level costs (`level_cost`), and the orb sizes. |
| `spell_tags` | One multiplier per spell tag (`level_1`, `nuke`, `cleave`, `dot`, `nova`, `area`, `pet`, `sustain`, `buffs`, `defensive`), all 1.0 today. A damage tag scales damage (and a hit's burn, and an area's impact), `pet` also every talent pet; `sustain` scales healing, `buffs` buff amounts, `defensive` shields, weakening and reflected damage. Never cooldowns or durations. |
| `challenge_factor` | Challenge Mode (zones.md, "Challenge Mode"): every enemy of a challenge run takes Ahn'Qiraj's D, then its final health and damage are × this (1.1) × the challenge slider (1x-5x), one flat factor outside the kind and zone powers; a challenge run's talent experience is Ahn'Qiraj's × this (never the slider). |

The whole formula is written out at the top of the file and in zones.md, section 4:

    health = 240 × kind health × growth^(D − 1) × health clock(minute) × toughness × health option × 1.22^endless minutes
    damage =   8 × kind damage × growth^(0.6 × (D − 1)) × damage clock(minute) × damage option × 1.12^endless minutes

In Challenge Mode D is Ahn'Qiraj's in every zone, and both lines end × `challenge_factor` × the challenge slider.

### `enemy_kinds.ron`

* `enemies`: the roster kinds by name (`Fodder`, `FastFodder`, `Mid`, `ToughMid`, `FastMid`,
  `Ranged`, `Elite`), each with its speed (`move_speed`, yards a second), seconds between attacks
  (`attack_interval`) and cost against the spawn budget (`spawn_cost`). `elite: true` makes a kind
  elite; `ranged: Some((range, cast_time))` makes it a caster. A zone file's enemy names its
  kind (`kind: Fodder`). You may add kinds (example in section 5).
* `boss_move_speed`: how fast every boss walks (6 yards a second).
* `boss_abilities`: each boss ability kind's standard numbers (cooldown, radius, warning delay,
  damage as a multiple of the boss's attack, missiles, and the adds `Summon` calls for a zone's
  first, second ... fifth boss).

### `spell_categories.ron`

One line per spell category (`DirectDamage`, `DamageOverTime`, `AreaOfEffect`, `Melee`,
`Ranged`, `CrowdControl`, `Sustain`, `Shield`, `DamageReduction`, `Reflect`, `Buff`, `Summon`).
Each line multiplies seven numbers of every spell in that category: `damage`, `cooldown`,
`range`, `radius`, `duration`, `effect` (the strength of its on-hit effect) and `effect_seconds`
(how long that effect holds an enemy). All 1.0 today. A spell in several categories gets every
matching line, multiplied together; a number left out counts as 1.0. Example, every effect over
time 10 % stronger: `(category: DamageOverTime, damage: 1.1),`. Details: spells.md, section 8.

### `run_options.ron`

The four difficulty sliders before a run: enemy density, health, damage and speed. Each has its
lowest and highest setting and its step in percent (`min_pct`, `max_pct`, `step_pct`), its
`name` and `tooltip`, and `reward_weight`: how much each 100 % above 100 % adds to the run's
reward bonus (more drops and talent experience; the bonus stops at ×2). You can change their
numbers and words; a fifth slider needs code (section 15).

### `words.ron`

The words cards put around their numbers, so every card reads alike: after an upgrade card's
number (`upgrades`: "+34% damage", with a singular where it differs, "+1 target"), what Power
raises on a heal or a shield, "for every spell", one line per hero stat (`stats`, used by
amulet cards), and the tooltips of the buffs on the hero (`buffs`, one per buff kind:
`kind: Speed`, `Berserk`, `Damage`, `Haste`, `Shield`, `Regen`, `Immunity`, `Aura`,
`HealingCharges`). The loader refuses a missing upgrade, a buff kind with no words, an unknown
one, and a stat an amulet gives with no line here. A spell's own text lives with the
spell (`description`), a ring's or trinket's with the piece (`text`).

### `prestige.ron`

What prestige points buy once a class has all 51 talent points
(classes.md, "Prestige points"), in the order the hero sheet shows them. One line per stat:

```ron
(id: "damage", name: "Damage", icon: "Spell_Nature_AbolishMagic", effect: Stat(stat: Damage, per_rank: 0.01)),
(id: "block_chance", name: "Block chance", icon: "INV_Shield_06", effect: Stat(stat: BlockChance, per_rank: 0.01), classes: ["warrior", "paladin"]),
(id: "extra_lives", name: "Extra life", icon: "INV_Misc_Orb_04", effect: Stat(stat: ExtraLives, per_rank: 1.0), max_ranks: Some(1)),
(id: "spell_duration", name: "Spell duration", icon: "Spell_Arcane_ArcaneResilience", effect: AbilityMod(filter: Any, kind: Duration, per_rank: 0.01)),
```

`effect` is a talent effect (section 9): `Stat` for a hero stat, `AbilityMod` for a number of
spells; one point buys one rank, worth `per_rank` (a share for percentage stats: 0.01 is 1 %).
`classes` limits a stat to some classes (every class when left out), `max_ranks` caps it (no cap
when left out). Change a step or a cap and the next run follows; a stat removed is simply no
longer given (the ranks the save holds for it keep their points until a talent reset gives them
back). The loader refuses an id listed twice, an unknown class, a milestone effect, a step of 0 or less, a cap of 0, and
anything that adds targets (`ExtraTargets`, or a `Targets`, `Stacks`, `Strikes`, `Imps`, `Pets` or
`Critters` card kind). How dear each point is lives in code (`game::rewards`,
`PRESTIGE_POINT_COST_GROWTH`).

### `balance.ron`'s `zone_scaling`

Everything that follows a zone's difficulty D, in one block. It is set
from the hero we expect a player to bring to each zone (section 16, "Power budgets"), never from a
hero's actual power, and the code reads it from there:

| Number | What it does | Today |
|---|---|---|
| `normal_health` | normal enemy health by D: `(D, ×)` points joined by straight lines, past the last at the last step's pace; bigger steps where the expected hero gains a milestone row (D 3, 5, 7) | 1, 1.4, 2.25, 4, 5.75, 6.75, 7.25, 8.75, 9, 11, 12, 17.25 (D 1–12; fitted 2026-10-25, refitted with the final talents to the original game's kill seconds with the baseline hero: rare and epic amulets, bought caches, kiting) |
| `elite_health_power`, `boss_health_power` | elites and rares take that multiplier to the first power, bosses to the second: gentler, so their fights keep their length | 0.87, 0.7 |
| `boss_clock_power` (`balance.ron`, beside the clocks) | bosses and final bosses read the health clock to this power, so a zone's bosses last about as long as each other (fitted to the boss-time goals) | 0.3 |
| `damage_share`, `elite_boss_damage_share` | damage × the enemy's own health multiplier to this power: normal enemies, then elites, rares and bosses (not read while `normal_damage` is set) | 0.6, 0.45 |
| `normal_damage`, `elite_damage_power`, `boss_damage_power` | normal enemy damage by D, its own table (the blows the health table gave before its 2026-10-25 refit, kept); elites and rares take it to the first power, bosses to the second | 1, 1.15, 1.4, 2.05, 2.45, 2.7, 2.7, 3.25, 3.25, 3.75, 4.25, 5; 0.65, 0.5 |
| `gear_exponent`, `gear_steeper_from`, `gear_growth_after` | gear stats × D^exponent up to `gear_steeper_from`, then × `gear_growth_after` a step of D | 0.55, 9, 1.3 |
| `gear_drop_chance` | each boss gear piece's drop chance, in `(D, chance)` steps: each holds from its D until the next, no blending | 100 %, 80 % from 2, 70 % from 4, 60 % from 6, 40 % from 8 |
| `talent_exponent` | a run's talent experience × D^this (with the point cost it meets the talent schedule) | 0.7 |
| `gear_tier_starts` | the D at which gear tiers 1 to 4 begin (the pieces' quality colour) | 2, 4, 7, 9 |

Change one, then run `balance_check` (section 16) to see what it does to every zone.

## 4. A zone, with its roster and bosses

**File:** a new file, `content\zones\<id>.ron`. Start from
[templates/zone.ron](templates/zone.ron) or copy a shipped zone. Reference: zones.md, sections 1
to 5.

**Fields that matter**

* `id`: the file's name, lowercase. Saves and unlocks keep it, so do not rename a zone later.
* `name`, `subtitle`, `description`: what the zone selector shows.
* The place: `map_id`, `area_id`, `spawn` (x, y, z, facing), `arena_radius`, `hour`, `weather`;
  optional `enemy_spawn_ring` / `boss_spawn_ring`, how far from the hero enemies appear (default 22-34 / 24-28 yd).
  Next to a wall or map edge a spot steps in toward the hero (2.5 yd at a time, never nearer
  than half the inner edge), so spawns still come from that side, closer (zones.md).
  The spot must be real ground in the 1.12.1 world with room for the arena
  (zones.md, "Picking a place in the world"). A dungeon map with no terrain (Dire Maul) works:
  its building is the ground. Its rooms are small, so measure the room with the `arena_ground`
  example before picking `arena_radius`. In a dungeon, enemies appear only where they can walk
  to the hero, and they walk round walls (zones.md, "The arena").
* `difficulty`: D. It sets the enemies' strength, the gear, the drop chance, the talent
  experience bonus and the zone's place in the list. Pick it between the zones it should sit
  between (zones.md, section 4, has the shipped ones: Elwynn 1, Duskwood 2, Warsong Gulch 3 ...
  Gates of Ahn'Qiraj 10; 6 is kept free between Dire Maul and the Arena).
* `enemies`: the roster. Each has a `name`, a model (`display_id`), `scale`,
  `creature_type`, its `kind`, `toughness` (normal enemies) and `appears_at_minute`; a caster
  (kind `Ranged`) also names the look of its spell (`look_spell`). An elite's `scale` is
  min(2.0, 6.0 / its model's size), to 2 decimals: 1.3× to 2× its model, never drawn above 6; a
  boss is drawn at 9, a final boss at 12 (zones.md, "Sizes").
* `bosses`: in arrival order by `spawn_minute`; the last one has `is_final: true` (section 6).

**Example** (`content\zones\my_zone.ron`; at D 1.6 it comes between Duskwood and Warsong Gulch):

```ron
(
    id: "my_zone",
    name: "My Zone",
    subtitle: "A Test of Data",
    description: "A zone written as data alone.",
    map_id: 0,
    area_id: 12,
    spawn: (x: -9396.0, y: -633.0, z: 68.0, facing: 1.6),
    arena_radius: 80.0,
    hour: 12,
    weather: (kind: None, intensity: 0.0),
    difficulty: 1.6,
    enemies: [
        (name: "Test Wolf", display_id: 165, scale: 0.7, creature_type: Beast,
         kind: FastFodder, toughness: 0.25, appears_at_minute: 0.0),
        (name: "Test Kobold", display_id: 26, scale: 0.9, creature_type: Humanoid,
         kind: Fodder, toughness: 0.5, appears_at_minute: 0.0),
        (name: "Test Caster", display_id: 163, scale: 1.0, creature_type: Humanoid,
         kind: Ranged, toughness: 0.5, appears_at_minute: 0.5, look_spell: Some(133)),
        (name: "Test Brute", display_id: 377, scale: 1.62, creature_type: Humanoid,
         kind: Elite, appears_at_minute: 4.0),
    ],
    bosses: [
        (name: "Test Chief", display_id: 384, scale: 3.0, creature_type: Humanoid,
         spawn_minute: 3.0, yell: "Test!", abilities: [Charge(look_spell: 100), Summon()],
         is_final: false),
        (name: "Test Lord", display_id: 2029, scale: 6.0, creature_type: Humanoid,
         spawn_minute: 6.0, yell: "The test ends here!",
         abilities: [Stomp(look_spell: 339), Volley(look_spell: 686),
                     GroundBurst(telegraph_spell: 5740, impact_spell: 13812)],
         is_final: true),
    ],
)
```

The shipped zones have 12 roster enemies (9 normal, 3 elite) and 5 bosses; nothing requires
those numbers (a zone has room for 90 enemies and 10 bosses). The game numbers every creature
entry the client is told about as the content loads (`esurv_content::entries`), so a zone
file names none.

**How to check it**

* `cargo test -p esurv-content`. The loader names the file and the problem for a file whose
  `id` is not its name, an id used twice, two zones of the same difficulty, a difficulty of 0 or
  less, an unknown enemy kind, a caster without `look_spell` or a non-caster with one, a boss
  past the end of the standard `Summon` list without its own count, or two final bosses; a test
  checks that the final boss comes last. The tests that list the shipped zones (their count,
  their difficulties, their first enemies, the order a test zone lands in) then fail on purpose
  (section 1): update them.
* `cargo run -p esurv-sim --example measure_runs -- my_zone`: every class plays it; check
  the boss times and the damage taken against the zones around it.
* In the game: the zone selector lists it in its place. To play it before a class has opened it,
  give it for a while the lowest difficulty of all (for example 0.9), so it comes first and is open
  to every class, and start the game with `ESURV_ZONE=my_zone`; then put its real
  difficulty back.

**Still code:** a new kind of weather, the arena's shape and the rares (5 or 6 a run,
`esurv-sim` `game::rares`), the talent experience numbers per minute,
kill, boss and win and the talent point costs (`esurv-sim` `game::rewards`: the `TALENT_XP_`
numbers, `FIRST_TALENT_POINT_COST`), and the most zones a gear item number can
hold (100, `game::gear::MAX_GEAR_ZONES`). The release installer's list of world files to copy is
not built yet (design step 9).

## 5. An enemy kind

**File:** `content\enemy_kinds.ron`, the `enemies` table. Reference: zones.md, section 5.

**Fields that matter:** `move_speed` (yards a second), `attack_interval` (seconds between
attacks), `spawn_cost` (what one costs against the spawn budget, and what a kill adds to the
spawner's kill pressure); optional `elite: true`; optional `ranged: Some((range: ..., cast_time:
...))` for casters. A kind has no health or damage: those come from the enemy formula.

**Example:** a fast swarming kind and a long-range caster. The numbers here only show the
shape.

```ron
    enemies: {
        // ... the shipped kinds stay as they are ...
        Swarmer: (move_speed: 9.0, attack_interval: 1.0, spawn_cost: 0.5),
        Sniper: (
            move_speed: 4.0,
            attack_interval: 4.0,
            spawn_cost: 3.0,
            ranged: Some((range: 30.0, cast_time: 2.0)),
        ),
    },
```

Then a zone's enemy names it: `kind: Swarmer`, or `kind: Sniper, look_spell: Some(133)` (a
caster always names the look of what it casts). One enemy may also keep its kind and override a
single number: `kind: Fodder, move_speed: Some(7.0)`; casters can override `range` and
`cast_time` the same way.

**How to check it:** `cargo test -p esurv-content` (an unknown kind fails with the list of
known ones), then `measure_runs` on a zone that uses it.

**Still code:** how enemies move, swing and cast (`esurv-sim` `game::enemy_ai`,
`game::enemy_attacks`), how elites are timed and how the spawner picks (`game::director`). A
new kind of enemy behaviour (one that flees, explodes, heals others ...) is new code there, then a
new field in `EnemyKind` (`crates\esurv-content\src\kinds.rs`).

## 6. A boss and its abilities

**File:** the zone's file, its `bosses` list. Start from [templates/boss.ron](templates/boss.ron).
Reference: [bosses.md](bosses.md).

**Fields that matter:** `name`, `yell` (shouted on arrival), `display_id`,
`scale` (also its reach), `creature_type`, `spawn_minute`, `abilities`, and `is_final`. Health
and damage come from the formula's `boss` kind (`final_boss` for the final one) at its arrival
minute. `move_speed: Some(..)` is optional.

**Abilities:** any mix of the five kinds, each naming its looks: `Charge(look_spell: 100)`,
`Stomp(look_spell: ...)`, `GroundBurst(telegraph_spell: ..., impact_spell: ...)`,
`Volley(look_spell: ...)`, `Summon()`. Their numbers are the kind's in `enemy_kinds.ron`. To
give one boss its own number, add an override to its line.

**The building blocks ([bosses.md](bosses.md) section 6):** marked
attacks `Marked(shape, at, impact_spell, ...)` and `Rush(look_spell, ...)` (every area attack
marked on the ground before it lands, hitting the hero only), objects the hero walks over
`Lay(object, near, ...)` (broken or set off by half a second's contact, never by damage; shown
as a game object, a creature model or a weapon planted in the ground, `planted_item`, which may
wear an `aura_look`), lasting
zones, stacks (`on_hit`), a `script` (lines `on: Arrival | Health(%) | Every(s) | After(s) |
Death | AddDied | HeroFell` with actions: yells with the original voice, waves, spawn rules,
Raise Dead, shields, objects, phase kits, weapons, kneel, enrage, the scripted pause, actions
later, acting as another boss, calling and reviving bosses, heals with beams, casting a block at
once, bolstering adds, Blood Siphon, runs of waves the boss waits behind with their counter, the
horde pulled into a clump, spell immunity, rage, healing adds, class calls, the fallen rising, a
figure rising for a moment), and `warn` for a raid warning and a timer bar. A reworked boss keeps
the damage a hero cannot avoid (`swing_mult`). Roster enemies only a boss brings carry
`only_summoned: true`; a boss only a script brings, `called: true`. A roster enemy the hero must
focus (adds that heal or guard a boss) carries `focus_as_elite: true`: heroes and assists pick it
as an elite and it glows as one, its health, damage and rewards still its own kind's.
Give it the elite `scale` too, min(2.0, 6.0 / its model's size) (zones.md, "Sizes"): the
Nightmare Suppressors 1.13, the Black Guard Sentries 1.86.

**Example:** a boss with a wider, slower stomp than the standard one, a volley, and its adds:

```ron
        (
            name: "Test Warden",
            display_id: 384,
            scale: 3.5,
            creature_type: Humanoid,
            spawn_minute: 4.5,
            yell: "You will not pass!",
            abilities: [
                Stomp(look_spell: 6343, radius: Some(12.0), cooldown: Some(10.0)),
                Volley(look_spell: 133),
                Summon(),
            ],
            is_final: false,
        ),
```

Put it in the list in arrival order. `Summon()` takes its adds from its place in the list (6, 7, 8,
10, 12 for the first to fifth boss); a sixth boss or later needs its own count,
`Summon(count: Some(14))`.

**How to check it:** `cargo test -p esurv-content -p esurv-sim`, then `measure_runs`
on its zone: the boss table shows how long it took each class against what the hero's spells
say they deal.

**Still code:** a new kind of boss ability (a pull, a shield, a beam ...). bosses.md, section 4,
lists the four places: `BossAbilityKind` (`crates\esurv-content\src\schema.rs`),
`BossAbilityEntry`, its standard numbers and their resolving (`kinds.rs` and
`content\enemy_kinds.ron`), and what it does (`esurv-sim` `game::boss_abilities`, `cast`).
Also code: enrage at 30 % health (`game::enemy_ai`, the `ENRAGE_` numbers), the gap between two
abilities (`game::enemy_attacks`, `ABILITY_GAP`), and the rares' fixed set of abilities
(`game::boss_abilities`, `rare_abilities`).

## 7. A spell

**File:** `content\abilities.ron`. Start from [templates/spell.ron](templates/spell.ron).
Reference: [spells.md](spells.md), every section.

**Fields that matter:** `id`, `name`, `description` (the card text, a template filled with the
spell's numbers: spells.md 2a), `icon`, `look_spell` (its look, and its missile's speed),
`school`, `delivery` (how it finds enemies: `Projectile`, `Nova`, `Melee` ...), `categories`,
`tags`, `upgrades`, then the numbers its delivery uses (spells.md, section 5) and `effect`.

**Example:** a single-target frost missile. As a nuke on a 3 s cooldown its role asks for 60
damage a second at level 1, so `damage` is 180.

```ron
    (
        id: "frost_lance",
        name: "Frost Lance",
        description: "Hurls a lance of ice at {targets:the nearest enemy|the # nearest enemies} within {range} yd for {damage} {school} damage. Bosses and elites are hit first.",
        icon: "Spell_Frost_FrostBolt02",
        look_spell: 116,
        school: Frost,
        delivery: Projectile,
        categories: [DirectDamage, Ranged],
        tags: [Nuke],
        upgrades: [Power, Cooldown, Targets],
        damage: 180.0,
        cooldown: 3.0,
        range: 30.0,
        radius: 0.0,
        targets: 1,
        duration: 0.0,
        ticks: 0,
        effect: None,
    ),
```

Then let a class learn it: add `"frost_lance"` to a class's `abilities` in `classes.ron`
(section 8). Talents, amulets, the category lines, the tag coefficients and the legendary
amulet of its delivery kind reach it by themselves (spells.md, section 9).

**How to check it**

* `cargo test -p esurv-content`: the description must name only numbers that exist, every
  id must resolve, and the tags must follow the rules (one damage tag, `Level1` only on starting
  spells, `Sustain`, `Buffs`, `Defensive` by its on-hit effect).
* **A class spell needs a role:** add a line for it to `ROLES` in
  `crates\esurv-sim\tests\spell_roles.rs`, here `("frost_lance", Nuke),`, then
  `cargo test -p esurv-sim`. The test checks its damage against its role within 3 %.
* `cargo test -p esurv-app`: its card text fills in at every rank.
* In the game: it shows on level-up cards for the class whose pool lists it.

**Still code:** a new delivery kind (`Delivery` in `crates\esurv-content\src\schema.rs`, with
`Delivery::ALL`, its casting in `esurv-sim` `game::casting`, a legendary amulet recipe for
it, since the loader wants one per delivery kind, and that check in `Content::validate`); a new
on-hit effect (`SpellEffect` in `crates\esurv-content\src\schema.rs`, how it scales in
`SpellEffect::scaled`, its tag in
`tags.rs`, its effect in `game::casting`, and its numbers for card texts in `game::spell_numbers`
and `crates\esurv-content\src\text.rs`); a new card text number; a new category or tag
(`SpellCategory`, `SpellTag`).

## 8. A class

**Files:** `content\classes.ron`, plus three trees in `content\talents.ron` and one set in
`content\gear.ron`, each naming the class's `id`. Start from
[templates/class.ron](templates/class.ron) and [templates/gear_set.ron](templates/gear_set.ron).
Reference: classes.md, sections 1 and 5.

**What "a class" means today.** The game already uses all nine classes the 1.12 client knows,
and it tells classes apart by `wow_class_id`: the create screen, the class icons, the gear item
numbers and the save all use it. So with data alone you **reshape** one of the nine: its name,
health, armor, lifesteal, swing, spell pool, starting spell, talents and gear. Keep its `id` and
`wow_class_id` (the save keeps each class's talents, gear and wins by its `id`; a new `id` starts
from nothing, and every `class:` in `talents.ron`, `gear.ron` and every race's `added_classes`
must follow it).

**Fields that matter:** `name`, `description`, `power`, `base_health`, `base_damage_reduction`,
`base_lifesteal`, `auto_attack`, `abilities` (its spell pool), `starting_options` (section 8a),
`spell_damage_scale`. `color` and `icon` are kept in the file but not read yet: the menus use the
stock colour and icon of the class's `wow_class_id`.

**Example:** the mage with a sturdier body, a melee staff swing and the Frost Lance from
section 7 in its pool:

```ron
    (
        id: "mage",
        name: Class(8),                // the client's name for class 8
        wow_class_id: 8,
        power: Mana,
        color: "#69CCF0",
        icon: "INV_Staff_13",
        description: "Huge area damage, a cold lance, and a staff for close calls.",
        base_health: 200.0,
        base_damage_reduction: 0.1,
        base_lifesteal: 0.0,
        auto_attack: Some((damage: 14.0, swing: 1.8, range: 6.0)),
        abilities: [
            "fireball", "frostbolt", "arcane_missiles", "frost_nova", "arcane_explosion",
            "blizzard", "flamestrike", "fire_blast", "cone_of_cold", "blast_wave", "pyroblast",
            "ice_barrier", "frost_lance",
        ],
        starting_options: [ /* section 8a */ ],
        spell_damage_scale: 1.0,
    ),
```

A starting option's spells are tagged `Level1` and stay out of the card pools; to make a card
spell a starting spell, move it out of `abilities` and give it the `Level1` tag (section 8a).

**How to check it:** `cargo test -p esurv-content -p esurv-sim` (every id the class
names exists, it has three talent trees and one gear set, every pool spell has a role), then
`measure_runs` for a zone or two, and the class picker in the game.

**Still code:** a tenth class (a new `wow_class_id` needs a class the client knows: its create
screen entry, icon and colour in `esurv-app` `menus::art::class_cell` and
`ui::CLASS_COLOURS`, how the server finds a class by its id in `esurv-server` `world`, and
the gear item numbers in `esurv-sim` `game::rewards`); a new kind of power bar or a use for
`power` in play (the roadmap's rage and procs); the hero's stats per level (`game::stats`).

### 8a. Starting options

Before a run the player picks one of the class's **starting options**
(three; the Druid four) on the warband screen. Each option is a level-1 spell (or a strike and its
finisher) with a **passive**, and may bring its own card pool, a form, a resource and a weapon
swing. `HeroSetup::start` names the option picked (its `id`); none, or one the class lacks, means
the first. The level-1 spells and what the passives cast are the `Level1` spells, and **a starting
option's spell is never a card** (the checks refuse a level-1 spell that is in a card pool).

```ron
        starting_options: [
            (
                id: "denounce",                  // unique among every class's options
                name: "Denounce",                // our text, or a pointer: Spell(1464)
                icon: "Spell_Holy_LesserHeal",
                description: "...",              // our words; {Spell(id)} pointers allowed
                spells: ["denounce"],            // level-1 spells, the main one first
                passive: (
                    id: "holy_power",            // what PassiveTriggered events carry
                    name: "Holy Power",
                    icon: "Spell_Holy_Power",
                    description: "...",
                    kind: Counter((
                        meter: HolyPower,        // what the mana bar shows
                        fill: [Cast(1.0)],       // +1 per cast of the main spell
                        max: 5.0,
                        release: Spell("radiance"),
                    )),
                ),
                // optional:
                card_pool: Some([...]),          // in place of the class's `abilities`
                common_cards: ["swipe"],         // pool cards offered twice as often (COMMON_CARD_WEIGHT_FACTOR); two
                                                 // strikes that give combo points take turns (two known)
                form: Some((look_spell: 5487, bonus_health: 40.0, bonus_damage_reduction: 0.05)),
                resource: Some(Rage(per_swing: 20.0, per_health_lost: 50.0, cap: 100.0)),
                swing: Some((damage: 27.0, swing: 1.8, range: 6.0)),
                pet: Some((kind: HunterPet, pets: ["Cat", "Wolf", "Bear"], count: 1, seconds: 0.0, spells: ["hunter_pet_attack"])),
            ),
        ],
```

* **`spells`:** the main spell first: its upgrade cards are the option's cards, and its casts,
  hits and crits feed the passive. A second spell is the strike's finisher (Envenom, Rip).
* **`passive.kind`** (`PassiveKind`): `None` (the resource, the spell itself or the form is the
  passive); `Counter` (a mana-bar counter, below); `SwingSpeedStacks` (Zeal); `SwingSpeedOnHit`
  (Quick Shots); `PetFrenzy`; `FreeCasts` (Lock and Load); `Orbs` (Holy Orbs); `Marks` (Lightning
  Rod); `Empower` (Arcane Knowledge).
* **A counter** fills by every `fill` source: `Cast(n)` per main-spell cast, `Crit(n)` per crit,
  `Damage(per, heal_factor)` one point per `per` damage dealt (healing counts `heal_factor` times,
  overhealing not), `Time(n)` per second, `FrozenHit(n)` per main-spell hit on a frozen enemy,
  `KillUnderSpell(n)` per enemy dying under the main spell (once per cast), `SummonExpired(n)` per
  summon running out, `MeleeHit(fast, slow)` a chance of +1 per melee hit. At `max` it releases:
  `Spell(id)` casts an ability no card offers (it grows with the main spell's Power cards), or
  `Summon([...])` brings summons; then it starts over. `shatter` (optional) multiplies the first
  frost hit on each enemy the release froze.
* **Summons** (`SummonSpec`): `kind` (Clone, EyeTentacle, ClawTentacle, Infernal, Imp, Critter,
  Plant, WaterElemental, HunterPet), the `pets` whose models it shows (`pets.ron`, in turn; none for the
  hero's clone), `count`, `seconds` (0: the whole run) and the `spells` it hits with (the damage
  meter lists its damage under that spell's name; several: one at random each time).
  Plants sprout evenly spaced in a ring 3 yards round the hero, each
  walks at its own enemy (7 yards a second, stopping 2 short) and bursts where it stands 2 s after
  it sprouted, its spell striking every enemy in its radius; tentacles rise anywhere 4 to 8 yards
  round the hero; the hero's clone is the one seen casting its spells.
* **`resource`:** `Rage(per_swing, per_health_lost, cap)` (+`per_swing` per weapon swing that hits,
  +`per_health_lost` per whole max health lost, so 25 is 1 per 4 %; the Warrior's options give 10 a
  swing and 25, half the first starts' 20 and 50; Bear Form keeps 20 and
  50), or `Energy(per_tick, tick_every, cap, combo_points)` (+`per_tick` every `tick_every` sec,
  ticks of 20 every sec: 1.12's ticks at twice their pace, so the
  average stays the run's 20 a second; Energy % cards make each tick bigger,
  faster cooldowns bring the ticks sooner, so the bar fills in steps). A spell that costs it casts
  by itself once the hero has its cost.
* **`form`:** the shapeshift spell whose look the hero takes, and the form's flat health and
  damage reduction.
* **A spell's own rule** (`kit: (mechanic: Some(...))` on the ability, for the starting spells and
  the cards of their pools): among others `FreshTargetFirst` (Rip), `StacksUpTo(max)` (an effect
  over time that stacks up to `max` on each enemy, Lacerate) and `AttacksStrikeMore` (a buff whose
  attacks also strike `targets` more enemies, Berserk); `esurv-content` `starts::Mechanic`
  lists them all, and spells.md ("Effects over time") says how they play.
* **Card kinds** a starting spell may list in its `upgrades` in place of Cooldown or Targets:
  `RageGain`, `EnergyGain`, `Chance` (half a Cooldown card's strength, at most +200 %; Chance above
  100 % works as double cast), and `Stacks`, `Strikes`, `Imps`, `Pets` (up to 3 pets), `Critters`
  (+1 each, as Targets).

**What the interface is told** (events of `esurv-sim` `game::run::Event`):
`ResourceChanged { kind, value, max }` for rage, energy, combo points and every counter
(`ResourceKind`); `PassiveTriggered { passive }` (the passive's `id`); `SummonAppeared { id, kind,
at, until }` and `SummonGone { id }` (a summon that shows a pet's model is also a pet:
`PetSummoned` and `PetGone` with the same id come just before); `StacksChanged { kind, stacks,
max, until }` for the buff bar (`StackKind`).

**How to check them:** `cargo test -p esurv-sim kit` plays every option's rules, and
`cargo run --release -p esurv-sim --example balance_check -- --starts --kite` measures every
option in Elwynn, Feralas and Ahn'Qiraj with each zone's arrival hero, the hero kiting the swarm as
a player would: its damage a second (its spells alone, with its passive, and on one enemy) against
the level-1 budget of about 40 a second, the enemies its spell strikes per cast, its rage or energy
income, how often its passive fires, how often it would have died, and whole runs' kill seconds
against the targets (`--targets`). Without `--kite` the hero stands still in the pack (it takes
far more damage, so rage runs high). `--own-tree` spends the arrival talents in the option's own
tree first.

### 8b. Weapon loadouts

**File:** `content\loadouts.ron` (roadmap R3, classes.md "Weapon
loadouts"). Each class has **three** loadouts, the default first; the player picks one per class
on the warband screen, under the starting spell, and the save keeps it (a loadout id the class no
longer has falls back to its first). `HeroSetup::loadout` names it in the sim.

```ron
    (
        class: "warrior",
        loadouts: [
            (
                id: "shield",                       // unique within the class; the save keeps it
                name: "One-hander and shield",      // the warband screen's row
                weapons: [
                    // hand: MainHand, OffHand or Ranged; kind sets the grip and where it rests;
                    // swing: seconds between swings and share of the class's auto_attack damage
                    // (only for a class that swings; none for a shield or a caster's weapon);
                    // looks: its look in each zone's armaments (a zone's rows as gear.ron's),
                    // the item's name, ItemDisplayInfo.dbc display and, where the item is
                    // another kind than this weapon's, its own kind
                    (hand: MainHand, kind: OneHandedSword, name: "One-handed sword", swing: Some((seconds: 1.8, share: 1.0)), looks: [
                        (zone: "elwynn", name: "Heirloom Sword", display: 20161),
                        (zone: "scarlet", name: "Ironspine's Fist", display: 15726, kind: Some(OneHandedMace)),
                        // ... a line for every zone, and for Blackrock's second row (row: 1)
                    ]),
                    (hand: OffHand, kind: Shield, name: "Shield", looks: [ /* ... */ ]),
                ],
                strength: "30% block (blocked blows -30%), +10% max health",   // shown green
                drawback: "-10% damage",                                       // shown red
                suits: "Protection (Revenge)",      // a hint only
                effects: [                          // the talents' vocabulary, at rank 1
                    Stat(stat: BlockChance, per_rank: 0.3),
                    Stat(stat: MaxHealth, per_rank: 0.1),
                    Stat(stat: Damage, per_rank: -0.1),
                ],
            ),
            // ... two more
        ],
    ),
```

* **Swings:** a class with an `auto_attack` swings with its loadout's weapons (each loadout must
  have one that swings); `auto_attack.damage` is the one-hand swing's, so `share: 2.0` is a
  two-hander's double damage and `0.5` a dual-wield off hand. Swing times: two-handers 3.6 s,
  one-handers 1.8 s, dual wield a slow main and a fast off hand at 50 %. The Hunter's `Ranged`
  weapon sets Auto Shot; its `kind` (Bow, Crossbow, Gun) sets the shot's look.
* **Effects:** `Stat(...)` (`BlockChance`, `RagePerSwing`, `CritChance`, `MaxHealth`,
  `CooldownRecovery`, `ExtraTargets` ...), `AbilityMod(filter: Any, kind: Cooldown, per_rank: 0.1)`
  for "-10 % cooldowns" (every cooldown × 0.9), `Milestone(...)` for swing procs with
  `filter: WeaponSwing` (two maces' `Afflict` stun, two swords' `StrikeAgain`).
* **Caps:** drawbacks at most −10 %; Priest, Mage and Warlock at most −5 % max health;
  move speed between −2 % and +5 %.
* **Looks:** every weapon has a look in every zone's armaments (each row
  of the class set's `looks`); the hero shows the armaments of the class gear weapon it wears,
  and one weapon drop of a zone unlocks its looks for all three loadouts (gear.md, section 3).
* **Check:** `cargo test -p esurv-content -p esurv-sim` (three a class, ids once each,
  one weapon a hand, each with a look in every zone's armaments, swings only for a class that swings), then compare the
  class's three: `balance_check --loadouts` (section 16).

## 9. A talent

**File:** `content\talents.ron`, in one of a class's three trees (`class`, `name`,
`background`, `icon`, `talents`). Start from [templates/talent.ron](templates/talent.ron).
Reference: classes.md, section 4.

**Fields that matter:** a unique `id` (the save keeps ranks by it), `name`, `icon`, `text` (each
`{v}` shows the next effect's number at the current rank), `row` (0 to 6; a row opens once the
tree has 5 points for each row above it), `col` (the shipped trees use 0 to 2; one talent to a
spot), `max_rank`, `requires` (a talent of the same tree that must be at its last rank first)
and `effects`:

* `Stat(stat: ..., per_rank: ...)`: adds to a hero stat each rank.
* `AbilityMod(filter: ..., kind: ..., per_rank: ...)`: improves one number (`Power`, `Cooldown`,
  `Targets`, `Area`, `Duration`) of every spell the filter lets through (`Spell("id")`,
  `Spells([...])`, `School(Fire)`, `Delivery(DamageOverTime)`, `Category(DamageOverTime)` ...).
* `CooldownSeconds(filter: ..., per_rank: ...)`: seconds off the listed cooldown of every spell the
  filter lets through (write `cooldown -{v} sec.` in the talent's text).
* `Milestone(...)`: one of the rule-changing kinds in classes.md's table, with its trigger and
  filter.

**Example:** a talent for the mage's Fire tree, on its free spot (row 0, column 0). A group of
spells (a school) takes the standard 5 % power per rank:

```ron
            (
                id: "fire_focus",
                name: Spell(5834),     // the client holds this name: a pointer
                icon: "Spell_Fire_FlameBolt",
                text: "Your Fire spells deal {v}% more damage.",
                row: 0,
                col: 0,
                max_rank: 5,
                requires: None,
                effects: [
                    AbilityMod(filter: School(Fire), kind: Power, per_rank: 0.05),
                ],
            ),
```

**How to check it:** `cargo test -p esurv-content` (every spell and pet a talent names
exists; `requires` names a talent of the same tree), then the hero sheet's Talents tab in the
game: its spot, its text at each rank, and learning it.

**Still code:** a new milestone kind (`Milestone` in `crates\esurv-content\src\schema.rs`,
what it does in `esurv-sim` `game::milestones`, its number for the tooltip in
`esurv-app` `hero_sheet::talents`), a new trigger or filter (`Trigger`, `SpellFilter`), a
new hero stat (section 15), and the talent point curve and experience numbers
(`game::rewards`). A new talent pet is data: add it to `content\pets.ron` and name it in a
`Pet(pet: "...")` or `PetOnTrigger` milestone. A beast may name its `family` (`Wolf`, `Cat`,
`Bear`, `Raptor`): it then grows with its hero's level, its size the family's
(`PetFamily::scale_range`, from the player's `CreatureFamily.dbc`) in place of `scale`. A pet
that flies names its `hover`, yards above the ground (the assist's Gargoyle: 2): it hovers that
high, bobbing gently.

## 10. A race passive

**File:** `content\races.ron`, the race's `passives` list. Reference: classes.md, section 1a.

**Fields that matter:** `name`, `stat` (any hero stat), `value` (0.05 = 5 %), `text` (its
tooltip line) and, where the stock racial spell that shows it in the spellbook has another name
than `name`, `stock_spell: Some(Spell(...))`. A passive whose name matches no stock racial spell
still works; it just has no spellbook entry.

**Example:** the orc's pets hit harder, shown as the orc's stock pet racial (names the client
holds are pointers; `text` leaves the name out, as the tooltip shows it above):

```ron
        passives: [
            (
                name: Spell(20573),
                stat: MaxHealth,
                value: 0.1,
                text: "Maximum health +10%.",
            ),
            (
                name: Spell(20575),
                stat: PetDamage,
                value: 0.05,
                text: "Pet damage +5%.",
            ),
        ],
```

The race spell is the race's `spell` (an id in `abilities.ron`); its card says that spell's
`description`. A race spell is an ordinary spell record with **no tags** (section 2); change its
numbers in `abilities.ron`.

**How to check it:** `cargo test -p esurv-content` (the race spell exists, no race is listed
twice, every added class exists), then a run's spellbook and character pane in the game.

**Still code:** a new race (`wow_race_id` must be one the 1.12 client knows, 1 to 8, and all
eight are in use), and a new hero stat (section 15).

## 11. A gear set or piece

**File:** `content\gear.ron`. Start a new set from [templates/gear_set.ron](templates/gear_set.ron).
Reference: gear.md, sections 3 to 5.

**What is in it:** `slots` (the eight armor slots, the same stats
for every class: these are the Elwynn values), `bonus_stats` (eight pairs a piece adds from the
second zone on), and `sets` (one per class: armor type, the bonus pair of each slot and weapon
(`bonus_pairs`), a look per zone (eight names and eight displays; Blackrock two rows), and its
weapon lines).

**Example 1, a piece's stats:** every chest, for every class, gets more health. The chest's
Elwynn stats become:

```ron
            stats: [
                (stat: FlatMaxHealth, value: 20.0),
                (stat: DamageReduction, value: 0.01),
            ],
```

A chest from Ahn'Qiraj (D 10, gear factor 4.35) then gives 20 × 4.35, rounded up, 87 health. Keep Elwynn values whole
(whole percents, whole points), so every zone's stats stay whole after rounding.

**Example 2, a set:** the template is a mage-like set renamed `my_class`, with a look for two zones
and a second row for one of them. A new set goes with a reshaped class (section 8); every
class needs exactly one.

**How to check it:** `cargo test -p esurv-content -p esurv-sim` (one set per class, 8
slots of 4 names, 8 bonus pairs, 8 names and 8 looks in every zone's look, rows from 0, a look
of every loadout weapon for each), then the hero sheet's Collection tab: each zone's row (its
rows) shows the pieces, what they add and which boss drops
them.

**Still code:** a ninth armor slot (`game::gear::ARMOR_SLOTS`, the paper doll); more than two
weapon lines in a set (a piece's item number holds one digit for the slot: 0 to 7 armor, 8 and
9 weapons; `game::rewards::gear_item_entry`); which boss drops which slot
(`game::rewards::boss_drop_slots`); how stats are rounded (`game::gear`).

## 12. A ring or trinket

**File:** `content\jewelry.ron`. Start from [templates/jewelry.ron](templates/jewelry.ron).
Reference: gear.md, section 1, "Caches and jewelry offers".

**Fields that matter:** a unique `id`, `name`, `icon`, `slot` (`Ring` or `Trinket`), `text`
(its line, with `{v}` for its value), `stat` and `value` (its base value). A new piece starts at
1.5 × the rarity's multiplier × `value`; each upgrade adds a little less than the last.

**Example:** a trinket for movement speed.

```ron
    (
        id: "swift_charm",
        name: "Charm of Swiftness",
        icon: "INV_Misc_Rune_01",
        slot: Trinket,
        text: "+{v}% movement speed",
        stat: MoveSpeed,
        value: 0.05,
    ),
```

**How to check it:** `cargo test -p esurv-content`, then in the game
`ESURV_GRANT=swift_charm` (or `swift_charm@4` for a legendary roll) gives it to every hero
from the first second of a run: look at its card text and the character pane.

**Still code:** where each stat's upgrades stop (`esurv-sim` `game::offers::jewelry_cap`:
no cap for damage, cooldowns, crit, crit damage and area; 3 × the base for health, damage
reduction, regen, luck, health per kill and lifesteal; 5 × for the rest), the special rules of
extra lives and effect-over-time stacks, which pieces are offered only when they help (buff power,
effect-over-time stacks), and the rarity odds and multipliers (`game::offers`, `game::choices`).

## 13. An amulet

**File:** `content\amulets.ron`. Start from [templates/amulet.ron](templates/amulet.ron).
Reference: gear.md, section 1, "Amulets".

**Fields that matter:** a unique `id`, `name`, `icon`, `text` (its tooltip), `effects` (the same
shapes as talents, at rank 1), `rarity` (`Common`, `Uncommon`, `Rare`, `Epic` or `Legendary`: an amulet
offer that rolls this rarity may show it) and `growth` (`Fastest`, `Fast`, `Medium`, `Slow` or
`Slowest`: how fast its numbers grow each time it is offered again).

**Example:** an uncommon amulet that makes every effect over time last longer and tick harder:

```ron
    (
        id: "plague_charm",
        name: "Plaguebearer's Charm",
        icon: "INV_Jewelry_Necklace_02",
        text: "Effects over time deal 10% more damage and last 10% longer.",
        effects: [
            AbilityMod(filter: Delivery(DamageOverTime), kind: Power, per_rank: 0.1),
            AbilityMod(filter: Delivery(DamageOverTime), kind: Duration, per_rank: 0.1),
        ],
        rarity: Uncommon,
        growth: Medium,
    ),
```

An amulet that gives a `Stat` needs that stat's line in `words.ron` (the loader says so if it is
missing).

**Legendary amulets** (one per spell): `content\legendary_amulets.ron`
holds one record per spell a hero can know (a level-1 strike and its finisher share one). A
record:

```ron
(
    spells: ["fireball"],          // the spells it remakes, the main one first (id legendary:fireball)
    name: "Phoenix",               // our name
    icon: "Spell_Fire_FlameBolt",  // the main spell's icon
    text: "Fireball deals 40% more. A crit calls a phoenix that fights for 8 sec, up to 2.",
    effects: [                     // the talent shapes, at rank 1
        AbilityMod(filter: Spell("fireball"), kind: Power, per_rank: 0.4),
        Milestone(PetOnTrigger(chance: 1.0, seconds: 8.0, max: 2, pet: "Phoenix", trigger: Crit, filter: Spell("fireball"))),
    ],
    rarity: Legendary,
    growth: Slowest,
),
```

The loader holds every class card, form card and level-1 spell to exactly one legendary. A new
spell needs its legendary too. Its `AbilityMod` may use every upgrade kind (`Stacks`, `Pets`,
`Imps` … add whole units, `RageGain` and `EnergyGain` raise what the spell gives).

**How to check it:** `cargo test -p esurv-content`, then `ESURV_GRANT=plague_charm` (or
`legendary:<spell id>` for a legendary one) and a run.

**Still code:** which milestone kinds grow when an amulet is empowered
(`Milestone::grows_when_empowered` in `schema.rs`), the empower curve (`esurv-sim`
`game::amulets`: `EMPOWER_RATE`), and the offer weights
(`game::choices`).

## 13a. A player character and its assist

**File:** `content\player_characters.ron`. Start from
[templates/player_character.ron](templates/player_character.ron). Reference:
[assists.md](assists.md); the Warsong bosses in zones.md, "Player-looking enemies".

`player_characters.ron` is the only list of hand-made characters. A character added there is at
once a Warsong Gulch boss for heroes of the other faction (its bosses are `played_by: Some(Any)`)
and an assist a battle standard summons for heroes of its own faction (its race's). No code
lists them and nothing else needs changing.

**Fields that matter:** a unique `id`, our own `name`, `race` and `class` (ids in `races.ron` and
`classes.ron`), `gender`, optional `form` (a druid in `Cat`, `Bear` or `Moonkin` form), `yell`
(as it arrives) and optional `death_yell` (as it dies or its assist ends), `assist` (what its
assist does, as a player reads it: write the script's numbers there) and `assist_script` (what it
does). Without an `assist_script` it takes its class's default from `content\class_assists.ron`
(its form's, for a druid in a form), and the game's log names it among the characters still on a
default. An ally stays 15 sec (`balance.ron` `battle_standard.assist_seconds`) unless
its script's `seconds` gives another time: its script, then its rotation (`behaviour`, below);
the loader refuses steps that take longer.

**The script:** `arrive`, then `steps` one after the other (a step starts once the one before is
done), then `leave`.

| Part | Choices |
|---|---|
| `arrive` | `how`: `Plain` (it is there), `Blink`, `Stealth` (fades in), `Portal` (steps out of a summoning portal); `beside: Some(<targets>)` to appear by an enemy instead of the hero; `look_spell` played as it appears (a spell's look brings its sound: Vanish's); `sound` (a `SoundEntries.dbc` id) |
| `leave` | `how`: `Fade` (the default), `Vanish`, `Hearth`, `RunAway`, `Dies`; `look_spell` in place of the way's own |

The blocks (`steps`):

| Block | What it does |
|---|---|
| `Move(to, how, look_spell)` | goes to the enemies `to` picks (beside one, the middle of several): `how` `Run`, `Charge` or `Blink` |
| `Cast(look_spell)` | a look on the ally only (Sprint, Shield Wall, a potion, a portal it opens) |
| `Hit(targets, damage, look_spell, melee, effects, wears, over_time)` | strikes the enemies `targets` picks: with `damage: true` the assist's damage to each, `effects` on each (`Stun`, `Root`, `Fear`, `Slow`, `Vulnerable`, `Weaken`: the hero's own rules, a boss held for a quarter, fear never on a boss), each wearing one of `wears` while they last (Blind's, Sap's, Polymorph's look), `over_time` damage `(every, seconds, look_spell)`; `melee` steps to the first one |
| `Area(at, radius, seconds, every, damage, effects, look_spell, most)` | a ground area at `Hero`, `Ally` or `BiggestPack`, striking every enemy inside every `every` sec (with `damage: true`; with `most`, only the nearest that many its middle) |
| `Buff(stats, seconds, heal, every, immune, look_spell, totem)` | on the hero for `seconds`: `stats` added (`[(Damage, 0.15)]`, positive only), `heal` of max health every `every` sec, `immune` no damage taken; with `totem: Some(<spell>)` it comes from a totem dropped beside the hero |
| `Heal(share, look_spell)` | heals the hero `share` of max health at once |
| `Taunt(targets, seconds, look_spell)` | the enemies picked attack the ally for `seconds` (a boss for a quarter); the ally stays while it holds |
| `Summon(pet, count, seconds)` | `count` of a `pets.ron` pet fight for the hero for `seconds`; their attacks are the assist's damage, each for the pet's attack interval |
| `Repeat(every, times, steps)` | its steps `times` times, one every `every` sec (takes `every × times`) |
| `Wait(seconds)` | waits |

**Targets** (always within the screen: the zone's enemy spawn ring's inner radius, 22 yd):
`Nearest(n)` (to the hero), `NearHero(radius, most)`, `NearAlly(radius, most)` (`most` left out:
all), `Strongest` (a boss, else a rare, an elite, the most max health), `StrongestNotBoss`,
`Healthiest` (most health left), `Farthest`, `OnScreen`, `Same` (the enemies the step before
picked). **Damage:** a step only says whether it deals damage (`damage: true`); the amount is
the same for every assist, `balance.ron` `battle_standard.assist_damage_per_second` (5 % of the
struck enemy's max health a second on a normal enemy, 1 % on an elite, a rare or a boss) × the
seconds the hit stands for: a hit of the script one global cooldown (1.5 sec), a hit of a repeat
the repeat's `every`, an area's pulse or an effect over time's tick its `every`
([assists.md](assists.md), "Rules").

**Example:** a rogue who strikes up to 5 enemies near the hero every second for 8 sec, then
vanishes:

```ron
    (
        id: "mir",
        name: "Mir",
        race: "orc",
        class: "rogue",
        gender: Male,
        yell: "...",
        assist: "Appears with Vanish's sound, uses {Spell(22482)} and strikes up to 5 enemies near the hero every sec for 8 sec, then vanishes.",
        assist_script: Some((
            arrive: (look_spell: Some(1856)),
            steps: [
                Cast(look_spell: 22482),
                Repeat(every: 1.0, times: 8, steps: [
                    Hit(targets: NearHero(radius: 10.0, most: 5), damage: true, melee: true),
                ]),
            ],
            leave: (how: Vanish),
        )),
        // then: Blade Flurry, Sinister Strike, Eviscerate, strafing and jumping in melee
        behaviour: Some((style: ErraticMelee, rotation: [13877, 11294, 31016])),
        gear: Random,
    ),
```

**Rules:** an assist never harms the hero (buffs are positive, the effects land on enemies only);
the ally cannot be hurt or targeted; `Same` needs a step before it that picked; a `Summon` names
a pet `pets.ron` has. "Near the hero" is 10 yd unless the spell it uses has its own radius
(Whirlwind 8, Arcane Explosion and Blizzard 9).

**Its rotation** (`behaviour`, [assists.md](assists.md), "Its rotation"): once its script is done
the ally loops `rotation`, 1.12 spells by `Spell.dbc` id, until its stay ends, moving `style`:
`Melee` (runs to the enemy nearest it), `ErraticMelee` (and in melee strafes left and right and
jumps, a rogue's A-D-A-D), `Caster` (beside the hero, following it), `NovaKiteCaster` (and jumps
away after an instant cast with an enemy close: Frost Nova, a jump, Blizzard), `Healer` (close
to the hero, its heals on the hero) or `LeavesAfterScript` (nothing: it leaves once its script is
done and no taunt of its holds). An empty rotation casts nothing: a `Melee` or `ErraticMelee` ally
then only auto attacks (a swing every 2 sec at the assist's rate). Each spell is a row in
`content\assist_spells.ron`: its cast time and cooldown as `Spell.dbc` holds them (a channel,
Blizzard, also its `channel`, `tick_every` and `radius`) and where it is aimed (`Enemy`, `Hero`,
`Caster`, `Ground`); `cargo run -p esurv-content --features client --example check_assist_spells -- <spell
id>...` prints the rows to write and checks the whole table against the client. A 1.5 sec global
cooldown runs between casts. Each cast strikes what the script's damaging steps strike, each enemy
once, at the assist's rate over its cycle (its cast time, at least the global cooldown); the
script's heals are spread over the script and the rotation's casts. Without a `behaviour` it
takes its class's default (the game's log names it).

**A class default:** `content\class_assists.ron`, one `(class, form, text, script, behaviour)` per
class (every class needs one without a form; a druid form may have its own).

**How to check it:** `cargo test -p esurv-content` (every character's assist, its own or its
class's, loads and validates).

**Still code:** a new block or target rule (section 15).

## 14. Changing a whole kind of thing at once

| To change | Edit |
|---|---|
| every enemy's health or damage, or the run clocks | `balance.ron` (section 3) |
| one roster kind everywhere, or one boss ability kind everywhere | `enemy_kinds.ron` |
| every spell of a category | `spell_categories.ron` |
| every spell of a tag (every nuke, every heal ...) | `balance.ron` `spell_tags` |
| one class's spell damage | its `spell_damage_scale` in `classes.ron` |
| every gear piece's growth by zone, drop chances, talent experience by zone | `balance.ron` `zone_scaling` |
| how much harder Challenge Mode is, and its talent experience | `balance.ron` `challenge_factor` |
| a card's words | `words.ron` |

## 15. Where the code lives

What data cannot do yet, and where it would go:

| New ... | Code |
|---|---|
| boss ability kind | bosses.md, section 4 (`schema.rs`, `kinds.rs`, `esurv-sim` `game::boss_abilities`) |
| enemy behaviour | `esurv-sim` `game::enemy_ai`, `game::enemy_attacks`, `game::director` |
| spell delivery kind | `Delivery` in `schema.rs`; `esurv-sim` `game::casting`; a legendary recipe |
| on-hit effect | `SpellEffect` in `crates\esurv-content\src\schema.rs`; `game::casting`; `game::spell_numbers` and `text.rs` for its card numbers |
| assist block or target rule | `AssistStep`, `Targets` in `crates\esurv-content\src\assists.rs` (and its checks there); the assists in `esurv-sim` |
| milestone, trigger or filter | `Milestone`, `Trigger`, `SpellFilter` in `schema.rs`; `esurv-sim` `game::milestones` |
| hero stat | `Stat` in `schema.rs`; `esurv-sim` `game::stats`; a line in `words.ron` |
| run option | `esurv-sim` `game::rewards` (`option_bonus`) and `game::enemy` (`RunOptions`), the menus |
| class or race | sections 8 and 10 |
| reward curves (talent experience per run and point costs; the D-driven parts are in `balance.ron` `zone_scaling`) | `esurv-sim` `game::rewards` |

The content types themselves (every field above) are in `crates\esurv-content\src\schema.rs`
(as played) and `kinds.rs` (zone files as written); the loader and its checks are in
`crates\esurv-content\src\lib.rs` (`Content::load`, `validate`) and `zone_files.rs`.

## 16. Power budgets

Every zone is built on the hero we expect a player to bring there:
one won run in each zone before it, in order. That hero's talent points and gear are written down,
each source of power has a budget, and enemies follow one smooth curve over D built from that
expected power (`balance.ron` `zone_scaling`, section 3). Nothing in the game ever reads a hero's
*actual* power: a player who is ahead stays ahead, one who is behind can come back with more wins
in an easier zone, and a zone added later at its D fits the curve without numbers of its own. The
budgets live here and in the check below; the game code never reads them.

**The expected hero.** Talent points (a player who wins each zone once, in order; the talent
schedule): the 11th point during Duskwood (D 2), the 31st during the Scarlet Monastery (D 4), 41
arriving in Feralas (D 6), the 51st during Stratholme (D 8), so all 51 first on arriving in
Blackrock (D 9) and Ahn'Qiraj and the zones after it add no talent power (measured arrivals with
the smooth zone bonus D^0.7: Duskwood 6, Warsong 15, Scarlet 24, Sunken Temple 33-34, Feralas
41-43, Arena 50-51, Stratholme 51: the 51st point one zone early). The talent experience numbers
(`rewards.rs`, classes.md section 4) are set to this schedule. Gear: the previous zone's whole class
set. Amulets: the rare and epic amulets a run offers, taken when offered (and empowered when offered
again), the bought caches opened as the gold allows; legendaries are a bonus on top, not part of
the expected hero.

| Source of power | Budget: the hero's DPS with it ÷ without it, everything else as expected |
|---|---|
| a regular talent point | about +1 % |
| the milestone at 11 points (row 2), first reached in Warsong Gulch (D 3) | **+5 %** |
| the milestone at 21 points (row 4), first reached in the Scarlet Monastery (D 4) | **+10 %** |
| the capstone at 31 points (row 6), first reached in the Sunken Temple (D 5) | **+25 %** |
| a zone's whole class set, over the previous zone's set, up to the set worn in Ahn'Qiraj | **+10 %** |
| each set after that, standing in for a zone of talents | about **+25 %** |

**The rule for talents:** a new or reworked damage milestone must measure within **±20 %** of its
row's budget (+4 to +6 % for row 2, +8 to +12 % for row 4, +20 to +30 % for row 6; measured as the whole talent, `--whole-talent`, kiting, amulets declined) in the zone
where its row is first reached. Tune only its main number (the bold field in classes.md, section
4: chance, share, amount ...), in clean steps (whole percents, 0.05 for shares). Defensive and
utility milestones (cheat death, heals, shields, slows ...) are listed but not held to a budget.
Check it with:

    cargo run --release -p esurv-sim --example balance_check -- --milestones --add-zone mid_zone=feralas:6

(`--talents my_talent` plays only that talent; the zone at D 6 is added in memory, from Dire Maul,
so the rows fall in the zones above.) One run differs from the next by far more than a milestone
adds, so each case plays 200 runs a side (`--check-seeds`) and prints its gain with a standard
error (`±`, about 3–6 %); read a gain within one `±` of the band as a pass.

**The rule for legendary amulets**: a damage legendary adds **+30 %** (±20 %) to the
hero's total damage; a heal legendary **+100 %** to what its spell heals (and shields absorb), a
defensive buff's (immunity) +100 % to its uptime. A hybrid (a damage spell that also heals or
shields, as Holy Nova), a shield and a buff that adds power (damage, haste, speed, rage or energy)
count as damage, +30 % total damage: kiting, the hero's own shield already takes most of the damage
struck at it (Power Word: Shield about 80 %, Ice Barrier about 85 %), so what it absorbs cannot
double, and a buff's uptime is near full already. The same holds a heal that answers low health
(Renew): it already heals most of the damage taken, so its measure cannot double either; a measure
that could would hold the hero to a harder fight (standing in the pack, or a zone above) and count
the health it ends with or the deaths it is spared. Check it with:

    cargo run --release -p esurv-sim --example balance_check -- --legendaries all --kite --no-amulets

(or `--legendaries fireball,slam`; `--check-seeds` 100 by default). Each is played in the first zone
and in Ahn'Qiraj with the arrival hero of each, from its first cache on, against the same hero and
seeds without it; each line ends with its role, the healing and absorbing a second with and without
it, the damage struck at the hero a second and the hero's whole damage a second without it.

**The rule for weapon loadouts**: a class's three loadouts should be about as
strong, each nudged within about 10 % of the others. Compare them with

    cargo run --release -p esurv-sim --example balance_check -- --loadouts --seeds 10 --threads 14

(every class × loadout with each of the class's starting options, kiting, amulets declined, in
Elwynn, Feralas and Ahn'Qiraj or `--zones`): run DPS over the starters, ÷ the class's loadouts'
mean, each starter's DPS and the cheat deaths. The zone table can be measured over every pair too:

    cargo run --release -p esurv-sim --example balance_check -- --seeds 4 --no-amulets --threads 14 --start all --loadout all

(the same per-zone lines, each over every starting option × loadout of every class).

**The rule for gear:** `--gear` prints each zone's set against the set before it. Gear stats round
to whole numbers, so single sets alternate around the budget; the sets from Duskwood to Blackrock
together should come to about ×1.1 each.

**The rule for zones:** with our target kill seconds (`--targets`), a normal enemy's kill seconds
(its health ÷ the expected hero's DPS) should sit within ×0.8–1.25 of the target from minute 5 to
18 in every zone, and elites, rares and bosses should keep their kill seconds and their damage
per hit next to the hero's health. Tune `zone_scaling`'s growths for that, never a single zone.
