# Classes, abilities and talents: reference

How heroes are defined, how their spells work in combat, and how to add or change them. Data:
`content\classes.ron`, `content\races.ron`, `content\abilities.ron`, `content\talents.ron`. Types:
`crates\esurv-content\src\schema.rs`. Rules: `crates\esurv-sim\src\game\{abilities,stats,offers,level}.rs`.

Status markers: **confirmed** (settled and built), *inferred*, **pending**.
Plain-language version: `guides\classes.txt`.

## 1. A class

```ron
(
    id: "warrior",
    name: Class(1),             // a name the client holds is a pointer, read from the player's
                                // client (ChrClasses.dbc id); our own name would be text
    wow_class_id: 1,            // ChrClasses.dbc id: the client shows class name, icon, bars by it
    power: Rage,                // Mana, Rage or Energy (shown by the client; use in rules: pending)
    color: "#C79C6E",           // not read yet: the menus use the stock colour of wow_class_id
    icon: "INV_Sword_27",       // not read yet: the class picker uses the stock class icon
    description: "…",
    base_health: 240.0,         // level-1 max health before bonuses
    base_damage_reduction: 0.2, // damage reduction before bonuses (capped at 60 % with them)
    base_lifesteal: 0.02,       // lifesteal before bonuses
    auto_attack: Some((damage: 28.0, range: 6.0)),   // a one-hand swing's damage, or None; the
                                                     // swing time is the weapon loadout's
    abilities: ["cleave", "whirlwind", …],   // the 10 spells level-up cards can offer
    starting_options: [ ... ],               // picked before a run (AUTHORING.md 8a)
    spell_damage_scale: 1.0,                 // multiplies this class's spell damage only (1 if left out)
)
```

**Class base stats**, by armor type:

| Armor | Classes | Base health | Damage reduction |
|---|---|---|---|
| plate | Warrior, Paladin | 240 | 20 % |
| mail | Hunter, Shaman | 220 | 15 % |
| leather | Rogue, Druid | 200 | 15 % |
| cloth | Mage, Priest, Warlock | 180 | 8 % |

Lifesteal: Rogue 3 %, Warrior 2 %, Paladin, Priest and Warlock 1 %, the others none. Weapon
swings: the class's `auto_attack` `damage` is a one-hand swing's (Warrior 28, Paladin 27, Rogue
14, Shaman 27, Hunter's Auto Shot 40.5); the swing time and the damage share come from the
weapon loadout (below). The next swing comes after √(swing factor) × the swing time: talents,
rings and trinkets speed it at their full cooldown recovery though they give spells only
half (`spells.md` 5a). Slice and Dice adds +20 % attack speed while it
is up, and Hand of Justice a chance for a swing to hit twice (`gear.md`).

### Weapon loadouts

Each class
has three weapon loadouts (roadmap R3; `content\loadouts.ron`, AUTHORING.md section 8b), chosen on the
warband screen under the starting spell (screens.md) and kept per class in the save; the first
is the default (an older save, or a loadout the class no longer has). A loadout's effects join
the hero as the run begins, in the talents' vocabulary.

* **Swings:** for Warrior, Paladin, Shaman, Rogue and Hunter the swing (time and damage) is the
  loadout's: two-handers 3.6 s for twice the one-hand damage, one-handers 1.8 s, dual wield a
  main hand and an off hand at a share of its damage (Warrior 2.6 s + 2.4 s at 40 %, Rogue 50 %), each hand on its own timer (and each hand's hit
  gives its rage). The Hunter's bow, crossbow or gun sets Auto Shot's time and damage, and the
  shot's look (a bow's draw, a rifle's for gun and crossbow).
* **Casters'** weapons never swing; they change stats only. **Druid forms** get every bonus and
  keep their own swing; Cat and Bear show no weapon, Moonkin shows them put away. **Sheath holds**
 : Moonkin keeps its weapons put away all run and a shaman with the
  two-hander keeps it in hand all run, casting too; the server tells the client (`ESURVI`,
  `SheathHolds`) and the client's swings, casts and clips leave a held hero's weapons where they
  are (a deviation from the stock client). Code: `esurv-server` `starting_looks::sheath_hold`,
  `benilla-app` `creature_anim::SheathHolds`.
* **Block:** a shield loadout blocks 30 % of melee blows, a blocked blow doing 30 % less.
* **Procs:** two maces: 10 % of swings stun for 1 s, +10 % damage to stunned (and other held)
  enemies; two swords: 10 % of swings strike again.
* **Rule (owner, our rule):** two upsides and one downside per loadout, every value 10 %, 5 % or 2 %; the weapon's own traits (swing times, off-hand shares, ×2 two-hander swings, Auto Shot speeds, 30 % block) sit outside it; within a class the three stay within about ±10 % of each other's damage.
* **Looks:** the hero shows its loadout's weapons in main hand, off hand
  and ranged, in the armaments of the class gear weapon it wears: every zone has a weapon look for
  each loadout (`loadouts.ron` `looks`; Blackrock and Stratholme two rows), and one "<zone>
  Armaments" drop unlocks the zone's looks for all three loadouts (gear.md, section 3). Elwynn's
  without one. Stratholme's armaments mix Stratholme and Scholomance
  drops, row 0 mostly Stratholme's, row 1 ("Scholomance sets") mostly Scholomance's.

| Class | Loadout (id) | Weapons | Strength | Drawback |
|---|---|---|---|---|
| Warrior | Two-hander (`two_hander`) | two-handed sword 3.6 s | ×2 swing damage, +10 % rage per swing, +5 % crit damage | −10 % max health |
| | Two one-handers (`dual_wield`) | swords 2.6 s + 2.4 s (40 %) | +5 % swing speed, +10 % crit damage | −10 % damage |
| | One-hander and shield (`shield`) | sword 1.8 s, shield | 30 % block, +10 % max health, +10 % thorns | −5 % damage |
| Paladin | One-hander and off-hand (`off_hand`) | mace 1.8 s, off-hand | +5 % crit, +10 % heal and shield power | −5 % max health |
| | One-hander and shield (`shield`) | mace 1.8 s, shield | 30 % block, +10 % max health, +10 % thorns | −10 % damage |
| | Two-hander (`two_hander`) | two-handed mace 3.6 s | ×2 swing damage, +5 % damage | −5 % max health |
| Shaman | Staff (`staff`) | staff 3.6 s (one-hand damage) | +10 % spell area, +5 % crit | −5 % move speed (and the slow swing) |
| | Two-hander (`two_hander`) | two-handed mace 3.6 s | ×2 swing damage, +5 % crit | −10 % max health |
| | One-hander and shield (`shield`) | mace 1.8 s, shield | 30 % block, +10 % heal and shield power, +10 % max health | −5 % damage |
| Hunter | Bow (`bow`) | Auto Shot 2.2 s (90 %) | +5 % move speed, +2 % cooldown recovery | −10 % Auto Shot damage |
| | Crossbow (`crossbow`) | Auto Shot 3.4 s (160 %) | +10 % crit damage, +10 % damage to elites | −5 % move speed |
| | Gun (`gun`) | Auto Shot 2.7 s | +5 % crit, +10 % crit damage | −10 % pet damage |
| Rogue | Two daggers (`daggers`) | 1.4 s + 1.4 s (50 %) | +5 % crit, +10 % crit damage | −10 % max health |
| | Two maces (`maces`) | 2.6 s + 1.8 s (50 %) | 10 % swing stun 1 s, +10 % damage to held enemies | −5 % energy regeneration |
| | Two swords (`swords`) | 2.4 s + 1.8 s (50 %) | 10 % swing strikes again, +5 % swing speed | −5 % max health |
| Druid | Mace and off-hand (`mace_off_hand`) | mace, off-hand | +10 % heal and shield power, +2 % cooldown recovery | −5 % max health |
| | Two-hander (`two_hander`) | two-handed mace | +10 % damage, +10 % max health | −5 % move speed |
| | Two-handed staff (`staff`) | staff | +10 % spell area, +10 % spell duration | −5 % max health |
| Mage | Staff (`staff`) | staff | +10 % spell area, +5 % damage | −5 % cooldown recovery |
| | Sword and off-hand (`sword_off_hand`) | sword, off-hand | +10 % damage to elites, +10 % crit damage | −5 % move speed |
| | Dagger and off-hand (`dagger_off_hand`) | dagger, off-hand | +5 % crit, +10 % damage to held enemies | −5 % spell area |
| Warlock | Staff (`staff`) | staff | +10 % spell duration, +10 % damage to afflicted enemies | −5 % cooldown recovery |
| | Sword and off-hand (`sword_off_hand`) | sword, off-hand | +5 % crit, +10 % crit damage | −5 % move speed |
| | Dagger and off-hand (`dagger_off_hand`) | dagger, off-hand | +10 % pet damage, +2 % cooldown recovery | −5 % damage |
| Priest | Staff (`staff`) | staff | +10 % heal and shield power, +5 % damage | −5 % cooldown recovery |
| | Mace and off-hand (`mace_off_hand`) | mace, off-hand | +10 % damage reduction, +10 % heal and shield power | −5 % damage |
| | Dagger and off-hand (`dagger_off_hand`) | dagger, off-hand | +5 % crit, +10 % damage to afflicted enemies | −2 % cooldown recovery |

Compare a class's three with `balance_check --loadouts` (AUTHORING.md section 16).

**The hero's stats** (confirmed), from class, level `L` and every bonus:

| Stat | Start | Notes |
|---|---|---|
| max health | `base_health` + 9 per level after 1 | flat bonuses add first, then % bonuses multiply |
| damage | 1 + 3 % per level after 1 | + bonuses |
| cooldowns | ×1 | divided by (1 + cooldown-recovery bonuses) |
| crit chance / damage | 5 % / ×2 | crit chance above 100 % becomes extra crit damage |
| damage reduction | `base_damage_reduction` | capped at 60 % |
| lifesteal | `base_lifesteal` (rogue and hunter 3 %, warrior 2 %, paladin, priest and warlock 1 %, others 0) | + talent, amulet and ring lifesteal; every hit heals the hero for damage dealt × lifesteal, up to max health |
| health regen | 1 + 0.15 per level after 1 per s (our numbers) | |
| move speed | ×1 | capped ×2.2; base run speed 7 yd/s |
| pickup range | 10 yd (our number) | each bonus multiplies |
| experience, gold, area | ×1 | + bonuses |

Races add their own bonuses (section 1a).

**Levels.** Experience to go from level L to L + 1 is
`round(min(200 + 65·(L−1)^1.5, 20 000))` (200 at level 1, 265 at 2, 1955 at 10, 16 031 at 40, and
20 000 from level 46 on). A hero with no experience bonus is about level 40 at minute 16 in Elwynn;
level 60 asks for experience bonuses (the Human racial and an experience trinket, up to ×1.6). The
numbers (constant, factor, power and the cap `max`) are `content\balance.ron` `experience.level_cost`. What
each kill drops is in gear.md ("Experience orbs").
Confirmed: the cap is 99,
leftover experience carries over to the next level, and every level gives a 3-card choice and
heals 20 % of max health.

## 1a. Races (confirmed)

The hero is a real character of its race: the client shows that race's model, and the hero
unit carries its faction. Each race adds **passives** (lasting stat bonuses, folded into the
stat sheet like any other modifier) and a **race spell**, which sits in a slot of its own: it
casts by itself like learned spells, first after 0.5 s, retries after 0.5 s when it has nothing
to do, and is never offered on upgrade cards. Talents and the amulet still reach it through
their filters.

| Race | Faction | Race spell (listed numbers) | Passives |
|---|---|---|---|
| Human | Alliance | Perception: every 30 s, +15 % damage for 10 s | +10 % experience, +20 % gold |
| Orc | Horde | Blood Fury: every 40 s, +35 % damage for 12 s | +10 % max health |
| Dwarf | Alliance | Stoneform: every 45 s, a shield of 30 % max health | +5 % damage, +30 % pickup range |
| Night Elf | Alliance | Shadowmeld: below 35 % health, 3 s immunity (75 s) | +10 % move speed, +5 % damage reduction |
| Undead | Horde | Will of the Forsaken: below 35 % health, 4 s immunity (60 s) | +1 health per kill, +5 % damage reduction |
| Tauren | Horde | War Stomp: every 15 s, stuns enemies within 8 yd for 2 s | +15 % max health |
| Gnome | Alliance | Escape Artist: every 20 s, roots enemies within 8 yd for 2 s | +10 % cooldown recovery, +20 % luck |
| Troll | Horde | Berserking: every 35 s, +30 % cooldown recovery for 10 s | +2 health per second, +3 % crit |

The race spells are ordinary records in `abilities.ron`, so their numbers are changed there.
**They play exactly as listed**: the class's `spell_damage_scale`
never reaches them, so Perception comes every 30 s and Stoneform's shield is 30 % of max
health, as the tooltips say. The hero's cooldown recovery and talent or amulet modifiers still
apply.

```ron
(
    id: "dwarf",
    name: Race(3),                  // the client's name (ChrRaces.dbc id), read as the game loads
    wow_race_id: 3,                 // ChrRaces.dbc id: 1 human … 8 troll
    faction: Alliance,
    faction_template: 3,            // the hero unit's faction template
    spell: "stoneform",             // the race spell (an id in abilities.ron)
    passives: [
        (name: Spell(5626), stat: Damage, value: 0.05, text: "…"),   // the client's name
        (name: "Treasure Finding", stat: PickupRange, value: 0.3, text: "…",   // our own name
         stock_spell: Some(Spell(2481))),   // optional, see below
    ],
)
```

`stat` takes the same names as talents (section 4). A run's spellbook shows each passive as the
stock racial spell of its name ([server.md](server.md), "The
spellbook"); `stock_spell` points at that spell (`Spell(id)`) where its `Spell.dbc` name differs
from ours. A `name` the client holds is a pointer, `Spell(id)`, and our own is text (section 2);
`text` leaves the name out, as the tooltip shows it above. Code: `esurv-sim`
`game::stats::race_modifiers`,
`game::spells::Spellbook::start`.

## 1b. Race and class pairs

Besides every pair the stock 1.12 create screen offers, six more
can be made: **Dwarf Shaman, Human Hunter, Gnome Priest, Orc Mage, Troll Warlock, Undead
Paladin.** A race lists the classes it adds in `races.ron`:

```ron
(
    id: "dwarf",
    ...
    added_classes: ["shaman"],      // class ids from classes.ron
    totem_look: Some((              // its shamans' own totem look
        part: Weapon(23618),        // Ironfoe's ItemDisplayInfo display (item 11684)
        scale: 1.6,                 // the mace's size
        pitch_degrees: -90.0,       // stood head up (a weapon lies along +X from its grip)
        turn_degrees: 0.0,          // about the vertical
        sink: 0.48,                 // yards its foot (the pommel) goes into the ground
        raise_effects: -0.41,       // yards the totem's glow on its moving bones rises (lowered)
        hide_totem_body: true,      // the stock totem's body hidden, its effects kept
    )),
)
```

* **Numbers:** none of their own. A hero of an added pair plays with its class's numbers
  (spells, pets, totems, talents, gear) and its race's (passives, race spell) unchanged.
* **The create screen** offers them like the stock ones (class button, tooltip, description);
  1.12's dead Dwarf Mage row stays unoffered. The server refuses a pair that is neither stock
  nor added ([server.md](server.md)).
* **The outfit**: 1.12 has no starting outfit for these pairs, so each wears its
  class's stock starting outfit from another race: one of its own faction first, else of the
  other, the lowest race id that has one. Dwarf Shaman wears the Orc's, Human Hunter the
  Dwarf's, Gnome Priest the Human's, Orc Mage the Undead's, Troll Warlock the Orc's, Undead
  Paladin the Human's. It is only a look: the create preview and the character list show it;
  in the world no hero wears its starting outfit yet (the server sends no worn items); the order
  of races each pair borrows from is `Content::added_pairs`, for whatever dresses them there.
* **The Dwarf Shaman's totems** are the mace Ironfoe planted in the ground by its grip, head up,
  standing like a totem pole, with the stock totem's spell effects swirling around it: the
  totem's body is hidden, its glow cards, particle effects and their bones' animation stay.
  `totem_look` holds the named values, in yards of the totem's model space, measured from the
  models: the stock totem's body stands from 0.53 to 2.01 over the ground (z 1.93 to 3.41 at
  rest, lowered 1.40 as it stands) and its elemental glow sits at 1.61 to 1.83; the mace lies
  along +X from its grip, 0.30 to the pommel and 0.72 to the head's top, the head from 0.30. At
  scale 1.6 (tried at 2.8, 1.4 and 2.1 before), the head's top stands at
  1.15, the head from 0.48; sunk 0.48 the grip meets the ground. The glow is lowered 0.41 to just
  over the head's top (1.20), as the orb crowns
  the stock totem; the glow cards stay low around the shaft. The
  client builds that look while it runs, from the player's own data: the stock totem model
  (`Creature\Spells\WaterElementalTotem`, Magma Totem's `FireElementalTotem`) and Ironfoe's model
  (`Item\ObjectComponents\Weapon\hammer_1h_standard_c_03thaurissan`) in its own texture,
  skinned to the bone the stock totem's body rides, so it sways and wiggles with the totem's own
  animation (its rise as it is put down, its pulses) as the stock body does; it is placed where
  it stands in the first frame of the totem's stand (`benilla_app::ComposedDisplays`). 1.12's data has no item table, so the display id is
  Ironfoe's as servers give it (23 618); the player's `ItemDisplayInfo.dbc` names the model and
  texture. The weapon's own glow (item visual 29) is not shown. The server sends the look as a
  display of our own, `9 000 000 + race × 100 000 + the stock look` (9 304 587 and 9 304 589 for
  the dwarf). Other races' totems keep the stock look.

Code: `esurv-content` `looks` (the pairs, the outfit order, the totem looks),
`esurv-app` `races` (hands them to benilla), `esurv-server` `world::create_hero` and
`game::totems`.

## 2. An ability

```ron
(
    id: "fireball",
    name: Spell(133),           // a name the client holds: a Spell.dbc id with that name (any rank;
                                // we use the lowest), read from the player's client. Our own
                                // name is text: name: "Frost Lance"
    icon: "Spell_Fire_FlameBolt",
    look_spell: 133,            // Spell.dbc id the client plays (missile, impact, sound)
    school: Fire,               // Physical, Holy, Fire, Nature, Frost, Shadow, Arcane
    delivery: Projectile,       // how it reaches targets: see the table below
    categories: [DirectDamage, Ranged], // what kind of spell it is (spells.md, section 8)
    upgrades: [Power, Cooldown, Targets], // what cards may improve: Power, Cooldown, Targets, Area, Duration
    damage: 72.0,               // base damage per hit (for areas and effects over time: the total, split over ticks)
    cooldown: 1.8,              // seconds; abilities cast automatically when ready
    range: 30.0,                // yards; 0 = around the hero
    radius: 0.0,                // yards, for novas and areas
    targets: 1,                 // targets per cast (0 = 1)
    duration: 0.0, ticks: 0,    // for areas, DoTs and buffs
    pulse_spell: Some(8187),    // optional: a totem's pulse spell (its look and damage line)
    effect: None,               // Slow, Root, Stun, Fear, Heal, Shield, DamageBuff, …
)
```

To make a new spell, follow [spells.md](spells.md) (template: [templates/spell.ron](templates/spell.ron)).

**Delivery kinds** (confirmed):

| `delivery` | Kind | Behaviour | Damage factor |
|---|---|---|---|
| `Projectile` | projectile | a missile at each of the nearest enemies in range | 1 |
| `Chain` | chain | random targets in range, hit one after another 0.12 s apart | 1 |
| `Nova` | nova | everything around the hero within the radius | 0.5 |
| `GroundArea` | ground area | a lasting area on the hero, or on the densest group in range; ticks | 0.5 |
| `Bounce` | bouncing chain | jumps from enemy to enemy (Chain Lightning only; a legendary amulet makes jumps lose nothing) | 1 |
| `Melee` | melee | the nearest enemies in reach | 1 |
| `DamageOverTime` | damage over time | prefers enemies not yet afflicted; can stack | 1 |
| `Finisher` | finisher | melee; double damage to enemies below 30 % health | 1 |
| `Cone` | cone | a cone in front of the hero | 0.5 |
| `SelfEffect` | self effect | heal (below 85 % health), shield (when none is up), immunity (below 35 %), or buff (when an enemy is within 30 yd) | — |
| `Totem` | totem | placed when an enemy is within 25 yd | — |

**Damage of one hit** (confirmed):
`hero damage × ability damage × spell_damage_scale × card power × kind factor`, then a crit
roll per target (crit chance; ×crit damage), then the per-target bonuses (vs crowd-controlled,
below 30 %, vs afflicted), at least 1. Heals and shields on the hero and aura damage take the
same `spell_damage_scale`; cooldowns and durations never do. The spell tag coefficients in
`balance.ron` (`spell_tags`, spells.md 8a) are already in `ability damage` and the effect
amounts, and multiply with it.

**`spell_damage_scale`**: one damage knob per class in
`classes.ron`, 1 for every class, for tuning a class's spells as a whole. There is no global
spell factor (spells.md 5b). Race spells never take it.

**Targets per cast**: `targets` + hero extra targets + card extra targets (an ability with
`targets: 0` always hits one).

## 3. Level-up cards

Each level offers 3 different cards, picked by weight from (confirmed):

* a **new spell** from the class's pool, for every pool spell not yet known, while the hero
  holds fewer than **6** spells: weight **1.6** each while fewer than 3 are known, **0.9** after;
* an **upgrade** for every known spell that still has an upgrade to offer: weight **1.3** each.

If nothing can be offered, the screen shows a 25-gold card. Each card rolls a rarity (60 / 25 /
10 / 4 / 1 % at the start; luck and level raise the rarer ones).

A **new spell** card of rarity above common starts the spell stronger:

| Rarity | Damaging spell | Other spells |
|---|---|---|
| common | no bonus | no bonus |
| uncommon | +18 % power | −7 % cooldown |
| rare | +25 % | −10 % |
| epic | +34 % | −13 % |
| legendary | +46 % | −17 % |

### Upgrade cards (confirmed)

An upgrade card improves one **upgrade kind** of the spell, picked at random from the kinds it
can still take (two kinds on a legendary card). The kinds a spell can ever take are its `upgrades`
in `abilities.ron`: `Power`, `Cooldown`, `Targets`, `Area`, `Duration`. One step, by card
rarity (common → legendary):

| Kind | Step | Limit |
|---|---|---|
| power | ×(1 + [0.15, 0.25, 0.35, 0.5, 0.65] ÷ (1 + 0.5 × power upgrades taken)) | none; shrinks each time |
| cooldown | multiplier − [0.07, 0.10, 0.13, 0.17, 0.20] × room, room = (multiplier − 0.2) ÷ 0.8 | 0.2 (−80 %) |
| targets | + [0, 0, 1, 2, 2] (no Targets card below rare; a card left with nothing to add rolls rare) | +12 |
| area | + [0.10, 0.15, 0.25, 0.35, 0.5] × (5 − area) × 0.25 | 5× |
| duration | + [0.15, 0.20, 0.30, 0.40, 0.60] × (6 − duration) ÷ 5 | 6× |

A new spell is rank 1. Every step raises the rank by 1 and then multiplies power by
×(1 + 0.04 ÷ (1 + 0.125 × (rank − 2))) at the new rank: ×1.04 at rank 2, ×1.0356 at rank 3,
×1.032 at rank 4 …

**There is no maximum rank.** A kind is offered only while one more legendary step of it would
still add at least 0.5 %, and only where it makes sense:
* power: spells that deal damage, and heals, shields and auras;
* cooldown: not for a buff that already lasts as long as its cooldown;
* targets: spells with a target count, except novas, ground areas, cones and self effects;
* area: novas, ground areas, bouncing chains, cones, and totems with a radius;
* duration: ground areas, effects over time, totems, and timed self effects that are not heals,
  shields or always-up buffs.

Rerolls: 3 free (observed), then 100, 300, 900, 2700 gold … (confirmed: ×3 each time).

The run **waits while a screen is open**. Several level-ups in a row open one screen after
another; a paid-for cache opens its own screen after them. Taking a new-spell card learns the
spell at once, with the card's rarity bonus.

Code: `esurv-sim` `game::upgrades`, `game::choices`.

## 4. Talents

Three trees per class, 13 talents each, rows 0–6 (row 6 = capstone). A tree's `name` points at
the client's talent tree name, `TalentTab(161)` (a `TalentTab.dbc` id); a talent's `name` is
`Spell(id)` where the client holds it word for word, else our own text; its `text` writes a
spell the client names as `{Spell(id)}`. A talent's effects:

* `Stat(stat, per_rank)`: per rank, adds to a hero stat (same stats as in section 1).
* `AbilityMod(filter, kind, per_rank)`: per rank, improves one number of every spell `filter`
  lets through (the filters listed under milestones below, e.g. `Spell("rend")`,
  `School(Holy)`, `Delivery(DamageOverTime)` for every effect over time, or our
  `Category(...)`). `kind` is `Power`, `Cooldown`, `Targets`, `Area` or `Duration`. All of a
  spell's modifiers add up (confirmed): power, area and duration ×(1 + sum); cooldown ×(1 − sum), at most 60 % shorter;
  targets + the sum, rounded. They are a second set of multipliers, on top of upgrade cards.
* `CooldownSeconds(filter, per_rank)`: per rank, that many seconds off the listed cooldown of every
  spell `filter` lets through (Improved Fireball, Improved Frostbolt, Divine Fury, Bane: 0.1 sec a
  rank); with the `Cooldown` cuts, at most 60 % shorter.
* A talent is a **milestone** when it has a `Milestone(…)` effect, or when it is the one-rank
  talent of a milestone row (rows 2, 4 and 6: 11, 21 and 31 points), whatever its effects (a
  milestone may only make spells stronger).
* `Milestone(…)`: a rule change, written as one of the typed kinds below. A rank multiplies its
  main number (marked **bold**). Most kinds react to a **trigger** from spells that pass a
  **filter**.

  **Triggers** (confirmed): `Hit`, `Crit`, `DotTick`, `HitOrTick`, `Kill`, `Cast`,
  `StruckInMelee` (an enemy hits the hero in melee), `HealthBelow(share)` (an enemy hits the hero in melee while
  below that share of max health), `ShieldBreak`, `Block` (an enemy's melee blow at the hero is
  blocked; a blocked blow does 30 % less damage).

  **Filters** (confirmed): `Any`, `Spell("id")`, `Spells(["id", …])`, `School(Fire)` (never
  pets), `Delivery(Nova)` (a delivery kind, see spells.md), `WeaponSwing` (the hero's auto-attack),
  `Pets`, and `Category(DamageOverTime)` (a spell category, see spells.md section 8).

  | written as | rule | example |
  |---|---|---|
  | `FreeCast(`**`chance`**`, cooldown, spell, trigger, filter)` | chance to cast `spell` for free (empty: the triggering spell again), at most once per `cooldown` s | Revenge |
  | `EveryNth(count, spell, trigger, filter)` | every `count`-th event casts `spell` for free | Rampage, Hot Streak |
  | `PetOnTrigger(`**`chance`**`, seconds, max, pet, trigger, filter, count, afflicted)` | chance to summon `count` of `pet` (missing: 1) for `seconds`, at most `max` at once; with `afflicted: true` a `Kill` counts when the dead enemy carried a matching effect over time, whatever killed it | Demonic Pact; Serpent's Brood, Doom |
  | `ShieldBurst(school, `**`share`**`, radius, look_spell, trigger, filter)` | a breaking shield bursts for `share` of what it absorbed | Divine Aegis |
  | `Splash(`**`share`**`, radius, look_spell, requires_aura, trigger, filter)` | the hit splashes around its target (only on targets with `requires_aura`, if given) | Splitting Ice |
  | `ExtraTargets(count, `**`share`**`, trigger, filter, per_swing_second)` | the hit sweeps on to `count` more enemies; with `per_swing_second: true` a weapon swing's `share` is per second of the weapon's swing time, so slow weapons sweep for more | Sweeping Strikes |
  | `StrikeAgain(`**`chance`**`, times, share, trigger, filter)` | chance to strike `times` more for `share` of the hit | Windfury |
  | `Reflections(count, seconds, interval, radius, targets, `**`share`**`, trigger, filter)` | `count` see-through copies of the hero run ahead, then chase and spin for `seconds`, each spin every `interval` striking up to `targets` within `radius` for `share` of the hit (our rule) | Blade Dance |
  | `Afflict(`**`chance`**`, effect, trigger, filter)` | chance to slow, root, stun, make vulnerable, or heal the hero | Blackout |
  | `DotOnHit(look_spell, `**`share`**`, seconds, trigger, filter)` | the target takes `share` of the hit again over `seconds`; one per enemy: a new one adds what the old one had left and starts afresh | Ignite, Deep Wounds |
  | `ResetOnKill(min_interval, filter)` | a killing blow readies the spell again | Mortal Wounds |
  | `ExplodeOnDeath(afflicted, `**`share`**`, radius, look_spell, filter)` | enemies killed this way explode for `share` of their max health | Pyroclasm |
  | `SpreadOnDeath(count, filter)` | an effect over time jumps to the `count` nearest on death (not a drain, which goes on by itself) | Contagion |
  | `TimedBuff(stat, `**`amount`**`, max_stacks, seconds, trigger, filter)` | stacking timed bonus to a hero stat | Flurry (shaman) |
  | `HealOnTrigger(`**`share`**`, cooldown, trigger, filter)` | heal `share` of max health | Inspiration |
  | `ResourceOnTrigger(`**`amount`**`, trigger, filter)` | gives `amount` rage or energy (the hero's starting resource; nothing without one) | none yet |
  | `DotCrits(`**`bonus`**`)` | the hero's effects over time can crit (its crit chance); a critical tick deals `bonus` more | Pandemic |
  | `CooldownOnTrigger(`**`seconds`**`, spells, trigger, filter, every)` | the listed spells the hero knows come `seconds` sooner, at most once every `every` seconds | Kindling |
  | `MoreSummonsEveryNth(count, extra, filter)` | every `count`-th cast of a matching summoning spell calls `extra` more summons | Nature's Grace |
  | `FasterSwings(`**`amount`**`, swings, trigger, filter)` | the next `swings` weapon swings come `amount` faster; a new trigger starts the count over | Flurry (warrior) |
  | `CheatDeath(cooldown, health, immune_seconds)` | survive a killing blow at `health` | Last Stand, Cheat Death |
  | `OverhealShield(max_share)` | overhealing from heal spells and heals over time becomes a shield (confirmed) | Tree of Life |
  | `HealingStrikes(share, radius)` | healing from heal spells and heals over time (all of it, at full health too) also damages nearby enemies | Tranquility |
  | `TotemHeal(share)` | every totem pulse heals the hero for `share` of max health (confirmed) | Amulet of the Ancients |
  | `ChainNoFalloff` | chain spells lose no damage per jump | Elemental Fury |
  | `Periodic(interval, spell)` | casts `spell` by itself every `interval` s (0: its cooldown as a cast sets it), times the hero's cooldown factor, never under 1 s; 0.3 s after a cast that found nothing to do; the first cast comes as soon as an enemy is in its reach | Killing Spree |
  | `StartWith(spell)` | every run starts with `spell` known | Mortal Wounds |
  | `Pet(pet)` | a pet from `pets.ron` fights beside the hero | Tame Beast |
  | `LineHit(every, `**`share`**`, length, width, behind, hits_target, look_spell, runners, trigger, filter)` | every `every`-th event (1: each) also strikes every enemy in a line from the hero through the struck one, `length` yd from the hero and `width` yd either side, for `share` of the hit; `behind: true` only past the struck enemy, `hits_target: true` the struck one too; `look_spell` (else the hit's spell) flies from the hero to the line's end and names the damage; `runners: Some((displays: [(display, scale), ..], count: (least, most), speed))`, a look only, sends a row of that many creatures of those looks, each drawn at its scale running the line's length at `speed` yd/s ([screens.md](screens.md), "Looks") | Thori'dal, Stampede, Netherstrand Longbow |
  | `MarkAttackers(seconds, `**`bonus`**`, heal, look_spell, filter)` | an enemy that strikes the hero in melee is marked for `seconds` (aura `look_spell` on it); matching hits deal `bonus` more to it and each heals `heal` of max health | Mark of the Fallen Champion |
  | `GrowingArea(`**`per_hit`**`, max, filter)` | a matching lasting area grows `per_hit` of its first radius per enemy it damages, up to `max` more; its look is put down again each 10 % it grows | Sanctified Ground |
  | `AreaFollowsHero(filter)` | a matching lasting area lands on the hero (with an enemy within the spell's range) and moves with the hero; its look is put down again every 2 yd | Ranger-General's Barrage |
  | `PullEveryNth(count, radius, look_spell, filter)` | every `count`-th cast of a matching spell with an enemy within `radius` first pulls those enemies (not bosses) to 2 yd from the hero, `look_spell` going from the hero onto them | Magnetic Pull |
  | `FireTrail(`**`share`**`, radius, seconds, look_spell, filter)` | an enemy carrying a matching effect over time leaves a patch of fire where it was each time it has walked `radius` yd (at most one a second, at most 12 of a hero's at once); a patch burns every enemy within `radius` once a second for `seconds`, `share` of the effect's damage a second, shown as `look_spell` on the ground | Legion Flame |
  | `FearedTurn(`**`chance`**`, filter)` | an enemy a matching fear spell hits (never a boss) has `chance` to turn on the others for the fear's time: it chases the nearest other enemy and strikes it with its own blow (its swing shown, counted as the hero's other damage, setting off no milestones), never the hero | Dominate Mind |
  | `Every(count, per, per_enemy, chance, first_only, filter, then)` | every `count`-th cast (`per: Casts`, its own casts, counted as they start; the counted cast's hits until the next cast) or hit or tick (`per: Hits`; with `per_enemy` counted on each enemy apart) of a matching spell also sets off the milestones in `then`, on their own triggers; `chance` (1 when left out) for each counted time; `first_only`: only the counted cast's first event that sets one off | Gruul's Shatter, Arcane Bomb, Silithid Swarm |
  | `ResetOnTrigger(`**`chance`**`, min_interval, trigger, filter)` | the matching spell that set it off is ready again | Incinerate Flesh |
  | `StrikeOne(pick, reach, `**`share`**`, effects, look_spell, reflection, trigger, filter)` | one more enemy within `reach` of the hero (0: the spell's range or radius), `Nearest`, `Healthiest` or `Farthest`, takes `share` of the hit and `effects`; with `reflection: true`, a look only, a see-through reflection of the hero strikes it instead of the hero ([screens.md](screens.md) "Looks") | Hateful Strike, Blink Strike |
  | `Burst(`**`share`**`, of, radius, at, delay, count, scatter, effects, look_spell, trigger, filter)` | `count` bursts at the struck enemy (`Target`, following it for a `delay`) or the hero (`Hero`), spread up to `scatter` yd, each `share` of `of` (`Hit`: the hit, or a broken shield's absorbed amount × damage; `SpellHit`: one hit of the spell; `MaxHealth`: max health × damage) on every enemy within `radius`, with `effects` | Living Bomb, Rocket Strike, War Stomp |
  | `DotBurst(`**`share`**`, radius, on_end, on_death, spreads, look_spell, filter)` | a matching effect over time bursts when it runs out on a living enemy and/or when its carrier dies: `share` of its whole damage on every enemy within `radius`; `spreads`: each struck gets it | Whisper of Death, Inevitable Doom |
  | `DotSpread(count, reach, max_carriers, every, moves, growth, trigger, filter)` | on a tick, the ticking effect passes to the `count` nearest lacking it (on a hit: the spell's burn), `growth` stronger; `moves`: it leaves the first; at most `max_carriers` carry it, at most once every `every` s | Corrupted Blood, Unbound Plague |
  | `LeaveArea(`**`share`**`, of, seconds, radius, at, count, scatter, slow, poisons, follows_hero, rides_target, look_spell, spirits, trigger, filter)` | lasting areas that strike every enemy inside once a second for `share` of `of` (as `Burst`), slowing by `slow`, putting the option's poison on with `poisons`; moving with the hero or riding with the struck enemy; `spirits: Some((count, form, scale, strike_look))`, a look only, runs that many see-through druid-form creatures beside the hero while it lasts, each striking with `strike_look` as it strikes (Sprint of the Wild) | Lava Bomb, Earthquake, Tornado |
  | `WhileBuff(spell, stats, then, once)` | while the buff `spell` left is up, `stats` added and the milestones in `then` on; `once`: one of them once a buff | Endurance Aura, Mirror Image, Wind Walk |
  | `Spree(every, seconds, interval, spell, stats)` | every `every` s (with an enemy within 30 yd), for `seconds`, `spell` cast for free every `interval` and `stats` added | Bladestorm |
  | `Barrage(seconds, interval, reach, `**`share`**`, of, look_spell, trigger, filter)` | for `seconds`, every `interval` a random enemy within `reach` takes `share` of `of` | Starfall |
  | `CastAgain(delay, opposite, elsewhere, filter)` | each own cast is cast again for free `delay` s later; `opposite`: a returning spell flies the other way; `elsewhere`: a ground area keeps off the spots the first covered | Fan of Knives, Halo of Elune, Flame Strike |
  | `HealAfter(`**`share`**`, seconds, filter)` | a heal of the spell heals `share` of it again over `seconds` | Healing Wave |
  | `DotFromHeal(`**`share`**`, seconds, targets, reach, order, look_spell, filter)` | a cast of the heal puts an effect over time on `targets` enemies within `reach` yd (the nearest, or in target `order`, e.g. `BossesFirst`), each dealing `share` of the cast's whole healing over `seconds` (a tick a second, crits only as other ticks do) | Renew (Deplete) |
  | `CastOnCooldown(filter)` | a spell that waits for a condition (a heal for low health, an immunity) is cast whenever it is ready and an enemy is within 30 yd | Renew |
  | `ApplyPoison(trigger, filter)` | the struck enemy gets the starting option's poison (Mutilate's) | Fan of Knives |
  | `ComboPoints(points, trigger, filter)` | `points` combo points (with a combo resource) | Feast |
  | `Retaliate(`**`share`**`, look_spell)` | a melee blow taken strikes its attacker back for `share` of it | Divine Aegis |

  All of them play in the run (`esurv-sim`
  `game::milestones`): a hit, a crit, a tick, a kill, a spell's own cast (not a free one), a
  melee blow taken, damage taken and a shield breaking each set them off. Spells a milestone
  casts set off no milestones; other milestone damage can set off more only three deep
  (spells.md 5b). A milestone's damage over time ticks once a second. Pets
  (Tame Beast, Demonic Pact …) fight beside the hero (`game::pets`); when not attacking or
  running at an enemy, a pet stands 5 yd to the hero's
  left; a second one 5 yd to the right, then behind and on the diagonals. `Afflict`
  effects: `Slow(amount, seconds)`, `Root(seconds)`,
  `Stun(seconds)`, `Vulnerable(amount, seconds)`, `HealFromDamage(share)`,
  `HealMaxHealth(share)`, and ours `Fear(seconds)`, `Silence(seconds)` (no ranged
  shot or ability) and `Knockback(yards)` (straight away from the hero; never a boss). Two
  vulnerabilities do not add up: the stronger stays while it lasts. Our additions to the
  filters: `Look(id)` (hits shown as that spell); to the triggers: `BuffEnds` (a spell's buff on
  the hero ends, or Holy Shield's last charge goes). `Splash` and `ExtraTargets` may carry
  `effects`; `ExtraTargets` a `growth` per further enemy and a `reach`.

**Pets** (`content\pets.ron`): Wolf, Raptor, Spirit Wolf, Imp, Felguard, Treant: model, scale,
damage, swing time, and a ranged spell for the Imp (Fireball). The mage's Water Elemental is the
Frostbolt starter's passive (`Summoned Water Elemental`; AUTHORING.md 8a), no talent pet. The
legendary amulets' summons: Snake (display 2958, a green snake),
Phoenix (3248, a red carrion bird: the client has no phoenix), Avatar of Vengeance (4629, a
shade), Doomguard (1912), Panther (599, a black tiger, Cat family), Lava Spawn (1070, a fire
elemental at half size); Force of Nature calls the Treant. Their damage and swing are
placeholders copied from the Spirit Wolf, Imp, Felguard, Felguard, Wolf and Imp, to be set.

**The mage Frost capstone, Winter's Chill (`water_elemental`, row 6).** The Water Elemental is already the Frostbolt starter's passive, so the capstone gives
no second one. Like every 31-point milestone it starts each run with an extra spell, Frost Nova,
casting itself every 9 s; in place of the pet it gives Shatter: +8 % damage to slowed, held or
fleeing enemies (`DamageVsCrowdControlled`) and every Frostbolt hit splashing 5 % of itself onto
the enemies within 5 yd of its target; Frost spells +9 %, Frostbolt +20 %. The numbers keep each
mage starter's gain from the capstone where the pet version had it (`balance_check --milestones
--kite --whole-talent --talents water_elemental --start <option>`, 4,000 runs a side). Base
(v0.3.0): a Water Elemental pet (50 damage, 2.2 s swing, Frostbolt shots), Frost Nova every 9 s,
Frost spells +9 %, Frostbolt +90 %. The talent keeps its id, so saved points stay.

**Talent experience**, earned by each run for the hero's class:
`minutes survived × 10 + kills ÷ density × 0.05 + 40 per boss + 250 for a win`, then × the zone bonus D^0.7
(`balance.ron` `zone_scaling` `talent_exponent` 0.7, ×1, 1.62, 2.16, 2.64, 3.09, 3.51, 3.9, 4.29, 4.66, 5.01 for D 1-10),
then × the run-option bonus (up to ×2). The numbers are ours
(`rewards.rs`), set so a player who wins each zone once, in order, follows the talent schedule of
the power budgets (AUTHORING.md, "Power budgets"): at least 11 points arriving in Warsong Gulch
(D 3), 31 arriving in the Sunken Temple (D 5), 41 arriving in Feralas (D 6), and all 51 first on
arriving in Blackrock (D 9), so Ahn'Qiraj and the zones after it add no talent power.
**Talent points:** the n-th point costs `110 × 1.024^(n − 1)`, rounded (110, 113, 115 … 360 for the 51st; about 1,370 for 11 points, 2,960 for 21, 4,980 for 31, 10,780 for all 51); at most 51 per class. With the zone bonus, the expected hero arrives with 6 points in Duskwood, 15 in Warsong Gulch, 24 in the Scarlet Monastery, 33-34 in the Sunken Temple, 41-43 in Feralas, 50-51 in the Arena and 51 from Stratholme on: the 51st point comes one zone early, in the Arena (the smooth zone bonus; the schedule asks for it in Stratholme).

**Prestige points.** Talent points stop at 51, but talent experience
keeps coming, and past the 51st point it buys **prestige points**, more slowly each time: the
first costs what a 52nd talent point would (`FIRST_TALENT_POINT_COST × TALENT_POINT_COST_GROWTH^51`,
369 at today's 110 × 1.024), each next one 5 % more than the one before
(`PRESTIGE_POINT_COST_GROWTH`, `esurv-sim` `game::rewards`: about 369, 387, 407 … 572 for
the 10th, 932 for the 20th). There is no last one. One point buys one rank of a stat of
`content\prestige.ron`; prestige ranks are the class's, like its talents: kept in its save
(`ClassProgress::prestige`), brought into every run as the talents' stat effects are
(`HeroSetup::add_prestige`), bought and taken back on the hero sheet's Talents tab (free, one at a
time), and **a talent reset gives every prestige point back too**. Rules: `game::prestige`.

The stats (one rank each; all classes unless said; the sim's own caps still hold: damage
reduction 60 %, movement ×2.2, double cast, spell cooldown cuts 60 %):

| Stat | Per rank | Notes |
|---|---|---|
| Damage, max health, damage reduction, cooldown recovery, crit chance, crit damage | +1 % | the class's basic stats and the defences |
| Lifesteal | +0.25 % | finer steps |
| Base health | +5 max health | flat |
| Health per second | +1 | |
| Pickup range, area of effect, experience, gold | +1 % | |
| Movement speed | +0.5 % | finer steps |
| Swing speed | +1 % | Warrior, Paladin, Rogue, Shaman (the weapon-swing classes and the Shaman) |
| Block chance | +1 % | Warrior, Paladin |
| Luck, buff strength | +1 % | what rings and trinkets give |
| Double cast chance | +0.5 % | finer steps |
| Health per kill | +1 | |
| Effect-over-time stacks | +1 | at most 3 ranks (for safety: whole stacks) |
| Extra life | +1 | at most 1 rank (for safety: lives would end the danger) |
| Spell duration | +1 % | every spell, as a duration card |
| Rage, energy and proc chance | +1 % | every spell, as a rage gain, energy refill or proc chance card (they share one number) |

Never targets, nor what stands in for them on cards (stacks, strikes, imps, pets, critters); the
loader refuses one. The spell cards' damage, cooldown and area are already covered by the hero's
damage, cooldown recovery and area of effect above.

**Talent values, first pass.** Every talent without a milestone
effect uses one standard value per rank for its stat or spell change, so its strength follows
only from what it changes and how many ranks it has (milestone talents keep theirs for now):

| Change | Per rank |
|---|---|
| Damage | 2 % |
| Damage reduction | 1 % |
| Crit chance | 1 % |
| Max health | 3 % |
| Cooldown recovery | 1 % for spells, weapon swings as from 2 % (`SpellCooldownRecovery`: half for spells, attack speed in full) |
| Health per second | 0.5 |
| Crit damage | 8 % |
| Move speed | 3 % |
| Pet damage | 10 % |
| Weapon damage | 5 % |
| Damage vs crowd-controlled enemies | 5 % |
| Luck | 6 % |
| Damage vs elites | 4 % |
| Double cast | 2 % |
| One spell: power / cooldown / area / duration | 10 % / 3 % (halved) / 10 % / 10 % |
| A group of spells (a list, a school, a delivery kind): power / cooldown / area / duration | 5 % / 1.5 % (halved) / 10 % / 10 % |

Damage below 30 % health, thorns, heal per kill, lifesteal and extra targets keep their values.

**Buying a rank** (the armory): a point must be free, the talent short of its last rank, its tree
must have 5 points per row in the rows above it, and the talent it `requires` must be at its last
rank. A rank can be taken back for free when every rank left still meets those rules. Code:
`esurv-sim` `game::talents`; the save keeps the ranks per class.

## 5. Adding or changing

* **Change a number:** edit the ability or class in `content\*.ron`; restart.
* **New ability:** add a record to `abilities.ron` with a new `id`, pick a `delivery` kind from
  the table and a `look_spell` from 1.12's `Spell.dbc` that fits it, then list its id in a
  class's `abilities` and give it a role in `crates\esurv-sim\tests\spell_roles.rs`
  (spells.md, section 5c).
* **A class:** classes are told apart by `wow_class_id` (the create screen, the class icon, the
  gear item numbers, the save), and the nine ids the 1.12 client knows (1–5, 7–9, 11) are all in
  use. So data reshapes one of the nine: keep its `id` and `wow_class_id` and change the rest
  of its record, its three trees in `talents.ron` and its one set in `gear.ron` (templates:
  [class.ron](templates/class.ron), [gear_set.ron](templates/gear_set.ron),
  [talent.ron](templates/talent.ron)). A tenth class needs code ([AUTHORING.md](AUTHORING.md),
  section 8).
* **Change a race:** edit its passives or point `spell` at another ability in `races.ron`. New
  races need a `wow_race_id` the 1.12 client knows (1–8). A race may be one more class: add the
  class id to its `added_classes` (section 1b); it gets an outfit by the rule there, and its
  create button and tooltip come with the class.
* **New delivery kind or milestone:** needs code: a new `Delivery` (with `Delivery::ALL`) or
  `Milestone` variant in `esurv-content`'s schema and its case in `esurv-sim`, then data
  can use it. (Planned roadmap mechanics such as rage and procs plug in here.)
* Check with `cargo test -p esurv-content -p esurv-sim`. Worked examples of each:
  [AUTHORING.md](AUTHORING.md).
