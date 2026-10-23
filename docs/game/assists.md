# Assists

A battle standard lying on the field summons a hand-made player character to fight beside the
hero for a few seconds. How to add a character and write its assist: [AUTHORING.md](AUTHORING.md),
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
- **For a look at it** (dev switches, [server.md](server.md#switches)): `ESURV_STANDARDS=<s>,...`
  lays a standard at the hero's feet at those run clocks (taken at once),
  `ESURV_ASSISTS=<id>,...` picks the characters they summon, in order, and
  `ESURV_GRANT=battle_standard:<yards>` lays one ahead of the hero to look at. An ally whose steps
  are all instant still stays 5 sec.

## The summon

Taking it summons one random character from `content\player_characters.ron` of the hero's own
faction (its race's faction), not one already summoned this run while others remain. It appears
beside the hero (or where its script says), drawn at twice its size, yells its `yell`, performs its
assist, then yells its `death_yell` (if any) and leaves. An assist lasts at most 10 sec unless its
script gives a longer time; what it leaves behind (a ground area, a buff, a totem, a bleed) runs
out on its own time.

The ally cannot be hurt or targeted by the hero; enemies attack it only while it taunts them.

## One list

`player_characters.ron` is the only place a character lives. The Warsong bosses (`played_by`) and
the assists both read it: a character added there is a Warsong boss of its faction and an assist
of its faction, with no code change and no second file. Its assist is a short script in its own
entry (`assist_script`), beside the `assist` text a player reads. A character without a script
takes its class's default (its form's, for a druid shown in a form) from
`content\class_assists.ron`; the game's log names every character still on a default.

## Rules

- **Damage as a share of max health** (`MaxHealthShare`): on a boss or a rare, a hit deals at most
  what it would deal to a normal enemy of that zone at that minute.
- **Crowd control:** on a boss or a rare, an assist's stuns, roots, fears, polymorphs, blinds and
  saps follow the hero's own rules: held for a quarter of the time, a chain of holds capped and
  followed by immunity, and fear and polymorph never work on a boss. On any other enemy an
  assist's stun or root holds its full written time, outside the hero's chain of holds (the hero's
  own holds are unchanged). A taunt on a boss lasts a quarter.
- **Its own damage:** an assist's hits (its pets' too) take no hero bonus, never crit, and set off nothing of the
  hero's (no milestone, proc, lifesteal, heal per kill or resource). A kill by an assist is still
  the hero's for its rewards: experience, gold, drops.
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
- **It stays at least 5 sec**, so a script of instant steps plays out, and for the whole written
  time of each taunt it lays (up to its own time), even when the taunted enemies die or a boss is
  held only a quarter.
- **Steps** run one after another; a repeat's first pass goes at once and the next every `every`
  seconds, and the repeat takes `every × times`. At its time the ally leaves whatever is left.
  It also leaves at once when its hero falls.
- **Its hits** go to the enemy as written: a boss's shield still takes its share, but nothing
  else changes them (not even a vulnerability on the enemy), so the cap holds exactly. The pets
  it summons fight beside the hero and stay their time after it has gone; their attacks follow
  the same rule (the summon's `damage`, else the pet's own in points, through the cap, never a
  crit, setting off nothing of the hero's; a ranged pet's bolt deals it as it lands). A pet that
  flies (`hover` in `pets.ron`) hovers that high above the ground, bobbing gently.
- **Crowd control** on a boss or a rare is put on exactly as a hero's spell puts it on. The mark an enemy wears (`wears`) shows for as long as the
  effect really holds, and not at all when it does not take (a fear on a boss).
- **A taunt:** the taunted enemies walk to the ally and swing at it at their own pace (it takes
  nothing; a ranged enemy drops its cast and comes too); their abilities go on as before. A taunt
  on an enemy that dies ends, but the ally stays its written time.
- **Ground areas** strike at once, then every `every` seconds while they last. **Bleeds** tick
  every `every` seconds, the first after `every`, the last at their end.
- **Buffs** add their stats while they last (a raised max health raises health with it); their
  heals come every `every` seconds, the first after `every` (each second when `every` is left out).
  A totem is planted 1.5 yd to the hero's side and stands the buff's time. Heals are a share of max
  health as written: the hero's heal power does not raise them.
