# Zones, enemies and bosses: reference

How a zone is defined, what each field does in a run, and how difficulty rises from one zone to
the next. Each zone is a file of its own, `content\zones\<id>.ron`,
with the shared numbers of each enemy kind and boss ability kind in `content\enemy_kinds.ron`;
the types are in `crates\esurv-content\src\schema.rs` (as played) and `kinds.rs` (as
written), the loading in `zone_files.rs`.

Status markers: **confirmed** (settled and built), *inferred* (likely, not yet checked),
**pending** (not decided yet; do not rely on it). This page is updated as the game grows.

## 1. A zone

One file per zone, named after its `id` (`content\zones\elwynn.ron` holds the zone with
`id: "elwynn"`). The game loads every `.ron` file in `content\zones\` and orders the zones by
`difficulty`, lowest first; nothing else lists them (section 5, "Order is the unlock path").

```ron
(
    id: "elwynn",                  // stable key, and the file's name: saves, unlocks and the zone picker use it
    name: Area(12),                // the client's zone name, an AreaTable.dbc id (any, not only
                                   // area_id), read as the game loads; our own name is text
    subtitle: "Trouble at the Mine",
    description: "…",              // shown in the zone picker / loading screen
    map_id: 0,                     // Map.dbc: which continent / instance map the run is on
    area_id: 12,                   // AreaTable.dbc: zone name, music and ambience shown
    spawn: (x: -9396.0, y: -633.0, z: 68.0, facing: 1.6),   // where the hero appears (radians)
    arena_radius: 80.0,            // confirmed: the arena circle around `spawn`, yards
    enemy_spawn_ring: (22.0, 34.0),  // confirmed: roster enemies appear this far from the hero, yards
    boss_spawn_ring: (24.0, 28.0),   // confirmed: the same for bosses
    hour: 10,                      // time of day the run is locked to (0-23)
    weather: (kind: None, intensity: 0.0),   // None | Rain | Snow | Sandstorm, intensity 0-1
    difficulty: 1.0,               // the zone's difficulty D: enemies, gear and rewards grow with it (section 4);
                                   // also its place among the zones, lowest first (section 5)
    enemies: [ … ],                // the roster, in unlock order (section 2)
    bosses: [ … ],                 // the boss schedule (section 3)
)
```

### Picking a place in the world

* `map_id`, `area_id` and `spawn` must describe a real spot of the 1.12.1 world with ground the
  hero can stand on and room for the arena circle. Get coordinates by standing there in a dev
  build (`.gps`-style readout from the client, or the debug panel) and copying x, y, z, facing.
* Map ids the first zones used: 0 Eastern Kingdoms, 1 Kalimdor, 30 Alterac Valley (since replaced
  by Stratholme, 329),
  489 Warsong Gulch, 529 Arathi Basin (since replaced by the Sunken Temple, 109).
  Any map in `Map.dbc` works, as long as the installer
  copies its terrain (step 9 builds the asset list from what the game loads).
* A WMO-only map works too: a dungeon with no terrain at all, only one building (20 of the 43
  shipped maps, Dire Maul 429 and the Stockade 34 among them). The server stands the run on the
  building's own floors and walls (`esurv-server`'s `wmo_floor`). Floors can stack, so every
  height is the floor "nearest below" the hero's height (or the spawn's), and walls block spawn
  sight. Dungeon rooms are far tighter than open ground: measure one with
  `cargo run -p esurv-server --example arena_ground -- <map> <x> <y> <z> <radius>` (or
  `-- <zone id>`) and keep `arena_radius` near the "open reach" it prints. Inside Dire Maul at
  (806.63, 482.04, 37.32) the reach is 13 yd at least, 22 yd on 9 headings in 10, 33 yd on half
  and 58 yd at most: about 30 yd fits, and 80 crosses walls.
* The D 6 zone is Dire Maul (map 429, area 2557), its id
  `feralas`, which saves and unlocks use. Its arena is centred in the open circle at a measured `.gps` (814.63, 482.04, 37.32;
  open 19 yd every way) at radius 80, chosen after a play test: past the walls the hero is sent
  back as anywhere, and spawns come only from floors walked to, so a wide radius plays.
* The D 8 zone is Stratholme (map 329, area 2017), id `strat`.
  A save naming the old id `av` reads as
  `strat` wherever it kept it (wins, gear won, worn and its looks, a hero's zone, the zone played
  last, the zones discovered: `esurv-server` `save::RENAMED_ZONES`), so nobody loses progress.
  Its arena is Slaughter Square, the open square before Baron Rivendare's Slaughterhouse, its
  four gates shut (section "Walls, doors and bosses at their own spots"); the spawn (4033, -3428,
  117.6) stands in front of the Slaughterhouse's door, facing it, and the stray-catching circle
  is 100 yd round the square's middle (4042, -3405). The square is about 160 yd north to south
  and 100 yd across; from the spawn every ring out to 30 yd is walked to, half the 60 yd ring.

### The arena (confirmed)

* The arena is a circle of `arena_radius` around `spawn`. The hero may roam 10 yd past it;
  going further puts them back at radius + 6 yd with "You cannot leave the battlefield." A hero
  who falls more than 15 yd below the ground is put back on it.
* The run starts the moment the hero first moves ("Move to begin").
* Roster enemies appear `enemy_spawn_ring` from the hero (default: 22–34 yd), bosses
  `boss_spawn_ring` (default: 24–28 yd); a zone that leaves them out gets those. Both are kept inside the arena
  circle and on ground within 25 yd of the hero's height, in the hero's sight on open ground.
  A spot that does not fit (a wall or a map edge in the way) is first stepped in toward the hero
  along its direction, 2.5 yd at a time (`SPAWN_STEP_IN`), all the way down to
  but never nearer than half the ring's inner edge (`spawn_inner_floor`, 15 yd for a 30-40 ring); so a
  hero beside a wall still gets spawns on that side, closer. Only when no step fits is a new
  direction drawn (up to 12 times; the last drawn spot is used if none fits).
  **Dungeon maps only:** sight is not enough. The spot, drawn in the
  ring as anywhere, must be walked to from the hero over the floors within twice the ring's far
  edge (`SPAWN_WALK_REACH`), on a floor within 25 yd of the hero's height. So a level joined by
  a ramp or a stair counts, and a balcony you can see but not walk down from does not. Holding
  the walk to the ring itself left the level across the ramp nearly empty: its spots are near
  in a line but far on foot. Pickups and rares follow the same rule. Walking here (`esurv-server`'s
  `walk_graph`): one-yard floor cells, a step between neighbours rising at most
  1.2 yd per yd (about 50°), 2 yd of headroom, and no wall crossing the line at knee height
  (0.6 yd). Enemies chase the nearest hero straight while nothing stands between them, otherwise
  they head down that hero's walking distances (its flow field, out to 100 yd), up to 8 cells at
  a time, round walls and pillars and up ramps.
  On terrain where no walking is needed, a spot must be in the hero's sight instead: a clear
  line between points 1.5 yd over the hero's floor and the spot's (`SIGHT_HEIGHT`). Open terrain
  never blocks sight, so outdoor zones are unchanged. A blocked or unreachable spot is re-rolled
  the same way as a height miss.
  Enemies keep no height of their own: one is shown on the floor at its spot that the nearest
  hero walks to soonest (the floor its spawn was accepted on and that it follows), else the floor
  nearest below that hero's height.
  Leave room: an arena much smaller than about 35 yd makes enemies pile up at its edge.
  Every zone writes 30–40 yd for roster enemies and 25–30 yd for
  bosses.

### Walls, doors and bosses at their own spots

First used by the Scarlet Monastery (`scarlet`, D 4: the fourth zone,
after Warsong Gulch, its gear tier 2):

* **The building is the arena.** On a party dungeon (`Map.dbc` instance type 1) whose buildings
  stand on its tiles rather than in its WDT, the buildings placed on its tiles are its floors and
  walls (`ground::tile_ground`), walked as a WMO-only map's one is, together with the tiles'
  terrain where it has any (Stratholme's streets and squares lie on terrain between its
  buildings; the Scarlet Monastery walks as before, its terrain lying far below its floors). A
  tilted building is left out (Stratholme's cave tunnels, far from its square), the floors being
  read upright. The Scarlet Monastery's courtyard and cathedral (`monestary_cathedral.wmo`) hold
  the run; `arena_centre` puts the stray-catching circle between its two ends (`arena_radius`
  170 yd round 1042, 1399).
* **`walls`**: lines across the floor (from, to). A hero who crosses one is put back where they
  last stood; no walk, flow field, sight line or spawn crosses one. The Scarlet Monastery walls
  the doorway behind its spawn (measured `.gps` spots at its two edges), so the courtyard has no way
  out. Warsong Gulch's middle field ends at a line across the gulch's full width on each side:
  the Horde's in the south (x 1116.85) and the Alliance's in the north (x 1364.76), measured
  `.go` spots; its circle (radius 150 round 1240.8, 1455) only catches strays.
* **`doors`**: the map's own door object (`object`, `display_id`, `at`, our `name`), shown
  shut, and its `doorway` a wall, until `opens_at_minute` on the run clock; then it swings open
  for every client, the doorway stops being a wall, and `announce` shows as a raid warning. The
  cathedral's door opens at 10:00: "Forces are emerging from the cathedral. Its door now stands
  open." A fresh run (Play again) shuts it again. A door without `opens_at_minute` stays shut
  until a boss's script opens it (`Door(name, open)`, bosses.md section 6), and shuts again if a
  script says so; one without `announce` swings silently. Stratholme's four (the map's own doors
  where vmangos stands them): the Slaughter Square Gate (175358) in the lane to the south, the
  Gauntlet's portcullis (175373, 27 yd of doorway) to the east, the Slaughterhouse's door (175405)
  to the west and the Baron's own door inside it (175796); only the Baron's end event opens the
  last two.
* **`props`**: map objects of our own (`name`, `display_id` from `GameObjectDisplayInfo.dbc`,
  `at`, `scale`), shown where they stand; a model with a collision hull is solid to the client, so
  the hero stands on it. **`platforms`**: floor of our own, a disc (`centre`, `radius`, `z`) the
  server walks, spawns and stands the hero on, laid under props that cover a hole. The Sunken
  Temple's pit (round, about 5 yd from its centre, a 95 yd drop) is plugged with a round table
  (the small inn table, display 6508, at ×5.3: a top about 11 yd across) sunk so its top sits
  flush with the floor round it, its legs down in the pit, over a disc of 5.5 yd at that floor's
  height.
* **A boss's `at`**: it comes at that spot instead of the boss ring round a hero.
  **`health_share`**: its share of the health its kind's formula gives (1 when left out).
* **Several final bosses** may come, all at the same minute (loading refuses others); the run
  is won when the last of them falls, and Endless brings them back together. The Scarlet
  Monastery's are Scarlet Commander Mograine and High Inquisitor Whitemane at 18:00, behind the
  altar at measured `.gps` spots, each with 75 % of a final boss's health, each calling 6
  adds a Summon (one final boss's 12 between them). Only the last of them to fall drops the
  weapons (gear.md).

### Rosters by faction

For Warsong Gulch: a zone may write a roster for each faction the
heroes may be, beside its own (`by_faction: Some((vs_alliance: (enemies, bosses), vs_horde:
(enemies, bosses)))`, each roster written as a zone's `enemies` and `bosses` are). `vs_alliance`
is what Alliance heroes fight, `vs_horde` what Horde heroes fight. A run in such a zone takes the
roster its party's faction meets in place of the zone's own (`Zone::for_faction`): the faction
most of its heroes' races share, the first hero's on a tie (`party_faction`). The spawn
director, the bosses and everything after see only that roster. The zone's own `enemies` and
`bosses` stay what the lists outside a run show.

Each roster loads with creature entries of its own (`entries::faction_roster_place`: zone
places from 50 on), so the client never mixes them up; each roster must have enemies and a final
boss, as a zone must. Warsong Gulch needs none: its player-looking enemies (below) take the
other faction's races by themselves, so one roster serves both.

### Player-looking enemies

For Warsong Gulch: a roster enemy or boss may write
`looks_like: Some((class: "warrior"))` (with `form: Some(Cat)` or `Some(Bear)` for a druid in a
form) to be shown as a player character of the faction the party fights, the other one than the
party's (`esurv-server` `game::player_enemies`):

* **its race** is one of that faction's that our game lets be its class (the stock pairs and ours:
  a human hunter, an orc mage, a dwarf shaman, an undead paladin, a gnome priest, a troll warlock;
  the create screen's data, `GameData::allows`), its gender either, its skin, face, hair and facial hair within the
  client's own ranges for that race and gender (`CharSections.dbc`);
* **its gear** is its class's in `contentpvp_looks.ron`: a piece per armor slot picked from the
  slot's list (the class's dungeon set 1 piece and up to ten level 50-60 greens and blues of its
  armor type it can wear, never an epic and never PvP rank gear) and one of its sixteen weapon
  sets (kinds its class carries: two-handers or two one-handers, a one-hander and a shield, a
  staff or a one-hander and a wand, a bow, gun or crossbow); a druid in a form shows the night elf's or the
  tauren's form by its faction;
* **ranked** (`rank: Some(n)`, 1 to 14): it wears its faction's rank 12 rare PvP set (head,
  shoulders, chest, legs, feet, hands; the other faction's where its own has none) over the rest,
  and is named the rank's title as the client has it (`{AllianceRank(n)}`, `{HordeRank(n)}`, from
  the client's `GlobalStrings.lua`) before a made-up player name from `pvp_looks.ron`;
* **a joke** (`pvp_looks.ron` `jokes`: "Noob", "Clicker", "Backpedaler" ...) is picked for each
  where its name writes `{Joke}` (`"{Joke} {Class(1)}"`);
* every pick follows from the enemy's id, so a player who arrives later sees the same; its name
  reaches the clients before it does (a player is named by guid), and it fights as its `kind`
  says: melee kinds close in, `Ranged` ones stand and cast their `look_spell`;
* a player guid has no creature query, so the clients are told the rank of each elite, rare and
  boss among them as it appears (`ESURVER`, `ToClient::EnemyRanks`), for its glow
  ([screens.md](screens.md), "Glows round elites, rares and bosses").

**Hand-made player characters** (`content\player_characters.ron`, written one by
one): an id, a name, a race and a class (any pair our game allows), a gender, a face (`skin`,
`face`, `hair_style`, `hair_color`, `facial_hair`, as the create screen counts them from 0; left
out, one is picked within its race's dials each time it shows) a `yell` (said as it arrives: a boss it plays
yells it in place of its own), optionally a `death_yell` (said as it dies, in place of a boss's
own death yell), optionally a `form` (`Cat`, `Bear` or `Moonkin`: shown in its faction's look of
that druid form), an `assist` (how it assists a hero, in words) with its `assist_script` (what
it does when a battle standard summons it for a hero of its faction: [assists.md](assists.md);
left out, its class's default) and gear: `Random` (its class's PvP looks, as above) or
`Picked` (pieces by look per slot and its weapons, an off-hand item `HeldInOffHand` among them; any
slot left out takes its class's looks, unless it is listed in `bare: [Head]` and so on, which shows
it empty). A boss with `played_by: Some(Any)` is played by one
of the faction the party fights, dealt at random (the pool shuffled once for the run, by its seed,
and dealt to its bosses in turn: none plays two while there are enough); `Some(Named("<id>"))` names one. It shows that
character by its own name, with no title, its class's looks under
hand-picked pieces only where none is picked. While the pool has none of that faction, the boss's
`looks_like` stands in, with a made-up player name. Later mechanics will draw from
the same list. Optionally `boss_abilities`: its spec's boss abilities in place of its class's
(see the Warsong roster below).

**Looks are 1.12's**: every display id in `player_characters.ron` and
`pvp_looks.ron` is the one the server's item table (vmangos `item_template`) gives the item at
its highest `patch` row, its 1.12 look; the item ids of the hand-made sets are Wowhead Classic's. The
test `the_player_looks_are_the_client_s` (`esurv-server`, real data) finds every look in
`ItemDisplayInfo.dbc`.

Warsong Gulch's roster: a joke before the class (`"{Joke} {Class(1)}"`) for every class, melee warriors, paladins, rogues, shamans and cat druids
(our rule, owner: the rogue and the cat run 6.8 yd/s in play, `move_speed: Some(7.16)`, just under
the hero's 7; their kind, FastFodder, runs 7.6),
casting mages, priests, warlocks, shamans and druids, shooting hunters; elites of ranks 5, 8 and
11; five bosses, each carrying the heroes' own flag from its arrival (a boss's `carries`: the client's
Silverwing Flag aura, 23335, against Alliance heroes, the Warsong Flag, 23333, against Horde heroes),
each played by a hand-made character (`played_by: Some(Any)`), no titles, stand-ins
while there are none. Each fights as its class: a Warsong boss writes
`class_abilities: 2` (the final one 3) and only its `Summon()` in `abilities`; it takes that many
of its class's boss abilities first (`Content::boss_abilities`): its character's own
(`player_characters.ron` `boss_abilities`, its spec's), else its class's in its druid form, else
its class's (`pvp_looks.ron` `boss_abilities` / `form_boss_abilities`), their numbers the kinds'
in `enemy_kinds.ron` as any boss's. Every look is its class's own spell (its `Spell.dbc` family)
and none stuns, fears, roots or slows: warrior Charge, Whirlwind, Mortal Strike; paladin
Consecration, Hammer of Wrath, Judgement; hunter Multi-Shot, a Volley marking an Explosive Trap
burst, Arcane Shot; rogue a Sprint rush, Sinister Strike, Eviscerate; priest Mind Blast, Holy
Nova, Holy Fire; shaman Chain Lightning, Fire Nova, Lightning Bolt; mage Pyroblast, Arcane
Explosion, a Blizzard marking a Flamestrike; warlock Soul Fire, a Rain of Fire marking a
Conflagrate, Hellfire; druid Wrath, Moonfire, Starfire (Moonkin Form the same, Starfire first),
Cat Form Claw, Rake, Ferocious Bite, Bear Form Swipe, Maul, Demoralizing Roar. Specs: the tanks
(Quintys, Edgar, Stormwall) Charge, Revenge, Shield Slam; Cdevv (enhancement) Stormstrike, Chain
Lightning, Fire Nova; Nerf (fire) Pyroblast, Fire Blast, Flamestrike. Before (2026-10-20) each
boss slot had fixed abilities whatever class was dealt to it, so a warrior in the mage's slot cast
Frostbolt volleys and a paladin in the warlock's Shadow Bolt volleys. The test
`warsong_bosses_and_players_fight_with_their_class_s_spells` (`esurv-server`, real data) checks
every character and stand-in in every slot. A boss with a spell stands and casts it at the hero
from a caster's range: its character's own `boss_spell` (Nerf's
Fireball), else Starfire in Moonkin Form, else its class's (`pvp_looks.ron`: a mage's Fireball
(Frostbolt before 2026-10-20: it slows), a priest's Smite, a warlock's Shadow
Bolt, a hunter's Auto Shot, flying an arrow, its bow drawn or its gun or crossbow shouldered as it
shoots); a cat or bear swings. Each hit is the boss formula's
damage (weakened and enraged as a swing is) × `caster_boss_damage`, 0.5, as it cannot be stepped out of.

### Sizes: elites up to 6, rares 1.35× their elite, bosses 9, final bosses 12

The strong kinds are drawn larger than the rest, so they read at a glance, with a ceiling so no
model blows up: every elite at 1.3× to 2× its model's own size and never above **6**, every rare
at 1.35× its elite (**8.1** above a full-size elite), every boss at **9** and every final boss at
**12** (a third bigger). Normal enemies and the adds a boss summons keep the `scale` their zone
gives them.

* **Drawn size** = the model's size × `scale`. A model's size is the cube root of the volume of
  the vertex box in its M2 header (the x, y and z extents of the six floats at byte 0xB4..0xCC):
  the display's model path comes from `CreatureDisplayInfo.dbc` and `CreatureModelData.dbc`, and
  `cargo run --release -p benilla-formats --example dump_file -- 'Creature\X\X.m2' > X.m2`
  (with `WOW_DATA` set) writes the file out. `scale` is the whole render scale: the display's own
  DBC scale is not multiplied in.
* **An elite's `scale`** (`kind: Elite`) is between 1.3 and 2.0, and its drawn size is never
  above 6.0: `scale` = min(2.0, 6.0 / model size), rounded to 2 decimals. Where that falls below
  1.3, the 6.0 ceiling wins: a big model is drawn at 6 at a `scale` under 1.3. A Kobold (1.35)
  2.0, drawn at 2.7; a Worgen (3.88) 1.55, drawn at 6; a Flesh Golem (6.18) 0.97; a Tarantula
  (6.72) 0.89; a human man (3.12) 1.92; a human woman (2.66) 2.0, drawn at 5.3.
* **One to focus** (`focus_as_elite`, the Nightmare Suppressors and the Black Guard Sentries)
  takes the elite `scale` by the same rule, its kind and numbers its own: a Dragonspawn caster
  (5.30) 1.13, a Skeleton (3.23) 1.86.
* **A rare** is its elite × `rares::SCALE` (1.35, nothing per zone): 6 × 1.35 = 8.1 for a
  full-size elite, 3.65 for the Kobold one.
* **A boss's `scale`** = 9 / its model's size, a final boss's 12 / size, rounded to 0.05: a human
  man 2.90 and 3.85, an Ogre Warlord (4.74) 1.90 and 2.55.
* **Warsong Gulch** keeps its own steps, as every enemy there is a player character and its
  bosses are drawn at about 6: normal enemies **1.0**, elites **1.2**, rares **1.62**, bosses
  **1.9**, the final boss **2.0**. A hand-made player character playing a boss takes the boss's
  `scale`.

Collision follows the `scale` as everywhere, not the drawn size: roster 1.2 × `scale` + 2.2
(an elite at 2.0 is 4.6 yd round, an Ancient Protector elite at 0.48 is 2.8 yd; Warsong's
3.4 yd normal, 3.64 elite, 4.14 rare), bosses 1.5 × `scale` + 3.5 (Warsong's 6.35 yd, 6.5 final).

## 2. Enemies (the roster)

Each zone has **12** roster entries (the number is not fixed by the code).

```ron
(
    name: "Kobold Geomancer",
    display_id: 163,         // CreatureDisplayInfo.dbc: the model and skin
    scale: 1.0,
    creature_type: Humanoid, // Beast, Dragonkin, Demon, Elemental, Giant, Undead, Humanoid, ...
    kind: Ranged,            // its numbers come from content/enemy_kinds.ron (section 5)
    toughness: 0.5,          // normal enemies only: health × this (default 1; section 4)
    appears_at_minute: 2.0,  // confirmed: cannot spawn before this run minute
    look_spell: Some(133), // ranged kinds only: the WoW spell it casts
)
```

The kind gives the entry its `move_speed` (yards per second), `attack_interval` (seconds between
attacks, melee swings or ranged casts), `spawn_cost` (price against the spawn director's
budget), whether it is an elite, and for casters `range` and `cast_time`. An entry can override
any of these numbers for itself, for example `move_speed: Some(5.2)` or `range: Some(20.0)`.

### What enemies hold

A creature enemy or boss may hold weapons, as the WoW creature of its name does in the
vmangos database (every one, by zone: [enemy-weapons.md](enemy-weapons.md)):

```ron
    weapons: (
        main_hand: Some((kind: OneHandedSword, display: 7483, inventory_type: 13, sheath: 3, material: 1)),
        off_hand: Some((kind: Shield, display: 1705, inventory_type: 14, sheath: 4, material: 1)),
        // ranged: Some((kind: Gun, ...)),
    ),
```

`kind` is a `WeaponKind` (its item class and subclass, which the client picks the swing by);
`display` an `ItemDisplayInfo.dbc` look; `inventory_type`, `sheath` and `material` are the item's
own `item_template` values. The loader refuses a ranged weapon in a hand or a melee one in the
ranged slot. The server sends them as the creature's virtual items, drawn; one with a weapon in
each hand swings them in turn (off-hand swings, `HITINFO_LEFTSWING`). A rare holds its
elite's. Only the look: no number changes. Left out, it holds nothing; a player-looking enemy
wears its class's looks instead.

How a roster plays out (**confirmed**):

* **Elites never come from the budget.** The director spends only on normal enemies
  (`elite: false`); elites have their own timer (below).
* The director earns budget over time (a run starts with 3) and spends it on enemies. `spawn_cost` is what each one
  costs, so it sets how many of that enemy appear compared with cheaper ones. Fodder costs 1,
  mid-tier 2–4, elites 12–14.
* `appears_at_minute` staggers the roster, the same
  in every zone: normal enemies at 0, 0.5, 1, 2, 3, 4.5, 6, 7.5 and 9 minutes, elites at 4, 8.5
  and 13. An unlock only opens an elite kind; elites keep coming on the elite timer (below).
* Killing a normal enemy (not an elite, rare or boss) adds its `spawn_cost` to the director's *kill pressure*, which can speed
  spawning up to 3× when the hero clears fast.
* Which unlocked enemy is chosen each time is a weighted random pick (**confirmed**): an
  enemy's weight is `min(4, 1 + 0.5 × minutes since it unlocked) ÷ spawn_cost`. A newly unlocked
  enemy is rare, grows four times as common over six minutes, and a costly enemy stays rarer than
  a cheap one.

* **Population floor:** at least (1.6 × minutes + 4) × density normal enemies are kept alive
  (×0.45 while a boss is up, ×0.3 for the final boss), never more than 36 × density; up to
  3 × density are added per tick to get there.
* **Elites:** the first arrives at 4:00, then one every max(25, 55 − 2.5 × minutes) seconds,
  picked evenly from the elites already unlocked ("<name> enters the battlefield!").
  Five or six of those turns go to **rares** (section 3, "Rares").
* **Swarms:** the first at 1:20, then every max(45, 80 − 2 × minutes) seconds: (2.5 × minutes +
  12) × 0.6 × density copies of one cheap unlocked normal enemy (spawn cost ≤ 1.2) appear on a
  26-yard ring around the hero ("Swarm surrounds you!"). So keep at least one cheap enemy in a
  roster from minute 0.

### How enemies behave (confirmed)

* **Chasing:** an enemy heads for a point on a small circle around the hero; the point's angle
  moves 0.15 rad each time it re-decides, so a crowd spreads around the hero. Moves are at most
  14 yd long; it re-decides every 0.25–0.7 s (the first time within 0.3 s of appearing; a boss
  first after 1 s, and it stands for 1.5 s as it arrives). Melee enemies aim at 0.8 × their collision radius
  (1.2 × `scale` + 2.2, at least 1.5 yd) and attack within the collision radius.
* **Attacking:** in reach it stops and attacks every `attack_interval` seconds (the first after
  half of it). Melee hits land at once. A ranged enemy (a kind with `ranged`) aims at `range`, starts
  casting once within `range + 4`, stands still for `cast_time`, then fires its spell: a homing
  missile that flies at the spell's WoW speed (`Spell.dbc` Speed, as in spells.md) and always
  hits when it arrives.
* **Crowd control:** slows cut speed for their duration; feared enemies run 8 yd straight away
  from the hero at 80 % speed, re-deciding every 0.8 s; weakened enemies hit softer.
* **Bosses** enrage at 30 % health for the rest of the fight: 25 % faster, 40 % harder hits,
  ability cooldowns ×0.7.
* **Rares** chase and fight as any enemy does (no lair, no leash).
  They move at their enemy's own `move_speed` (× speed% × the game's `enemy_speed`).

The roster pattern per zone, useful as a template:

| Slot | Role | Unlocks | Cost | Toughness |
|---|---|---|---|---|
| 1–3 | swarm fodder (one fast beast) | 0–1.2 min | 1 | 0.25–0.5 |
| 4 | first caster (ranged) | 2 min | 2 | 0.5–0.75 |
| 5–6 | sturdier melee | 3–4.5 min | 1.5–3 | 0.5–2.25 |
| 7 | fast flanker | 6 min | 1.5 | 0.75–2.25 |
| 8 | second caster | 7.5 min | 2 | 0.75 |
| 9 | heavy | 9 min | 3 | 2.5–3.25 |
| 10–12 | elites | 4, 8.5, 13 min | 12–14 | none (elite kind) |

## 3. Bosses

Each zone has **5** bosses; exactly one has `is_final: true`, and it comes last.

```ron
(
    name: "Edwin VanCleef",
    display_id: 2029,
    scale: 8.24,              // bosses are scaled up to read at a distance
    creature_type: Humanoid,
    spawn_minute: 16.0,       // confirmed: arrives at this run minute
                              // speed: boss_move_speed (enemy_kinds.ron); health and damage
                              // come from the formula (section 4)
    yell: "None may challenge the Brotherhood!",
    abilities: [              // special attacks, each on its own cooldown (section 3);
        Charge(look_spell: 100),            // the numbers come from content/enemy_kinds.ron
        Stomp(look_spell: 1680),
        GroundBurst(telegraph_spell: 5740, impact_spell: 13812),
        Summon(),
    ],
    is_final: true,           // killing it wins the run
)
```

* **Confirmed:** while any boss is alive, normal spawning slows to 45 %; while the final boss is
  alive, to 30 %.
* **Boss abilities** (confirmed): each names its kind and its looks; its numbers (cooldown,
  radius, delay, `damage_mult`, `count`) are the kind's in `content\enemy_kinds.ron`
  (`boss_abilities`), unless the ability overrides one (`Stomp(look_spell: 339, radius:
  Some(12.0))`). Each has its own cooldown, starting at 60 % of it when the
  boss arrives. At or below 30 % health the boss **enrages** for good: cooldowns ×0.7, hits
  ×1.4, 25 % faster. "Attack damage" below is the boss's damage after all scaling. Five kinds:
  * `GroundBurst(telegraph_spell, impact_spell, radius, delay, damage_mult)`: marks the
    hero's spot; `delay` seconds later it bursts there for `damage_mult` × attack damage if the
    hero is still within `radius`;
  * `Volley(look_spell, count, damage_mult)`: `count` missiles of that WoW spell, flying at its speed (at
    least 10 yd/s): the first at the hero's spot, the rest 2–6 yd around it; each hits within
    2.5 yd for `damage_mult` × attack damage when it lands;
  * `Summon(count)`: the boss shouts for help; `count` adds (the standard number for the boss's
    place in the zone's list) on a 5-yard ring around the boss, the
    roster's first enemy, with every third the second enemy; they are scaled like any spawn;
  * `Stomp(look_spell, radius, damage_mult)`: `damage_mult` × attack damage at once if the
    hero is within `radius` of the boss;
  * `Charge(look_spell, damage_mult)`: plays that WoW spell (100 = Charge) and rushes to the
    hero's spot at 24 yd/s (at least 1.1 s); landing hits within 4.5 yd for `damage_mult` ×
    attack damage.
* **No crowd control**: a boss never stuns, fears, roots or slows a
  hero. Every ability above only takes health; a look spell is only a look (a War Stomp, a
  Frost Nova or a Psychic Scream look in the dungeons' zone files plays, and the hero keeps
  running). Checked when this rule came (2026-10-20): no boss ability of any zone, nor the
  rares', applied such an effect, so none was removed. The test
  `no_boss_ability_stuns_fears_roots_or_slows_a_hero` (`esurv-sim` `enemy_attacks`) casts every
  zone's boss abilities and the rares' on a hero and finds its speed, buffs, modifiers and
  spot unchanged. Warsong Gulch's bosses go further: not even their looks impair (below).
* **Rares** use a fixed set (not in the zone files): Charge every 9 s, a Thunder Clap stomp
  (8 yd, ×1.4) every 7 s, Summon 4 every 16 s (without the shout),
  each starting at half its cooldown.
* **Pending:** the boss health bar.
* **Boss loot (confirmed):** gear (see gear.md), gold `40 + 20 × bosses slain` (this boss
  included), and a magnet if one is due. Experience, `2400 + 800 ×
  bosses already slain` (`balance.ron` `experience.boss_kill`).

To add or change a boss, follow [bosses.md](bosses.md) (template: [templates/boss.ron](templates/boss.ron)).

### Rares

There are no lairs. A run sends **5 or 6 rares** (drawn
per run, evenly, from the run's seed), on the elite timer:

* **What:** a copy of one of the zone's elite enemies, drawn 1.35× larger and kept apart from its
  elite by its own creature entry (`esurv-server` `entries::rare`). Nothing in the zone
  files lists rares: they come from the roster's elite enemies (kind `Elite`), so a zone needs
  at least one elite to have rares.
* **When:** the rares are due at points spread evenly between the first elite (4:00) and the
  final boss's minute (16 if a zone has none), each in the middle of its share: with 6 in a
  16-minute run at 5:00, 7:00, 9:00, 11:00, 13:00 and 15:00. The first elite the elite timer
  sends at or after a due point comes as a rare instead: the same pick from the unlocked
  elites, the same spawn ring around a hero (`enemy_spawn_ring`, walked to in a dungeon). A
  rare is never held back by the live cap. Endless sends no more.
* **Look:** its target frame and portrait wear the **silver dragon** (1.12's "rare"
  classification; the creature query answers its entry with rank 4), where elites wear the gold
  one (rank 1). Its name is the elite's.
* **Silent**: a rare says nothing when it comes, when it calls its adds or
  when it dies, and no banner announces it.
* **Health and damage**: the rare kind of the enemy formula
  (section 4) at the minute it appears: 240 × 10 × `normal_health`(D)^`elite_health_power` × health clock and 8 × 3 × `normal_damage`(D)^`elite_damage_power` ×
  damage clock. Rares slain do not make the others stronger; the count only raises the gold.
* **When slain:** gold `25 + 5 × rares slain`, an instant level for the killer (experience to
  the next level; its level-up card follows as usual), a 50 % chance to drop a magnet that is
  due, and a **Rare Cache** beside it, **free to open** (gear.md, Caches).
* **Fighting:** it chases as any enemy does, with the rares' abilities (section 3). Below 30 %
  health its abilities come 30 % sooner.

Code: `esurv-sim` `game::rares` (numbers, schedule, spawning, the kill), `game::enemy_attacks`
(boss and rare abilities, ranged shots).

## 4. How difficulty rises from zone to zone

Every enemy kind, from fodder to the final boss, takes its health
and damage from one formula, read once when it appears at run minute `m`. The numbers live in
`content\balance.ron`; the zone gives only its difficulty D, and a normal enemy its toughness.
`balance.ron` also holds the experience numbers (classes.md) and the spell tag coefficients
(`spell_tags`, spells.md 8a).

### The full enemy formula

```
health = 240 × kind health × growth^(D − 1)    × health clock(m) × toughness × health% × 1.22^endless
damage = 8   × kind damage × growth^(0.6 (D − 1)) × damage clock(m)            × damage% × 1.12^endless
speed  = speed% × enemy_speed × base speed   (every enemy alike; `balance.ron` `enemy_speed`, 0.95)
```

* **Base values:** 240 health and 8 damage, the same for the whole game (`base_health`,
  `base_damage`).
* **Kind** (`normal`, `elite`, `rare`, `boss`, `final_boss`):

  | Kind | Health | Damage |
  |---|---|---|
  | normal enemy | ×1 | ×1 |
  | elite | ×10 | ×3 |
  | rare | ×10 | ×3 |
  | boss | ×37.5 | ×2 |
  | final boss (`is_final`) | ×94 | ×1 |
* **D:** the zone's `difficulty`, its place in the unlock order: Elwynn 1, Duskwood 2 … Ahn'Qiraj
  10 (section 5 leaves room for a zone at 6). Every number that
  follows D lives in one block, `balance.ron` `zone_scaling`, and is built on the hero we expect a
  player to bring to the zone (one won run in each zone before it: its talent points and the
  previous zone's gear), never on a hero's actual power (AUTHORING.md, "Power budgets"):
  * normal enemies: health × `normal_health` at D, a table (D 1–12: ×1, 1.4, 2.25, 4, 5.75,
    6.75, 7.25, 8.75, 9, 11, 12, 17.25; straight lines between points, past the last at the last step's
    pace), fitted so the baseline hero (rare and epic amulets, bought caches, kiting) meets the
    original game's kill seconds; damage × `normal_damage` at D, its own table read the same way
    (×1, 1.15, 1.4, 2.05, 2.45, 2.7, 2.7, 3.25, 3.25, 3.75, 4.25, 5: the blows the health table gave
    before its refit, kept), so `damage_share` (0.6) is not read while it is set;
  * elites and rares: the health multiplier^`elite_health_power` (**0.87**), bosses
    ^`boss_health_power` (**0.7**) and the health clock to `boss_clock_power` (**0.3**), so a
    fight with them keeps about the same length next to the expected hero from zone to zone; their
    damage is `normal_damage` to `elite_damage_power` (**0.65**) or `boss_damage_power` (**0.5**),
    the blows they had before;
  * gear, drops, gear tiers and talent experience (below, gear.md and classes.md).
* **Clocks** (`health_clock`, `damage_clock`): through these points, in straight lines between
  them, and past each clock's last point at its last step's pace (health past minute 20 at the
  16–20 pace, damage past minute 16 at the 12–16 pace). Bosses read them at their arrival minute
  too. Health has a point at minute 20 (×65, from ×54) so it keeps
  curving upward after minute 16 instead of growing in a straight line.

  | Minute | 0 | 4 | 8 | 12 | 16 | 20 |
  |---|---|---|---|---|---|---|
  | Health | ×1 | ×2.5 | ×7 | ×18 | ×36 | ×65 |
  | Damage | ×1 | ×1.5 | ×2.5 | ×4.5 | ×7 | ×9.5 |
* **Toughness:** per normal enemy, on health only, in steps of 0.25 (at least 0.25); 1 when left
  out. It says how much sturdier the enemy is than its zone's average normal enemy. Elites,
  rares and bosses have none.
* **R:** a run-wide factor every run starts at **1** and nothing changes; health × R and
  damage × √R. It is kept as a knob for future modes.
* **Endless** (**confirmed**): after the final boss you may continue; then health ×1.22 and
  damage ×1.12 per minute on top, and spawning speeds up. These Endless minutes count from the
  final boss's latest death. In Endless the final boss may come back: in the second before each
  swarm, each tick has a 0.2 % chance to bring it again, and its death starts the Endless minutes
  over.
* **Run options** (confirmed, per hero): enemy density 100–500 %, health 100–500 %,
  damage 100–300 %, speed 100–150 %.
* **Live cap** (`balance.ron` `live_cap`): at most 60 normal enemies alive at 100 % density, rising in a straight line with the density option to 120 at 500 %.

Boss abilities work from the boss's damage after all of this (section 3).

### The zones

| Zone | D | Normal enemy at minute 0 (health / damage) | Boss gear drop chance | Talent experience | Final boss at |
|---|---|---|---|---|---|
| Elwynn Forest | 1 | 240 / 8.0 | 100 % | ×1.00 | 16 min |
| Duskwood | 2 | 336 / 9.2 | 80 % | ×1.62 | 16 min |
| Warsong Gulch | 3 | 540 / 11.2 | 80 % | ×2.16 | 16 min |
| Scarlet Monastery | 4 | 960 / 16.4 | 70 % | ×2.64 | 18 min (two at once) |
| Sunken Temple | 5 | 1380 / 19.6 | 70 % | ×3.09 | 18 min |
| Dire Maul | 6 | 1620 / 21.6 | 60 % | ×3.51 | 18 min |
| Gurubashi Arena | 7 | 1740 / 21.6 | 60 % | ×3.90 | 18 min |
| Stratholme | 8 | 2100 / 26.0 | 40 % | ×4.29 | 20 min |
| Blackrock Mountain | 9 | 2160 / 26.0 | 40 % | ×4.66 | 20 min |
| Gates of Ahn'Qiraj | 10 | 2640 / 30.0 | 40 % | ×5.01 | 20 min |

Besides D, a later zone is harder because its **run is longer**: bosses move from
3/6/9/12/16 min to 4/7/11/14/18 and 4/8/12/16/20. The spawn budget grows with time
(`balance.ron` `spawn_budget`, per second at 100 % density: 0.5 / 1.5 / 3 /
5 / 7.5 / 10.5 at minutes 0 / 4 / 8 / 12 / 16 / 20, straight lines between, and the 16-20 pace
after) and the clocks keep climbing, so a longer run reaches
a denser, sturdier horde before the final boss.

**Gear tier and rewards**: zones share a gear tier for their gear's look (Elwynn 0; Duskwood,
Warsong 1; the Scarlet Monastery, the Sunken Temple, Dire Maul 2; Arena, Stratholme 3; Blackrock, Ahn'Qiraj 4).
A zone's tier comes from its D (`zone_scaling` `gear_tier_starts`):
tier 1 from D 2, tier 2 from 4, tier 3 from 7, tier 4 from 9, so a zone added between two others
takes their tier and one past Ahn'Qiraj takes tier 4.
Boss gear drops by steps of D, `gear_drop_chance` (little grinding for
gear; 100 % at D 1, 80 % from 2, 70 % from 4, 60 % from 6, 40 % from 8 on; see gear.md), and
talent experience is multiplied by
D^`talent_exponent`, D^0.7 (×1.0 in Elwynn up to ×5.01 in Ahn'Qiraj), so
harder zones trade drop chance for faster talent progress. Both follow the zone's own D, so a zone
added anywhere leaves the others as they are.

### Challenge Mode

The mode for after the last zone.

* **Unlock.** A class that has won Gates of Ahn'Qiraj (`aq`, the zone in
  `esurv_sim::game::challenge::CHALLENGE_ZONE`) has Challenge Mode open. It is kept the way every
  zone's clear is, per class in the save's `zones_won` (server.md), and told to the client at the
  end of `Progress` (`challenge_open`). Until then the zone select looks as it always has. After
  it, two tabs sit over the zone list, **Normal Mode** and **Challenge Mode**, and a challenge
  slider under the second (menus.md, screens.md).
* **Normal Mode** is the game as it is: nothing changes.
* **Challenge Mode.** Every zone is open. A zone keeps its own enemies, roster, toughness, bosses,
  mechanics and clocks, but its zone scaling is Ahn'Qiraj's: every enemy's health and damage come
  from the formula above with Ahn'Qiraj's D (read from its zone file, never a written 10). Then
  the final health and the final damage of every enemy, normal, elite, rare, boss and final boss
  alike, are × **`challenge_factor` × slider**, one flat factor that never goes through the kind
  and zone powers (`elite_health_power`, `boss_health_power`, `damage_share`,
  `elite_boss_damage_share`), on top of everything else, the run options included:

  ```
  challenge health = (the formula's health at Ahn'Qiraj's D) × challenge_factor × slider
  challenge damage = (the formula's damage at Ahn'Qiraj's D) × challenge_factor × slider
  ```

  `challenge_factor` is **1.1** (`balance.ron`). The slider runs **1x to 5x** in **0.25** steps
  (1.00, 1.25 ... 5.00), shows its value as "1.25x", stacks with the run options of
  `run_options.ron` (which stay as they are) and is kept in the save with them. The mode and the
  slider reach the server in the run options line (`options`, server.md); the server runs
  Challenge Mode only for a class that has it open.
* **Rewards.** A challenge run pays:
  * **class gear** its bosses drop, exactly as in Normal Mode for that zone (the zone's own D for
    the drop chance and the tier, × the run options' bonus);
  * **talent experience**: what the same run would pay in Ahn'Qiraj (Ahn'Qiraj's D in the zone
    bonus), × `challenge_factor`; the slider does not scale it, and the run options keep their
    bonus. It buys talent and prestige points as ever.

  It pays nothing else: no hero records (best level, longest run, runs and wins), no zone won and
  none opened or discovered, and nothing new for the codex (rings, trinkets, amulets and
  legendaries taken in the run). What a run gives inside itself (levels, gold, caches, cards,
  amulets) is play, not reward, and stays as it is.


## 5. Adding or changing a zone

**Standard roster numbers.** Every enemy uses its kind's numbers,
defined once in `content\enemy_kinds.ron` (`enemies`); a roster entry names its kind
(`kind: Fodder`). Change a number there and every enemy of that kind follows. The kinds:

| Kind | `move_speed` | `attack_interval` | `spawn_cost` | Use for |
|---|---|---|---|---|
| `Fodder` | 6 | 1.5 | 1 | weak early melee, toughness 0.25–0.5 |
| `FastFodder` | 8 | 1.5 | 1 | fast beasts among the fodder |
| `Mid` | 5 | 2 | 2 | mid melee |
| `ToughMid` | 5 | 2 | 3 | mid melee at toughness 2 and up |
| `FastMid` | 8 | 2 | 2 | fast mid melee (flankers, beasts) |
| `Ranged` | 5 | 3 | 2 | casters: range 18 yd, cast time 1.5 s; the entry names its `look_spell` |
| `Elite` | 5.5 | 2 | 12 (not read: elites come on their timer) | elites |

A special enemy can override a number for itself (`move_speed: Some(5.2)`, `attack_interval`,
`spawn_cost`, and for casters `range`, `cast_time`); no enemy uses one today. New kinds can be added to
`enemy_kinds.ron` and named in a zone file. Loading fails with a clear message on a kind that
does not exist, a caster without `look_spell`, or a `look_spell` on a kind that does not cast.

1. Copy a zone file in `content\zones\` (or `docs\game\templates\zone.ron`) to
   `content\zones\<id>.ron` and set its `id` to that same name. That file is the whole zone:
   no list or code names it.
2. Set the place: `map_id`, `area_id`, `spawn`, `arena_radius`, `hour`, `weather`.
3. Build a roster of enemies: pick `display_id`s from `CreatureDisplayInfo.dbc`, pick each one's
   `kind` from the table above, and set `toughness` (normal enemies), `appears_at_minute` and,
   for casters, `look_spell`, following the pattern in section 2. Give each elite the `scale`
   min(2.0, 6.0 / its model's size) and each boss the one that draws it at 9 (12 the final),
   section 1, "Sizes".
4. Add bosses with rising `spawn_minute`; mark the last one `is_final: true`. Their abilities
   name a kind and its looks (bosses.md).
5. Pick `difficulty`: the next whole number after the last zone, or a step between two zones (for
   example 11 and 12 after Ahn'Qiraj, where room is kept), against the table in section 4. It sets
   the zone's place too (step 7); no two zones may share one.
6. Run `cargo test -p esurv-content`: the loader checks ids, final-boss rules and
   references. It fails with a message naming the file on a zone file whose `id` is not its
   name, an id used twice, two zones of equal difficulty, or a difficulty that is not above 0.
7. **Order is the unlock path:** zones are ordered by difficulty,
   lowest first, and every list of zones (the zone picker, the class picker's count, the
   collection log) shows them in that order. A class starts with the first zone; **winning** a
   zone opens every zone up to it and the next one after it ("<zone> is now open to your
   <class>s"). The save keeps the zones each class has won, not a count, so a zone added below a
   class's hardest win is open at once and nothing opened before closes; a zone added above it
   opens like any other, with a win in the zone before it. Its gear tier comes from its
   difficulty too (section 4, gear.md). **Discovery:** a zone no
   class of the save has opened yet is listed only as "???" with a lock; once any class opens
   it, the save keeps it discovered (`zones_discovered`, server.md) and every hero sees it in
   full, though each class still opens it itself to play it.
8. **Pending (step 9):** the installer's asset list.

## 6. Changing an existing zone

Edit the values in the zone's file, `content\zones\<id>.ron` (the kinds' shared numbers in `content\enemy_kinds.ron`,
the enemy formula's numbers in `content\balance.ron`) and restart the game; no rebuild is
needed. Keep your own changes in a separate commit so they are easy to compare.
