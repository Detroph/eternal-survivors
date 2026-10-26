# Gear: reference

How gear is organised, how it works, and how to add or change it.

Status markers: **confirmed** (settled and built), **observed** (checked in play), *inferred*,
**pending**.

## 1. What kinds of gear exist

| Kind | Where it comes from | Kept between runs | Status |
|---|---|---|---|
| **Starting outfit** | every new hero, by class | yes, until replaced | confirmed (section 2) |
| **Class gear** (armor + weapon) | bosses drop it for the hero's class, "for good" | yes | confirmed (section 3) |
| **Rings** | Battleground Caches and rares' caches | no: reset every run (**observed**) | 9, in `content\jewelry.ron` |
| **Trinkets** | caches | no: reset every run (**observed**) | 13, in `content\jewelry.ron` |
| **Amulets** (necklaces) | caches; one worn at a time; offered again → "empowered", growing toward 3× | no: reset every run (**observed**) | confirmed (section 1, Amulets) |

So the split is: **class gear is the permanent progression**; rings, trinkets and amulets are
**run-only** power, like the level-up cards.

The armory shows worn gear per hero; the codex lists every piece a run has ever offered.

### Boss drops (observed)

The drop chance follows the zone's difficulty D in steps, high so gear takes little grinding
(`balance.ron` `zone_scaling` `gear_drop_chance`, `(D, chance)` steps: each holds from its D until
the next, with no blending, and a zone added later takes the step its D falls in), and the
run-option bonus multiplies it (`rewards::zone_drop_chance`):

| D | 1 | 2–3 | 4–5 | 6–7 | 8 and up |
|---|---|---|---|---|---|
| Drop chance | 100 % | 80 % | 70 % | 60 % | 40 % |
| Zones | Elwynn | Duskwood, Warsong | Scarlet Monastery, Sunken Temple, Dire Maul | Gurubashi | Stratholme, Blackrock, Ahn'Qiraj |

Run-option bonus = `min(2, 1 + 0.1·(density − 1) + 0.4·(health − 1) + 0.6·(damage − 1) +
2·(speed − 1))` (each option as a fraction: 150 % = 1.5). So maxed speed alone doubles the odds.
The weights are each option's `reward_weight` in `content/run_options.ron`; density's is 0.1.

Each zone drops its own whole set: its four regular bosses own
two armor slots each and its final boss drops the class's weapons; a piece is the zone's own,
with the zone's own names and looks and the quality of the zone's tier (section 3).

### Caches and jewelry offers (observed)

A Battleground Cache (and a rare cache) opens a choice screen with **3 offers**, like a
level-up, drawn from rings, trinkets and amulets (legendary necklaces). Each offer is one of:

* a **new item** for an empty slot,
* an **upgrade** to an item already worn (it gains a rank), or
* an **amulet** that takes the place of the one worn (rings and trinkets are never replaced).

Every offer rolls a **rarity**, on the same principle as the level-up spell cards. Rarity decides:

* for an **upgrade**: how much better the next rank is, and
* for a **new item**: what rarity the item is.

At the top rarity, **legendary (orange)**, a ring or trinket offer is simply at its strongest;
an amulet offer that rolls legendary can be a **legendary necklace**. Legendary necklaces only
come from an amulet offer's legendary roll.

**The rules:**

* **Rarity odds** (shared with level-up spell cards): with `a = 1 + luck + min(0.3, level × 1 %)
  + bonus`, the weights are common 60, uncommon 25·a, rare 10·a², epic 4·a³, legendary a⁴. At
  a = 1 (spell cards, no luck, level 0) that is 60 / 25 / 10 / 4 / 1 %. The bonus: none for a
  spell card or a ring or trinket from a cache bought with gold; +0.5 for an amulet offer; and a
  rare cache adds +0.25 to each of its offers (a ring or trinket +0.25, an amulet +0.75).
  An amulet from a bought cache starts at a = 1.5: 43.3 / 27.1 / 16.2 / 9.7 / 3.7 %. Empowering
  the worn amulet does not roll: it keeps the amulet's own rarity. Code:
  `game::choices::cache_rarity_bonus`.
* **What a cache can offer**, picked by weight, 3 different ones: an upgrade to a worn item
  (weight 2), a new ring or trinket (weight 0.5), an amulet (weight 2.5), or
  "empower your necklace" while one is worn (weight 1).
* **Two of each:** a hero wears at most 2 rings and 2 trinkets. With 2 rings worn, no new ring is
  offered (the worn ones still offer their upgrades); the same for trinkets.
* **Only what helps:** a new buff-power piece is offered only to a hero who knows a spell buff
  power strengthens (a shield, a damage or haste buff, an aura; its race spell counts), a new
  DoT-stacks piece only to one who has learned a damage-over-time spell.
* **Upgrade caps:** a worn item stops being offered as an upgrade near its cap: no cap for damage,
  cooldowns, crit, crit damage and area; 3× its base value for health %, damage reduction, regen,
  luck, heal per kill and lifesteal; 5× for the rest; double-strike stops at 54 %, DoT stacks at
  3, and the Soulstone (extra life) while you already have 2.
* If fewer than 3 offers exist, a gold card fills the screen.
* **Cooldown jewelry counts half**: Quick Strike Ring
  (5 %) and Talisman of Ephemeral Power (2.5 %) give `SpellCooldownRecovery`: spell
  cooldowns recover that much faster, weapon swings speed up as from twice it, so attack speed
  is what it was (`spells.md` 5a).
* **Hand of Justice** keeps its double cast (5 % base, same new,
  upgrade and 60 % cap) and adds a chance for each weapon attack (melee or ranged swing) to hit
  twice: 2 % + (its double cast − 7.5 %) ÷ 5, 7.5 % being a new Common piece's double cast.
  New Common 2 %, Rare 3.05 %, Epic 4.25 %, Legendary 5.3 %; each upgrade adds 1/5 of its
  double-cast step (first upgrade Rare +1.6 %, Epic +2.3 %, Legendary +3.0 %); it stops at
  12.5 % with the double cast at its cap. The second hit is a full weapon hit of its own (its
  own crit roll, rage and what a swing sets off), shown as a second swing (off hand where the
  off hand swung). `jewelry.ron` `double_hit: Some((base, from, per))`, `{w}` in its text;
  stat `WeaponDoubleHit`. Code: `esurv-sim` `game::casting` (`swing_hand`).
* While the screen is open the run waits. Code: `esurv-sim` `game::choices`.

* **How big an offer is** (rarity multiplier ×1.0 / 1.3 / 1.7 / 2.5 / 3.2 (our numbers) from common to
  legendary):
  * a **new** ring or trinket starts at 1.5 × multiplier × its base value, never past its cap
    (an epic or legendary piece of a 3× stat starts at 3× its base; double-strike at most 60 % per piece, and at most 200 % from every source together: 3 casts);
  * an **upgrade** to a stat with no cap adds multiplier × base ÷ (1 + 0.08 × current rank), so
    each rank adds a little less;
  * an **upgrade** to a capped stat adds a share of the room left below its cap, so items close
    in on their maximum smoothly;
  * the Soulstone adds 1 extra life (2 on an epic or legendary roll), at most 2: its card offers
    only the lives still missing; DoT-stack items add 1 stack (2 on rare or better), at most 3.
* **Spell cards follow the same rarity idea:** a damaging spell gets +18 / 25 / 34 / 46 % power
  (uncommon / rare / epic / legendary), other spells −7 / 10 / 13 / 17 % cooldown; common gives no
  bonus.

### Where caches and pickups come from (confirmed)

* **Cache price:** opening a Battleground Cache costs gold (a rare cache is free). The first costs **20**; each opening raises the
  price to price × 1.25 + 5, rounded down (20, 30, 42, 57, 76, …). Without enough gold the
  game says "This cache needs N gold." (at most every 4 s). The gold card on a short offer
  screen is worth half the current price, at least 60.
* **Battleground Caches:** 2 are placed 18–50 yd from the hero at the start of a run; every
  70 s (first check at 60 s) another appears (18–50 yd away) while fewer than 3 lie on the field; with more than 4 the
  oldest is removed. **Every elite** drops one where it dies. A slain
  **rare** drops a **Rare Cache** beside it instead: **free to open** (no gold, the cache price stays as it is; its name reads
  "Rare Cache (Free)"), never removed by the 4-cache limit, its screen headed
  **Rare Cache** ("Free to open: choose a ring, a trinket or an amulet.").
* **Gold:** a normal kill drops 2–4 gold with chance 16.5 % ÷ density (×100 %); an elite always
  drops 15; rares 25 + 5 × slain; bosses 40 + 20 × slain.
* **Experience orbs** (`content\balance.ron` `experience`). A normal
  kill drops `100 × 100 ÷ density %`, rounded at random (so the average is exact: 100 at 100 %,
  33 or 34 at 300 %); an elite drops 1000 whatever the density; a boss 2400 + 800 × bosses already
  slain (2400, 3200, 4000, …); a rare gives an instant level-up instead of an orb. An orb
  looks small below 200, medium below 1000 and large and red from 1000. Confirmed: past 44 loose orbs, new experience joins the nearest orb.
* **Runes:** every 45 s (first check at 40 s), if at most one rune lies on the field, a random one of three appears
  15–40 yd away: **Speed** (+60 % move speed, 10 s), **Berserker** (+50 % damage, 15 s),
  **Restoration** (heals 50 % of max health, **observed**). Taking a rune whose buff is up
  restarts its timer.
* **Magnets:** 2 or 3 per run. The time until the final boss is cut into equal windows; magnet
  *i* is due at the start of window *i* and appears by itself (with a beam of light and a
  message saying so) 60–100 % into its window. A boss slain while one is due drops it; a
  rare does so half the time. Taking a magnet pulls everything on the field to the hero.
  Magnets appear 25–50 yd from the hero.

### Picking things up (confirmed)

* **Experience orbs** fly to the hero once it is within pickup range (24 yd/s), and **by
  themselves from anywhere once they are 6 s old** (32 yd/s); the experience counts on
  arrival, times the hero's experience bonus (rounded).
* **Gold** is taken as soon as it is within pickup range, times the gold bonus (rounded).
* **Caches, runes and magnets** are taken by walking within 3 yd of them. A cache only opens
  when the hero can pay its price; otherwise it stays.
* A **magnet** sends every orb and coin on the field flying to the hero at 40 yd/s.
* A run **opens** with 4 of the zone's first enemy, 2 caches and the magnet schedule.

Code: `esurv-sim` `game::loot`, `game::pickups`, `game::rares`.

### Amulets (confirmed)

`content\amulets.ron` holds **17 amulets**, each with a rarity (`Common` … `Legendary`), a
**growth** (`Fastest` … `Slowest`) and one to three effects in the same shapes as talents (stat bonuses, spell
changes, milestones such as cheat death or exploding kills).

**An amulet offer** rolls a rarity (the cache odds above) and picks, uniformly, an amulet of
exactly that rarity. On a **legendary** roll the pool also holds a **legendary amulet for every
spell the hero knows**. The worn amulet is never offered as a new one; if the pool is empty the
first amulet (Mark of Fordring) stands in. Taking an amulet replaces the worn one.

**Legendary amulets: one per spell.** `content\legendary_amulets.ron`
holds a legendary amulet for every spell a hero can know: every class card, every form's card
pool (Moonkin and Restoration share the caster pool; Cat and Bear have their own) and every
level-1 spell; a level-1 strike and its finisher (Mutilate and Envenom …) share one, which remakes
both. Each is modelled on a boss, enemy or Warcraft III hero who uses a bigger version of the
spell, with our own name and words; the item's id is `legendary:<main spell>` and its name is the
legendary's own (e.g. `Phoenix` for Fireball). Each record lists the `spells` it remakes (the
main one first), its `name`, `icon` (the main spell's), `text` (what it does, shown on its card
and tooltip) and `effects` in the talent shapes (`AbilityMod`, `Stat`, `Milestone`, at rank 1).
The content check holds every such spell to exactly one legendary.

**The pool** on a legendary roll holds the legendaries of the spells the hero knows (its class,
its level-1 pick and the cards it took; a form's card pool brings its own), once each.

**Budget**: a damage legendary adds **+30 %** to the hero's total damage; a heal
legendary **+100 %** to what its spell heals (and shields absorb), a defensive buff's (Divine Shield)
+100 % to its uptime; ±20 % counts as in budget. Shields and buffs that add power (damage, haste,
speed, rage or energy) are held to the damage budget, +30 % total damage: kiting, the hero's own
shield already takes most of the damage struck at it, so what a shield absorbs cannot double, and a
buff's uptime is near full already. Measured with the balance check's `--legendaries` mode
(AUTHORING.md, "Power budgets"); the numbers below are tuned to it. A heal or shield grows with the
square root of its power (`Power` 3 heals twice as much). A note "pick" marks a number
the proposals do not give (ours, tuned with the rest); `TODO(rule)` marks an effect still
to settle.

| Class | Spell | Legendary | What it does |
|---|---|---|---|
| warrior | `slam` | Gruul's Shatter | Slam deals 30% more. Every 4th Slam turns the enemies it hits to stone for 1 sec, then they shatter, each dealing 35% of its hit to enemies within 5 yd of it. |
| warrior | `whirlwind` | Blademaster's Bladestorm | Whirlwind deals 35% more. Every 15 sec you Bladestorm for 3 sec: Whirlwind spins every 0.5 sec and you move 30% faster. |
| warrior | `revenge` | Mark of the Fallen Champion | Revenge deals 260% more. Enemies that hit you are marked for 5 sec; Revenge deals 650% more to marked enemies and each of its hits on one heals you 7% of your health. |
| warrior | `cleave` | Hateful Strike | Cleave deals 75% more. Every 2nd Cleave also hits the healthiest enemy in reach for 550%. |
| warrior | `thunder_clap` | Hammer of the Mountain King | Thunder Clap deals 60% more and reaches 50% farther. Each clap throws a Storm Bolt at the healthiest enemy it hit: 300% and a 2 sec stun. |
| warrior | `mortal_strike` | Blade Dance | Mortal Strike deals 25% more. Each strike sends out 2 ghostly reflections of you that spin after enemies for 5 sec, each spin striking up to 3 enemies near them for 3.5%. Our rule (owner): reflections spinning as the Ravager's Bladestorm, in place of a dash through 3 enemies for 35% each; the same 105% at most. |
| warrior | `execute` | Gorehowl | Execute deals 130% more. A kill readies it again and gives Bloodlust: 20% more damage for 20 sec, up to 5 times. |
| warrior | `demo_shout` | Bellowing Roar | Demoralizing Shout fears the enemies it hits for 2 sec, and they take 95% more damage from you for 6 sec. |
| warrior | `battle_shout` | Endurance Aura | Battle Shout lasts twice as long. While it lasts your swings are 30% faster and give 3 more rage, and every blow you strike lands again for 25%. |
| warrior | `bloodthirst` | Vampiric Bite | Bloodthirst deals 35% more. Each hit heals you 7% of your health and gives Essence of the Blood Queen: 20% more damage for 10 sec, up to 3 times. |
| warrior | `rend` | Corrupted Blood | Rend deals 90% more. Each tick passes the bleed to an enemy near the target, up to 5 enemies bleeding. |
| warrior | `shield_slam` | Storm Bolt | Shield Slam deals 20% more. The shield bounces to 3 more enemies for 60%, stunning each for 1 sec. |
| paladin | `denounce` | Light of the Naaru | Denounce deals 110% more and leaps 3 more times; its leaps lose no damage and each heals you 4% of your health. |
| paladin | `holy_shield` | Reflective Shield | Holy Shield has 2 more stacks and strikes back 185% harder. When its last stack breaks, a holy nova hits everything within 10 yd for 1150% of its strike. |
| paladin | `seal_of_command` | The Ashbringer | Seal of Command deals 15% more. Every 11th proc bursts in holy flame around the target: 50% to every enemy within 8 yd. |
| paladin | `consecration` | Sanctified Ground | Consecration deals 15% more. It grows 5% each time it damages an enemy, up to 50% wider. |
| paladin | `exorcism` | Righteous Fire | Exorcism deals 45% more. The target burns for 70% over 4 sec, and the fire jumps to enemies that touch it. (pick: "touch" is 3 yd) |
| paladin | `hammer_of_wrath` | Hand of Ragnaros | Hammer of Wrath deals 8% more. The hammer explodes on impact: 10% to enemies within 6 yd, knocking them back. (pick: knocked back 6 yd) |
| paladin | `holy_shock` | Light's Barrage | Holy Shock deals 50% more and fires 2 more bolts. Each bolt that crits fires another for 120%. |
| paladin | `holy_wrath` | Holy Wrath | Holy Wrath deals 20% more. Each hit also strikes one more enemy near its target for half as much. |
| paladin | `judgement` | Mark of the Naaru | Judgement deals 45% more. The target takes 25% more damage from you for 8 sec; if Judgement kills it, it leaps to the next. |
| paladin | `hammer_of_justice` | Hammer of Uther | Hammer of Justice deals 80% more. The hammer bounces to 2 more enemies for 200%, stunning each for 1.5 sec. |
| paladin | `holy_light` | Light of Uther | Holy Light heals twice as much; healing past your health becomes a shield of up to 2% of your health. (a heal grows with the square root of its power: 3 more power heals twice as much) |
| paladin | `divine_shield` | Avatar of the Light | Divine Shield lasts 240% longer. While it lasts you deal 30% more, and every blow against you strikes its attacker back for 510% of the blow. When it ends you heal 100% of your health. |
| hunter | `steady_shot` | Thori'dal, the Stars' Fury | Steady Shot deals 25% more. Every 5th shot is an arrow of light that pierces every enemy in a line for 60%. |
| hunter | `kill_command` | Stampede | Kill Command deals 10% more and you have one more pet. Every 9th command sends a stampede of beasts through the enemies ahead: 40% to each. |
| hunter | `explosive_shot` | Rocket Strike | Explosive Shot deals 25% more. Each blast marks the spot; 2 sec later a rocket lands there for 95% within 6 yd. |
| hunter | `arcane_shot` | Searing Arrows | Arcane Shot deals 70% more. Its arrows ignite for 90% over 3 sec, and it has a 35% chance to fire again. |
| hunter | `multi_shot` | Windrunner's Volley | Multi-Shot deals 20% more and hits 3 more targets; each arrow ricochets once to an enemy near its target for 50%. |
| hunter | `serpent_sting` | Serpent's Brood | Serpent Sting deals 20% more. Each sting releases 2 snakes that bite nearby enemies for 8 sec. (pick: at most 6 snakes at once (what a sting every 3 sec keeps up)) |
| hunter | `volley` | Ranger-General's Barrage | Volley deals 30% more and lasts 80% longer. The arrows fall around you and move with you. |
| hunter | `explosive_trap` | Lava Bomb | Explosive Trap deals 50% more. It leaves 3 burning patches for 4 sec, each burning for 35% a second. (pick: patches 3 yd wide, up to 4 yd apart) |
| hunter | `concussive_shot` | Silence | Concussive Shot deals 170% more and hits 2 more targets. Dazed enemies take 110% more damage from you and cannot cast. |
| hunter | `aimed_shot` | Netherstrand Longbow | Aimed Shot deals 7% more. It pierces every enemy in a line behind the target for 25%. |
| hunter | `immolation_trap` | Lava Splash | Immolation Trap deals 8% more and lasts 20% longer. Enemies dying in it erupt for 6% of their max health. (pick: the eruption reaches 7 yd, the trap's own size) |
| hunter | `frost_trap` | Flash Freeze | Enemies that stay in Frost Trap are frozen for 2 sec, and frozen enemies take 110% more damage from you. ("3 sec in it" is its second strike on the enemy (it strikes every 2.1 sec)) |
| rogue | `mutilate` + `envenom` | Poison Cloud | Mutilate and Envenom deal 10% more. Envenom leaves a poison cloud on each target for 4 sec that poisons enemies in it. (pick: the cloud is 5 yd wide) |
| rogue | `sinister_strike` + `eviscerate` | Vengeance | Sinister Strike and Eviscerate deal 185% more. A kill with Eviscerate calls an avatar of vengeance that fights for 6 sec, up to 2. |
| rogue | `hemorrhage` + `rupture` | Blood Siphon | Hemorrhage and Rupture deal 55% more. Each Rupture tick heals you 5% of your health, and it spreads to 1 more enemy when its target dies. |
| rogue | `blade_flurry` | VanCleef's Whirling Blades | Blade Flurry deals 30% more. It throws two blades that circle you for 6 sec, cutting everything they touch for 25% a second. (the glaives' ring is Blade Flurry's own 7 yd) |
| rogue | `kidney_shot` | Cheap Shot | Kidney Shot deals 610% more and hits 2 more targets. Stunned enemies take 400% more damage from you. |
| rogue | `gouge` | Blinding Powder | Gouge deals 325% more and hits 2 more targets. Gouged enemies stumble blind for the stun and take 210% more damage from you. |
| rogue | `ghostly_strike` | Blink Strike | Ghostly Strike deals 55% more. After it a reflection of you appears at another enemy within 15 yd, strikes it for 105% and fades. |
| rogue | `adrenaline_rush` | Mirror Image | Adrenaline Rush lasts twice as long. While it lasts two images copy your strikes for 130% each. |
| rogue | `backstab` | Critical Strike | Backstab deals 70% more and has a 75% chance to strike again for 500%; a kill readies it again. |
| rogue | `slice_and_dice` | Wind Walk | Slice and Dice lasts twice as long. While it lasts you wind-walk: you move 30% faster and every strike hits again for 260%; the first strike after it starts hits again for 500%. |
| rogue | `fan_of_knives` | Fan of Knives | A second ring of Fan of Knives follows 0.5 sec later, and every knife applies your poison. (the poison is the starting option's (Mutilate's); other options have none) |
| rogue | `garrote` | Silent Strangler | Garrote deals 10% more and hits 2 more targets. Garroted enemies cannot cast, and it spreads on death. |
| priest | `smite` | Penance of Light | Smite deals 90% more. Every 2nd Smite becomes a beam that also strikes 2 more enemies near the target for 215%. |
| priest | `holy_fire` | Holy Fire | Holy Fire deals 300% more. Its burn spreads to 2 enemies near the target, and burning enemies take 65% more damage from you. ("10 % more holy damage" is 10 % more damage (a vulnerability is not by school)) |
| priest | `mind_spike` | Shadow Spike | Mind Spike deals 10% more. Each spike calls 3 shadow spikes onto spots near the target, 20% within 3 yd each. (pick: the spots are up to 5 yd from the target) |
| priest | `sw_pain` | Whisper of Death | Shadow Word: Pain deals 1% more and hits 1 more target. When a pained enemy dies, shadow bursts from it: 2% within 6 yd, and every enemy hit is pained. |
| priest | `mind_blast` | Eye Beam | Mind Blast deals 30% more. It chains to 2 more enemies within 8 yd for 95%, 20% stronger each. |
| priest | `holy_nova` | Holy Nova | Holy Nova deals 90% more and reaches 50% farther. It heals you 1% of your health per enemy hit. |
| priest | `pw_shield` | Divine Aegis | Power Word: Shield absorbs 60% more. Each time it forms, for 15 sec holy light strikes a random enemy within 20 yd every 0.5 sec for 65% of your max health; when it breaks it bursts for 40% of its value within 10 yd. (a shield grows with the square root of its power: 1.56 more power absorbs 60 % more; pick: the burst reaches 10 yd (as Ironbranch's)) |
| priest | `renew` | Light's Blessing | Renew is cast whenever it is ready and an enemy is within 30 yd. Each Renew also casts Deplete on 3 enemies within 30 yd, bosses, elites and rares first, then the nearest, dealing 5000% of that Renew's healing to each as damage over 10 sec. (a damage legendary, set to 5000 % although under budget; the Deplete look is Vampiric Embrace, 15286) |
| priest | `mind_flay` | Lunatic Gaze | Mind Flay deals 50% more and reaches 50% farther. Flayed enemies take 20% more damage from you. |
| priest | `psychic_scream` | Dominate Mind | Psychic Scream reaches 50% farther. Feared enemies take 85% more damage from you, and 60% of them turn on the others for the fear. |
| priest | `devouring_plague` | Inevitable Doom | Devouring Plague deals 7% more and hits 2 more targets. When it ends it detonates for 15% within 6 yd. |
| priest | `divine_star` | Halo of Elune | Divine Star deals 100% more. You throw 2 stars at once, in two directions. |
| priest | `power_infusion` | Prophet's Infusion | Power Infusion lasts twice as long. While it lasts each spell has a 45% chance to cast twice. |
| shaman | `lightning_bolt` | Thrall's Doomhammer | Lightning Bolt deals 60% more and has a 55% chance to fork into a chain lightning at 3 more enemies for 85%. |
| shaman | `windfury_weapon` | Doomwinds | Windfury Weapon deals 55% more. Each proc also throws chain lightning at 3 enemies for 85% and slows them. (each extra hit of a proc throws it; pick: the slow lasts 2 sec (Wellspring's), 40 %) |
| shaman | `wellspring` | Tornado | Wellspring deals 30% more. Every 3rd Wellspring raises a tornado that wanders through the enemies for 6 sec, 30% a second. (the tornado rides along with the enemy it rose on; pick: 4 yd wide) |
| shaman | `chain_lightning` | Forked Lightning | Chain Lightning deals 95% more and jumps 3 more times; its jumps lose nothing. |
| shaman | `earth_shock` | Earthquake | Earth Shock deals 5% more. It leaves a quake on the target's spot for 4 sec: 3% a second and 50% slower. (pick: the quake is 6 yd wide) |
| shaman | `flame_shock` | Magma Blast | Flame Shock deals 30% more and hits 2 more targets. It spreads on death. |
| shaman | `frost_shock` | Frost Nova | Frost Shock deals 4% more. It bursts around the target: 8% to enemies within 8 yd, freezing them for 2 sec. |
| shaman | `lightning_shield` | Static Charge | Lightning Shield strikes back 220% harder and lasts 450% longer, and each strike arcs on to 2 more enemies for 450%. |
| shaman | `magma_totem` | Lava Spawn | Magma Totem deals 35% more and lasts 210% longer. The totem spawns 2 lava spawns that fight for 8 sec. |
| shaman | `searing_totem` | Serpent Wards | Searing Totem deals 280% more. You plant 2 totems instead of 1. |
| shaman | `fire_nova` | Wrath of Ragnaros | Fire Nova deals 15% more and reaches 50% farther. It knocks enemies back and leaves the ring burning for 3 sec, 10% a second. (the ring is the nova's reach (12 yd, 50 % farther); pick: knocked back 6 yd, the ring burns for 25 % a second) |
| shaman | `healing_wave` | Healing Wave | Healing Wave heals 50% more, then 60% more over 6 sec. |
| mage | `arcane_blast` | Arcane Bomb | Arcane Blast deals 25% more. Every 6th blast is an arcane bomb: 70% to every enemy within 8 yd of the target. |
| mage | `fireball` | Phoenix | Fireball deals 125% more. A crit calls a phoenix that fights for 8 sec, up to 2. |
| mage | `frostbolt` | Frostbolt Volley | Frostbolt deals 55% more. Every 3rd bolt is a volley that also strikes 4 more enemies for 140%. |
| mage | `arcane_missiles` | Shade's Missiles | Arcane Missiles deals 170% more and fires 3 more missiles. |
| mage | `frost_nova` | Ice Tomb | Frost Nova reaches 50% farther. Frozen enemies are entombed for 3 sec and take 100% more damage from you. |
| mage | `arcane_explosion` | Magnetic Pull | Arcane Explosion deals 140% more. Every 3rd explosion first pulls the enemies within 15 yd to you. |
| mage | `blizzard` | Blizzard | Blizzard deals 15% more, is 50% wider and lasts 40% longer. |
| mage | `flamestrike` | Flame Strike | Flamestrike deals 25% more. A second pillar falls on the next thickest crowd. |
| mage | `fire_blast` | Living Bomb | Fire Blast deals 15% more. Every 4th blast makes its target a living bomb: 3 sec later it explodes for 55% within 6 yd. |
| mage | `cone_of_cold` | Frost Breath | Cone of Cold deals 145% more and reaches 50% farther. It leaves frost on the ground for 3 sec: 3% of its hit a second, and 60% slower. (the frost slows as the cone does (60 %) and deals 5 % of the hit a second; pick: 6 yd wide, on the first enemy hit) |
| mage | `blast_wave` | Flame Wreath | Blast Wave deals 40% more and reaches 50% farther. It leaves a ring of fire for 4 sec that burns enemies in it. (the ring is the wave's reach (10 yd, 50 % farther); pick: it burns for 30 % a second) |
| mage | `pyroblast` | Pyroblast | Pyroblast deals 15% more. The impact explodes for 10% within 8 yd. |
| mage | `ice_barrier` | Frozen Shell | Ice Barrier absorbs 60% more. Each time it forms, for 20 sec a shard of ice strikes a random enemy within 20 yd every 0.5 sec for 55% of your max health. When it breaks, enemies within 10 yd are frozen for 2 sec and take 6% of what it absorbed. (a shield grows with the square root of its power: 1.56 more power absorbs 60 % more) |
| warlock | `drain_soul` | Harvest Soul | Drain Soul deals 85% more. A kill while draining harvests the soul: 10% more damage for 20 sec, up to 5 times, and the drain goes on. (the drain goes on to the next enemy by itself) |
| warlock | `hand_of_guldan` | Skull of Gul'dan | Hand of Gul'dan deals 60% more, is 50% wider and calls 2 more imps. |
| warlock | `shadow_bolt` | Shadow Bolt Volley | Shadow Bolt deals 50% more. Every 3rd bolt is a volley that also strikes 4 more enemies for 120%. |
| warlock | `corruption` | Unbound Plague | Corruption deals 215% more. Every 3 sec it jumps to another enemy, 20% stronger each jump. |
| warlock | `immolate` | Legion Flame | Immolate deals 90% more. Burning enemies leave a trail of fire that burns others for 60% a second. |
| warlock | `curse_of_agony` | Doom | Curse of Agony deals 775% more. A cursed enemy that dies rises as a doomguard that fights for 10 sec, up to 1. |
| warlock | `rain_of_fire` | Rain of Fire | Rain of Fire deals 25% more, is 50% wider and lasts 75% longer. |
| warlock | `hellfire` | Immolation | Hellfire deals 35% more and goes off 4 more times, and never harms you. (Hellfire never harms the hero here already; 4 more novas: 3 become 7) |
| warlock | `death_coil` | Death Coil | Death Coil deals 65% more. It bounces to 2 more enemies for 160% and heals you 50% of its damage. |
| warlock | `searing_pain` | Incinerate Flesh | Searing Pain deals 20% more and hits 1 more target. Its crits ready it again. |
| warlock | `drain_life` | Blood Siphon | Drain Life drains every enemy within 10 yd of its target at once, for 8% as much. (each tick drains everything within 10 yd of the target, healing as the drain does) |
| warlock | `fear` | Sleep | Fear hits 2 more targets. Feared enemies take 270% more damage from you. |
| druid | `starfire` | Starfall | Starfire deals 65% more. Every 2nd Starfire rains stars around you for 4 sec, one a second on an enemy within 30 yd. (pick: a star strikes for 75 % of a Starfire) |
| druid | `claw` + `rip` | Mark of Arlokk | Claw and Rip deal 185% more. A Rip kill calls a panther that fights for 8 sec, up to 2. |
| druid | `maul` | Mangle | Maul deals 75% more. Mauled enemies take 40% more damage from you. (pick: mauled for 6 sec (Maul's own stacks last 6)) |
| druid | `friend_of_the_forest` | Force of Nature | Friend of the Forest deals 15% more. Every cast also calls 2 treants that fight for 10 sec. |
| druid | `wrath` | Wrath of Cenarius | Wrath deals 65% more and hits 1 more target, with a 55% chance to strike again for 165%. |
| druid | `moonfire` | Moonfire | Moonfire deals 25% more and hits 2 more targets. It spreads on death. |
| druid | `hurricane` | Eye of the Storm | Hurricane deals 6% more, is 50% wider and lasts 30% longer. |
| druid | `insect_swarm` | Silithid Swarm | Insect Swarm deals 160% more. Every 2 sec insects leave each swarmed enemy, biting the nearest other enemy for 160% and healing you 6% of your health. ("every 2 sec" is every second tick on an enemy (it ticks every 1.05 sec)) |
| druid | `entangling_roots` | Iron Roots | Entangling Roots deals 150% more and hits 2 more targets. Rooted enemies take 95% more damage from you. |
| druid | `rejuvenation` | Lifebinder's Gift | Rejuvenation heals 90% more. When it ends, a bloom heals you 10% of your health and hits enemies within 8 yd for as much. ("hits for 100 %" is 100 % of the bloom's heal) |
| druid | `regrowth` | Nature's Fury | Regrowth heals 40% more, then 50% more over 6 sec. |
| druid | `thorns` | Thorns Aura | Thorns strikes back 30% harder. While it lasts, thorny vines lash a random enemy within 15 yd every 0.5 sec for 30% of your max health. |
| druid | `rake` | Rend Flesh | Rake deals 95% more and hits 2 more targets. It spreads on death. |
| druid | `shred` | Saber Lash | Shred deals 45% more. Each shred also cuts 2 enemies near the target for 60%. |
| druid | `tigers_fury` | Frenzy | Tiger's Fury lasts twice as long. While it lasts your swings are 30% faster, and every hit and tick you deal strikes again for 30%. |
| druid | `pounce` | Ravage | Pounce deals 450% more and leaps to 2 more enemies, stunning each. |
| druid | `ravage` | Pounce | Ravage deals 60% more. It also hits 2 enemies near the target for 45%. |
| druid | `faerie_fire_feral` | Moonglow | Faerie Fire hits 2 more targets. Marked enemies take 165% more damage from you. |
| druid | `dash` | Sprint of the Wild | Dash lasts twice as long. While it lasts your swings are 20% faster, and wild spirits run with you, striking every enemy within 5 yd for 10% of your max health each sec. |
| druid | `swipe` | Swipe | Swipe deals 50% more and reaches 50% farther. |
| druid | `ferocious_bite` | Feast | Ferocious Bite deals 430% more and tears into 2 more enemies near the target for 110%. It heals you 5% of your health and gives back 2 combo points. |
| druid | `powershift` | Shapeshift | Powershift gives twice the energy and comes back twice as fast. Each shift you leap out in flame: 4% of your max health to enemies within 8 yd, and the ground there burns for 4 sec, 2% a sec. |
| druid | `demoralizing_roar` | Deafening Roar | Demoralizing Roar reaches twice as far. Enemies it hits take 15% more damage from you for 6 sec, and each one gives you 9 rage. |
| druid | `bash` | War Stomp | Bash deals 20% more and becomes a stomp: every enemy within 8 yd takes 45% of its hit, is stunned for 1.5 sec and takes 10% more damage from you. (the stomp strikes for Bash's hit; its enemies take more for the stun's 1.5 sec) |
| druid | `enrage` | Surge | Enrage gives twice the rage. Each time it fires you surge with a roar: 10% of your max health to enemies within 8 yd, and your next 3 swings are 30% faster. |
| druid | `frenzied_regeneration` | Roar of the Ancients | Frenzied Regeneration heals 180% more. |
| druid | `barkskin` | Ironbranch | Barkskin absorbs 60% more, and it bursts for 30% of its value within 10 yd when it breaks. (a shield grows with the square root of its power: 1.56 more power absorbs 60 % more) |
| druid | `lacerate` | Brutal Swipe | Lacerate deals 25% more and hits 2 more targets. It spreads on death. |

All legendary amulets are `Legendary` with growth `Slowest`.

**Empower:** an offer of the worn amulet raises its rank. All its numbers are multiplied by

    k = 3 − 2 × (1 − J/2)^(rank − 1),   J = 0.6 Fastest, 0.5 Fast, 0.35 Medium, 0.25 Slow, 0.2 Slowest

so k is 1 at rank 1 and grows toward 3. Row 0 gives 1.6, 2.02, 2.31 …; row 4 gives 1.2, 1.38 ….
An empowered amulet's spell cooldown cut stops at 50 %, and its extra targets are whole numbers
(rounded down), as are its extra stacks, strikes, imps, pets and critters. The milestones that
grow their main number (the one a talent rank multiplies, see classes.md section 4) are those
`Milestone::grows_when_empowered` lists; the milestones an `Every` or a `WhileBuff` holds grow
too; other milestones do not grow.

Code: `esurv-sim` `game::amulets` (`amulet_candidates`, `legendaries_of`, `legendary_named`,
`legendary_for`, `empower_factor`, `empowered`); the milestone kinds the legendaries brought in
`game::proc_effects` and `game::field_milestones`.

## 2. Starting outfit (confirmed)

A table of real 1.12 items, each:

| Field | Example (Worn Shortsword) | Meaning |
|---|---|---|
| name | `Worn Shortsword` | the item's name |
| item entry | 25 | `Item` id, as in the 1.12 item database |
| display id | 1542 | `ItemDisplayInfo.dbc`: the model the hero wears |
| inventory type | 21 | slot: 21 main hand, 17 two-hand, 13 one-hand, 14 shield, 15 ranged, 5 chest, 7 legs, 8 feet, 4 shirt… |
| item class | 2 | 2 weapon, 4 armor |
| item subclass | 7 | for weapons: 7 one-hand sword, 10 staff, 4 one-hand mace, 0 one-hand axe… |
| sheath | 3 | how the weapon sits when sheathed |

The names come from the real starter gear: `Recruit's Shirt`, `Footpad's Shoes`,
`Apprentice's Robe`, `Worn Shortsword`, `Bent Staff`, `Worn Mace`, and so on.

Ours: the look and slot of every piece come from the player's `CharStartOutfit.dbc`; the
weapons and the shield (13 items) carry their kind (`Staff`, `Shield` ...; it sets the item
class, subclass and sheath) in `content\outfit_weapons.ron`, by item entry and look (staves rest on
the back, one-handers on the hip, two-handers and the shield on the back, ranged weapons show
only drawn). See server.md, "What the hero wears in a run".

## 3. Class gear sets (confirmed)

**Every zone has its own look.** A piece is a
**slot** (8 armor slots, then the class's weapons as slots 8, 9) from a **zone**; it is named and
looks as its zone's own look of the class set (`gear.ron` `looks`), and its weapons show the
zone's **armaments** (below). Its stats still follow the zone's difficulty D ("Stats"), and its
quality colour the zone's tier: Elwynn 0, Duskwood and Warsong Gulch 1, the Scarlet Monastery,
the Sunken Temple and Dire Maul 2, Gurubashi Arena and Stratholme 3, Blackrock Mountain and
Gates of Ahn'Qiraj 4 (`balance.ron` `zone_scaling` `gear_tier_starts`: tier 1 from D 2, tier 2
from 4, tier 3 from 7, tier 4 from 9). A piece's tooltip says the zone it came from ("From
Duskwood"), and the hero sheet's Collection Log lists the pieces in a row per zone's look.

The looks:

| Zone (D) | Armor look | Armaments (the three loadouts' weapons) | Source draft |
|---|---|---|---|
| Elwynn Forest (1) | the Recruit's starting look (unchanged) | level 10–15 green world drops and quest rewards | `looks-elwynn.md` |
| Duskwood (2) | Deadmines, Shadowfang Keep, Blackfathom Deeps, Gnomeregan | the same dungeons | `looks-duskwood.md` |
| Warsong Gulch (3) | Warsong reputation and level 25–35 pieces | Warsong reputation and others | `looks-warsong-gulch.md` |
| Scarlet Monastery (4) | "Adventurer's <slot>", level 30–45 dungeon pieces | Scarlet Monastery | `armaments-tier1.md` |
| Sunken Temple (5) | Sunken Temple, Zul'Farrak, Maraudon, Uldaman | the same dungeons | `looks-sunken-temple.md` |
| Dire Maul (6) | Dungeon Set 1 (Valor, Lightforge ...) | pre-raid: Dire Maul, Sunken Temple, Blackrock Depths and Spire | `armaments-tier2.md` |
| Gurubashi Arena (7) | the Zul'Gurub (Zandalar) sets, gaps from Zul'Gurub drops and crafted sets | Zul'Gurub | `looks-gurubashi-arena.md`, `armaments-tier3.md` |
| Stratholme (8) | **two rows**: Dungeon Set 2 (Heroism, Soulforge ...); the Scholomance sets (below) | Stratholme and Scholomance drops (below) | this section |
| Blackrock Mountain (9) | **two rows**: Tier 1 (Molten Core), Tier 2 (Blackwing Lair) | Molten Core; Blackwing Lair | `looks-blackrock.md` |
| Gates of Ahn'Qiraj (10) | Tier 2.5, gaps from Temple drops | Temple of Ahn'Qiraj | `looks-ahn-qiraj.md` |
| (Naxxramas, D 11: no zone yet) | Tier 3 | Naxxramas | `looks-naxxramas.md` |

The Naxxramas looks are in the content already (zone id `naxxramas`) and wait for its zone;
Mount Hyjal (D 12) has no 1.12 models and stays in the docs.

**Blackrock's two rows:** the zone has two looks with the same stats,
row 0 the Tier 1 sets with the Molten Core weapons, row 1 the Tier 2 sets with the Blackwing
Lair weapons. A Blackrock piece unlocks the same piece in both rows: it shows row 0 of its own,
and the player picks row 1 with the appearance choice (Show This Look), like any look. Any
zone may have more rows the same way (`row: 1`, `row: 2` ...).

**Stratholme's two rows.** Stratholme has two looks with the same
stats, as Blackrock: row 0 Dungeon Set 2 (Heroism, Soulforge ..., unchanged), row 1 the
**Scholomance sets**, the row's name (`row_name` in `gear.ron`): the Collection Log heads the
row "Scholomance sets" where other second rows read "Second look", and a look's tooltip reads
"Scholomance sets: the second look of pieces from Stratholme" (no name while the zone is
undiscovered). A Stratholme piece unlocks the same piece in both rows. Row 1 is each armor type's Scholomance set (5 pieces), its
other three slots real Stratholme drops of the same colour and texture family:

| Armor (classes) | Scholomance set | Head | Shoulders | Chest | Waist | Legs | Feet | Wrists | Hands |
|---|---|---|---|---|---|---|---|---|---|
| plate (warrior, paladin) | Deathbone Guardian | Helm of the Executioner (34902) | Wailing Nightbane Pauldrons (24115) | Deathbone Chestplate (24102) | Deathbone Girdle (25225) | Deathbone Legguards (25226) | Deathbone Sabatons (25227) | Vambraces of the Sadist (24110) | Deathbone Gauntlets (25224) |
| mail (hunter, shaman) | Bloodmail Regalia | Crown of Tyranny (24045) | Stratholme Militia Shoulderguard (31194) | Bloodmail Hauberk (25222) | Bloodmail Belt (25219) | Bloodmail Legguards (25223) | Bloodmail Boots (25220) | Twilight Void Bracers (24180) | Bloodmail Gauntlets (25221) |
| leather (rogue, druid) | Cadaverous Garb | Grand Crown (28022) | Wyrmtongue Shoulders (18971) | Cadaverous Armor (25249) | Cadaverous Belt (25248) | Cadaverous Leggings (26966) | Cadaverous Walkers (11571) | Chillhide Bracers (24190) | Cadaverous Gloves (25253) |
| cloth (priest, mage, warlock) | Necropile Raiment | Bonecaster's Crown (26277) | Necropile Mantle (25247) | Necropile Robe (25245) | Duskwoven Sash (28155) | Necropile Leggings (25255) | Necropile Boots (18863) | Necropile Cuffs (4607) | Darkbind Fingers (24177) |

The armaments mix Stratholme and Scholomance drops: row 0 mostly Stratholme's, row 1 mostly
Scholomance's (Blade of Necromancy is Scholomance's; Hand of Edward the Odd, Heartseeking
Crossbow and Shell Launcher Shotgun are world drops that fall in both dungeons, as neither has a
fitting caster mace, crossbow or gun of its own). Item kinds as `loadouts.ron` writes them:

| Class | Loadout | Hand | Row 0 (item, display) | Row 1, Scholomance sets (item, display) |
|---|---|---|---|---|
| Warrior | `two_hander` | main hand | Runeblade of Baron Rivendare (24166) | Barovian Family Sword (25649) |
|  | `dual_wield` | main hand | Skullforge Reaver (25036) | Lord Blackwood's Blade (35571) |
|  | `dual_wield` | off hand | Bone Slicing Hatchet (31189, OneHandedAxe) | Ebon Hilt of Marduk (25173) |
|  | `shield` | main hand | Skullforge Reaver (25036) | Lord Blackwood's Blade (35571) |
|  | `shield` | off hand | Crest of Retribution (23825) | Lord Blackwood's Buckler (2456) |
| Paladin | `off_hand` | main hand | Scepter of the Unholy (24033) | Hand of Edward the Odd (19729) |
|  | `off_hand` | off hand | Book of the Dead (24039) | Spellbound Tome (31138) |
|  | `shield` | main hand | The Cruel Hand of Timmy (24111) | Hammer of the Vesper (31126) |
|  | `shield` | off hand | Crest of Retribution (23825) | Lord Blackwood's Buckler (2456) |
|  | `two_hander` | main hand | Malown's Slam (25629) | Frightskull Shaft (25148) |
| Hunter | `bow` | ranged | Screeching Bow (31240) | Ancient Bone Bow (30926) |
|  | `crossbow` | ranged | Carapace Spine Crossbow (31239) | Heartseeking Crossbow (22929) |
|  | `gun` | ranged | Willey's Portable Howitzer (18298) | Shell Launcher Shotgun (28743) |
| Rogue | `daggers` | main hand | Bonescraper (25614) | Specter's Blade (20574) |
|  | `daggers` | off hand | Bonescraper (25614) | Frightalon (20592) |
|  | `maces` | main hand | The Cruel Hand of Timmy (24111) | Hammer of the Vesper (31126) |
|  | `maces` | off hand | Bonesnapper (28689) | Bonechill Hammer (25096) |
|  | `swords` | main hand | Skullforge Reaver (25036) | Lord Blackwood's Blade (35571) |
|  | `swords` | off hand | Skullforge Reaver (25036) | Ebon Hilt of Marduk (25173) |
| Priest | `staff` | main hand | Redemption (20384) | Staff of Metanoia (34894) |
|  | `mace_off_hand` | main hand | Scepter of the Unholy (24033) | Hand of Edward the Odd (19729) |
|  | `mace_off_hand` | off hand | Book of the Dead (24039) | Blackwood's Thigh (28647) |
|  | `dagger_off_hand` | main hand | Gift of the Elven Magi (24046) | Witchblade (24775) |
|  | `dagger_off_hand` | off hand | Scepter of Interminable Focus (34807) | Spellbound Tome (31138) |
| Shaman | `staff` | main hand | Staff of Balzaphon (35563) | Staff of Metanoia (34894) |
|  | `two_hander` | main hand | Malown's Slam (25629) | Frightskull Shaft (25148) |
|  | `shield` | main hand | Scepter of the Unholy (24033) | Hand of Edward the Odd (19729) |
|  | `shield` | off hand | Husk of Nerub'enkan (4107) | Rattlecage Buckler (25138) |
| Mage | `staff` | main hand | Staff of Balzaphon (35563) | Headmaster's Charge (24771) |
|  | `sword_off_hand` | main hand | Blade of Necromancy (34810) | Lord Blackwood's Blade (35571) |
|  | `sword_off_hand` | off hand | Tome of Knowledge (24072) | Spellbound Tome (31138) |
|  | `dagger_off_hand` | main hand | Gift of the Elven Magi (24046) | Witchblade (24775) |
|  | `dagger_off_hand` | off hand | Scepter of Interminable Focus (34807) | Blackwood's Thigh (28647) |
| Warlock | `staff` | main hand | Staff of Balzaphon (35563) | Headmaster's Charge (24771) |
|  | `sword_off_hand` | main hand | Blade of Necromancy (34810) | Ebon Hilt of Marduk (25173) |
|  | `sword_off_hand` | off hand | Book of the Dead (24039) | Blackwood's Thigh (28647) |
|  | `dagger_off_hand` | main hand | Gift of the Elven Magi (24046) | Witchblade (24775) |
|  | `dagger_off_hand` | off hand | Scepter of Interminable Focus (34807) | Spellbound Tome (31138) |
| Druid | `mace_off_hand` | main hand | Scepter of the Unholy (24033) | Hand of Edward the Odd (19729) |
|  | `mace_off_hand` | off hand | Tome of Knowledge (24072) | Blackwood's Thigh (28647) |
|  | `two_hander` | main hand | Peacemaker (31174, Polearm) | Frightskull Shaft (25148) |
|  | `staff` | main hand | Redemption (20384) | Staff of Metanoia (34894) |

**A look shown over a worn piece is a zone's**: the slot's piece of
that zone in the collection unlocks it (every row of the zone).

| Class | Armor | Weapons (slot 8, 9) |
|---|---|---|
| Warrior | plate | weapon |
| Paladin | plate | weapon |
| Hunter | mail | ranged weapon |
| Rogue | leather | main hand + off hand |
| Priest | cloth | weapon |
| Shaman | mail | main hand + shield |
| Mage | cloth | weapon |
| Warlock | cloth | weapon |
| Druid | leather | weapon |

**Weapons shown come from the weapon loadout** (roadmap R3,
classes.md "Weapon loadouts"). A class gear weapon is a zone's **armaments** ("<zone>
Armaments", a rogue's or shaman's second "<zone> Off-hand Armaments"): worn, it gives its stats,
and **one weapon of a zone unlocks that zone's weapon looks for all three loadouts**. The hero
(in a run, on the character list and on the hero sheet) shows its class's chosen loadout's
weapons in main hand, off hand and ranged, in the armaments of the class gear weapon worn (the
look chosen for its slot, else its own zone's; Elwynn's with none; the latest where it wears
two); the starting outfit's weapons are dropped. Each loadout weapon has a look per zone (and
row) in `content\loadouts.ron`, with the item's name, display id and, where the item is another
kind than the loadout's weapon (an axe in a line of swords, a polearm for the druid's two-handed
mace), its own `kind`, which sets the grip and where it rests. A loadout weapon answers the item
query by its kind and look (no name), so the client holds and sheathes it right. Druid forms
show no items (they still get the stats); Moonkin shows the weapon sheathed. Code: `esurv-sim`
`game::gear` (`weapon_look`, `loadout_armaments`), `esurv-server` `looks::armed`.

### Stats

**Each zone drops its own gear.** A piece is a slot from a zone, and its every stat is

    stat = its Elwynn value × gear factor(D), rounded UP to a whole number
    gear factor(D) = D^0.55 up to D 9, then × 1.3 for every step of D past 9

where D is the `difficulty` of the zone it dropped in (`content\zones\<id>.ron`), and 0.55, 9 and
1.3 are `gear_exponent`, `gear_steeper_from` and `gear_growth_after` in `balance.ron`
`zone_scaling`. The factor is ×1, 1.46, 1.83, 2.14, 2.42, 2.68, 2.92, 3.14, 3.35 and 4.35 from
Elwynn to Ahn'Qiraj (5.66 and 7.36 for zones at D 11 and 12). It is set to the gear budget
(AUTHORING.md, "Power budgets"): each zone's whole set adds about 10 % to the expected hero's
damage over the set before it, up to the set worn in Ahn'Qiraj; past that, when talents stop
growing, each set stands in for a zone of talents (budget about ×1.25), hence the steeper part.
The ×1.3 past D 9 follows our target kill times instead and measures about +35 to +50 % a set,
above that budget.
Every stat rounds **up**, percentages to whole percents, flat stats (health,
health per kill) to whole points, so no gear stat ever shows a decimal, bonus pairs included (a
health per second of 0.2 would show as 1). A whole number stays as it is. A zone added later at
any D gets its own gear level automatically.
A piece's double cast keeps a single piece's cap (60 %), and the hero's double cast from every
source stops at 200 %; extra targets (on no piece today) are never scaled.

Elwynn values, the same for every class:

| Slot | Elwynn stats |
|---|---|
| 0 head | +1% crit, +4% crit damage |
| 1 shoulders | +3% area, +5 max health |
| 2 chest | +15 max health, +1% damage reduction |
| 3 waist | +10% pickup range, +5% gold |
| 4 legs | +10 max health, +2% double cast |
| 5 feet | +2% move speed, +10 max health |
| 6 wrists | +2% cooldown recovery, +2% heal and shield power |
| 7 hands | +10% luck |
| weapon (main) | +8% damage |
| weapon (Rogue and Shaman off-hand) | +4% damage, +6 max health |

**Heal and shield power** is a stat of our own: it multiplies the healing of the hero's spells
(instant heals, heals over time, healing charges, healing from a spell's damage) and the
shields the hero casts. Health per second, lifesteal and health per kill are not touched.

From the **second zone on** (zone index 1 and up: not Elwynn) a piece also gets a **class bonus
pair**: the one its class's set names for the slot (`bonus_pairs` in `gear.ron`, a place in
`bonus_stats` for each armor slot, then each weapon; the shipped choice is
`(wow_class_id × 3 + slot) mod 8`, and a new weapon slot names its own), scaled and rounded up the
same way:

| # | Bonus pair (Elwynn values) | # | Bonus pair (Elwynn values) |
|---|---|---|---|
| 0 | +2% damage, +1% crit | 4 | +1% cooldown recovery, +2% luck |
| 1 | +5 max health, +1% damage reduction | 5 | +10 max health, +1 health per kill |
| 2 | +5 max health, +1% cooldown recovery | 6 | +2% damage, +1% move speed |
| 3 | +1% move speed, +1% crit | 7 | +3% area, +1% crit |

Per zone (the gear factor and the rounded-up stats; a bonus pair's values follow the rows with the same
Elwynn value):

| | Elwynn | Duskwood | Warsong | Scarlet Monastery | Sunken Temple | Dire Maul | Gurubashi | Stratholme | Blackrock | Ahn'Qiraj |
|---|---|---|---|---|---|---|---|---|---|---|
| D | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
| Gear factor | 1.00 | 1.46 | 1.83 | 2.14 | 2.42 | 2.68 | 2.92 | 3.14 | 3.35 | 4.35 |
| 1 % (head crit, chest reduction, pairs) | 1 | 2 | 2 | 3 | 3 | 3 | 3 | 4 | 4 | 5 |
| 2 % (legs double cast, feet speed, wrists, pairs) | 2 | 3 | 4 | 5 | 5 | 6 | 6 | 7 | 7 | 9 |
| 3 % (shoulders area, pair 7) | 3 | 5 | 6 | 7 | 8 | 9 | 9 | 10 | 11 | 14 |
| 4 % (head crit damage, off-hand damage) | 4 | 6 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 18 |
| 5 % gold; 5 health (shoulders, pairs 1, 2) | 5 | 8 | 10 | 11 | 13 | 14 | 15 | 16 | 17 | 22 |
| 6 health (off-hand) | 6 | 9 | 11 | 13 | 15 | 17 | 18 | 19 | 21 | 27 |
| 8 % damage (weapon) | 8 | 12 | 15 | 18 | 20 | 22 | 24 | 26 | 27 | 35 |
| 10 % (pickup, luck); 10 health (legs, feet, pair 5) | 10 | 15 | 19 | 22 | 25 | 27 | 30 | 32 | 34 | 44 |
| 15 health (chest) | 15 | 22 | 28 | 33 | 37 | 41 | 44 | 48 | 51 | 66 |
| 1 health per kill (pair 5) | 1 | 2 | 2 | 3 | 3 | 3 | 3 | 4 | 4 | 5 |

Example: a warrior chest from Ahn'Qiraj = 66 health and 5 % damage reduction, plus its pair 5:
44 health and 5 health per kill.

*Inferred:* only the piece **worn** in a slot counts (the stat sheet reads one class-gear list).

### Item entries and saves

**A save keeps pieces by name.** Each class keeps its `collection` (every piece won), what it
has `worn` and the look chosen per slot (`looks`), a piece as its slot id and its zone id:
`(slot: "head", zone: "elwynn")`, a look as its zone id and row: `"feet": (zone: "brm", row: 1)`
(row 0 is left out). The slot ids are the eight armor slots' (`head`, `shoulders`,
`chest`, `waist`, `legs`, `feet`, `wrists`, `hands`), then three weapon sets of two
(`main_hand`, `off_hand`, `main_hand_2`, `off_hand_2`, `main_hand_3`, `off_hand_3`). A zone
added, moved or taken out changes nothing in a save; a piece of a zone the content does not
have is kept, and shows again when the zone comes back.

**Item entries are worked out per session and never saved:** a piece's entry is
**`2 000 000 + wow_class_id × 100 000 + slot × 1000 + zone × 10`**, with slot 0-7 the armor and
`8 + 2 × set + hand` a weapon (room for 14 slots and more), and zone the zone's place in the
difficulty order (0-99); its last digit stays 0. A piece worn in a look not its own (a place in
the class set's `looks`, 0-99) shows in a run as **`10 000 000 + (piece entry − 2 000 000) / 10 ×
100 + look`**. Example: a warrior's off hand of the second weapon set from Ahn'Qiraj (zone 9) is
2 000 000 + 100 000 + 11 000 + 90 = 2 111 090; in look 10 (Blackrock's second row) it shows as
10 000 000 + 11 109 × 100 + 10 = 11 110 910. Code: `esurv-sim` `game::rewards::gear_item_entry`,
`game::gear::decode_item_entry`, `slot_id`; `esurv-server` `looks::shown_item_entry`.

**Older saves** kept item entries and are moved on once, when opened or imported
(`gear_numbering` below 2; code: `esurv-server` `save`, `older::OlderSave`):

* the first numbering, `100 000 + class × 1000 + slot × 100 + tier × 10` (a piece was a tier):
  every piece of tier *t* becomes the piece of the **later zone of that tier** (tier 0 → Elwynn,
  1 → Warsong Gulch, 2 → Dire Maul, 3 → Stratholme, 4 → Gates of Ahn'Qiraj);
* the second, `1 000 000 + class × 10 000 + slot × 1000 + zone × 10`: the zone is read from the
  save's `gear_zones` (or, without it, the nine shipped zones' first order).

An entry that names no piece is dropped.

**Looks chosen by tier** (every save before each zone had its own look kept `appearance`, slot
→ tier 0-4) are moved on once when read (`SaveFile::migrate`): a
slot's look of tier *t* becomes the look of the **latest zone of that tier whose piece of the
slot the class has won** (tier 0 Elwynn; 1 Duskwood, Warsong Gulch; 2 the Scarlet Monastery,
the Sunken Temple, Dire Maul; 3 Gurubashi Arena, Stratholme; 4 Blackrock Mountain, Gates of
Ahn'Qiraj), row 0; with none won the choice goes and the slot shows its own look. The collection,
the worn gear and the loadout are kept as they were.

### Names and looks

Every zone its own look.

* **Name:** each zone's look names its eight pieces (`names`): Elwynn's the Recruit's set's
  (`"Recruit's Helm"`, `"Squire's Girdle"` ..., the starting look's names), the Scarlet
  Monastery's `"Adventurer's <slot>"` with the armor type's slot noun, every other zone the
  approved item's own name (`"Sparkmetal Coif"`, `"Helm of Might"`). A weapon is its zone's
  armaments, `"<zone> Armaments"` (an off-hand weapon `"<zone> Off-hand Armaments"`), the zone's
  name from the client's own table. A piece is named as its own zone's first row wherever its
  look is from; worn in another row of its own zone (Blackrock's or Stratholme's second), it takes that row's
  name.
* **Slot nouns by armor type** (cloth / leather / mail / plate), e.g. head: Circlet / Cover /
  Helm / Helm; chest: Robes / Chestpiece / Breastplate / Breastplate: the Scarlet Monastery's
  names use them. A cloth chest is a robe (inventory type 20).
* **Looks:** an `ItemDisplayInfo.dbc` id per zone's look and slot (`displays`). A **0** borrows
  the nearest look that has one: one look before, then one after, then two before ...; Elwynn's
  starting look has no helm (or mantle) for several classes, which show Duskwood's. A zone
  without a look of its own (one added later) shows the look of the nearest zone before it.
* **Weapons** carry their `kind` (`TwoHandedSword`, `Dagger`, `Staff`, `Bow`, `Shield` ...: the
  item class and subclass the item query tells of the piece) and `hand` (`MainHand`, `OffHand`,
  `Ranged`); what they look like is the loadout weapons' look per zone (section 3).
* **Armor slots** are named (`slot: Head`, `Shoulders`, `Chest`, `Waist`, `Legs`, `Feet`,
  `Wrists`, `Hands`); the slot gives the client's inventory type and equipment slot.
* **Flavour text:** each piece has a short line in its tooltip, written for this game.
* **Quality colours** per tier: *inferred* white → orange.

### Which boss drops what (confirmed)

A boss drops pieces of its own zone, **rolled the moment it dies**
(after its experience, gold and any due magnet). Each candidate piece rolls on its own at the
zone's drop chance × the run-option bonus (section 1); a piece won shows its pop-up at once and joins the collection.
Code: `esurv-sim` `game::rewards` (odds, slots), `game::casting` (the roll on a boss kill).

* **Non-final bosses** split the 8 armor slots of their own zone: with `n` non-final bosses, boss number
  `i` owns the slots `s` where `s mod n = i`. With four: boss 0 → head + feet, 1 → shoulders +
  wrists, 2 → chest + hands, 3 → waist + legs, in every zone.
* **Final bosses** drop the class's weapons (slots 8 and up; rogue and shaman roll two).
  Where several final bosses come together (the Scarlet Monastery's
  Mograine and Whitemane), only the last of them to fall rolls the weapons.
* Pieces won are collected during the run and kept afterwards.

**What a drop does (observed):** a won piece goes into the hero's **collection** for good and
is announced with a pop-up. A piece already in the collection changes nothing (the pop-up still
shows). Gear is **never equipped automatically**: the player equips it in the armory, and it takes
effect from the next run.

## 4. Content format: `content\gear.ron`

One file, edited by hand:

```ron
(
    slots: [ (slot: Head,
              names: ["Circlet", "Cover", "Helm", "Helm"],
              stats: [(stat: CritChance, value: 0.01), (stat: CritDamage, value: 0.04)]), … ],
    bonus_stats: [ [(stat: Damage, value: 0.02), (stat: CritChance, value: 0.01)], … ],   // 8 pairs
    sets: [
        (
            class: "warrior",
            armor: Plate,
            bonus_pairs: [3, 4, 5, 6, 7, 0, 1, 2, 3],   // a bonus pair per slot, then per weapon
            looks: [                                   // a look per zone, in difficulty order
                (zone: "elwynn", names: ["Recruit's Helm", …], displays: [0, 26164, …]),
                …
                (zone: "brm", names: ["Helm of Might", …], displays: [31260, …]),
                (zone: "brm", row: 1, names: ["Helm of Wrath", …], displays: [34215, …]),
                …
                (zone: "strat", row: 1, row_name: Some("Scholomance sets"),   // optional name
                 names: ["Helm of the Executioner", …], displays: [34902, …]),
                …
            ],
            weapons: [(stats: [(stat: Damage, value: 0.08)], kind: TwoHandedSword, hand: MainHand)],
        ),
        …
    ],
)
```

The weapons' looks, each zone's armaments, are in `content\loadouts.ron`, a list per loadout
weapon:

```ron
(hand: MainHand, kind: TwoHandedSword, name: "Two-handed sword", swing: Some((seconds: 3.6, share: 2.0)), looks: [
    (zone: "elwynn", name: "Edged Bastard Sword", display: 26585),
    (zone: "scarlet", name: "Ravager", display: 22221, kind: Some(TwoHandedAxe)),   // its own grip
    (zone: "brm", row: 1, name: "Ashkandi, Greatsword of the Brotherhood", display: 32000),
    …
]),
```

The loader checks every look has 8 names and 8 displays, no zone and row twice, a zone's rows
from 0, no blank row name, at most 100 looks a set, and that every loadout weapon has a look for every look of its
class's set. Code: `esurv-sim` `game::gear` (`piece_modifiers`, `zone_gear_factor`, `zone_looks`,
`zone_look`, `piece_name`, `armor_display`, `armament`, `armor_inventory_type`,
`decode_item_entry`).

## 5. Adding or changing gear

* **Change a stat:** edit a slot's `stats`, a weapon's `stats`, or a `bonus_stats` pair (their
  Elwynn values; every stat is rounded up to a whole number after scaling). Any
  hero stat works (`Damage`, `CooldownRecovery`, `FlatMaxHealth`, `CritChance`, …).
* **Change how fast gear grows from zone to zone:** edit `gear_exponent`, `gear_steeper_from` and
  `gear_growth_after` in `balance.ron` `zone_scaling` (section 3, "Stats"), then check the gear
  budget with `balance_check --gear` (AUTHORING.md, "Power budgets"); a new zone file in
  `content\zones\` gets its level from its `difficulty`.
* **Change a look or a name:** in the class's `looks`, swap a display id for another from
  `ItemDisplayInfo.dbc` that fits the slot, or the name; a weapon look in the loadout weapon's
  `looks` (`loadouts.ron`), with `kind` where the item is another kind than the loadout's.
* **A new zone's look:** add a look with the zone's id to every class's `looks` and a line for
  it to every loadout weapon; a second row of looks for a zone is `row: 1` (both need their
  weapons' lines), optionally named with `row_name: Some("...")` (the Collection Log's row head
  and the look's tooltip; left out, "Second look"). Until then the zone's pieces show the zone's
  before it.
* **New weapon line** (e.g. the roadmap's loadouts): add to the set's `weapons`; it becomes the
  next slot number. A set holds at most six (three weapon sets of two, slots 8 to 13), as the
  loader checks.
* **A class's set:** every class needs exactly one ([templates/gear_set.ron](templates/gear_set.ron)).
* Run `cargo test -p esurv-content -p esurv-sim` to validate (with the player's data, `esurv-app`'s
  `every_gear_and_armament_look_is_in_the_players_data` checks every display id is in it).
