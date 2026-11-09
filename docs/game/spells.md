# Making a spell

How to add a new spell (an "ability") or change an existing one. The ready-to-copy block is
[templates/spell.ron](templates/spell.ron); the plain-language version of this page is
[guides/spells.txt](guides/spells.txt). How spells fit into classes, cards and talents is in
[classes.md](classes.md).

## 1. Steps

1. Copy [templates/spell.ron](templates/spell.ron) into the list in `content/abilities.ron`.
2. Give it a new `id` (lowercase, unique) and a `name`.
3. Pick its **look**: `look_spell` (a WoW spell id) and `icon` (section 3).
4. Pick its **delivery** (section 4) and fill the numbers that delivery uses (section 5).
5. Optionally add an **on-hit effect** (section 6), then choose its **upgrades** (section 7)
   and **categories** (section 8), and give it its **tags** (section 8a).
6. Add the id to a class's `abilities` list in `content/classes.ron` so level-ups can offer it,
   or make it the class's `starting_ability`.
7. Run `cargo test -p esurv-content -p esurv-sim`. The loader checks that every id a class or
   talent names exists.

## 2. Fields

| Field | Meaning |
|---|---|
| `id` | Unique key. Classes, talents, amulets and saves refer to the spell by it. |
| `name` | Shown on cards and tooltips. A name the WoW client holds is a pointer, `Spell(133)`: a `Spell.dbc` id with that name (any rank; the shipped spells use the lowest), read from the player's client as the game loads. A name of our own is text, `"My New Spell"`. |
| `description` | What a card says the first time it offers the spell: a template in our own words, its `{name}` placeholders the spell's numbers at the rank offered (section 2a); `{Spell(133)}` shows the client's name for a spell. Every spell needs one. |
| `icon` | Texture name under `Interface\Icons` in the WoW client, e.g. `Spell_Frost_FrostBolt02`. |
| `look_spell` | The WoW spell (`Spell.dbc` id) the client plays: cast, missile, impact and sound. Its `Speed` in `Spell.dbc` is also the missile's flight speed. |
| `school` | `Physical`, `Holy`, `Fire`, `Nature`, `Frost`, `Shadow` or `Arcane`. Talents and milestones can filter on it. |
| `delivery` | How the spell finds and hits enemies: `Projectile`, `Nova`, `Melee` ... (section 4). |
| `categories` | What kind of spell it is, e.g. `[DamageOverTime]` (section 8). |
| `tags` | What the spell is about, for the tag coefficients in `balance.ron`, e.g. `[Nuke, Sustain]` (section 8a). |
| `upgrades` | What level-up cards may improve, e.g. `[Power, Cooldown, Targets]` (section 7). |
| `damage` | Base damage per hit. For ground areas and effects over time: the total over `duration`, split evenly over `ticks`. For totems: one pulse. 0 for spells that deal none. |
| `cooldown` | Seconds between casts. In play a spell recasts every `cooldown` × its cuts, at least 1 s, a hard cap (section 5a). Spells cast on their own whenever ready. |
| `range` | Yards it reaches; 0 = around the hero. |
| `radius` | Yards, for novas, ground areas, cones and totems. |
| `targets` | Enemies hit per cast (0 counts as 1). Hero and card extra targets add to it. |
| `duration`, `ticks` | For ground areas, effects over time, buffs and totems: how long, and how many hits. For a nova (`Nova`), `ticks` of 2 or more makes it repeat around the hero: `ticks` novas, `duration ÷ ticks` seconds apart, the first at once (`duration: 3.0, ticks: 3` goes off at 0, 1 and 2 s); 0 or 1: one nova (section 5c, "A nova that repeats around the hero"). Hellfire. |
| `pulse_spell` | Optional. A totem's pulse (`Totem`): the `Spell.dbc` id each pulse shows and hits with, `Some(8187)`. Any other spell: the id its hits are logged under, for a spell whose look would show its numbers wrong (Fan of Knives looks like Throw, 2764, a ranged basic shot whose numbers the client shows white like a swing; its hits are logged under the Throw 10277, which shows them yellow as special damage). Missing: its `look_spell`. |
| `double_on_low_health` | Optional. `true`: its hits deal double to enemies under 30 % health, whatever its delivery (finishers always do). Hammer of Wrath. |
| `burn` | Optional. Each enemy its hit lands on also burns: `Some((damage: 105.0, duration: 5.0, ticks: 5))` is 105 in all, in 5 even ticks over 5 s, from when the hit lands. Scales as the hit does; duration upgrades add whole ticks (section 5c, "A hit that leaves a burn"). Holy Fire. |
| `impact` | Optional, ground areas (`GroundArea`) only. Every enemy inside the area as it lands takes this much at once, before its ticks: `impact: 46.0`. Scales as the area's hits do; missing or 0: no impact (section 5c, "A ground area that lands with an impact"). Flamestrike. |
| `heal_below` | Optional. A heal on the hero waits until health is at or below this share of max health (missing: 0.85). Holy Light uses 0.65. |
| `effect` | An extra effect on what it hits, or on the hero for self effects (section 6). |
| `free_casts_can_proc` | Optional, `false` if left out. `true`: when a milestone casts this spell for free, its hits can still set off milestones (section 5b). |

## 2a. The description

A new spell's card is the spell written out (screens.md, "What a card says"): `description` is
a template the card fills with the spell's numbers at the rank it offers, for the hero taking it
(talents, amulet, stats and the card's head start counted; the run's own rules work them out in
`esurv-sim` `game::spell_numbers`). Write what it does in our own words, truthfully: what it
hits and whom it prefers, how far, how much damage of which school, what it does to what it hits,
what bosses shake off.

| Placeholder | What it is (the spells that have it) |
|---|---|
| `{school}` | its school's name: Fire, Holy ... |
| `{damage}` | one hit; for ground areas and effects over time, all their ticks together; a totem's, one pulse |
| `{tick}`, `{ticks}`, `{every}` | one tick, how many, seconds apart (ground areas, effects over time) |
| `{pulses}`, `{every}`, `{totem_range}` | a totem's pulses, seconds apart, and how near an enemy must be before it is placed |
| `{low_damage}`, `{low_health}` | a finisher's hit (or a `double_on_low_health` spell's) on an enemy below that share of its health |
| `{burn}`, `{burn_for}` | what a hit's burn deals in all on each enemy it lands on, and over how many seconds (spells with `burn`) |
| `{impact}` | what a ground area deals at once to each enemy inside it as it lands (spells with `impact`) |
| `{novas}`, `{every}` | how many times a repeating nova goes off per cast, and how many seconds apart (`{damage}` is one nova's) |
| `{targets}`, `{range}`, `{radius}` | how many it hits, how far it reaches, its area (a cone's length) |
| `{duration}`, `{cooldown}` | how long it lasts, seconds between casts |
| `{falloff}` | how much weaker each jump of a bouncing chain is |
| `{slow}`, `{slow_for}` | a slow's share and seconds |
| `{hold}`, `{boss_hold}` | a stun or root's seconds, and on a boss |
| `{fear}` | seconds an enemy flees (bosses never do) |
| `{leech}` | the share of the damage that heals the hero |
| `{charges}`, `{charge_heal}` | healing charges a hit grants, and the share of max health each restores |
| `{heal}`, `{shield}` | shares of max health: a heal or an absorb shield (on the hero, or on each hit) |
| `{heal_below}`, `{immune_below}` | the health at or below which a heal or an immunity casts |
| `{buff}`, `{haste}`, `{buff_range}` | more damage, faster cooldowns, and how near an enemy must be |
| `{weaken}`, `{weaken_for}` | how much less the weakened enemies' attacks deal, for how long |
| `{aura}` | the damage an aura strikes melee attackers back with |

A spell has only the numbers that fit what it does; a totem's pulses carry no on-hit effect in play,
so they have none of those (an effect over time has its `{leech}` or its other effect's numbers: see
section 6). `{targets:the nearest enemy|the # nearest
enemies}` picks the words by whether the number is 1 (`#` stands for it); `{{` and `}}` are braces.
The loader refuses a template naming an unknown number, and a test fills every description at
ranks 1 to 10, so a number a spell lacks fails it.

## 3. Look and flight speed

`look_spell` is a **WoW spell id**, not a visual id: the client looks the spell up in
`Spell.dbc` and plays its visual kit. Find ids on any vanilla spell database (for example
Fireball 133, Frostbolt 116, Shadow Bolt 686, Frost Nova 122).

**Missiles** (confirmed in benilla): a projectile flies at that
spell's `Spell.dbc` `Speed` (yards per second), so it lands after `distance ÷ Speed` seconds.
The client draws the missile at the same speed, so the hit always lines up with what the player
sees. A spell whose `Speed` is 0 hits at once. Enemy projectiles home in and always hit on
arrival. For a projectile spell, pick a `look_spell` whose WoW spell is itself a missile
(Fireball 24, Frostbolt 28, Shadow Bolt 20, Arcane Shot 40, …).

## 4. Delivery kinds

| `delivery` | Kind | Targets | Uses |
|---|---|---|---|
| `Projectile` | projectile | the first enemies in `range` in target order (below) | `targets` |
| `Chain` | chain | the enemies in `range` in random order, one after another 0.12 s apart; with fewer enemies than hits it goes round again | `targets` |
| `Nova` | nova | everything within `radius` of the hero; with 2 or more `ticks`, that many times per cast, `duration ÷ ticks` seconds apart, each around where the hero then stands | `radius` (`duration`, `ticks` to repeat) |
| `GroundArea` | ground area | an area on the hero (only with an enemy inside it) or, with a `range`, on the densest group among the 24 nearest enemies in `range`; hits every tick | `radius`, `duration`, `ticks` |
| `Bounce` | bouncing chain | starts on the nearest enemy in `range`; each jump strikes the nearest enemy within `radius` of the last one that it has not struck yet (none: the chain ends), 0.08 s apart, 8 % weaker per jump | `targets`, `radius` |
| `Melee` | melee | the first enemies in reach in target order (below) | `targets`, `range` |
| `DamageOverTime` | over time | bosses first, then elites, then the healthiest enemies; can stack (section 5b) | `duration`, `ticks`, `targets`, `range` |
| `Finisher` | finisher | melee; double damage below 30 % health | `targets` |
| `Cone` | cone | everything up to `range` yards ahead, 60° either side of where the hero faces | `range` |
| `SelfEffect` | self effect | the hero (heal, shield, buff, immunity) | `effect`, `duration` |
| `Totem` | totem | placed 1.5 yd in front of the hero (to its left when a milestone casts it) when an enemy is within 25 yd; pulses after 0.5 s, then every 2 s while it lasts (section 5b): with a `radius`, hits everything in it; without, the `targets` nearest enemies within `range` | `radius` or `range`, `targets`, `duration` |

**Target order** (`Projectile`, `Melee` and `Finisher`; `Chain` shuffles the same list): the enemies in range,
bosses first, then elites, each nearest first; then the ordinary enemies within 6 yd of the
nearest ordinary one (it included), the lowest share of health left first; then the other ordinary enemies,
nearest first (`esurv-sim` `casting::enemies_in_target_order`). Their descriptions say
"Bosses and elites are hit first."

Self effects cast only when useful: heals at or below 85 % health, shields when none is up,
immunity at or below 35 %, buffs when an enemy is within 30 yd. Cast again, a heal over time,
a shield or a buff replaces the one the same spell left; immunities stack. Anything the spell
leaves on the hero lasts at least 0.1 sec.

**One hit** = hero damage × `damage` × the class's `spell_damage_scale` (classes.md) × card power, then
crits and per-target bonuses. Race spells leave out the class knob. The listed `damage` already
carries the spell's tag coefficient from `balance.ron` (section 8a). Every delivery kind hits for its
listed `damage`: novas, ground areas and cones carry no hidden factor.

## 5. Which numbers matter for which delivery

* Projectile, chain, bouncing chain, melee, finisher: `damage`, `cooldown`, `range`, `targets`.
* Nova, cone: `damage`, `cooldown`, `radius`.
* Ground area: `damage` (in total), `cooldown`, `range` (where it may be placed), `radius`,
  `duration`, `ticks`.
* Over time: `damage` (in total), `cooldown`, `range`, `duration`, `ticks`.
* Self effect: `cooldown`, `duration`, and its `effect` (e.g. `Shield(share: 0.3,
  seconds: 10.0)`); `damage` 0.
* Totem: `pulse_spell`, `cooldown`, `radius`, `duration`, and `damage` if it attacks.

## 5a. Cooldowns and casting (confirmed)

Every spell the hero has sits in a **slot** and casts by itself when the slot's timer runs out.

* **Seconds between casts** = `cooldown` × upgrade-card cut × talent/amulet cut × the hero's
  cooldown factor (1 ÷ (1 + cooldown recovery)), never under **1 s**, a hard cap on cooldown cuts. No other factor: the
  listed cooldown and duration are what plays (section 5b).
* **Talent, ring and trinket cooldown recovery counts half**: their
  stat is `SpellCooldownRecovery` (talents 0.5 % a rank, Quick Strike Ring 5 %, Talisman of
  Ephemeral Power 2.5 %), and a talent's cut to one spell's cooldown is 2 % a rank, to a group's
  1 % a rank. A proc that takes seconds off cooldowns is not a cut (Fire's 0.3 s off its fire
  spells on a Fireball crit). Spell cards, amulets, prestige, loadouts, gear, legendaries
  and buffs give full `CooldownRecovery`. **Attack speed:** the weapon swing
  comes every √(swing factor) × the swing time, the swing factor being 1 ÷ (1 + cooldown
  recovery + the `SpellCooldownRecovery` once more + the talents' `SwingCooldownRecovery`, 1 %
  a rank), so these sources speed swings in full.
  The 1 s floor holds. Tooltips show only the cooldown value; the swing pace is a hidden
  mechanic. Code: `esurv-sim` `game::stats` (`swing_factor`).
* **Self-buff uptime 40 %**: every self-buff spell with a
  duration and a cooldown lasts 40 % of its cooldown at base values (before cards, talents,
  cooldown recovery and legendaries, which may still raise it): Adrenaline Rush 12 s, Slice and
  Dice 8 s, Tiger's Fury 10 s, Dash 8 s, Power Infusion 12 s, Battle Shout 10 s, Berserk 12 s,
  Arcane Power 12.2 s. The race spells keep their own durations (Perception 10 s, Blood Fury
  12 s, Berserking 10 s). Shields, heals over time, auras and immunities are not counted as
  buffs here.
* **Cooldown after the effect** (`kit: (cooldown_after_effect: true)`): the spell's
  cooldown starts only once the effect it put on the hero has ended (run out or taken away), not
  at the cast, so however far its cooldown is cut it is never up all the time. While the effect
  holds the action bar sweeps its time left (the spell unavailable), then sweeps the cooldown.
  Only Divine Shield has it (its 6 s immunity, then its cooldown). Code: `esurv-sim`
  `game::casting` (`effect_holds_cooldown`); class spells only, not the race's spell slot.
* A spell just learned (and the race's spell at the start of a run) first casts after **0.5 s**.
* A cast that finds nothing to hit (no enemy in range, a heal at full health …) tries again
  after **0.2 s**; the race's spell after **0.5 s**.
* Upgrade cards cut cooldowns down to 20 % of the listed value; talents and the amulet down to
  40 %; both cuts multiply.
* A spell cast for free by a talent or amulet (a "proc") that the hero has not learned casts at
  rank 1, with the average card power of the hero's spells (at least 1).
* **Double-cast** (confirmed): a spell that casts goes off again once for every whole 100 % of
  the hero's double-cast chance, and once more with the rest as a chance: 250 % gives two extra
  casts and a 50 % chance of a third.

## 5b. Spell numbers

* **No global spell factor; one damage knob per class.** The listed numbers of every class
  spell are what plays: `cooldown` and `duration` as written. Each class has one knob,
  `spell_damage_scale` in `classes.ron` (1 for every class), which multiplies only the damage
  of that class's spells and the amounts that go with it (heals and shields on the hero, aura
  damage); never cooldowns or durations, and never the race spell. Buff, haste and aura
  amounts, and on-hit heals, shields and effect lengths on enemies, are as listed.
* **The starting spells** are the spells of the class's starting options (AUTHORING.md 8a),
  never cards; their budget is about 40 damage a second at level 1, rank 1. Their timed ones as
  shipped:

  | Spell | `cooldown` | `damage` |
  |---|---|---|
  | Steady Shot | 2 | 75 |
  | Smite | 3 | 106 |
  | Mind Spike | 2 | 70 |
  | Fireball | 3 | 97.5 |
  | Frostbolt | 3 | 105 |
  | Arcane Blast | 3 | 110 |
  | Shadow Bolt | 3 | 105 |
  | Lightning Bolt | 2 | 35 to each of 2 |
  | Wellspring | 4 | 45 to each of 3 |
  | Starfire | 4 | 160 |

  The test `the_starting_spells_play_their_numbers` (`game::spell_numbers`) checks that the
  card shows what plays. The other options' spells spend rage or energy, or come from a swing,
  a pet or a summon (AUTHORING.md 8a). The damage of the card spells is set by role (section 5c).

## 5c. Spell roles

Every damaging card spell has a role, and its `damage` is set so
that, at level 1 with no upgrades, talents or gear, it deals its role's damage. The role never
sets cooldowns or durations.

| Role | Target | How `damage` follows |
|---|---|---|
| filler (single target, cooldown up to 2.5 s) | 40 damage a second | 40 × `cooldown` |
| fixed (single target) | a damage per second set by hand for that spell, whatever its cooldown (the nukes 50; Arcane Shot, Fire Blast and Searing Pain 45; Hammer of Wrath 40; Bloodthirst 35); a hit's burn counts with it | target × `cooldown`, less the burn |
| few targets (2–3) | 45 a second in total | 45 × `cooldown` ÷ `targets` |
| bouncing chain | 50 a second over the whole chain | 50 × `cooldown` ÷ (1 + 0.92 + 0.92² + …, one term per target) |
| random hits (`Chain`) | 60 a second in total | 60 × `cooldown` ÷ `targets` |
| finisher (`Finisher`) | 40 a second in total, twice that on enemies under 30 % health | 40 × `cooldown` ÷ `targets`; few-target finishers use 45 |
| over time (`DamageOverTime`) | 100 damage to each enemy for every 4 s it lasts | `damage` (the total of all ticks) = 100 × `duration` ÷ 4 |
| fixed total (over time) | a total to each enemy set by hand for that spell, whatever it lasts | `damage` = that total |
| fixed over a pack (over time) | a damage per second set by hand, over every target's whole effect | `damage` = target × `cooldown` ÷ `targets` |
| totem (`Totem`) | 20 a second to each enemy it hits, counting all its pulses over its `cooldown` (Magma Totem 5); duration upgrades add pulses and cooldown upgrades let totems overlap | `damage` (one pulse) = 20 × `cooldown` ÷ pulses |
| own numbers: areas, utility (stuns, roots, fear, slows), buffs, heals, shields | none: their numbers are set one by one | — |

Effects over time deal no up-front hit; `damage` is spread evenly over the ticks.

The test `every_damaging_class_spell_hits_its_role_s_damage_target`
(`crates/esurv-sim/tests/spell_roles.rs`) checks every card spell against its role,
within 3 %, using the sim's own damage formulas; give a new card spell a role there
(`every_class_spell_has_a_role` refuses one without).

**The card spells, class by class.** Their numbers as in `abilities.ron`: `cooldown` in seconds;
`damage` a hit, or for ground areas and effects over time the total over their ticks; a
totem's one pulse.

**Warrior:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Cleave | few targets (3) | 3 | 45 to each of 3 | |
| Mortal Strike | fixed, 50 a second | 6 | 300 | |
| Bloodthirst | fixed, 35 a second | 6 | 210 | 5 healing charges, each 4 % of max health |
| Execute | finisher | 3.5 | 140 | twice that under 30 % health |
| Shield Slam | few targets (2) | 6 | 135 to each of 2 | 1.5 s stun |
| Rend | fixed total, 150 | 3 | 150 over 7 s (7 ticks) to each of 3 | |
| Thunder Clap | own numbers | 7 | 24.5 to each enemy | 50 % slow for 3 s |
| Battle Shout | own numbers | 25 | — | +30 % damage for 10 s |
| Demoralizing Shout | own numbers | 10.5 | 4.2 to each enemy | weakens by 35 % for 6 s |

**Paladin:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Exorcism | fixed, 50 a second | 8 | 400 | |
| Judgement | fixed, 50 a second | 6 | 300 | |
| Hammer of Wrath | fixed, 40 a second | 4 | 160 | double on enemies under 30 % health, checked as the hammer lands (`double_on_low_health: true`) |
| Holy Shock | random hits (2) | 8 | 240 to each of 2 | heals 10 % of max health per hit |
| Consecration | own numbers | 13.5 | 136 to each enemy over 8 s (16 ticks of 8.5) | |
| Holy Wrath | own numbers | 10 | 67 to each enemy | |
| Hammer of Justice | own numbers | 10 | 59.5 to each of 2 | 3 s stun |
| Holy Light | own numbers | 16.5 | — | heals 35 % of max health, once health is at 65 % or less (`heal_below: Some(0.65)`) |
| Divine Shield | own numbers | 60 | — | 6 s immunity; its cooldown starts once the immunity ends |

**Hunter:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Arcane Shot | fixed, 45 a second | 3 | 135 | |
| Aimed Shot | fixed, 50 a second | 10 | 500 | |
| Multi-Shot | few targets (3) | 6 | 90 to each of 3 | |
| Serpent Sting | fixed total, 150 | 3 | 150 over 7 s (7 ticks) to each of 2 | |
| Concussive Shot | own numbers | 8 | 34 to each of 2 | 60 % slow for 4 s |
| Explosive Trap | own numbers | 8 | 30.5 to each enemy | |
| Immolation Trap | own numbers | 12 | 100 to each enemy over 8 s (8 ticks of 12.5) | |
| Volley | own numbers | 13.5 | 136 to each enemy over 8 s (8 ticks of 17) | |
| Frost Trap | own numbers | 10 | 25.5 to each enemy over 8.4 s (4 ticks) | 1.5 s root |

**Rogue:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Backstab | few-target finisher (2) | 4 | 90 to each of 2 | twice that under 30 % health |
| Ghostly Strike | fixed, 30 a second | 4 | 120 | a shield of 8 % of max health |
| Garrote | over time | 6 | 450 over 18 s (6 ticks) to each of 2 | |
| Blade Flurry | own numbers | 4 | 21 to each enemy | |
| Fan of Knives | own numbers | 6 | 40 to each enemy | |
| Kidney Shot | own numbers | 8 | 20 to each of 2 | 2.5 s stun |
| Gouge | own numbers | 6 | 30.5 to each of 3 | 1.5 s stun |
| Slice and Dice | own numbers | 20 | — | +25 % damage and swings 20 % faster (`kit: (swing_speed: 0.2)`) for 8 s |
| Adrenaline Rush | own numbers | 30 | — | +60 % haste for 12 s |

**Priest:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Mind Blast | fixed, 50 a second | 6 | 300 | |
| Shadow Word: Pain | over time | 3 | 210 over 8.4 s (8 ticks) to each of 2 | |
| Devouring Plague | fixed over a pack, 92.4 a second (66 % of Shadow Word: Pain's 140) | 6 | 185 over 8.4 s (8 ticks) to each of 3 | 30 % of the damage heals the hero |
| Holy Nova | own numbers | 6 | 34 to each enemy | heals 4 % of max health, and 4 % more for each enemy hit |
| Mind Flay | own numbers | 4 | 45 to each enemy | 50 % slow for 2 s |
| Psychic Scream | own numbers | 15 | 6.5 to each enemy | 3 s fear |
| Divine Star | own numbers | 8 | 50 to each enemy near its path, each way | flies toward the nearest enemy and back |
| Power Word: Shield | own numbers | 15 | — | absorbs 25 % of max health for 16.8 s |
| Renew | own numbers | 15 | — | heals 30 % of max health over 10 s, once health is at 85 % or less |
| Power Infusion | own numbers | 30 | — | +40 % haste for 12 s |

**Shaman:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Earth Shock | fixed, 50 a second | 4 | 200 | |
| Chain Lightning | bouncing chain (5) | 4 | 47, 8 % less per jump | |
| Flame Shock | over time | 4 | 200 over 8 s (8 ticks) to each of 2 | |
| Frost Shock | few targets (2) | 4 | 90 to each of 2 | 50 % slow for 4 s |
| Magma Totem | totem, 5 a second | 13.5 | 17 a pulse to each enemy, lasts 8 s (4 pulses) | |
| Searing Totem | totem, 20 a second | 15 | 60 a pulse, lasts 10 s (5 pulses) | |
| Fire Nova | own numbers | 7 | 41.5 to each enemy | |
| Lightning Shield | own numbers | 20 | — | strikes melee attackers back for 20.5, lasts 20 s |
| Healing Wave | own numbers | 12.5 | — | heals 26.5 % of max health once health is at 85 % or less |

**Mage:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Fire Blast | fixed, 45 a second | 4 | 180 | |
| Pyroblast | fixed, 50 a second | 10 | 500 | |
| Arcane Missiles | random hits (4) | 4 | 60 a missile, 4 missiles | |
| Arcane Explosion | own numbers | 3 | 16.8 to each enemy | |
| Frost Nova | own numbers | 10 | 14.5 to each enemy | 3 s root |
| Cone of Cold | own numbers | 6 | 28.5 to each enemy | 60 % slow for 3 s |
| Blast Wave | own numbers | 12 | 42.5 to each enemy | 50 % slow for 3 s |
| Blizzard | own numbers | 12 | 140 to each enemy over 8 s (8 ticks of 17.5) | 40 % slow for 1.5 s |
| Flamestrike | own numbers | 10 | an impact of 46 to each enemy inside as it lands (`impact`), then 70 over 8 s (4 ticks of 17.5) to each enemy standing in it | |
| Ice Barrier | own numbers | 20 | — | absorbs 35 % of max health for 21 s |

**Warlock:**

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Searing Pain | fixed, 45 a second | 2.5 | 112.5 | |
| Death Coil | fixed, 25 a second | 9 | 225 | heals 50 % of the damage it deals |
| Corruption | fixed total, 220 | 2 | 220 over 8 s (8 ticks) to 1 enemy | |
| Curse of Agony | fixed total, 600 | 5 | 600 over 10 s (10 ticks) to 1 enemy | with its cooldown half its length it keeps two enemies cursed |
| Immolate | fixed total, 600 | 6 | 600 over 12 s (12 ticks) to 1 enemy | with its cooldown half its length it keeps two enemies burning |
| Drain Life | fixed total, 200 | 6 | 200 over 4 s (4 ticks of 50) | heals 60 % of each tick |
| Rain of Fire | own numbers | 12 | 102 to each enemy over 8 s (4 ticks of 25.5) | |
| Hellfire | own numbers | 6 | 13.5 to each enemy around the hero, 3 novas 1 s apart (`duration: 3.0, ticks: 3`) | |
| Fear | own numbers | 8 | 7.5 to each of 3 | 4 s fear |

**Druid** (its caster card pool; the Moonkin and Restoration options use the same):

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Wrath | own numbers | 1.7 | 34 to each of 2 | |
| Moonfire | fixed total, 115 | 2.3 | 115 over 7 s (7 ticks) to each of 3 | |
| Insect Swarm | fixed total, 33 | 3 | 33 over 8.4 s (8 ticks) to each of 3 | weakens by 15 % for 12 s |
| Hurricane | own numbers | 15 | 175 to each enemy over 10 s (10 ticks of 17.5) | 40 % slow for 1.5 s |
| Entangling Roots | own numbers | 8 | 45.5 to each of 3 | 3 s root |
| Rejuvenation | own numbers | 15 | — | heals 35 % of max health over 10 s, once health is at 85 % or less |
| Regrowth | own numbers | 12.5 | — | heals 26.5 % of max health once health is at 85 % or less |
| Thorns | own numbers | 30 | — | strikes melee attackers back for 17.5, lasts 30 s |

**Druid, Cat Form's card pool** (with Regrowth and Rejuvenation above):

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Shred | finisher | 2 | 80 | |
| Ravage | fixed, 40 a second | 2.5 | 100 | double on enemies above 80 % health (`kit: (double_above: 0.8)`) |
| Rake | over time | 6 | 225 over 9 s (3 ticks) to each of 2 | |
| Pounce | over time | 10 | 450 over 18 s (6 ticks) to 1 enemy | 2 s stun |
| Ferocious Bite | own numbers | at 5 combo points | 225 | |
| Swipe (cat) | own numbers | 3 | 47 to each enemy | |
| Faerie Fire (Feral) | own numbers | 6 | 20 to each of 3 | they take 10 % more damage for 10 s |
| Tiger's Fury | own numbers | 25 | — | +30 % damage for 10 s |
| Dash | own numbers | 20 | — | +40 % movement speed for 8 s |
| Powershift | own numbers | 15 | — | +40 energy at 20 energy or less |

**Druid, Bear Form's card pool** (with Faerie Fire (Feral), Regrowth and Rejuvenation above):

| Spell | Role | `cooldown` | `damage` | Also |
|---|---|---|---|---|
| Lacerate | fixed total, 150 a stack | 3 | 150 over 9 s (3 ticks) to each of 3 | stacks up to 3 on each enemy |
| Swipe | own numbers | 3 | 21.5 to each enemy | |
| Bash | own numbers | 10 | 59.5 to each of 2 | 3 s stun |
| Demoralizing Roar | own numbers | 10.5 | 4.2 to each enemy | weakens by 35 % for 6 s |
| Enrage | own numbers | 15 | — | +20 rage |
| Frenzied Regeneration | own numbers | 15 | — | heals 30 % of max health over 10 s, once health is at 85 % or less |
| Barkskin | own numbers | 15 | — | absorbs 25 % of max health for 16.8 s |

**A nova that repeats around the hero.** A nova (`Nova`) with
`ticks` of 2 or more goes off `ticks` times per cast, `duration ÷ ticks` seconds apart, the first
as it is cast: Hellfire's `duration: 3.0, ticks: 3` goes off at 0, 1 and 2 s. Each nova is
centred on where the hero stands at that moment: it follows the hero and leaves nothing on the
ground.

* Each nova strikes every enemy within the radius for the spell's `damage`, scaled as any hit is
  (hero damage, power, the class's damage knob, the damage tag coefficient and category damage),
  worked out as the spell is cast. Each enemy rolls for a crit on its own.
* Each shows the spell's look, carries its on-hit effect (and burn, if any), and is the spell's
  damage on the damage meter and in milestones, as the first one is. A milestone's free cast
  repeats too, and its repeats set off no milestones.
* It is one cast: only the first nova may use a healing charge.
* Duration upgrades add only whole novas at the same spacing (4.5 s holds 4 novas, 3.9 s holds
  3), as for ticks of effects over time; a category duration multiplier adds or removes novas of
  the same size.
* Pending novas stop when their hero falls or the run ends. Each hero's novas are their own.

**A hit that leaves a burn.** A spell with `burn` leaves, on each
enemy its hit lands on (a missile, a chain, a bouncing chain, a nova, a melee strike or a cone),
an effect over time of `burn.damage` in all, in `burn.ticks` even ticks over `burn.duration`
seconds; the first tick comes one tick's time after the hit lands. Holy Fire's is
`burn: Some((damage: 105.0, duration: 5.0, ticks: 5))`.

* It scales as the hit does: hero damage, power, the class's damage knob, and the spell's damage
  tag coefficient (section 8a) and category damage (section 8).
* Duration upgrades add whole ticks of the same size at the same rate, as for effects over time.
* It stacks and renews as the spell's effects over time do (one stack per enemy, more with the
  hero's extra stacks; a full enemy's stack that runs out first is renewed). Its ticks are the
  spell's damage on the damage meter and in milestones, as other ticks are; they use no healing
  charge. Like an effect over time, it rolls for a crit once, as it lands, and every tick keeps it.
* Lasting areas, totems and effects over time leave no burn.

**A ground area that lands with an impact.** A ground area (`GroundArea`)
with `impact` strikes every enemy inside its radius at the moment it is placed for `impact` at
once, then ticks over its `duration`. Flamestrike's is `impact: 46.0`, with `damage:
70.0` over 4 ticks.

* It scales as the area's hits do: hero damage, power, the class's damage knob, and the spell's
  damage tag coefficient (section 8a) and category damage (section 8). Duration upgrades leave it
  alone.
* Each enemy rolls for a crit on its own. It is the spell's damage on the damage meter and in
  milestones, and carries the spell's on-hit effect, as the area's ticks do; it uses no healing
  charge.
* It shows with the spell's own look; there is no separate visual.

**Bloodthirst's healing charges.** When Bloodthirst hits, the hero
gains the buff "Bloodthirst": 5 charges for 8 s (`effect: HealingCharges(charges: 5, share:
0.04)`, the 8 s its `duration`). Each of that hero's next weapon swings, and each of its spell
casts that hits at least one enemy, uses one charge and restores 4 % of the hero's maximum health.

* One charge per swing or per cast, however many enemies the cast hits (a Whirlwind striking six
  uses one). A cast whose hits land later (a missile, a chain) uses its charge when the first one
  lands; an effect over time uses one as it lands on an enemy.
* Ticks of effects over time, ticks of lasting areas, totem pulses, pets and milestone damage
  (splashes, explosions …) use none. Bloodthirst's own hit uses none.
* Bloodthirst hitting again while the buff is up starts it over: 5 charges, 8 s. Charges left
  when the time is up vanish; the last charge ends the buff.
* Each hero has their own charges.
* The 4 % grows as heals do (`heal_amount`: the square root of the spell's power, and the class's
  damage knob); duration upgrades lengthen the 8 s. Its upgrades: Power, Cooldown, Duration.
* The buff shows in the hero's buff frame with its time left and the charges left as its stacks;
  its tooltip says how much health one charge restores.

* **Totems** pulse first 0.5 s after being placed, then every **2 s** for as long as they last,
  so a longer duration means more pulses. Their `ticks` is not used.
  The 2 s is `TOTEM_PULSE` in `esurv-sim` `game::casting`.
* **Effects over time:** every enemy can carry up to the stack limit (1 + the hero's extra
  stacks), and **a cast puts at most one stack on any enemy**. The order is **bosses, then
  elites, then normal enemies from the healthiest down**: over several casts the first in line
  fills up before the next one gets anything. A spell with several `targets` puts one stack on
  each of the first enemies in line. An enemy at its limit is passed over, unless a stack would
  wear off before the spell's next cast; then that stack is renewed instead of a new one being
  added. A renewed stack keeps the ticks it had left on top of the new ones, its next tick still
  due when it was (no tick is lost, so a spell recast as it runs out
  deals its full damage).
* **Effects over time that stack on their own** (`kit: (mechanic: Some(StacksUpTo(max: 3)))`, Lacerate):
  the spell's limit is `max` stacks on each enemy (or the hero's limit,
  if more), the rest as above: one stack per enemy a cast, and one more renews the stack that runs
  out first (the oldest), sharing out what both had left.
* **Attacks that strike more** (`kit: (mechanic: Some(AttacksStrikeMore))`, Berserk):
  a buff of the spell's `duration` on the hero (once an enemy is within
  `buff_range`); while it is up, each melee strike, finisher, weapon swing and effect over time of
  the hero also strikes the spell's `targets` (+1 a Targets card) enemies nearest its first target,
  within 8 yd of it (`PROC_SPREAD`), for its full damage. The damage meter lists the extra hits
  under the attack that made them; each set of extra hits shows a small sweeping slash
  (`EXTRA_STRIKE_LOOK`).

* **Spells cast by milestones** (free casts, every-Nth casts, periodic casts) set off no
  milestones themselves, nor do their projectiles, areas, effects over time or totems, unless
  the spell has `free_casts_can_proc: true`. Other milestone damage (splashes, explosions …)
  may set off milestones up to three deep.
* **Effects over time show:** when one lands, its look plays on each enemy it reached, and it
  stays on the enemy as a debuff (icon, stacks, and the spell's lasting effect) until its last
  tick.
* **Race spells** play exactly as their tooltips say (classes.md, section 1a).
* **Area looks show their true size:** a nova, cone, lasting area or boss stomp shows its look
  on the ground grown or shrunk to the yards it really reaches (its `radius` with the hero's
  area bonus), measured against the spell's own radius in `Spell.dbc`. Effects on a body keep
  their size, and a totem's pulse (nothing on the ground) plays as it is. (server.md, "Area looks at
  their true size".)

## 6. On-hit effects

| `effect` | Effect |
|---|---|
| `None` | nothing extra |
| `Slow(amount: 0.5, seconds: 4.0)` | slows by `amount` (0.5 = 50 %) |
| `Root(seconds: 3.0)` | held in place (same as a stun) |
| `Stun(seconds: 2.0)` | held in place. Holds: bosses for a quarter of the time; a chain of holds ends after 3 s; then 2 s with no new hold |
| `Fear(seconds: 3.0)` | runs away from the hero |
| `HealFromDamage(share: 0.2)` | heals the hero for a share of the damage dealt; on an effect over time, a share of each tick's damage as the tick lands, nothing when it is applied |
| `Heal(share: 0.15)` | heals the hero for a share of max health: on every hit for damaging spells, once for novas and self effects |
| `Shield(share: 0.3, seconds: 10.0)` | absorb shield, a share of max health (on a hit: tops the shield up to that size) |
| `DamageBuff(amount: 0.2, seconds: 8.0)` | more damage for a while |
| `HasteBuff(amount: 0.2, seconds: 8.0)` | faster cooldowns for a while |
| `Weaken(amount: 0.2, seconds: 6.0)` | weakens the target: its attacks deal `amount` less |
| `Immunity(seconds: 3.0)` | the hero takes no damage |
| `Thorns(amount: 0.3)` | a lasting aura on the hero that strikes back at attackers |
| `HealingCharges(charges: 5, share: 0.04)` | the hero gains `charges` healing charges for the spell's `duration`; each later weapon swing or spell cast that hits uses one and heals `fraction` of max health (Bloodthirst; section 5c) |

On an effect over time (`DamageOverTime`), any effect but `HealFromDamage` lands once, as the effect is
applied to the enemy (Insect Swarm weakens it then); `HealFromDamage` heals from each tick instead.

The hero's **heal and shield power** (a stat; class gear's wrists
carry it, gear.md section 3) multiplies what `Heal`, `HealFromDamage` and `HealingCharges` heal
(instant and over time) and the size of a `Shield`, on top of buff power for shields. Health per
second, lifesteal and health per kill are not touched.

## 7. Upgrades

`upgrades` lists what level-up cards may improve: `Power`, `Cooldown`, `Targets`, `Area`,
`Duration`. An upgrade is only offered where it
makes sense for the delivery (for example `Area` only for novas, areas, cones and bouncing
chains) and while it can still grow; see classes.md, "Upgrade cards".

## 8. Categories: changing a whole kind of spell at once

`categories` says what kind of spell it is. A spell can be in several (Frost Nova is
`[DirectDamage, AreaOfEffect, CrowdControl]`). Categories: with the shipped settings they
change nothing. They are for two things:

* **Uniform changes** in `content/spell_categories.ron`: one line per category multiplies the
  numbers of every spell in it. All 1.0 (as shipped) leaves every spell at its own numbers.
* **Talents and amulets** can listen to a category: the milestone filter `Category(DamageOverTime)`
  (classes.md, section 4).

| Category | Meaning | Given to (by the importer) |
|---|---|---|
| `DirectDamage` | deals its damage in single hits | damage > 0 and delivery `Projectile`, `Chain`, `Nova`, `Bounce`, `Melee`, `Finisher` or `Cone` |
| `DamageOverTime` | deals its damage in ticks | damage > 0 and delivery `GroundArea`, `DamageOverTime` or `Totem` |
| `AreaOfEffect` | hits everything in an area | delivery `Nova`, `GroundArea` or `Cone`; a totem with a radius |
| `Melee` | strikes in melee reach | delivery `Melee` or `Finisher` |
| `Ranged` | strikes from range | delivery `Projectile`, `Chain` or `Bounce` |
| `CrowdControl` | slows, roots, stuns or fears | effect `Slow`, `Root`, `Stun`, `Fear` |
| `Sustain` | heals the hero | effect `Heal`, `HealFromDamage` |
| `Shield` | absorb shield on the hero | effect `Shield` |
| `DamageReduction` | lowers the damage the hero takes | effect `Shield`, `Immunity`, `Weaken` |
| `Reflect` | strikes back at attackers | effect `Thorns` |
| `Buff` | more damage or speed for a while | effect `DamageBuff`, `HasteBuff` |
| `Summon` | places a creature | delivery `Totem` (totems) |

The importer fills `categories` with these rules; for a new spell, pick the ones that fit by
the same rules (the template starts with `[DirectDamage, Ranged]`). Every spell needs at least
one (the tests check it).

**`spell_categories.ron`** (applied when content loads; a missing number counts as 1.0):

| Number | Multiplies |
|---|---|
| `damage` | damage per hit, the total for ground areas and effects over time, or one pulse of a totem |
| `cooldown` | seconds between casts (lower = more often) |
| `range` | reach |
| `radius` | area size (for a cone, its length) |
| `duration` | how long the spell itself lasts: areas, effects over time, totems, and buffs, shields and immunity on the hero. Ticks keep their size and rate, so a longer effect over time also deals more in total (6 ticks of 10 over 6 s with `duration: 1.5` become 9 ticks of 10 over 9 s). Only whole ticks count: with `duration: 1.25` the same spell lasts 7.5 s and ticks 7 times, not 8. A totem likewise pulses only while it stands. |
| `effect` | strength of the on-hit effect: slow, heal, shield, buff, weaken or aura amount (slows and weakening stop at 100 %) |
| `effect_seconds` | how long the on-hit effect holds an enemy: slow, root, stun, fear, weaken |

A spell in several categories gets every matching line, multiplied together: with
`DirectDamage` damage 1.5 and `AreaOfEffect` damage 0.5, Frost Nova deals 0.75 ×. The changed
numbers are what the game, its tooltips and its upgrade cards then work from.

Example: every effect over time 10 % stronger and 20 % longer:

```ron
(category: DamageOverTime, damage: 1.1, duration: 1.2),
```

## 8a. Spell tags

Every spell has `tags` saying what it is about, and each tag has
a coefficient in `content/balance.ron` (`spell_tags`), all 1.0 as shipped, so the tags change
nothing in play. A coefficient scales only the part of the spell its tag is about, never
cooldowns or durations (nor tick counts, effect lengths or ranges). `tags` is a separate field
from `categories` (section 8), which other systems use.

| Tag | Coefficient | Given to | Scales |
|---|---|---|---|
| `Level1` | `level_1` | the starting options' spells | damage |
| `Nuke` | `nuke` | damage to 1 target | damage |
| `Cleave` | `cleave` | damage to 2–5 targets (missiles, chains, melee sweeps) | damage |
| `Dot` | `dot` | damage over time (`DamageOverTime`) | damage |
| `Nova` | `nova` | damage around the hero (`Nova`), and cones (`Cone`) | damage |
| `Area` | `area` | lasting ground areas (`GroundArea`) | damage |
| `Pet` | `pet` | creatures that fight for the hero: totems (`Totem`) | damage; also every talent pet's damage (`pets.ron`) |
| `Sustain` | `sustain` | spells that heal the hero | `Heal` (self heals and heals on hit), `HealFromDamage`, `HealingCharges` (each charge's heal) |
| `Buffs` | `buffs` | damage and haste buffs | `DamageBuff` and `HasteBuff` amounts |
| `Defensive` | `defensive` | shields, immunity, reflected damage, weakening enemies | `Shield` share, `Weaken` amount, `Thorns` (reflected) damage; `Immunity` has no amount and is left alone |

Rules for tagging:

* A spell that deals damage (`damage` > 0) has **exactly one damage tag** (`Level1` … `Pet`),
  chosen by its shape. A spell that deals none has no damage tag.
* `Sustain`, `Buffs` and `Defensive` go on every spell whose `effect` is of their kind, alone or
  on top of the damage tag (Bloodthirst is `[Nuke, Sustain]`).
* Several coefficients on one spell multiply, and the class's `spell_damage_scale` multiplies on
  top. Race spells have no tags and take no coefficient (nor the class knob): they are tuned
  one by one.
* The tests check every spell against these rules (`every_spell_is_tagged_by_its_shape_and_effect`).

The coefficients are applied to the spells' numbers when content loads (after
`spell_categories.ron`), so the game, its tooltips and its cards all work from the changed
numbers. In `balance.ron` (a missing number counts as 1.0):

```ron
spell_tags: (
    level_1: 1.0, nuke: 1.0, cleave: 1.0, dot: 1.0, nova: 1.0, area: 1.0, pet: 1.0,
    sustain: 1.0, buffs: 1.0, defensive: 1.0,
),
```

Example: every nuke 10 % harder and every heal 20 % stronger: `nuke: 1.1, sustain: 1.2`.

**Every spell's tags:**

| Tags | Spells |
|---|---|
| `Level1` | the starting options' spells and what their passives cast: starshards, claw, rip, maul, friend_of_the_forest, critter_strike, emerald_slumber, drain_soul, seed_of_corruption, hand_of_guldan, imp_firebolt, infernal_strike, infernal_immolation, soulfire, arcane_blast, hot_streak, colossal_smash, rampage_strike, rampage_slam, revenge_strike_back, water_elemental_nova, lightning_rod, windfury_weapon, maelstrom_lightning, wellspring, ancestral_lightning, ancestral_flame, inner_fire_burn, holy_echo, mind_spike, eye_tentacle, claw_tentacle, mutilate, deadly_poison, envenom, hemorrhage, steady_shot, kill_command, hunter_pet_attack, explosive_shot, denounce, arcane_surge, zeal_strike, reckoning, seal_of_command, slam, revenge, whirlwind, holy_shield, sinister_strike, eviscerate, rupture, smite, holy_fire, lightning_bolt, fireball, frostbolt, shadow_bolt, starfire |
| `Level1, Sustain` | radiance |
| `Nuke` | ferocious_bite, shred, ravage, faerie_fire_feral, mortal_strike, execute, exorcism, hammer_of_wrath, judgement, auto_shot, arcane_shot, aimed_shot, mind_blast, earth_shock, fire_blast, pyroblast, searing_pain |
| `Nuke, Sustain` | bloodthirst, death_coil |
| `Nuke, Defensive` | ghostly_strike |
| `Cleave` | bash, cleave, shield_slam, hammer_of_justice, multi_shot, concussive_shot, kidney_shot, gouge, backstab, chain_lightning, frost_shock, arcane_missiles, fear, wrath, entangling_roots, penance |
| `Cleave, Sustain` | holy_shock |
| `Dot` | rake, pounce, lacerate, garrote, rend, serpent_sting, sw_pain, flame_shock, corruption, immolate, curse_of_agony, moonfire |
| `Dot, Sustain` | devouring_plague, drain_life |
| `Dot, Defensive` | insect_swarm |
| `Nova` | divine_star, fan_of_knives, thunder_clap, holy_wrath, explosive_trap, blade_flurry, mind_flay, psychic_scream, fire_nova, frost_nova, arcane_explosion, cone_of_cold, blast_wave, hellfire, swipe, cat_swipe |
| `Nova, Sustain` | holy_nova |
| `Nova, Defensive` | demoralizing_roar, demo_shout |
| `Area` | consecration, volley, immolation_trap, frost_trap, blizzard, flamestrike, rain_of_fire, hurricane |
| `Pet` | magma_totem, searing_totem |
| `Sustain` | frenzied_regeneration, ancestral_heal, holy_light, renew, healing_wave, rejuvenation, regrowth |
| `Buffs` | tigers_fury, power_infusion, battle_shout, adrenaline_rush, slice_and_dice, arcane_power |
| `Defensive` | barkskin, divine_shield, pw_shield, lightning_shield, ice_barrier, thorns |
| none | powershift, dash, enrage, berserk |

Penance and Arcane Power are talent-granted. Race spells (perception, blood_fury, berserking,
stoneform, shadowmeld, will_of_the_forsaken, war_stomp, escape_artist) have no tags.

## 9. What comes for free

* **Legendary amulets** remake any spell by its delivery kind; a new spell gets one too.
* **Talents** can name the spell by `id` (`AbilityMod`), or reach it by school or category.
* **Category lines** in `spell_categories.ron` apply to it as soon as it lists the category.
* **Tag coefficients** in `balance.ron` apply to it as soon as it lists the tag.
* **Spell cards** at higher rarities start it stronger.
