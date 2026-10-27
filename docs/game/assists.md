# Assists

A battle standard lying on the field summons a hand-made player character to fight beside the
hero for 15 seconds: its signature assist first, then its rotation of real 1.12 spells, moving and
casting as a player does. How to add a character and write its assist: [AUTHORING.md](AUTHORING.md),
section 13a; a ready block: [templates/player_character.ron](templates/player_character.ron).

## The battle standard

- **When:** 1 or 2 per run (picked at random each run), each at a random time between the first
  elite (4:00) and the final boss, on the caches' ring; it lies until a hero takes it. In Endless
  (after the final boss) one more comes every 10 minutes. None ever lies in Warsong Gulch (`wsg`),
  where these characters are the bosses.
- **Its look:** the hero's faction's Alterac battle standard planted in the ground, with a soft
  glow (`CreatureDisplayInfo.dbc` 14541, the Alliance Battle Standard, for Alliance heroes; 14542,
  the Horde Battle Standard, for Horde heroes), planted with its faction's Battle Standard spell's look
  (`Spell.dbc` 23034, 23035). It pings the minimap where it lies, and a beam of light in its
  faction's colour stands over it. As it is laid the screen says "A Warband Standard was raised
  somewhere. Follow the beam of light."
- **Taking it:** walking over it, like a rune. The screen names who came: "<name> answers your
  call!" (e.g. "Pikaboo answers your call!").
- **The numbers:** `content\balance.ron`, `battle_standard`.

## What it looks like in the game

- **The standard:** the faction's Alterac battle standard (`battle_standard.display`, a creature
  model: the Alliance or the Horde Battle Standard) planted in the ground, its
  faction's Battle Standard look playing on it as it is planted (`battle_standard.planted_look`),
  with a short aura glow under it in the faction's colour (`AuraBlueShort` for the Alliance,
  `AuraRedShort` for the Horde), the stock minimap ping where it lies (the pulsing ring, 5 s), and
  a beam of light over it in the faction's colour, the magnet's beam without its sparkle
  (`AuraBlueVeryTall` and `AuraBlueTall` for the Alliance, `AuraRedVeryTall` and `AuraRedTall` for
  the Horde); the beam goes when it is taken.
- **The ally:** its character as the Warsong bosses show it (race, gender, looks, form, gear),
  drawn at `ally_scale` (2), a friendly player that cannot be selected. Its arrival: `Plain` it is
  there; `Blink` Blink's look; `Stealth` it comes see-through and fades in; `Portal` the arrival's
  look (Ritual of Summoning's). Its yells come as a boss's do, under its name. Its leaving: `Fade`
  it fades see-through and goes; `Vanish` the puff; `Hearth` a short Hearthstone cast; `RunAway`
  it runs off; `Dies` it falls and its body lies a moment.
- **What it does:** its steps' looks from it (missiles fly from it, impacts land on what it
  struck), its melee strikes as its own swings with their numbers, the looks it puts on enemies
  (the sheep, Blind, a bleed) on them while they hold, its buffs' looks on the hero; an enemy it
  taunted strikes it for nothing ("Absorb"). Areas, totems (its race's totem look) and pets it
  leaves show as the hero's.
- **Its rotation:** each spell as a player casts it: a spell with a cast time shows its cast bar
  and the cast animation (`SMSG_SPELL_START`), then goes off (`SMSG_SPELL_GO`: its missile, its
  impact on what it struck, a ground spell's look on the ground, a heal's on the hero); an instant
  goes off at once. It runs as a player runs; a rogue strafes left and right in melee with the
  strafe's animation and jumps, and a frost mage jumps back after Frost Nova, as a player's moves
  (the client's own strafe and jump, arc and landing). A channel (Blizzard) goes off at once and
  the ally holds the 1.12 channel look for its whole time: its channel spell and the storm it
  channels set on it (`UNIT_CHANNEL_SPELL`, `UNIT_CHANNEL_OBJECT`, as the hero's Drain Soul and the
  bosses' beams), its storm on the ground, standing still until the channel ends. An ally that
  only auto attacks swings at its enemy with the attack animation (`SMSG_ATTACKERSTATEUPDATE` with
  its hit).
- **For a look at it** (dev switches, [server.md](server.md#switches)): `ESURV_STANDARDS=<s>,...`
  lays a standard at the hero's feet at those run clocks (taken at once),
  `ESURV_ASSISTS=<id>,...` picks the characters they summon, in order, and
  `ESURV_GRANT=battle_standard:<yards>` lays one ahead of the hero to look at. Every ally stays
  its 15 sec (Vodu: until his taunt is over; Eggmike: until her portal is cast), looping its
  rotation after its script.

## The summon

Taking it summons one random character from `content\player_characters.ron` of the hero's own
faction (its race's faction), not one already summoned this run while others remain. It appears
beside the hero (or where its script says), drawn at twice its size, yells its `yell`, performs its
assist (its script, the opener), loops its rotation until its stay is up, then yells its
`death_yell` (if any) and leaves. Every summon stays 15 sec (`balance.ron`
`battle_standard.assist_seconds`)
unless its script gives another time; what it leaves behind (a ground area, a buff, a totem, a
bleed) runs out on its own time.

The ally cannot be hurt or targeted by the hero; enemies attack it only while it taunts them.

## One list

`player_characters.ron` is the only place a character lives. The Warsong bosses (`played_by`) and
the assists both read it: a character added there is a Warsong boss of its faction and an assist
of its faction, with no code change and no second file. Its assist is a short script in its own
entry (`assist_script`), beside the `assist` text a player reads. A character without a script
takes its class's default (its form's, for a druid shown in a form) from
`content\class_assists.ron`; the game's log names every character still on a default. Its
rotation is its `behaviour` beside the script, a move style and spell ids (each with its row in
`content\assist_spells.ron`); without one it takes its class's (and form's) default from
`class_assists.ron`, and the log names it too.

## Rules

- **Its damage, one rate for every assist** (`balance.ron`
  `battle_standard.assist_damage_per_second`): **5 % of the enemy's max health a second on a
  normal enemy, 1 % on an elite, a rare or a boss.** Each damage event deals the rate for the
  struck enemy's kind × its max health × the seconds the event stands for:

  | Event | Stands for |
  |---|---|
  | a hit of the script (an opener) | one global cooldown, 1.5 sec |
  | a hit of a repeat in the script | the repeat's `every` |
  | a rotation cast landing | its cycle: its cast time, at least the 1.5 sec global cooldown |
  | an auto attack swing | `AUTO_ATTACK_SECONDS`, 2 sec |
  | a channel's tick (Blizzard) | its tick interval |
  | a damage over time tick | its `every` |
  | a ground area's pulse | its `every` |
  | a pet's attack or bolt | the pet's attack interval (`swing` in `pets.ron`) |

  A step deals damage when it says `damage: true` (a hit, an area); an effect over time always
  does, and so do the pets of a summon. Which steps deal damage, and how many enemies each picks,
  is the script's; the amount is always the rate's. A rotation cast or a channel tick is one
  event: each enemy it strikes takes it once, however many of the round's steps pick it. The same
  assist putting its damage over time on an enemy again renews it, and laying a ground area of the
  same look again ends its earlier one.
- **Crowd control:** on a boss or a rare, an assist's stuns, roots, fears, polymorphs, blinds and
  saps follow the hero's own rules: held for a quarter of the time, a chain of holds capped and
  followed by immunity, and fear and polymorph never work on a boss. On any other enemy an
  assist's stun or root holds its full written time, outside the hero's chain of holds (the hero's
  own holds are unchanged). A taunt on a boss lasts a quarter. The assist Blizzard's slow goes
  through the hero's own slow, so a boss or a rare takes it as it takes a hero's. ("No boss crowd
  control" is about bosses never stunning, fearing, rooting or slowing the player; it does not
  stop an assist's crowd control on enemies.)
- **Only as an assist:** all of this (the script, the rotation, auto attacks, the channelled
  Blizzard) runs only for a character a battle standard summons. The same characters as Warsong
  enemies or bosses (`played_by`) keep their own boss behaviour (`boss_spell`, `boss_abilities`)
  and never run an assist's script or rotation.
- **Its own damage:** an assist's hits (its pets' too) take no hero bonus, never crit, and set off nothing of the
  hero's (no milestone, proc, lifesteal, heal per kill or resource). A kill by an assist is still
  the hero's for its rewards: experience, gold, drops.
- **Its rotation** strikes again what the script's damaging steps strike, at the rate (see "Its
  rotation" below), and spreads the script's heals over the stay. It puts on no crowd control,
  buff, taunt or summon: those stay the script's, once. An ally that only auto attacks swings at
  the rate.
- **The damage meter** shows an assist's damage on a row of its own, "Assist: <name>".
- **Never against the hero:** an assist only helps the hero (its buffs are positive, its effects
  land on enemies).

## How an assist runs

The simulation's side (`crates/esurv-sim/src/game/assists.rs`):

- **The standards' times** come from the run's seed on their own draw (as the rares' count), so a
  run with the same seed lays them at the same times; where each lies on the ring is drawn as the
  caches' spots are. A zone without a final boss spreads them over 16 minutes, as the rares.
- **The pick:** among the hero's faction's characters, one of those summoned the fewest times this
  run, at random: none comes a second time before every other has come once.
- **Where it stands:** 2.5 yd to the hero's left (where a pet appears), or 1.6 yd short of the enemy
  its arrival picks (where a melee pet stops). A move goes 1.6 yd short of one enemy, or to the
  middle of several. It runs at the hero's base run speed (7 yd/s) and charges at a boss's charge
  speed (24 yd/s); a blink, and the step beside an enemy a melee hit strikes, take no time.
- **It stays 15 sec** (its script's `seconds`, else `assist_seconds`): its script, then its
  rotation. A taunt's written time (10 sec at most) falls within it.
- **Steps** run one after another; a repeat's first pass goes at once and the next every `every`
  seconds, and the repeat takes `every × times`. A script still running at the end of the stay
  leaves whatever is left. The ally also leaves at once when its hero falls.
- **Its hits** go to the enemy at the rate: a boss's shield still takes its share, but nothing
  else changes them (not even a vulnerability on the enemy). The pets it summons fight beside the
  hero and stay their time after it has gone; each of their attacks deals the rate for the pet's
  attack interval (never a crit, setting off nothing of the hero's; a ranged pet's bolt deals it
  as it lands). A pet that
  flies (`hover` in `pets.ron`) hovers that high above the ground, bobbing gently.
- **Crowd control** on a boss or a rare is put on exactly as a hero's spell puts it on. The mark an enemy wears (`wears`) shows for as long as the
  effect really holds, and not at all when it does not take (a fear on a boss).
- **A taunt:** the taunted enemies walk to the ally and swing at it at their own pace (it takes
  nothing; a ranged enemy drops its cast and comes too); their abilities go on as before. A taunt
  on an enemy that dies ends, but the ally stays its written time.
- **Ground areas** strike at once, then every `every` seconds while they last. **Bleeds** tick
  every `every` seconds, the first after `every`, the last at their end. An area with `most` strikes
  only that many each time, the nearest its middle.
- **At most 5 at a time** (for performance with a crowd on screen; each target still takes its
  rate): Wayze's Thunderfury proc goes on the 5 enemies nearest the hero (his stun still
  takes every enemy on screen), so each of his rotation casts strikes 5 too; Doogz's and Scarfex's
  Garrote likewise (10 sec on each of the 5 nearest; their stun still takes every enemy on
  screen); Snutz's rain of fire (his script's and each rotation cast's) strikes at most 5 enemies
  a pulse, the nearest its middle, so each cast's Corruption, Curse of Agony, Shadow Bolt or Death
  Coil shows on at most 5. With 60 enemies about, a stay sends about ten times fewer hits and
  impact looks (Wayze 660 hits before, now 55; Snutz 3519, now 320). Moffel's Thunderfury
  lightning and its debuff show on her first swing only (our rule, owner), her eight swings
  still striking the 5 nearest her: 5 lightning looks a stay instead of 40.
- **A two-hander** in a character's main hand leaves its off hand empty (no class shield under
  it): Noami holds Sulfuras, Hand of Ragnaros (item 17182, display 29698) in both hands.
- **Buffs** add their stats while they last (a raised max health raises health with it); their
  heals come every `every` seconds, the first after `every` (each second when `every` is left out).
  A totem is planted 1.5 yd to the hero's side and stands the buff's time. Heals are a share of max
  health as written: the hero's heal power does not raise them.

## Its rotation

What the ally does once its script is done (`crates/esurv-sim/src/game/assist_rotation.rs`):

- **The spells:** its `behaviour`'s `rotation`, 1.12 spells by `Spell.dbc` id (the top ranks a
  level-60 player casts), cast in order, a spell skipped for the next ready one while its cooldown
  runs. Each spell's cast time and cooldown are `Spell.dbc`'s (`content\assist_spells.ron`, checked
  against the client by `cargo run -p esurv-content --features client --example
  check_assist_spells`); a 1.5 sec global cooldown runs between casts, and a cast with a cast time
  lands when it is done (the next one waits for it). A channel (Blizzard) is channelled for its
  `Spell.dbc` time, its ticks at its periodic interval and its radius (`channel`, `tick_every`,
  `radius` in the table, read by the same example); the next cast waits for its end.
- **Where it is aimed** (`assist_spells.ron` `on`): `Enemy` the one nearest the ally (shown on the
  first its round struck), `Hero` the hero (heals), `Caster` itself (novas, shouts, seals,
  Whirlwind), `Ground` where its round's area went, else under the enemy nearest it.
- **The plan:** at the summon the casts are laid out from the script's written end to the end of
  the stay, so how many there will be is known; every one of them is made. A script whose moves
  took time, or a cast with a cast time waiting while the ally runs (5 sec at most in all), shifts
  the rest of the plan, and the ally stays until its last planned cast has landed.
- **Its damage:** the script's damaging steps and heals as one round (its damaging hits, its
  damage over time, its damaging ground areas and its heals, a repeat's once per pass; every
  effect, wear and look left out; a step that only picks, a stun or a move, kept as a pick so
  `Same` still means what it meant). Each rotation cast, as it lands, strikes the round's picks
  again, each enemy once, for the rate over the cast's cycle (its cast time, at least the 1.5 sec
  global cooldown); a damage over time of the round is renewed and ticks at the rate, and a
  ground area of the round is laid again (its earlier one ends) and pulses at the rate. The
  script's heals are spread: the script heals `share` = 1 / (1 + casts) of each, and each cast
  heals the round's heals once more at that share. A script with no damage and no heal
  (Phigaro's Expose Armor, Vodu's taunt) gives its rotation nothing to deal: looks only. Buffs,
  totems, pets and crowd control stay the script's, once.
- **A channel: the assist's Blizzard** (unlike the hero's Blizzard): the ally stops and
  channels for the spell's 8 sec (`Spell.dbc`), its storm on the ground under the enemy nearest it,
  8 yd wide (the spell's radius). The cast's round comes in the channel's ticks (one a second,
  eight in all): each tick strikes its hits' own picks, and a ground area of the round strikes
  the enemies in the storm, each enemy once, for the rate over the tick's second; its heals come
  an even eighth a tick. Each tick slows the enemies in
  the storm by 40 % until the next tick, so the slow ends a second after an enemy
  leaves the storm or the channel ends. If the ally leaves mid-channel (its hero falls), the ticks
  stop and the storm goes.
- **Auto attacks only** (for Stormwall, Edgar and Phigaro): a `Melee` or
  `ErraticMelee` behaviour with an empty rotation casts nothing after the script; it runs to the
  enemy nearest it as `Melee` does and, in melee, swings at it every 2 sec (a 1.12 unit's base
  attack time), with the attack animation. Each swing deals the rate for its 2 sec (10 % of a
  normal enemy's max health, 2 % of an elite's, a rare's or a boss's) as any of its hits: logged
  as its own swing with its number, on its meter row, no hero bonus, a boss's shield taking its
  share.
- **Nothing after the script** (`LeavesAfterScript`, for Vodu and Eggmike): no
  rotation and no move; the ally leaves as soon as its script is done and no taunt of its holds,
  so Vodu runs in, drinks, taunts for 10 sec and dies, and Eggmike buffs the hero (15 % more
  damage for 1 min), casts
  her portal and hearthstones away at once.
- **A caster with an empty rotation** casts nothing after its script and keeps beside the hero
  until its stay is up, then leaves its way.
- **How it moves** (`style`): `Melee` runs to the enemy nearest it and stays 1.6 yd from it,
  strafing and jumping round it as `ErraticMelee` does (our rule, owner: melee allies never stand
  still as they strike, in their script too; one stands only while a taunt of its holds);
  `ErraticMelee` also, in melee, strafes left, then right (0.4 sec each at its run speed, facing its
  enemy) and every third time jumps sideways instead (a 1.12 jump's 0.82 sec in the air); `Caster`
  stands where it is and runs back beside the hero (2.5 yd to its left) once the hero is more than
  8 yd away; `NovaKiteCaster` also, after an instant cast with an enemy within 8 yd, jumps 5.8 yd
  straight away from it; `Healer` as `Caster`, following once the hero is more than 4 yd away. It
  never moves while it casts a spell with a cast time or channels; instants go off on the run.
  `LeavesAfterScript` does not move.

Each character's rotation (the class defaults in `class_assists.ron` follow one of them):

| Character | Style | Rotation |
|---|---|---|
| Wayze | ErraticMelee | Sinister Strike, Sinister Strike, Eviscerate, Kidney Shot |
| Doogz, Scarfex | ErraticMelee | Sinister Strike, Rupture, Eviscerate |
| Phigaro | Melee | auto attacks only, a swing every 2 sec |
| Dikkedop | ErraticMelee | Sinister Strike, Sinister Strike, Eviscerate |
| Mir, Rhys | ErraticMelee | Blade Flurry, Sinister Strike, Eviscerate |
| Pikaboo | ErraticMelee | Hemorrhage, Hemorrhage, Eviscerate, Kidney Shot |
| Ahlaundoh, Ghorktheorc | Melee | Mortal Strike, Whirlwind, Heroic Strike |
| Summit1g | Melee | Mortal Strike, Whirlwind, Cleave |
| Moffel | Melee | Mortal Strike, Whirlwind, Hamstring |
| Politimann | Melee | Mortal Strike, Sunder Armor, Whirlwind |
| Stormwall, Edgar | Melee | auto attacks only, a swing every 2 sec |
| Quintys | Melee | Thunder Clap, Sunder Armor, Shield Slam, Revenge |
| Xqc, Iberya | Melee | Seal of Command, Judgement, Hammer of Justice |
| Vodu | LeavesAfterScript | none: he dies once his taunt is over |
| Noami | Melee | Consecration, Judgement, Hammer of Wrath |
| Cdevv | Melee | Stormstrike, Earth Shock, Flame Shock |
| Sodapoppin (both) | Melee | Rake, Claw, Ferocious Bite |
| Pawsz | Melee | Shred, Shred, Rip, Ferocious Bite |
| Xaryu, Pkr | NovaKiteCaster | Frost Nova, Blizzard, Frostbolt |
| Venruki | NovaKiteCaster | Frost Nova, Frostbolt, Cone of Cold |
| Nerf | NovaKiteCaster | Frost Nova, Flamestrike, Blizzard |
| Eggmike | LeavesAfterScript | none: her 1 min buff (15 % more damage), her portal, then she hearths |
| Shroud | Caster | Chain Lightning, Lightning Bolt, Frost Shock |
| Snutz | Caster | Corruption, Curse of Agony, Shadow Bolt, Death Coil |
| Tebay | Caster | Arcane Shot, Multi-Shot, Serpent Sting, Aimed Shot |
| Regiss | Caster | Moonfire, Insect Swarm, Starfire |
| Cdew | Healer | Chain Heal, Lesser Healing Wave, Healing Wave |
| Banksey | Healer | Flash Heal, Holy Nova, Renew |
