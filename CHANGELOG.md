# Changelog

Every release of Eternal Survivors, newest first. Each lists its patch notes (gameplay, balance,
quality of life) and its changelog (fixes and details).

## v0.4.1

Updates come to you now: the game can update itself from GitHub.

### Patch notes

**Gameplay**
- **Morbent Fel:** his shield is a purple shadow bubble, and Morbent's Bane is the Torch of Holy Flame planted in the ground inside a golden bubble, so it is easy to spot.

**Quality of life**
- **Updates in the game:** when a new version is out, the character screen offers it with an Update button that downloads it, checks it and installs it, then restarts the game; your heroes and settings stay. Turn it off in Options > Interface > Check for updates.
- **Smart damage numbers hide more small hits:** every 30 seconds the game takes the average of your hits, each counted as much as it deals, and hides the numbers below 60% of it, so floods of small ticks no longer get through.

### Changelog

**Fixes**
- Item and talent procs, such as the Choker of Enlightenment's extra hits, no longer make your hero play a casting animation; their missiles and hits still show.

**Install and technical**
- The update check asks GitHub's public release list once at start, sends no player data, and stays silent when offline. A download is installed only when it matches the release's checksum file.
- Each release now comes with a `.sha256` checksum file next to the zip.
- From v0.4.0 or older, update by hand once; later versions offer updates in the game.

## v0.4.0

The Stratholme update: Alterac Valley gives way to Stratholme, and the game has a new name.

### Patch notes

**Gameplay**
- **New name:** the game is now called Eternal Survivors.
- **Stratholme replaces Alterac Valley:** hold Slaughter Square with its gates barred against the Scourge. New bosses: Magistrate Barthilas, Baroness Anastari, Nerub'enkan and Maleki the Pallid. Your Alterac Valley progress, gear and looks carry over.
- **Stratholme gear:** its armaments are Stratholme and Scholomance weapons, and its gear has a second look, the **Scholomance sets** (Deathbone Guardian, Bloodmail Regalia, Cadaverous Garb, Necropile Raiment). Every Stratholme piece unlocks both looks.
- **Dire Maul enemies:** the gnolls are gone. Whip Lashers from Dire Maul East and the ghostly Eldreth Sorcerers from the west take their places.

**Balance**
- **Mage, Frost:** the 31-point talent is now Winter's Chill. It no longer summons a Water Elemental, since the Frostbolt starting option already brings one. You still start every run with Frost Nova, casting itself every 9 seconds, and Frost spells deal 9% more. Enemies shatter: +8% damage to slowed, held or fleeing enemies, every Frostbolt hit splashes 5% onto enemies within 5 yd, and Frostbolt deals 20% more.
- **Druid:** Swipe (Cat) is free: no energy cost and no combo point, on its own 3-second cooldown.
- **Cooldown talents:** talents that speed every spell's cooldown give 0.5% a rank, talents for one spell 2% a rank, and talents for two or three spells (or all totems) 1% a rank. Weapon swings keep their pace.

### Changelog

**Stratholme**
- The arena is Slaughter Square (Stratholme), closed by its four gates.
- The Baron's finale follows the dungeon's own order, and spawns resume after it, so Endless keeps going.
- New boss-script pieces: doors a boss script opens, and add waves that come in at a set spot.
- Dungeon arenas now also walk on their terrain beside their buildings.
- The Collection Log and look tooltips can name a gear row; Stratholme's second row reads "Scholomance sets".

**Install and technical**
- The setup finds your existing game folder and moves it to the new name, saves and settings included, then updates it. Your characters' settings move to the new realm name.
- Saves naming Alterac Valley read as Stratholme: wins, gear, looks, the hero's zone and discovered zones carry over.
- The game exe is "Eternal Survivors.exe" and the setup "Eternal Survivors Setup.exe".

## v0.3.0

The boss update: every boss now fights with its own mechanics from WoW, reshaped for a survivors game.

### Patch notes

**Gameplay**
- **Every boss reworked:** all bosses of Elwynn Forest, Duskwood, Scarlet Monastery, Sunken Temple, Dire Maul, Gurubashi Arena, Alterac Valley, Blackrock Mountain and Ahn'Qiraj fight with their own WoW mechanics. Warsong Gulch keeps its player-character bosses.
- **Ground markings like FF14:** every boss area attack is marked on the ground in orange before it lands (circles, rings with a safe middle, cones and lines), with a fill that shows when it hits. Drifting zones and sweeping beams show arrows.
- **Charges run the whole line:** a charging boss runs through its marked line; stand in it and you're hit as it passes.
- **Boss warnings and timer bars:** big warnings with the raid warning sound, and timer bars for each boss's next abilities.
- **Scripted moments:** for example, Morbent Fel raises the dead (only undead follow), VanCleef calls his lapdogs, Whitemane revives Mograine, Hakkar drains everyone with Blood Siphon, Majordomo stays immune until his guards fall and Ragnaros rises, and Nefarian makes class calls.
- **Things to walk over:** break eggs, cocoons and totems, put out blood fires, and grab relics and crystals by standing on them.
- **Boss voices:** many bosses speak their original lines.
- **Bosses never stun, fear, root or slow you.** The only exceptions are two short scripted cutscenes.

**Balance**
- **Unavoidable boss damage stays the same:** only marked attacks you fail to dodge add damage.
- **Boss sizes:** bosses are now drawn at a consistent size, and final bosses are bigger.
- **Druid:** Blood Frenzy boosts both Swipes, and Swipe (Cat) has its own legendary amulet.
- **Zone health retuned** for the new fights.

**Quality of life**
- **Smart damage numbers:** Options → Interface → Damage numbers. "Smart" hides your smaller hits to declutter the screen, and "All" shows every number. Smart takes the average of your hits over each minute and, for the next minute, hides numbers below 60 % of it. Crits don't count toward the average unless your crit chance is above 50 %.
- **Damage meter names your procs:** talents, rings, trinkets, amulets and legendaries each get their own row instead of "Talent procs".
- **Options → Timers:** a new tab for the boss warnings, the warning sound (untick to mute), the timer bars, the bar handles and resetting their positions.
- **Movable timer bars:** drag them by their handle, and they stay where you put them.
- **Minimap buttons:** one shows or hides the timer bars' drag handles, the other shows or hides the damage meter.
- **New game icon** on the game exe and window.

### Changelog

**Bosses**
- The boss building blocks are marked circles, rings, cones, lines, charges, drifting zones and sweeping beams; lasting zones (fire, poison, plague, webs) with stacking effects; boss scripts on arrival, health, timers and death; add waves; spawn stops and roster swaps; shields and immunity; form changes; enrages; and invisible target units so a dodged spell still shows its impact.
- Warnings: centre-screen text with an icon and the raid warning sound, "soon" pre-warnings, and timer bars that flash under 5 seconds.
- Charges mark a line from the boss through you to 8 yards past you, capped at the ability's length.
- Herod enrages at 25 %, and Angerforge calls his adds at 30 %. Jeklik is silent, and no boss has a cast you need to interrupt.
- Zevrim, the Ironbark Protectors, Lokholar and Korrak use their original models. Jammal'an's and Jin'do's totems are the original troll wards.
- Boss sizes: each model's real size was measured so normal bosses match and final bosses are a third bigger. Nefarian, Azuregos, Eranikus and C'Thun keep their own sizes. Majordomo's Flamewakers and Ragnaros were fixed from far too small.

**Fixes**
- Boss attacks never hit or flinch normal enemies.
- Rogue's Two maces loadout now uses melee maces only, with no caster or healer maces.

**Install and technical**
- The game exe carries the new icon; the setup exe keeps its own.
- The damage meter's proc rows come from each hit's origin. Older clients still read the meter.

## v0.2.2

Hunter fixes and a Rogue boost.

### Patch notes
- **Hunters sound and look right:** crossbows no longer fire with gun sounds, guns play one gun shot per shot, and the bowstring sits on the bow and is light grey instead of black.
- **Hunters heal from damage:** 3% lifesteal, the same as Rogues.
- **Rogue finishers hit harder:** Rupture +50%, Envenom +25%, Eviscerate +24%.

### Changelog

**Classes and spells**
- Hunter: 3% lifesteal (as Rogues have).
- Rogue: Rupture deals 690 over 8 seconds.
- Rogue: Envenom deals 125 to each poisoned enemy.
- Rogue: Eviscerate deals 435.

**Fixes**
- Crossbow shots play crossbow sounds instead of gun sounds.
- Gun shots play exactly one gun sound each.
- The bowstring is attached to the bow's tips and drawn light grey.
- Crossbow bolts play the arrow hit sound when they land.

**Interface**
- On an amulet card that would replace your worn amulet, the "Replaces …" line is red.

## v0.2.1

A bug-fix release.

### Changelog

**Fixes**
- Race abilities are no longer changed by the 40% buff uptime rule from v0.2.0. Perception, Blood Fury and Berserking last 10, 12 and 10 seconds again, as in v0.1.0. The rule applies only to class buff spells.

## v0.2.0

### Highlights
- **Druid Cat gets area damage:** a new Swipe (Cat) that alternates with Claw.
- **Bosses speak:** 20 bosses play their original voice line when they arrive.
- **Enemies carry weapons** in Scarlet Monastery, Dire Maul and Blackrock Mountain, and swing them properly.
- **A cleaner warband screen** with Collection and Talents buttons that glow when something is new.
- **Bosses no longer stun, fear, root or slow you.**

### Changelog

**Classes and spells**
- Druid: new Swipe (Cat): 60 energy, a combo point, takes turns with Claw once learned, 47 damage to each enemy nearby, and can take the Energy card. Swipe (Bear) stays a separate rage spell.
- Druid: Swipe is offered twice as often for both Cat and Bear.
- Druid: Claw can take the Targets card.
- Druid: Blood Frenzy boosts both Swipes.
- Druid: Swipe (Cat) has its own legendary amulet.
- Warrior: Cleave now has a 3 second cooldown and deals 45 damage (same damage per second).
- Warrior: Bloodthirst no longer offers a Duration card.
- Rogue: Slice and Dice also gives +20% attack speed.
- Buff spells (Adrenaline Rush, Slice and Dice, Tiger's Fury, Dash, Power Infusion, Battle Shout, Berserk, Arcane Power, Perception, Blood Fury, Berserking) now start at 40% uptime: active for 40% of their cooldown before upgrades.
- Cooldown reduction from talents, rings and trinkets is halved for spell cooldowns.
- Fire's Fireball-crit cooldown talent stays at 0.5 seconds.
- Targets cards now start at rare: rare gives +1 target, epic and legendary +2.

**Items**
- Hand of Justice keeps its spell double cast and adds a chance for weapon attacks to hit twice: 2% on a common one, rising with rarity and upgrades.

**Bosses and enemies**
- Bosses can no longer stun, fear, root or slow you.
- 20 bosses play their original voice line on arrival, and their yell text now matches what they say.
- Warsong Gulch bosses fight with abilities that fit their class, and wear the right weapons.
- Quintys, Eggmike, Nerf and Pkr show no helmet.
- Enemies and bosses in Scarlet Monastery, Dire Maul and Blackrock Mountain hold their weapons and swing them with the matching animation; dual wielders alternate hands.

**Zones**
- Enemy health retuned for every zone after the balance changes.
- Gurubashi Arena enemies are less tough again.

**Animations**
- Dual-wielding Warriors and Rogues now show their off-hand swings.
- Two-handed weapons play the two-hand swing for Cleave, Heroic Strike, Mortal Strike, Revenge, Execute and Overpower.
- A Shaman with a two-hander keeps it drawn through casts.
- Moonkin keeps its weapon put away.

**Interface**
- Warband screen: Collection and Talents buttons above the armour bar. They glow when there is new gear or new talent points, and the glow clears once you've looked.
- Collection: double-click a piece to wear it.
- Modify character: shows the selected race's racial abilities.
- Victory and defeat screens: "Return to main menu" replaces "Change Hero".
- Options: new Camera following style setting (Only when moving, Always adjust camera, Never adjust camera).

**Setup**
- The setup works as a patcher: it tells you whether it is installing, updating or reinstalling, warns before a downgrade, and never touches saves or your own files.

## v0.1.0

The first alpha release.
