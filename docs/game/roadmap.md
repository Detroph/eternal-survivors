# Roadmap: planned changes after version 1

This page collects the changes planned **after** version 1,
as rough concepts. For each one it records what the build must prepare now so the change is
easy later. Nothing here is built before version 1 unless decided otherwise. The details of
each concept will be specified when its time comes.

Plain-language version: `guides\roadmap.txt`.

## Concepts

### R1. A choice of three starting spells per class

**Idea:** at level 1 each class picks one of three spells instead of always starting with the
same one. Each choice feeds a new passive mechanic (R2).

**Where the choice happens:** on the **launch screen where class and race are chosen** (the
hero picker), **before** the hero loads into the map. The player can read each spell and its
mechanic there and picks one. It is not a choice made after entering the world.

**Touches:** classes (`starting_ability` becomes a list of three choices, each with its
mechanic and a description), the hero picker (a spell-choice row with readable descriptions),
the hand-over from picker to server (the chosen spell travels with the hero into the run),
talents (several talents say "you start every run with X": these must still make
sense when the starting spell is a choice).

**Prepare during the build:**
* Give the hero picker a spell info panel and spell tooltip box driven by data, so three
  choices with descriptions are a data change.
* Keep "the spells a run starts with" as data on the class, not code.
* Plan how a picker choice reaches the server before the hero enters the world (it is part of
  the hero, like class and race).
* Pin down exactly how "start every run with X" talents apply (milestone `StartWith`), so the
  rule can be extended rather than rewritten.

### R2. Class mechanics: resources and procs

**Idea:** each starting spell comes with a class mechanic. Examples: warrior **rage**, generated
by melee auto-attacks and spent by the level-1 spell; mage passives per level-1 spell, such as
**Hot Streak** for Pyroblast or **Fingers of Frost** for an ice spell.

**Touches:** the hero (a resource bar: rage, energy, mana, or a custom one), auto-attacks,
abilities (costs, and spells that consume a resource or a proc), the client display.

**Prepare during the build:**
* **An event system in the simulation:** every melee swing, cast, hit, crit, kill and damage
  taken raises an event that a mechanic can listen to. The milestone talents
  already work this way in spirit ("every fourth Fire crit casts a free Pyroblast",
  "a crit makes the target bleed"), so this is needed for version 1 anyway. It must stay
  general, not hard-coded per talent.
* **A "mechanic" slot on the class:** a mechanic id plus parameters in data, with the
  behaviour in one module per mechanic.
* **Resources:** the class record already has a power type (mana, rage, energy). Decide whether
  power is used in play or is only cosmetic.
  The 1.12 client can already show a rage, energy or mana bar from the server's unit fields.
* **Procs on screen:** 1.12 has no spell-glow overlays; a proc is shown as a buff icon on the
  hero. Plan mechanics around buffs and auras.

**Watch out:** spells and icons that do not exist in 1.12 (Ice Lance, Hot Streak and Fingers of
Frost are later expansions) need a 1.12 spell visual and icon as stand-ins. Every visual must
come from the player's own 1.12 data (the installer's asset list).

### R3. Weapon loadouts

**Built (2026-10-19), the system; its looks are placeholders.** Three loadouts per class with
their own strengths and drawbacks (`content\loadouts.ron`; classes.md "Weapon loadouts",
AUTHORING.md 8b): swings per hand from the loadout (two-handers 3.6 s, one-handers 1.8 s, dual
wield with an off hand at 50 %, the Hunter's bow, crossbow or gun setting Auto Shot and its
shot look), block, stats and swing procs (maces stun, swords strike again), rage per swing;
Druid forms keep their swing and get every bonus (Cat and Bear show no weapon, Moonkin's put
away). Chosen per class on the warband screen under the starting spell (screens.md), kept in
the save (`loadout`; an older save takes the first), sent with the class's progress; the hero
and the character list show the loadout's weapons. Compared with `balance_check --loadouts`.
**Still to come:** each zone's own weapon looks and the "Armaments of <zone>" drops (the gear
rework), the look choice per loadout in the armory, and a loadout line on the hero sheet.

**Idea:** before a run, pick one of three weapon loadouts per class. Each has strengths and
drawbacks and suits one talent spec best, but all work with any spec. Paladin example:

| Loadout | Weapons | Effect |
|---|---|---|
| 1 | two-hander | 3.6 s swing, +100 % melee damage |
| 2 | one-hander + shield | 30 % chance to block a melee blow (a blocked blow does 30 % less damage; thorns/reflect from spells still happen), −10 % damage done |
| 3 | one-hander + off-hand (not a shield) | −10 % cooldowns on all spells, +1 target for spells that hit several |

**Where the loadout is chosen:** before the run, and also in the **character gear screen**
(the armory), where armor and armor appearances are selected and equipped. The loadout is
picked there like the rest of the hero's equipment.

**Weapon appearances drop per zone, as one bundle:** weapons never drop on their own. A weapon
drop is called **"Armaments of <name>"** and unlocks **every weapon appearance for the hero's
class from that zone at once**: the looks for all three loadouts. Example: a run with the
two-handed hammer loadout gets a weapon drop, and the hero also unlocks the matching one-hander
+ shield and one-hander + off-hand looks from that zone. The armory then lets the player pick
which unlocked look each loadout shows, **among looks that fit that loadout**: a loadout's
look must match its weapon types (a one-hander + shield loadout can only show a one-hander
and a shield, never the two-handed hammer).

**Touches:** the pre-run screens and the armory (loadout choice and weapon appearance choice),
auto-attacks (swing time and damage), the hero's stats (new ones such as block chance), gear
(weapon appearances per class **per zone** and per loadout, the "Armaments of ..." bundle drop,
appearance unlocks saved with the hero), all damage paths (block, reflect).

**Prepare during the build:**
* **One modifier vocabulary for everything:** talents, gear, rings, amulets, run options,
  loadouts and class mechanics should all express their effects as the same kind of stat
  modifiers (e.g. "+100 % melee damage", "−10 % cooldowns", "+1 target"), applied by one
  stat-sheet pipeline. The game already has most of these stats (damage, cooldown
  recovery, extra targets per ability, double-cast, thorns...). Extending the list with new
  stats (block chance, swing time) must be a data change plus one rule.
* **Auto-attack as data:** in version 1 only warrior, paladin and rogue have a melee
  auto-attack (damage, swing time, range on the class). Keep it as data so a loadout can
  override it and so other classes can gain one.
* **Weapon visuals:** the hero's look comes from item display ids per slot; a loadout picks
  main hand / off-hand / shield display ids and sheath types, which the gear structure already
  contains.
* **Appearance separate from power, but type-checked:** keep "which look is shown" separate
  from "what the piece does", so a loadout's look can be swapped without changing its effect.
  Each loadout slot has a weapon type (two-hander, one-hander, shield, off-hand), and only looks
  of that type can be chosen for it. The armory's appearance options (such as "Starting
  clothes" and "Hidden") are to be settled.
* **Unlocks saved with the hero:** the save format records which appearances a hero has
  unlocked, so bundles like "Armaments of ..." are just a list of looks to add.
* **Weapon looks per zone:** structure weapon appearances as class × zone × loadout slot
  (main hand, off-hand, shield), so a zone's armaments bundle is one data entry.
* **Room for three weapon sets:** every class gear set holds three
  weapon sets of two slots each (2 + 2 + 2: main hand and off-hand or shield, or a two-hander and
  an empty slot), so 8 armor slots + 6 weapon slots. The item numbering and the saves have room
  for them (slots 8 to 13: `main_hand`, `off_hand`, `main_hand_2` ...; gear.md, section 3).
* Settle how the gear weapon line, its drops (tier vs zone) and the class auto-attack
  interact, so loadouts slot in without breaking boss gear drops.

### R4. Faction banner pickup: summoned champions

A new field pickup alongside the runes and magnets: a **faction banner** of the hero's
faction (Alliance or Horde, from the hero's race). Taking it summons a **champion**: an NPC
drawn 1.5× size, picked from a pool the user will define. Each champion has its own look,
entrance, yell and action.

Example (user's): **Wayze**, a dwarf rogue in tier 2 with Tigereye goggles, Thunderfury and
Felstriker. He appears as if he had just used Vanish (its sound and smoke cloud), yells "Kick
here, stun this, bam bam bam!" and casts Kidney Shot on every enemy on screen, stunning them
for 5 s.

**Shape in the data:**

* A pickup kind `FactionBanner` next to the runes (`game::loot::Pickup`), with its own spawn
  rule (how often, how far). Its look depends on the hero's faction.
* `content\champions.ron`: one block per champion, with:
  * faction, or both;
  * look: race, gender, display or equipment display ids per slot, and scale (1.5);
  * entrance: a spell visual and sound;
  * yell text;
  * a list of actions, built from the same effect pieces the game already has (stun or root
    every enemy on screen, damage, heal, buff the hero);
  * how long it stays.
* Champions act through the shared event and effect system, so a new champion is data, not
  code, unless it needs a new kind of action.

**Needs from version 1:** the pickup list, crowd control on enemies (stun with a duration), a
"spawn a scripted NPC with a look" path in the embedded server, and the hero's faction (known
from the race).

### R5. Battlegrounds fight the other faction

The battleground zone **Warsong Gulch** (Arathi Basin was one, until it became the Sunken Temple)
gets **two rosters**: Horde enemies when the hero is Alliance, Alliance enemies when the hero is
Horde.

**Shape in the data (built):** a zone writes `by_faction: Some((vs_alliance: ..., vs_horde: ...))`,
each a roster of enemies and bosses (zones.md, "Rosters by faction"). A run picks the roster
from its party's faction at its start; zones without the field keep one roster. **Still to do:**
Warsong Gulch's two rosters.

**Needs from version 1:** the hero's faction available to the run setup, and zones loaded so a
roster choice can be added without changing the spawn director: the director only sees "the
roster", whichever one was picked.

### R6. Multiplayer: one player hosts, others join

One player hosts a run; friends join it from their own copy of the game and fight in the same
arena. The game already runs its own server inside the executable (the client connects to it
like to any 1.12 server), so hosting means letting that server accept other clients over the
network instead of only the local one.

**Shape:**

* **Host and join:** the host's game server listens on a port; joining players' clients connect
  to the host's address instead of their own server. Each player keeps their own hero, class,
  talents and gear from their own save.
* **Shared run, separate heroes:** one zone, one enemy population, one spawn director, one
  clock; each hero has their own health, level, spells, cards, jewelry and amulet.
* **Difficulty scaling with player count** (rules still to be designed): for example
  enemy health, density and boss health per extra player. The game already has a
  run-wide difficulty factor on zone difficulty (`R`, always 1 in single player) and keeps run
  options per hero: natural hooks for this.
* **Open questions for later:** who enemies target (nearest hero?), whether pickups and
  experience are shared or per player, how level-up and cache screens work without pausing
  everyone (the single-player game pauses), revives, and how loot and talent experience are
  split at the end.

**Needs from version 1 (the big one):** nothing in the simulation may assume a single hero.
Heroes live in a list; enemies pick a target hero; every event (hit, kill, pickup, level-up,
milestone) names the hero it belongs to; the run's pause is a run-wide state the screens ask
for. The embedded server's listen address must be configurable, and the world protocol must
already handle several player objects (it is the normal 1.12 protocol, which does). Version 1
still plays exactly the same with one hero.

### R7. Gamepad and touch input

Playing with a gamepad, and on a touch screen, instead of keyboard and mouse. Version 1 is played with
keyboard and mouse.

**Shape:** moving and aiming with a stick or a drag, the level-up and cache cards picked with
buttons or taps, the Escape menu and the menus before a run walkable with a pad (focus moving
between buttons), and touch targets large enough for fingers.

**Needs from version 1:** every screen of ours acts through named actions (choose a card,
reroll, start a run, pick a hero) rather than through mouse clicks in place, so a pad or a
touch layer can send the same actions later; the in-process scripts the dev switches already
use (`ESURV_MENUS_SCRIPT`, `ESURV_MENU_PRESS`) work this way.

## What this means for the build now

These concepts share four foundations. Building them properly in version 1 makes these
concepts mostly data and small modules later:

1. **An open stat and modifier pipeline:** every source of power (talents, gear, jewelry, run
   options, cards, later loadouts and mechanics) produces modifiers of one shared kind.
2. **An event and proc system:** milestones, and later class mechanics, react to swings, casts,
   hits, crits, kills and damage taken.
3. **Reusable choice screens:** the 3-option offer screen (level-ups, caches) and the
   pre-run screens: the hero picker (later the start spell and its mechanic), Run Options and
   the armory (later loadouts and weapon appearances).
4. **Many heroes from the start (R6):** the simulation holds a list of heroes, not one; enemy
   targeting, events, pickups, level-ups and pauses are per hero or run-wide, never "the
   hero"; the embedded server can listen beyond the local machine.

These change **how** step 2 and step 6 are built, not what version 1 does: version 1 still
plays the same.

## Open questions

* Is the class power type (rage, energy, mana) used in play at all?
* How do auto-attacks, the weapon line of class gear and gear stats interact?
* What can the armory change about a hero's look ("Starting clothes", "Hidden")?
* Do gear drops depend on the zone, the tier, or both?
* How exactly are "start every run with X" talents applied?
* Is there a single offer routine shared by level-ups and caches?
* Faction (answered): the hero is a real player unit of its race. The server sends race, class
  and gender in `UNIT_FIELD_BYTES_0` and the race's standard faction template (human 1, orc 2,
  dwarf 3, night elf 4, undead 5, tauren 6, gnome 115, troll 116), so Alliance or Horde is
  known from the race. Nothing else in the game uses it yet.
