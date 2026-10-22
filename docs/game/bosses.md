# Making a boss

How to add a boss to a zone, give it abilities, change them or take them away. The
ready-to-copy block is [templates/boss.ron](templates/boss.ron); the plain-language version of
this page is [guides/bosses.txt](guides/bosses.txt). Zones, rosters and rares are in
[zones.md](zones.md).

## 1. Steps

1. Open the zone's file, `content/zones/<id>.ron` (`content/zones/elwynn.ron` for Elwynn), and find its `bosses` list.
2. Paste [templates/boss.ron](templates/boss.ron) where the boss should come in the order;
   bosses arrive by `spawn_minute`.
3. Give it a `name`, a model (`display_id`) and its numbers.
4. Build its `abilities` list from the five kinds below (any number, any mix, including none).
   Each names its kind and its looks; the numbers come from `content/enemy_kinds.ron`.
5. Keep one boss with `is_final: true`, and keep it last. Several
   final bosses may come together, at the same minute; the run is won when the last falls
   (zones.md, "Walls, doors and bosses at their own spots"). A boss may also take a spot of its
   own (`at`) and a share of its health (`health_share`).
6. Run `cargo test -p esurv-content -p esurv-sim`.

**Remove an ability:** delete its line. **Add one:** copy a line from the table in section 3.
**Change one for every boss:** edit its kind's numbers in `content/enemy_kinds.ron`
(`boss_abilities`). **Change one for this boss only:** add an override to its line, for example
`Stomp(look_spell: 6343, radius: Some(12.0), cooldown: Some(10.0))`; any of the kind's numbers
(`cooldown`, `radius`, `delay`, `damage_mult`, `count`) can be overridden. An empty list
(`abilities: []`) gives a boss that only walks and swings.

## 2. Fields and what they become in play (confirmed)

| Field | Meaning |
|---|---|
| `name`, `yell` | Shown over its head; shouted when it arrives. |
| `display_id`, `scale`, `creature_type` | Its look, as for roster enemies (zones.md). |
| `spawn_minute` | Run minute it arrives. |
| (health, damage) | Not set per boss: the boss kind of the enemy formula (zones.md section 4) at the minute it arrives, 240 × 24 × `normal_health`(D)^`boss_health_power` × health clock and 8 × 2 × that multiplier^`elite_boss_damage_share` × damage clock; the final boss (`is_final`) takes ×28 health and ×1 damage instead. Then the run options and ×1.22 health, ×1.12 damage per Endless minute. It swings every 2 s and its abilities scale off this damage. |
| `move_speed` | Optional, `Some(..)`: its own yards per second instead of `boss_move_speed` in `enemy_kinds.ron` (6 for every boss). Both × the speed run option. |
| `abilities` | Its special attacks (section 3). |
| `is_final` | Killing the final boss wins the run; with several, killing the last of them. |
| `at` | Optional: the spot it comes at (x, y, z, facing) instead of the boss ring. |
| `health_share` | Optional: its share of the formula's health (two final bosses at 0.75 each). |
| `called` | Optional, `true`: only a script brings it (`Call`, section 6); the schedule passes it by (Whitemane, Cho'Rush the Observer, Ohgan). |
| `aggro_sound` | Optional, `Some(..)`: the original boss's own pull voice line, a `SoundEntries.dbc` id, played to everyone (`SMSG_PLAY_SOUND`) as it arrives, with its yell. The yell's text stays ours; the audio is the client's. Left out for a boss the 1.12 client gives no voice, and never played for one a hand-made character plays. The ids are in section 5. |

Its reach is 1.5 × `scale` + 3.5 yards, at most 9. A boss never uses an ability while stunned
or rooted; its cooldowns keep counting down meanwhile. Abilities never go off together: two
casts are at least 1.5 s apart (`ABILITY_GAP` in `esurv-sim` `game::enemy_attacks`), the one
that has waited longest first. Below 30 % health a boss **enrages** for
the rest of the fight: 25 % faster, hits 40 % harder, abilities come 30 % sooner. Each ability
is first used at 60 % of its cooldown after the boss arrives.

## 3. Ability kinds

"Attack damage" is the boss's damage in play (section 2), including enrage.

| Kind | Copy this | What it does |
|---|---|---|
| Charge | `Charge(look_spell: 100),` | Plays the WoW spell (100 = Charge) and rushes to where the hero stands at 24 yd/s (at least 1.1 s). Landing hits everything within 4.5 yd of that spot for `damage_mult` × attack damage. Unmarked: the rares keep it; a reworked boss charges with a `Rush` (section 6), run through its marked line. |
| Stomp | `Stomp(look_spell: 6343),` | Plays the spell; if the hero is within `radius` of the boss, `damage_mult` × attack damage at once. |
| GroundBurst | `GroundBurst(telegraph_spell: 10, impact_spell: 122),` | Marks the hero's spot with `telegraph_spell`; `delay` seconds later it bursts there (`impact_spell`) for `damage_mult` × attack damage if the hero is still within `radius`. |
| Volley | `Volley(look_spell: 133),` | `count` missiles of that WoW spell, flying at its speed (at least 10 yd/s): the first at the hero's spot, the rest 2–6 yd around it. Each hits within 2.5 yd for `damage_mult` × attack damage. |
| Summon | `Summon(),` | The boss shouts for help (`lines::SUMMON_YELL`): `count` adds on a 5-yard ring around the boss: the zone roster's first enemy, every third one its second enemy, scaled like normal spawns. The roster needs at least two enemies. |

All `look_spell`, `telegraph_spell` and `impact_spell` values are WoW spell ids (see
spells.md, section 3).

**Standard numbers.** Every boss uses the same numbers for a kind;
only which bosses have which abilities, and their looks, differ. They are defined once, in
`content/enemy_kinds.ron` (`boss_abilities`), and resolved when the game loads; Summon's adds
are picked by the boss's place in the zone's `bosses` list (`adds_by_position`). A boss past
the end of that list needs its own `Summon(count: Some(..))`, or loading fails.

| Kind | Numbers |
|---|---|
| Charge | cooldown 8 s, 2 × attack damage |
| Summon | cooldown 15 s; adds 6 / 7 / 8 / 10 / 12 for a zone's bosses 1 to 5 |
| Stomp | radius 10 yd, 2 × attack damage, cooldown 8 s |
| Volley | 6 bolts of 0.5 × attack damage, cooldown 6 s |
| GroundBurst | radius 10 yd, 2 s warning, 3 × attack damage, cooldown 6 s |

Enemy unlocks are the same in every zone too: the nine normal enemies at 0, 0.5, 1, 2, 3, 4.5,
6, 7.5 and 9 minutes, the three elites at 4, 8.5 and 13 minutes.

## 4. A brand-new kind of ability

The five kinds above cover any mix of numbers. A different behaviour (a pull, a shield, a
beam…) needs code first:

1. Add a variant to `BossAbilityKind` in `crates/esurv-content/src/schema.rs`, with the fields
   it needs (as played), and the matching written form to `BossAbilityEntry` in `kinds.rs`:
   its looks, plus an optional override for each number.
2. Give its standard numbers a block in `BossAbilityNumbers` (`kinds.rs`) and in
   `content/enemy_kinds.ron`, and resolve it in `resolve_ability`.
3. Teach `cast` in `crates/esurv-sim/src/game/boss_abilities.rs` what it does, with a test.
4. Describe it in this page's table and in guides/bosses.txt.

After that, any boss can use it from its zone file like the others.

## 5. Pull voice lines (confirmed)

Each boss's `aggro_sound` is the line the original boss speaks when pulled: the sound its
script plays with its aggro text (vmangos `broadcast_text.sound_id`), cross-checked against the
NPC's sounds on Wowhead Classic and the `SoundEntries.dbc` row's file (`Sound\Creature\...`).
The creature's own grunts (`CreatureSoundData` aggro) are not voice lines and are not used.
A boss whose original speaks only on another cue uses that line where it is the boss's first:
Whitemane's arrival, C'Thun's "already dead" whisper. Jeklik's only line is her transform line, so she stays silent. Where the voice plays, the yell text is the line it speaks (text and voice match); the Avatar of Hakkar keeps our text, its spoken line not being on record. A real-data test
(`esurv-server`) checks every id is a `SoundEntries.dbc` row.

| Zone | Voiced (`SoundEntries` id) | No voice in 1.12 |
|---|---|---|
| Elwynn | Rhahk'Zor 5774, Mr. Smite 5777 (his alarm), Edwin VanCleef 5780 | Hogger, Mother Fang |
| Duskwood | none | Naraxis, Mor'Ladim, Stitches, Lupos, Morbent Fel |
| Scarlet Monastery | Loksey 5841, Doan 5842, Herod 5830, Mograine 5835, Whitemane 5838 | Fairbanks |
| Feralas | none | Zevrim, Alzzin, Tendris, Immol'thar, King Gordok |
| Sunken Temple | Atal'alarion 5860, Jammal'an 5862, Avatar of Hakkar 5871 | Morphaz, Shade of Eranikus |
| Zul'Gurub arena | Venoxis 8421, Mandokir 8413, Jin'do 8425, Hakkar 8414 | Jeklik (muted: transform line only) |
| Stratholme | none | Barthilas, Anastari, Nerub'enkan, Maleki, Ramstein, Rivendare (no Stratholme line has a sound in 1.12) |
| Blackrock Mountain | Majordomo Executus 8035, Nefarian 8280 (Nefarius's "Let the games begin!"; his own 8288 as he lands) | Angerforge, Rend, Thaurissan |
| Ahn'Qiraj | Ossirian 8598, C'Thun 8581 (Rajaxx's 8638 as he joins after his waves) | Kurinnaxx, Huhuran, Rajaxx (his arrival line) |

Warsong's bosses are played by hand-made characters and have no voice.

## 6. Building blocks

Every boss plays its own WoW mechanic,
reshaped for a survivors game. Every boss **area attack is marked on the ground before it lands**
and can be walked out of (a wind-up of 1 to 2.5 s); **boss attacks hit the hero only**, never the
normal enemies (scripts that act on the horde, such as Raise Dead, still do); bosses never
stun, fear, root or slow the hero (the test `no_boss_ability_stuns_fears_roots_or_slows_a_hero`
guards it), the one exception being the scripted pause below. The five kinds of section 3 stay
valid; the building blocks come on top, each one module in `esurv-sim` (`game::telegraphs`,
`game::ground_zones`, `game::interactables`, `game::boss_scripts`, `game::scripted_pause`,
`game::boss_warnings`) and its content in `esurv-content` `boss_mechanics`.

### Marked attacks (`Marked`, `Rush`)

| Field | Meaning |
|---|---|
| `shape` | `Circle(radius)`, `Ring(inner, outer)` (safe in the middle), `Cone(length, angle)` (degrees, its whole width), `Line(length, width)`. |
| `at` | `Boss` (round the boss, or from it towards the hero; the boss stands still while it winds up and plays the impact itself), `Hero` (on the hero's spot), `NearHero(count, spread)` (the first on the hero, the rest up to `spread` yd round it). |
| | `Across(count, spread)`: lines (or cones) laid across the ground at random facings, each through a spot, the first the hero's, the rest up to `spread` yd round it (avalanches). |
| `impact_spell` | The WoW spell played as it lands (0: none). |
| `wind_up`, `damage_mult`, `cooldown` | Optional overrides of `enemy_kinds.ron` `marked` (1.5 s, 3 × attack damage, 8 s). |
| `warning_spell`, `fill_spell` | Optional: the looks for a client without a HUD (the game's client draws ground indicators instead, below); standard `marked`'s (Consecration's ranks 5 and 4 as dynamic objects, drawn by our client as Kel'Thuzad's Shadow Fissure in its own purple, half see-through: a fissure at 70 % of each disc covering the shape as the warning, a brighter one growing inside each in steps as the fill). |
| `leaves` | Optional: a lasting zone where it lands (one per covering disc for a cone or line, at most 4). |
| `leaves_object` | Optional: an object where it lands (Naraxis's cocoons). |
| `missile` | The impact flies from the boss to land as the wind-up ends (Touch of Death). |
| `warn` | Optional: its warning and timer bar (below). |
| `from` | Optional: laid from each living roster enemy of that name instead of the boss (round it with `Boss`, a line from it towards the hero, a missile flying from it: Immol'thar's Eyes, Jeklik's bat riders); none up, nothing is cast or announced. Laid at the hero from several, the first lands on the hero's spot and the others 3.5-7 yd round it (`ADDS_SPREAD`), never all on one spot. |
| `impact_on_bunny` | Optional, `true`: for a look that plays its impact only on a unit it hits (a smite, a strike, a bolt), the server puts an invisible target unit, a "bunny", where the mark lands (over its discs for a cone, ring or line; under a missile before it flies) and the spell goes hitting it, so the full look shows whether the hero is there or not; the bunny has no name, health bar or room, cannot be targeted, is no enemy of the simulation and goes 2 s later (`esurv-server` `game::bunnies`). The damage stays the simulation's, the hero's alone. Set per ability, for looks that need a unit (Whitemane's Holy Smite, Hogger's Head Butt, Touch of Death, Ivus's Moonfire and Starfire, Thaurissan's hammer, C'Thun's Eye Beam and Dark Glare, Glacius's Boulder, ...); a `Volley` takes it too, each shot then flying at a bunny where it lands (Ivus's Wrath, Majordomo's Fireballs, Nefarian's Shadow Bolts, Huhuran's poison). |
| `chain` | Optional: each further mark of a cast lands this many seconds after the one before (Eye Beams). |
| `brings` | Optional: `(enemy, count, rooted)`, adds bursting from where it lands; `rooted` ones never move (tentacles). |
| `sweep` | Optional, a line from the boss: `(turn, seconds, every, damage_mult, look_spell)`, once it lands it stays and turns `turn` degrees a second round its boss for `seconds`, hurting a hero in it every `every` s (a beam, drawn with `look_spell`; `game::beams`); the boss stands while it sweeps. |

`Rush(look_spell, fade_spell, length, width, wind_up, damage_mult, cooldown, warn)` is a charge
that runs through: a line from the boss through the
hero's spot to 8 yd past it (`RUSH_PAST_HERO`), never longer than `length` (standard `rush`:
30 × 5 yd, 1.5 s, 3 ×, 10 s), is marked for `wind_up` while the boss stands (wearing
`fade_spell`, Lupos's shadow). Then the boss runs the whole line, start to far end, at a
charge's 24 yd/s (`RUSH_SPEED`), and a hero inside the line's width as the boss passes it is hit
once for `damage_mult` × attack damage (`game::telegraphs::charges_tick`; the boss's front is
half the width deep). No landing circle, no knock, no stun; enemies are never hit. It goes on
from the far end, deciding afresh. The line's start, far end and width
(`Telegraph::line_ends`) are the mark's own `at`, `facing` and `Shape::Line`, the very line the
hit is tested against, so a client drawing the mark draws the path. A rush is dodgeable: its
damage counts as avoidable ("Damage a hero cannot avoid" below).

A hero is hit when it stands inside the shape itself (`Shape::contains`) as the mark lands (a rush: as its boss passes).

**Ground indicators** (Final Fantasy XIV's indicators in
place of WoW spell models). The game's client draws every mark itself, flat on the ground: the
shape exactly as the hit test takes it (circle; ring with its safe middle; cone; line), in a
see-through orange (26 %) with a near-opaque brighter outline. Over the wind-up a brighter fill
(46 %) sweeps from the origin to the edge (a ring's inner edge outwards, a cone's or line's near
end to its far end) with a bright front, so the timing reads at a glance; the hit lands as the
indicator goes. A lasting zone (fire, poison, plague, a blizzard once landed, webs) keeps its
spell look and gets a thin orange outline too; a moving one (a drifting zone, an aura following
its boss) shows chevrons the way it goes, and a sweeping beam is outlined along its length with
chevrons pointing the way it turns. Orange is the only colour (no option). The boss warnings and
timer bars are unchanged. The server tells each indicator to every client with a HUD over the
game's lines (`ESURVGI`: shape, place, ground height, facing, wind-up, time to land, speed, turn;
`ESURVGX`: gone), again when it strays from where the client reckons it or the run stops or goes
on (`esurv-server` `game::ground_indicators`); the client projects it onto the terrain and building
floors with the selection ring's decal projector, over the ground and under the units, no
shadow, never picked (`esurv-app` `ground_indicators`). The `warning_spell` and `fill_spell` looks
are laid only for a client without a HUD of its own (a stock 1.12 client), as ground looks over
the discs that cover the shape (`Shape::patches`). Dev aid: `ESURV_INDICATOR_SHOTS=<png>` writes a
shot of each kind of indicator as it first shows.

### Lasting zones (`leaves`, `Zone`, the script's `LayZone`)

`(look_spell, radius, seconds, damage_mult, every, drift, follows, affliction)`: a look on the
ground for `seconds` (0: while its boss lives) that every `every` seconds (1) hurts each hero
inside for `damage_mult` × the boss's attack damage and adds a stack of `affliction`. `drift`
moves it towards the nearest hero (yards a second: a moving zone); `follows` keeps it on its
boss (an aura). It never touches the horde.

An **affliction** `(look_spell, damage_mult, every, seconds, max_stacks)` is a stack on the hero:
each stack hurts `damage_mult` × attack damage every `every` seconds while it lasts, a new stack
renewing the time; damage only, shown as the hero's debuff. A boss's `on_hit` gives one with each
melee blow (Lupos).

### Objects on the ground (`Lay`, `leaves_object`, the script's `Place`)

Everything on the ground the hero interacts with (eggs, cocoons, totems, altars,
fires, crystals, relics) is **broken or set off by walking over it**, never by damage: standing
within `radius` (2 yd) of it for `contact` seconds in one go (0.5 s) does it, so brushing past
does not; its glow (`glow_spell`, Consecration's rank 3, gold) fills under it as the hero stands
there. Hero attacks never strike it (it is no enemy). `(name, display_id, scale, glow_spell,
contact, radius, hatch, on_contact, again_after, warn)`: `display_id` is a
`GameObjectDisplayInfo.dbc` model; left alone, `hatch: (after, enemy, count)` hatches into adds of
the zone's roster; taken, it runs `on_contact` (script actions) and comes back `again_after`
seconds later if set; `warn` is announced each time it is put down. `Lay(object, near, count,
cooldown, warn)` puts `count` (3) down `near: Boss(yards)` or `Hero(yards)`, fanned out so a hero
on one stands on no other, every `cooldown` (15 s).

Three more fields: `pulse: Some((seconds, run))` runs its actions every `seconds` while it stands,
from its own spot (a totem healing round it, `Heal(on: Near(..))`, or laying a mark round itself,
`Cast(ability: Marked(.., at: Boss))`, or on the hero, `at: Hero`); `left_alone: Some((seconds,
run))` makes it go after `seconds` and run its actions (Zevrim's sacrifice heals him); and
`on_all_taken: [..]` runs, besides `on_contact`, when it is the last of its name its boss has down
(the Avatar of Hakkar's fourth blood fire drops his shield).

Shown otherwise: `creature_display: Some(id)` shows it as a creature model instead (a
`CreatureDisplayInfo.dbc` id: the troll wards and totems), an unselectable unit standing there;
`planted_item: Some((display, sink))` shows a weapon's model planted head up in the ground, like a
torch stuck in the floor (an `ItemDisplayInfo.dbc` display, in its own texture, its foot `sink`
yards deep at scale 1): the client composes it from the player's own data in an unseen unit (the
invisible stalker, `looks::PLANTED_ITEM_BASE`), at the object's `scale`, one composed look per item
(`9 900 000 + display`). An object shown as a unit can wear a spell's lasting look as an aura,
`aura_look: Some(spell)` (Morbent's Bane in Divine Shield's golden bubble). A planted item's
`aura_lift` (yards at scale 1, default 0) moves the unseen unit's attachment points, which the aura's
look hangs on: the Bane's -0.8 brings the bubble down round the torch, its bottom at the ground.
`display_id` is unused then.

### Scripts (`script`)

`script: [(on: .., run: [..], warn: .., soon: ..)]`: `on` is `Arrival`, `Health(percent)` (once,
the first time its health is at or below it), `Every(seconds)`, `After(seconds)` (once), `Death`,
`AddDied` (each time a roster enemy named in the line's `add` dies while the boss is up; its
actions run at the add's spot: Hakkar's sons' blood) or `HeroFell` (each time a hero loses a life,
or falls, to the boss: its swing, its mark or its zone; Mandokir's level-up). An `Every` line may
carry `until_health: Some(50.0)` (it runs no more below that share, its bar running out: Jeklik's
bat swarms) or `from_health: Some(50.0)` (it runs only from that share on, its bar showing then:
her bat riders). `run` is a list of:

| Action | What it does |
|---|---|
| `Yell(text, sound)` | Shouts `text`; `sound`, the original boss's voice (`SoundEntries.dbc`), plays with it, and then the text is the line it speaks. |
| `Emote(text)` | A centre-screen line, `%s` the boss's name. |
| `Wave(enemy, count, at)` | `count` of roster enemy `enemy` at `Ring(yards)` round the boss, `NearHero(yards)` round the hero, `Spot(x, y, spread)` on a ring round a spot of the map (Stratholme's undead in at the south gate), from the `Edges` (the enemy spawn ring), or in `Sides(packs)` packs from far-apart sides of the edge, each pack's lane (14 × 4 yd, pointing at the hero) marked 2 s before it runs in (Loksey's hounds); past the live cap. |
| `Door(name, open)` | The zone's door of that name swings open (`open: true`) or shut, for every client; while shut its doorway is a wall (zones.md, "Walls, doors and bosses at their own spots"; the Baron's door opening as his Black Guard comes out). |
| `StopSpawns`, `ResumeSpawns` | The zone's own spawns stop (director, floor, elites, swarms) and start again; the boss schedule goes on. |
| `Roster(only)` | From now on, for the run, only roster enemies of that creature type spawn (another of the same rank, unlocked, of the nearest cost, takes a refused one's place). |
| `RaiseDead(within, rise_as, look_spell)` | Every living enemy that is not undead within `within` yd of a hero dies, giving nothing, and rises at its spot as `rise_as` (a script acting on the horde). |
| `Shield(damage_taken, look_spell)` | The boss takes only `damage_taken` of every hit, wearing the look. |
| `Mortal(seconds)` | The shield is down for `seconds`. |
| `Place(object, near, count)` | An object is put down (`count` of them, fanned out as `Lay` does). |
| `LayZone(zone)` | A lasting zone round the boss. |
| `Kit(abilities)` | A phase change: these abilities in place of its own (first used at 60 % of their cooldown). |
| `Weapons(weapons)`, `Look(display_id, scale)` | It holds these weapons / takes this model (at `scale` if set) from now on. |
| `Kneel(seconds)` | It kneels: stands still and does nothing (a window to strike). |
| `Enrage` | It enrages now (as below 30 %). |
| `Pause(seconds, text)` | The scripted pause: for at most 5 s the whole fight freezes, the heroes (rooted), the enemies, the spawns and every timer, while `text` shows. Only Whitemane's revive and Hakkar's Blood Siphon use it. |
| `Later(seconds, run)` | These actions run `seconds` later, as the same boss; counted through a scripted pause too, so a cutscene can be timed (Whitemane's "Arise, my champion!" 3 s into it). |
| `On(boss, run)` | These actions run as the zone's boss of that name, if it is up (Whitemane's shield on Mograine, Ohgan's death enraging Mandokir). |
| `Call(boss)` | The zone's boss of that name comes now: one marked `called: true`, which the schedule passes by (Whitemane, Cho'Rush the Observer, Ohgan); not if it is up already, nor by a boss brought back by `Revive`. |
| `Revive(boss, health, look_spell)` | The zone's boss of that name, fallen, rises where it fell with `health` of its health, quietly (no arrival yell), wearing the look as it rises. |
| `Unshield` | The shield goes for good (after `Later`: a shield for some seconds). |
| `Heal(on, share, look_spell, beam_spell, beam_seconds, from)` | Heals `share` of each healed one's health, `look_spell` on it: `on` is `Myself`, `Near(yards)` (every enemy, bosses too, round the healer) or `Boss(name)`; the healer is the boss, each living roster enemy named `from` (the Nightmare Suppressors), or the object whose pulse it is. A unit healing shows `beam_spell` channelled to the one it heals for `beam_seconds` (2). It acts on the boss's own side only. |
| `Cast(ability)` | The boss casts this building block now, off its kit (Doan's Detonation); from an object's pulse, laid from the object. |
| `ClearZones` | Its lasting zones go (a form's aura, as Alzzin shifts). |
| `Wear(look_spell, seconds)` | It wears the look for `seconds` (Herod spinning while his Whirlwind lasts). |
| `Speed(yards)` | It moves at this many yards a second from now on (× the speed run option; Alzzin's wolf). |
| `Bolster(enemy, damage, speed, look_spell)` | Each living roster enemy named `enemy` (the boss itself when left out) hits `damage` times as hard and moves `speed` times as fast from now on, wearing the look; an add already wearing it is passed by (Loksey's Bloodlust), the boss grows each time (Mandokir's level-up). |
| `Siphon(..)` | Blood Siphon (Hakkar; a script acting on the horde): the boss drains every hero and every other enemy at once, a beam to each hero for `seconds`. A hero takes `damage_mult` × attack damage and heals the boss `heal_share` of its health, unless it carries the `poison_spell` stack: then the boss loses `poisoned_share` instead. Each enemy loses `enemy_share` of its health and heals the boss `enemy_heal_share`. |
| `Waves(waves, gap, immune, aloof, look_spell, bar, counter, then)` | A run of waves (`game::boss_phases`): each wave `(run, warn)` runs in turn, the next once every add the last one brought is dead, or `gap` seconds after it came (0: only once dead). Meanwhile with `immune` the boss takes no damage (wearing `look_spell`; hero spells take it last), with `aloof` it stands apart doing nothing, its bars and timed lines kept for later. A timer bar `bar` counts to the next wave ("Next wave 2/7"; the boss's name before it joins), a counter `counter` the adds left ("Guards left 5/8", a bar that does not count down). Once the last wave is done, `then` runs and it fights. Where its adds fell is kept (`RaiseFallen`). |
| `Gather(within, ahead, look_spell)` | Every enemy of the horde within `within` yd of the boss slides into one clump `ahead` yd in front of it and stands there 2 s (a burst window); never a hero (a script acting on the horde: Arcane Vacuum). |
| `SpellImmune(seconds, look_spell)` | For `seconds` the boss takes no spell damage (spells, their ticks, magic procs); weapon swings, pets and physical procs still hurt it. |
| `Empower(look_spell, faster, harder, max_stacks, seconds)` | A stack of rage: per stack its marked attacks, rushes, objects and zones come `faster` sooner and hit `harder` harder, and it moves `faster` quicker, up to `max_stacks`, for `seconds` (0: for good). Its swing and its unmarked abilities keep their pace (the damage a hero cannot avoid stays). |
| `Mend(by, share, look_spell)` | Each living roster enemy `by` heals the boss for `share` of its health, `look_spell` from it to the boss. |
| `ClassCall(calls, seconds)` | Each living hero whose class has a call `(class, warn, weakens, look_spell, run)` is announced `warn` and for `seconds` deals `weakens` less damage, wearing `look_spell` as a debuff; `run` runs too (adds round it). Never crowd control. |
| `RaiseFallen(rise_as, look_spell, most)` | At each spot where an add of its waves fell, `rise_as` rises (`most` at most). |
| `Cameo(enemy, at, delay, seconds, rise_spell, sink_spell, run, abilities)` | A figure rises for a moment: `delay` s later roster enemy `enemy` rises at `at` round the boss (or where it fell), playing `rise_spell`; it takes no damage, never moves or swings, runs `run` (its lines) and fights with `abilities` (at the boss's attack damage), then after `seconds` sinks (`sink_spell`), giving nothing. A `Death` line's yells come from where the boss fell, in place of the standard last words. |

Roster enemies only a boss brings (a wave, hatchlings) carry `only_summoned: true`: they never
spawn by themselves. Every enemy a boss names must be in its zone's roster (checked as the content
loads). Shown names (objects', warnings', emotes') take pointers such as `{Spell(6730)}`.

### Warnings and timer bars

Clear raid-warning style announcements, in our own words. `warn:
Some((text, colour, icon_spell, bar))` on an ability, a script line or an object: as it starts,
`text` shows big across the screen in `colour` (`Red` by default: move out; `Orange` adds;
`Yellow` something to do; `Purple` a phase; `Green` poison; `Blue`) with the icon of `icon_spell`
(the ability's own look when left out) on each side, and the client's raid warning sound. A warned
ability's next use, and an `Every` or `After` line's, count down as a timer bar labelled `bar`
(the text when left out) at the left side; under 5 s a bar moves up to the bigger, flashing
"soon" bars; a bar run out goes after 2 s. A `Health` line's `soon` text is announced 5 points
before it runs ("Lapdogs soon" at 55 %). The bars count the run's own time. Options (Timers)
turns the timer bars, the warnings and their sound off or on (all on at first; our own CVars
`esurvBossTimerBars`, `esurvBossWarnings`, `esurvBossWarningSound`). A client without our HUD gets the
warnings as the stock raid warnings. Each group of bars (the "soon" bars and the later ones) is moved by dragging its handle, a small strip over it that always shows ("Show bar handles" in Options → Timers; turned off, it shows only while Shift is held); the places are kept in `boss_bars.ron` in the state folder, as the meter keeps its window's, and "Reset bar positions" in Options → Timers puts both back. Each handle carries benilla's `OverlayWindow` marker, so a press on it is the handle's alone: without it the world took the press as a camera look, which hid and locked the cursor, and the group could not follow it. `ESURV_BAR_DRAG=<soon|later>@<seconds>:<dx>,<dy>` (a dev aid, `boss_bar_drag_check`) drags a group by its handle in game with the game's own cursor and button events, logging the place before and after; `ESURV_BAR_DRAG_SHOTS=<png>` adds a shot before and after. The Timers tab holds every boss alert setting: Boss warnings and Warning sound (untick to mute) on the left, Boss timer bars, Show bar handles and Reset bar positions on the right (`esurv-app` `boss_bar_places`).

### Damage a hero cannot avoid

A reworked boss keeps the damage a hero takes from it **whatever it
does**, a second, as it was: its swings once caught, its unmarked hits (stomp, charge, a volley's
shots on the hero's spot) and the stacks its blows leave (`esurv-sim` `game::boss_damage`). The
marked attacks, lasting zones and objects are avoidable and come on top, landing only on a hero
who fails to avoid them. Where WoW's attack is unmarked and ours is marked, the boss's swing takes
the difference: `swing_mult` (1 when left out). The test
`a_reworked_boss_keeps_the_damage_a_hero_cannot_avoid` checks every reworked boss, in every phase
of its kit, against its v0.2.2 kit (within 3 %).

## 7. Elwynn and Duskwood

The unavoidable damage a second (in the boss's attack damage D, as before the rework) and what a
hero who avoids nothing takes on top (the marked hits over their cooldowns; zones, stacks and
hatchlings not counted):

| Boss | Mechanics | Unavoidable | On top |
|---|---|---|---|
| Hogger | Head Butt (6730), an 8 yd, 70° cone, 1.2 s; a marked charge line run through (up to 25 × 4 yd), 1.5 s; at 50 % he howls and six Riverpaw Gnolls run in from the edges | 0.75 D/s (swing ×1.5) | 0.68 D/s |
| Mother Fang | three marked 3.5 yd web circles round the hero (1 D each) leaving webs where a hero stacks her poison (744, 0.05 D a stack a second, 5 stacks); two egg sacs near her hatch three Mine Spiders each after 10 s unless walked over | 0.75 D/s (×1.5) | 0.11 D/s, the poison up to 0.25 |
| Rhahk'Zor | his Slam (6304) marked 10 yd round him, 1.8 s, every 9 s; from 50 % every 6 s, 1.5 s | 1.0 D/s (×2) | 0.33, then 0.5 D/s |
| Mr. Smite | dual axes (vmangos 2183); his stomp (6432) a ring 4-12 yd (hug him); at 66 % and 33 % he kneels 2 s with his original lines and voices (5778, 5779); then his two-handed hammer (vmangos 10756) and Smite Slam (6435) in a 14 yd, 90° cone; from 33 % both, faster | 1.0 D/s (×2) | 0.3 / 0.38 / 0.97 D/s |
| Edwin VanCleef | Thrash (3391) marked 6 yd round him; thrown knives, a ring 5-14 yd; his ground burst as before; at 50 % "Lapdogs, all of you!" with his voice (5782) and six Defias Blackguards in a ring | 1.0 D/s (×2) | 1.2 D/s |
| Naraxis | three marked 3 yd web blobs (1 D) that become cocoons hatching three Black Widow Hatchlings after 8 s unless walked over | 0.75 D/s (×1.5) | 0.1 D/s |
| Mor'Ladim | four marked 3.5 yd graves round the hero (Raise Dead's look); below 30 % he enrages (faster, the graves 30 % more often) | 1.0 D/s (×2) | 0.38 D/s |
| Stitches | slow (4 yd/s: kite him); Aura of Rot round him (7 yd, 0.15 D a second); a marked 14 yd, 60° plague cone leaving poison puddles (0.2 D a second) | 1.0 D/s (×2) | 0.3 D/s, the aura 0.15 |
| Lupos | each bite stacks a shadow wound (589, 0.05 D a stack a second, 10 s, 5 stacks); every 20 s he howls and four Starving Dire Wolves run in from the edges; he fades and pounces through a marked line (up to 28 × 5 yd), 1.8 s | 1.0 D/s (swing ×1.5, bites 0.25) | 0.27 D/s |
| Morbent Fel | on arrival Raise Dead: every living enemy within 45 yd dies and rises as a Skeletal Raider, and only the undead spawn from then on; shielded (a tenth of each hit) until the hero walks over Morbent's Bane (the Torch of Holy Flame's model planted upright in the ground, about 1.5 yd tall, in Divine Shield's golden bubble standing on the ground round it, over Consecration's glow), then mortal 15 s, the relic dropping again 30 s later; Touch of Death (3108) a slow bolt at the hero's spot, its 4 yd mark 2.5 s; his ground burst as before | 0.89 D/s (×1.77) | 0.88 D/s |

Their Summon, Stomp, Charge and Volley went (unmarked); VanCleef's and Morbent's ground bursts
stay. The shield's share (a tenth), the zones' and stacks' damage and the wind-ups are ours and
open to tuning.

## 8. Scarlet Monastery to Gurubashi Arena

As section 7, for the zones 3 to 6 (`esurv-sim` `boss_rules_zones_3_to_6_tests`). Every area
attack is marked; none is a cast to interrupt; the only pauses are Whitemane's
revive and Hakkar's Blood Siphon. A boss with `swing_mult: Some(0.0)` never swings (Cho'Rush, a
caster); one fighting beside another is counted with it (Ohgan with Mandokir). Every boss also
enrages below 30 % (section 2); Herod's and Gordok's 25 % lines and Immol'thar's 30 % frenzy come
with their own words.

| Boss | Mechanics | Unavoidable | On top |
|---|---|---|---|
| Houndmaster Loksey | as he comes and every 20 s eight Scarlet Tracking Hounds in four packs from far-apart sides, each lane marked 2 s first, then Bloodlust (6742) on them (×1.3 damage and speed); a marked charge line run through (up to 25 × 4 yd) | 0.75 D/s (×1.5) | 0.25 D/s |
| Arcanist Doan | Arcane Explosion (9433) a ring 4-13 yd; at 50 % Arcane Bubble (immune) and Detonation (9435), a 15 yd circle for 2.5 s, "Burn in righteous fire!" (5843) | 0.64 D/s (×1.27) | 0.33 D/s, Detonation 4 D once |
| Herod | every 15 s Whirlwind: "Blades of light!" (5832), a 7 yd circle with him for 6 s (0.5 D a second); Rushing Charge (8260) run through a marked line (up to 28 × 5 yd); at 25 % "Light, give me strength!" (5833) and enrage; as he falls twenty Scarlet Trainees, "The master has fallen! Avenge him, my brethren!" (5834) | 1.0 D/s (×2) | 0.25 D/s, the circle 0.5 |
| High Inquisitor Fairbanks | Curse of Blood (8282) on each blow (0.04 D a stack a second, 5 stacks); three marked 4 yd Holy Smites round the hero; every 5 s green beams (13540) heal the enemies within 15 yd of him a twentieth; every 18 s Power Word: Shield (11647), 0.3 of each hit for 6 s | 0.89 D/s (×1.37, curse 0.2) | 0.33 D/s |
| Scarlet Commander Mograine | Crusader Strike (14518) in a 10 yd, 70° cone; he falls (half a final boss's health) and calls Whitemane | 1.0 D/s (×2) | 0.38 D/s |
| High Inquisitor Whitemane | comes as he falls (5838); the scripted pause 5 s (Deep Sleep), 3 s in "Arise, my champion!" (5840) and Mograine rises at half his health, then "At your side, milady!" (5837); every 14 s Power Word: Shield (22187) on him (0.3 for 5 s), every 8 s a heal beam on him (a twenty-fifth); Holy Smite marks; 0.75 of a final boss's health | 0.64 D/s (×1.27) | 0.38 D/s |
| Atal'alarion | Sweeping Slam (12887) in a 12 yd, 80° cone; Ground Tremor (6524) a ring 5-14 yd, 1.8 s | 1.0 D/s (×2) | 0.65 D/s |
| Jammal'an the Prophet | two Flamestrikes (12468) round the hero leaving fire (0.25 D a second, 6 s); every 20 s a healing totem by him (4 % to every enemy within 12 yd every 3 s), every 22 s an Earthgrab Totem near the hero that lays a ring 2.5-9 yd round itself every 4 s (1.5 D); both broken by walking over them | 0.64 D/s (×1.27) | 0.3 D/s |
| Morphaz | Acid Breath (12884) in a 16 yd, 60° cone leaving acid pools (0.2 D a second, 8 s); Wing Flap (12882) a ring 4-12 yd | 1.0 D/s (×2) | 0.63 D/s |
| Avatar of Hakkar | immune while four blood fires (Eternal Flame) burn round him: walked onto, each goes out, and with the fourth his shield falls; Lash (6607) along a 25 × 4 yd line | 0.64 D/s (×1.27) | 0.33 D/s |
| Shade of Eranikus | Thrash (3391) marked 6 yd round him; every 10 s a green Nightmare cloud drifting after the hero (5 yd, 2 yd/s, 0.3 D a second, 14 s); every 25 s three Nightmare Suppressors, each healing him 1 % every 3 s with a beam (12623) | 0.89 D/s (×1.77) | 0.38 D/s |
| Zevrim Thornhoof | vmangos's model 11335; Intense Pain (22478) a 9 yd burst round him; every 20 s a sacrifice altar by him: stood on within 8 s it breaks, else it heals him a tenth | 0.64 D/s (×1.27) | 0.33 D/s |
| Alzzin the Wildshaper | tree form (13173) with a withering aura (6 yd, 0.12 D a second) and Wither marks round the hero; at 66 % a wolf (2352), 9 yd/s, with a charge line run through (up to 25 × 4 yd) every 8 s; at 33 % his satyr shape, four 3.5 yd shadow marks (2 D) every 6 s | 0.64 D/s (×1.27) | 0.33 / 0.38 / 0.33 D/s |
| Tendris Warpwood | two seeds at marked spots (1 D) that sprout an Ironbark Protector (vmangos's model 13489) after 8 s unless walked over; Trample (5568) 9 yd round him; Grasping Vines (22924) along a 22 × 3 yd line | 0.75 D/s (×1.5) | 0.68 D/s |
| Immol'thar | Infected Bite (16128) on each blow (as Fairbanks's curse); two Eyes of Immol'thar as he comes, three more every 20 s, each firing a marked 30 × 3 yd laser line at the hero every 7 s; Trample 9 yd; at 30 % his frenzy | 1.14 D/s (×1.87, bite 0.2) | 0.73 D/s (an Eye's line) |
| Cho'Rush the Observer | comes as King Gordok reaches 50 %; a caster that never swings: Arcane Explosion (13745) a ring 3-11 yd, three marked 4 yd bolts round the hero; 0.35 of a boss's health; killing Gordok wins without him | 0 | 0.71 D/s |
| King Gordok | War Stomp (16727) 10 yd round him; Berserker Charge (22886) run through a marked line (up to 25 × 5 yd); at 50 % Cho'Rush joins, at 25 % he enrages | 1.14 D/s (×2.27) | 0.58 D/s |
| High Priestess Jeklik | a bat (15191): Sonic Burst (23918) a ring 3-12 yd, six Bloodseeker Bats from the edges every 15 s; at 50 % her troll shape: the swarms stop, two Gurubashi Bat Riders come (two more every 25 s), each throwing a fire bomb (23970) every 6 s, the first at the hero's spot, the others 3.5-7 yd round it (4 yd, 2 s, 2 D, leaving fire 0.25 D a second); silent | 0.64 D/s (×1.27) | 0.33, then 0.63 D/s |
| High Priest Venoxis | Holy Nova (23858) a ring 3-12 yd; Holy Fire (23860) a slow bolt at the hero's spot, its 4 yd mark 2 s; at 50 % the snake (15102) with "Let the coils of hate unfurl!" (8421), four Razzashi Cobras, two poison clouds (23861) round the hero that stay 15 s (0.25 D a second), Venom Spit (23862) in a 14 yd, 40° cone | 0.89 D/s (×1.77) | 0.71, then 0.53 D/s |
| Bloodlord Mandokir | comes with Ohgan (0.7 and 0.3 of his health); Whirlwind (13736) 8 yd round him; Charge (24408) run through a marked line (up to 28 × 5 yd); each life the hero loses to him: "DING!", ×1.1 damage (24312); Ohgan killed first enrages him | 1.0 D/s with Ohgan (×1.25 and ×0.75) | 0.55 D/s |
| Jin'do the Hexxer | three Delusions (24306) marks round the hero; every 20 s a Powerful Healing Ward by him (3 % to every enemy within 30 yd every 3 s), every 22 s a Brain Wash Totem near the hero that marks the hero's spot every 4 s (1.5 D); both broken by walking over them; two Shades of Jin'do from the edges every 20 s | 0.64 D/s (×1.27) | 0.33 D/s |
| Hakkar the Soulflayer | Corrupted Blood (24328) a slow bolt at the hero's spot leaving a patch (0.3 D a second, 8 s); two Sons of Hakkar every 20 s; each Son that dies leaves poisonous blood (24320, 20 s) that poisons a hero walking through it (24321, 30 s); every 35 s Blood Siphon: the scripted pause 4 s, he drains the hero (3 D, heals him 8 %) and every enemy (half its health, 0.2 % to him each); a poisoned hero's blood hurts him 8 % instead | 0.89 D/s (×1.77) | 0.33 D/s, the Siphon 3 D unless poisoned |

The old Summon, Stomp, Charge, Volley and ground bursts went. The wind-ups, the zones', stacks',
heals' and shields' numbers, the siphon's shares and Cho'Rush's health share are ours and
open to tuning. Mograine and Whitemane together have the slot's 1.5 × a final boss's health, as
Mandokir and Ohgan together have his. The stand-in looks (the lanes, the totems' models 653, 4718 and 652, the
seed 391, the altar 227) are the data document's.

## 9. Stratholme, Blackrock Mountain and Ahn'Qiraj

As section 7: the unavoidable damage a second (D, as before the rework) and what a hero who avoids
nothing takes on top from the marked hits over their cooldowns (zones, stacks, adds not counted).
Unvoiced lines are text only; voiced ones say what the voice says.

| Boss | Mechanics | Unavoidable | On top |
|---|---|---|---|
| Magistrate Barthilas | Mighty Blow (14099) a marked 10 yd, 70° cone, 1.2 s, no knock back; Crowd Pummel (10887) a 10 yd circle round him, 1.8 s; every 20 s Furious Anger (16791) stacks (up to 5): his marks 10 % sooner and 10 % harder a stack, and he runs faster | 0.75 D/s (swing ×1.5) | 0.71 D/s, more with stacks |
| Baroness Anastari | Banshee Wail volleys (16565); Possess: she fades (24699) and dives through a marked line (up to 28 × 5 yd), 1.8 s; no silence, curse or mind control | 0.64 D/s (swing ×1, volleys) | 0.25 D/s |
| Nerub'enkan | each blow stacks Pierce Armor (6016, 0.05 D a stack a second, 5 stacks; damage only); Encasing Webs (4962) three marked 3.5 yd webs round the hero (1 D) leaving webs that stack a poison (744, 0.05 D a stack a second, 5 stacks); every 20 s six Crypt Scarabs round him; no root | 0.75 D/s (swing ×1, stacks) | 0.08 D/s, the poison up to 0.25 |
| Maleki the Pallid | Frostbolt (17503) a bolt at the hero's spot, its 4 yd mark 2 s, no slow; Ice Tomb (16869) a 5 yd circle on the hero, 1.8 s, no stun; from 60 % Drain Life (17238) heals him 4 % every 15 s | 0.75 D/s (×1.5) | 0.63 D/s |
| Ramstein the Gorger | the final slot's first: the Slaughterhouse door opens at 19:57 and he comes out of it at 20:00, the zone's spawns stopping, the Baron's line shown with his name; Trample (5568) 9 yd round him; Knockout (17307) a 10 yd, 90° cone, no knock back or stun; 0.3 of a final boss's health, at the final size; as he falls the Baron comes | 0.89 D/s (×1.771) | 0.63 D/s |
| Baron Rivendare | called by Ramstein's fall, 0.7 of a final boss's health; apart and immune in the doorway while 34 Mindless Undead pour in at the square's south gate, then (60 s later, or once all are dead) five Black Guard Sentries come out round him, "Who dares disturb our master?", his own door opening behind them; with the last dead, "Time to take matters into my own hands..." and he fights: Cleave (15754) a 10 yd, 90° cone; Shadow Bolt (17393) at the hero's spot (4 yd, 2 s); Unholy Aura (17467) 7 yd round him (0.15 D a second); every 15 s Raise Dead: six Mindless Skeletons, and 12 s later each one still up heals him 1 % (Death Pact, 17471); the spawns come back as he falls (Endless brings Ramstein back, and the event with him) | 0.89 D/s (×1.771) | 0.68 D/s, the aura 0.15 |
| Angerforge | Flurry (15088, Cleave's look) a marked 10 yd, 90° cone; at 30 % (as in WoW) eight Anvilrage Reservists from the edges and two Medics, each healing him 1 % every 5 s (Heal 15586) while it lives | 1.0 D/s (×2) | 0.38 D/s |
| Rend | the Spire's five waves first (whelps, dragonspawn, handlers, Blackrock Grunts; the next once one is dead or after 30 s), Rend apart and immune; then on Gyth (display 9723): flame breath (16390) a 16 yd, 60° cone leaving fire (Flamestrike's ground, 8 s, 0.25 D a second); at 50 % knocked off (his own model): Whirlwind (13736) a 7 yd circle that follows him 6 s (0.5 D a second), and his cleave a 8 yd, 90° cone | 1.0 D/s (×2) | 0.33, on foot 0.38 D/s and the whirlwind |
| Thaurissan | Princess Moira comes with him and heals him 2 % every 6 s while she lives (kill her or burst through); Avatar of Flame (15636) a ring 4-13 yd; Hand of Thaurissan (17492) a hammer flying at the hero's spot, its 4 yd mark 2 s | 0.89 D/s (×1.771) | 0.65 D/s |
| Majordomo | immune (Aegis' look 20620) until his eight Flamewakers (4 Elites, 4 Healers, no damage shield) are dead, "Guards left n/8"; his Fireball volleys as before; flames at the hero's feet (20618), 4 yd; beaten: "I submit!" (8038), and 4 s later Ragnaros rises (20568), immune and still, "TOO SOON! ..." (8043), brings down one marked 9 yd Sulfuras smash (20566, 2 s, 3 D) and sinks 8 s after rising | 0.89 D/s (×1.5, volleys) | 0.33 D/s; the smash once |
| Nefarian | arrives with Nefarius's "Let the games begin!" (8280); phase 1: four drakonid waves from the edges (the next once one is dead or after 25 s), he apart and immune; lands with "Well done, my minions..." (8288); Shadow Flame (22539) a 20 yd, 60° cone; his Shadow Bolt volleys; every 30 s a class call (text only): the hero's class deals half damage for 10 s (a warlock: three Corrupted Infernals round it instead), never crowd control; at 20 % "Impossible! Rise my minions!" (8291) and up to 16 Bone Constructs rise where the drakonids fell; his death line (8292) | 0.89 D/s (×1.5, volleys) | 0.3 D/s |
| Kurinnaxx | each blow stacks Mortal Wound (25646, 0.05 D a stack a second, 5 stacks); a sand trap marked under the hero (4 yd, 1.5 s) that stays 12 s (0.3 D a second); at 30 % he enrages | 0.75 D/s (×1, stacks) | 0.11 D/s, the traps 0.3 |
| Rajaxx | seven waves, each led by its captain (Qeez, Tuubid, Drenn, Xurrem, Yeggeth, Pakkon, Zerran) with three Qiraji Warriors and three Swarmguard Needlers (Zerran four and four), the next once one is dead or after 20 s, a timer bar each; his rally lines with his voice on waves 2-6 (8641-8645); he joins with "Impudent fool! I will kill you myself!" (8638); Thundercrash (25599) a 15 yd circle round him, 2.5 s | 1.0 D/s (×2) | 0.25 D/s |
| Huhuran | her poison bolt volleys (26053) as before; each blow stacks Acid Spit (26050, 0.05 D a stack a second, 5 stacks); two 4 yd poison clouds (Poison Cloud's look 23861) marked round the hero that stay 15 s (0.25 D a second); every 20 s a frenzy: 8 s of clouds 50 % sooner and 30 % harder (never her volleys or swing); at 30 % berserk; no sting | 0.89 D/s (×1, volleys, stacks) | 0.08 D/s, the clouds 0.25 |
| Ossirian | Supreme (25176): a tenth of each hit, until the hero walks over the crystal (6486) a little away: weak 30 s, the next crystal 35 s after it was taken; War Stomp (25188) a 10 yd circle; sand tornadoes (25160) marked at him that wander after the hero at 2.5 yd/s for 20 s (0.3 D a second) | 1.0 D/s (×2) | 0.43 D/s, the tornadoes 0.3 |
| C'Thun | the Eye creeps (3 yd/s); Dark Glare (26029): a marked 40 × 6 yd beam that, once it lands (2.5 s), sweeps 25° a second round him for 8 s (0.5 D every 0.5 s in it), he standing; Eye Beams (26134) chain over four 3 yd marks, each 0.5 s after the last; Eye Tentacles burst from three marked spots and stay there, Mind Flay (26143); at 50 % his body rises (15787): Claw Tentacles, a Giant Claw Tentacle under the hero and a Giant Eye Tentacle burst from marked spots and stay; no stomach | 0.89 D/s (×1.771) | 0.23, then 0.27 D/s; the glare 1 D/s |

Their Summon, Stomp, Charge and ground bursts went; the volleys stay where they were (Majordomo,
Nefarian, Huhuran; Anastari's Banshee Wail in Ivus's place). The wave counts and gaps, the heal
shares, the zones' and stacks' damage, the rage and frenzy shares, the class calls' half damage
and the wind-ups are ours and open to tuning.

**Stratholme.** Each Stratholme boss takes its slot's minute, numbers and the damage a hero
cannot avoid by the same recipe (Barthilas a swing and stacking rage, Anastari volleys,
Nerub'enkan stacks on each blow, Maleki a swing, Ramstein and the Baron a swing). The final slot is shared as Mandokir
and Ohgan share theirs (0.3 Ramstein, 0.7 the Baron), Ramstein a final boss
the schedule brings at 20:00, the Baron a called one; the run is won when the Baron falls. The end
event follows Stratholme's own (vmangos `instance_stratholme`): the Slaughterhouse door opens and
Ramstein comes out, the 34 Mindless Undead come as he falls, the five Sentries 60 s later and the
Baron's door opens with them; ours stops the zone's spawns from Ramstein's arrival to the Baron's
fall, the undead come in just inside the square's south gate (`Wave` at `Spot`, the gate itself
staying shut), the Slaughterhouse door opens on the run clock at 19:57 (`opens_at_minute`
19.95), the Baron's door by his script (`Door`), the Death Pact skeletons stay up (WoW kills
them), and the Baron's line before Ramstein is an emote with his name, as he is not up yet. Ours,
open to tuning: the 0.3 / 0.7 split, the wind-ups, Maleki's heal share and timer, the Death
Pact share, the adds' kinds and toughness, the six scarabs (WoW 4, 6 or 8). From WoW: the 34
undead, the 5 Sentries, the 60 s, the six skeletons every 15 s and 12 s later, the 10 yd Crowd
Pummel, the 10 s Knockout and Shadow Bolt, the spots. Dropped as crowd control or with no
building block: Barthilas's Draining Blow, Anastari's Silence, Banshee Curse and the mind control
itself, Nerub'enkan's root, Maleki's Drain Mana and every slow and stun, Ramstein's knock back and
stun, the Baron's Mortal Strike healing cut.
