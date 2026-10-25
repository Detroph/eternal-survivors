# The game server

Eternal Survivors runs its own WoW 1.12.1 server inside the game process (`crates/esurv-server`).
The client part (benilla) connects to it as to any 1.12 server, so everything the player sees
travels as normal WoW packets. Plain-language twin: [guides/server.txt](guides/server.txt).

## How a launch goes

1. `esurv.exe` loads the content, opens the save and the player's 1.12.1 data, and
   starts the server on its own threads: the logon server and one world listener for every
   zone, each on a free port of `127.0.0.1`.
2. It hands the client the logon address, the account name `HERO` and a password made for this
   launch (`WOW_HOST`, `WOW_USER`, `WOW_PASS`), so the client logs in by itself.
3. The realm list holds one realm, **Eternal Survivors** (`REALM_NAME`), the world listener. The
   launcher names it to the client (`WOW_REALM`), so the client goes straight on and the realm
   list never shows. The realm carries the game's name, so when the game is renamed, the setup
   moves the characters' settings benilla keeps under the realm's name (`camera/`, `chat/`,
   `layout/`, `bindings/`, `addons/`, `saved/`, `cache/<realm>.tsv` in `benilla-config/`) from the
   earlier names to the current one and deletes the rest (`esurv_setup::realm_files`). The window is
   titled **Eternal Survivors**. The zone is chosen with the hero on the character screen
   ([menus.md](menus.md)) and kept in the save with the hero.
4. The character screen lists the heroes from the save, each wearing its class's worn gear
   (below), and shows them in the hero picker's camps ([screens.md](screens.md)). Creating a
   hero takes the stock create
   screen's name, race, class, gender and look; a class or race the game does not have is
   refused, and names follow the stock rules (2 to 12 letters, stored capitalised, unique).
   So is a race and class pair the create screen does not offer: the stock pairs, read from
   the player's `CharBaseInfo.dbc` (every pair passes when the data is not open), and the six
   pairs `races.ron` adds ([classes.md](classes.md), "Race and class
   pairs"), which are saved and played like any other.
   A save holds at most nine heroes of either faction, 18 in all (the
   hero picker's camps); a tenth of one faction is refused with the stock "maximum number of
   characters on this realm" reply (`CHAR_CREATE_SERVER_LIMIT`).
5. Entering the world starts a run for that hero in the zone chosen for it (else the zone it
   entered last, else the first zone), hosted on its own thread: the hero stands at the zone's
   spawn point, with the zone's time of day and weather. The first step starts the run. Each
   hero's row in the character list stands at the spawn of that zone, so the loading screen and
   the world it streams are the zone's.

## What the server reads

| What | From |
|---|---|
| Zones, enemies, classes, spells … | `content/*.ron` (`$ESURV_CONTENT`, else `content/` in the project folder on a dev build or beside the executable) |
| The heroes | `saves/heroes.ron` beside the game: the game's folder, or the project folder in a dev build (`$ESURV_SAVE` names another file); a save left in benilla's state folder is copied over the first time |
| Ground heights under the arena | the map's ADT tiles in the player's own `Data\*.MPQ`; on a WMO-only map (Dire Maul) its building's floors: every group's collision faces except MOPY `DETAIL`, plus the shown doodads' hulls, asked for the floor nearest below a height |
| What blocks a spawn's sight | nothing on terrain; on a WMO-only map the building's faces except `NOCAMCOLLIDE` (the client's camera rule), plus the doodad hulls |
| Where a body can walk in a dungeon | a grid of one-yard floor cells over the building's floors, built as searches reach it (`walk_graph`); spawns (rares' too) and pickups must be walked to, and enemies follow the nearest hero's flow field round walls and up ramps. Costs at Dire Maul's spot: about 65 ms for the first field, then 5.5 ms per yard a hero walks, and 3.5 ms for 300 enemies' waypoints |
| How fast spell missiles fly | `Spell.dbc` in the same data |

Without the 1.12.1 data the server still runs, on flat ground with every spell hitting at once
(the tests use this).

### The save file

A RON file of our own:

| Part | Holds |
|---|---|
| `heroes` | each hero's wire guid, name, class and race (ids from `classes.ron` and `races.ron`), gender and look, its `records`: best level, best time survived (seconds), runs played, runs won, and its `zone`: the zone its next run is in (chosen before a run, or the one it entered last) |
| `next_guid` | the next free guid (guids are never reused) |
| `gear_numbering` | 2: class gear is kept by slot and zone id; a save with an older number kept item entries and is moved on once as it is read (gear.md, "Item entries and saves") |
| `classes` | per class id, what every hero of the class shares: `talent_experience` (all earned; points are bought from it), `talents` (ranks bought, by talent id), `zones_won` (the zone ids won; the zones open are every zone up to the hardest won by difficulty, and the next; the first is always open. A save from before kept `zones_open`, a count in the nine shipped zones' order, and is moved on once to the zone it says was won last), `collection` (every class-gear piece won), `worn` (one piece a slot), each piece as `(slot: "head", zone: "elwynn")`, and `appearance` (the look shown per slot id, where it is not the piece's own), and `seen` (`Some((pieces: n, points: n))`, the gear pieces won when the player last opened the class's Collection and the talent and prestige points earned when it last opened its Talents; more now is news, which the warband screen's buttons glow on, sent as `new_gear` and `new_points` at the end of `Progress`. A class of a save from before it is taken as having seen everything when the save is read; a class new to the save starts at nothing seen) |
| `codex` | every ring, trinket and necklace collected in play by any hero, by id, added the moment it is taken from an offer; the screens are told it (`ESURVK`, below) |
| `last_played` | the hero played last and the zone (ours, for the screens' defaults) |
| `run_options` | the run options the next run starts with (enemy density, health, damage and speed, whole percentages, each kept on its slider of `run_options.ron`): one set for the whole save; with them the mode (zones.md "Challenge Mode"): `challenge` (the Challenge Mode tab picked last; a class runs Challenge Mode only once it has won Ahn'Qiraj, `aq` in its `zones_won`) and `challenge_pct` (the challenge slider, 100-500 in steps of 25, kept when the tab changes). An older save: Normal Mode at 100 |
| `zones_discovered` | the zone ids any class of the save has opened, for good (the first zone always). Only what the zone lists show hangs on it: a zone not in it shows "???" and a lock everywhere; each class still opens a zone itself to play it. Added when a win opens a zone (`finish_run`); a save from before it is filled with every zone open to any of its classes the first time it is read with the content (`Saves::zones_discovered`). Sent at the end of `Progress` (`ESURVP`) |
| `challenge_news_seen` | the box telling that Challenge Mode is open (zones.md "Challenge Mode") was closed (`seen <class> challenge`): it shows once per save, the first time any class has won Ahn'Qiraj. Until then `Progress` ends with `challenge_news` 1 in every class's progress once a class has it open. An older save: not shown yet |

An older save without these parts still opens; they start empty.

**Export and import** (design step 6;
`save::files`). The folder is `exports` beside the save (`saves/exports` by default). An
export writes the whole save as it stands to a new file there, `<save's name>-<YYYY-MM-DD-HHMMSS,
UTC>.ron`, a number added when the name is taken. The list is the folder's `.ron` files, newest
first (the 50 newest), each read and checked. An import reads the named file (a plain file name
in the folder, nothing with a path), checks it, writes the save it replaces to the folder as
`<save's name>-before-import-<moment>.ron` (importable in its turn), and only then replaces the
save and writes it. A save is taken when it reads as our RON layout (older layouts too: missing
parts start empty) and keeps the save's rules: every hero of a class and a race the content has,
a gender of 0 or 1, a name the stock rules allow and stored as they store it, no two heroes with
one guid or one name (in any case), at most nine heroes of a faction and 18 in all; a next guid at
or under a hero's guid is raised past it.

### The character list

`SMSG_CHAR_ENUM` carries each hero's looks: for its 19 equipment slots, the item display id and
inventory type of what its class wears (`ClassProgress.worn`), each piece in the look chosen for
its slot (`appearance`), else its own tier (`esurv_server::looks`; armor through the gear
set's display table, a weapon through its display per tier). An empty slot is sent empty; the
hero picker dresses it in the race and class's starting outfit piece.

### What the hero wears in a run

In the world the hero shows what the character list shows, the 1.12 way (`esurv_server::looks`,
`Game::dress`), read once as the hero joins its run: a change on the hero sheet shows in the next
run, and nothing changes within one (a fresh run from the end panel keeps it).

* **Visible items.** The hero's player object carries an item entry in each of its 19
  `PLAYER_VISIBLE_ITEM_n_0` fields (no enchants): the class gear worn, each in its look, and in
  every slot the class wears nothing in, the race and class's starting-outfit piece
  (`CharStartOutfit.dbc` from the player's own data, an added pair in the outfit it borrows; none
  without the data). Every client in the run gets these fields, so every hero sees every other
  hero dressed as on their own warband screen.
* **Weapons from the loadout (roadmap R3).** The main-hand, off-hand
  and ranged slots show the class's chosen weapon loadout's weapons (`looks::armed`, gear.md
  section 3), never the outfit's; Cat and Bear Form show none, Moonkin Form enters with them put
  away. The Hunter's ranged weapon kind picks its shot look (`ranged_shots`).
* **Look entries.** A class gear piece's entry is `2 000 000 + class × 100 000 + slot ×
  1000 + zone × 10` (gear.md, section 3), worked out for the session and never saved, its last
  digit 0. The item query names the zone it came
  from in the item's description ("From Duskwood"). A piece worn in another tier's look is shown as its entry with
  `1 + look` in that digit, so each look is an entry of its own: the client keeps every item
  answer for the session, and an entry's answer never changes.
* **Weapons in hand, casters' put away.** A hero enters with its
  sheath state (`UNIT_FIELD_BYTES_2` byte 0) at 1, melee weapons drawn; from there the client's
  own animations draw and put away as the stock 1.12 client does (standing and running with
  weapons drawn; a spell cast puts them away, on the back or hip by each weapon's sheath); a
  ranged weapon shows only when drawn for ranged. Mages, warlocks and priests enter at 0,
  weapons put away, and keep them away: their fight is spells. At the warband camps every hero
  holds its weapons drawn.
* **Starting-outfit weapons put away.** The player's data holds only
  a stock weapon's look and slot, so `outfit_weapons.ron` gives each starting-outfit weapon and
  shield (13 items, by item entry and look) its kind, which sets its item class, subclass and
  sheath: staves on the
  back, one-handers on the hip, two-handers and the shield on the back, ranged weapons nowhere
  (shown only drawn). The item query answers with them, so a mage casting carries her staff on
  her back. The character select's figures hold their weapons drawn; one a figure puts away
  rests the same way (benilla's `OutfitWeaponSheaths`, filled from the same file by the look).
* **The items behind the character pane.** For its own client only, each class gear piece the
  hero wears is an item object (type item, the shown entry, owner and holder the hero, stack 1)
  and the hero's `PLAYER_FIELD_INV_SLOT_HEAD + 2 × slot` fields hold their guids
  (`0x4000 << 48 | hero << 8 | slot`), so the character pane shows the worn pieces in their
  slots. Starting-outfit pieces are seen on the body only.
* **The run's jewelry (`game::jewelry`).** Each ring, trinket and amulet the hero takes is an
  item of its own, for its own client only: the amulet in the neck slot (1), the first two rings
  in the finger slots (10, 11) and the first two trinkets in the trinket slots (12, 13), in the
  order put on, the rest in the backpack (`PLAYER_FIELD_PACK_SLOT_1 + 2 × slot`, blocked in a
  run). An item is made the tick its piece is taken; an upgrade or an empowering is another
  entry, so the old item is destroyed and a new one takes the slot; a fresh run takes them all
  away (the slots set to 0). Their guids are `0x4000 << 48 | 1 << 40 | hero << 16 | a number`.
  The body shows no jewelry, as in 1.12 (no visible-item fields).
* **Jewelry entries.** Each entry holds all its answer tells, fixed for the session:
  a ring or trinket is `1 000 000 000 + its index in jewelry.ron × 10 000 000 + the rarity it
  was put on at (0 common to 4 legendary) × 1 000 000 + its value now in ten-thousandths`; an
  amulet `1 300 000 000 + its index in amulets.ron × 1000 + its rank`; a remade legendary
  amulet `1 400 000 000 + the spell's index in abilities.ron × 1000 + its rank`.
* **The item query.** `CMSG_ITEM_QUERY_SINGLE` is answered with `SMSG_ITEM_QUERY_SINGLE_RESPONSE`:
  * class gear (a piece's entry or one of its look entries): the piece's own name from the
    content, its quality (its tier: tier 0 common, white, to tier 4 legendary, orange), the
    look's display and inventory type, item class and subclass (armour 4 with the set's armour
    type; a weapon's own, the top tier's subclass when its look is the top tier's) and a
    weapon's sheath;
  * a starting-outfit piece: its display and inventory type from the player's data, with no name
    (the 1.12 client has no item table, and the body needs only the look), and a weapon's class,
    subclass and sheath from `outfit_weapons.ron`;
  * jewelry: the content's name (a remade legendary amulet "<recipe>: <spell>"), the rarity as
    quality (common to legendary), the look that shows the content's icon (from the player's
    `ItemDisplayInfo.dbc`: the lowest look of that icon with no model, else the lowest), the
    slot (neck 2, finger 11, trinket 12; armour 4, subclass 0), and in the tooltip's quoted line
    what it does now in our words: a ring's or trinket's text with its value, an amulet's text,
    a legendary's "Remakes your <spell>.", and from rank 2 "Rank n: its effects at x%."
  Stats, prices and requirements are sent empty; any other entry is unknown.

### A run that ends

A run ends in one of three ways:

* **Defeat:** the last hero falls. Every enemy and pet stops where it is; the hero is not rooted
  and gets no death packet (its health field shows 0).
* **Victory:** the final boss's death only marks the moment; the run is won at the next tick
  that finds it running, so a choice the kill brought is made first. Every enemy and pet stops,
  and the hero is rooted (`SMSG_FORCE_MOVE_ROOT`). The run then waits: going on into Endless
  lets the hero go again (`SMSG_FORCE_MOVE_UNROOT`), and every enemy decides where to go at once.
* **Leaving:** a hero asks to leave (End Run or Quit Game on the Escape menu, Return to main menu on
  the end panel): it is out of the run as if it fell, the run pays it out (the save is written then)
  and tells it the end (`left`), and only then its client leaves the world
  (`SMSG_LOGOUT_COMPLETE`, back to character select). The run is over when no hero is left in it.
  With others still in it, their run goes on; a run left while paused stays paused until one of
  them resumes it (their Escape menu is open, as the pause is run-wide).

**Again and Reset.** Once the run is over (a fall or a victory), a hero still in it may ask for a fresh run
(from the end panel).
The run is paid out first, then everything it put on the field goes from every client
(`SMSG_DESTROY_OBJECT` for each enemy, corpse, pet, pickup with its beam, totem and ground look;
each boss bar at 0), and a new run in the same zone with the run options as saved waits for its
first move. Its heroes come in the same order, so each keeps its id, with what their classes
have earned by now (talents, worn gear); a hero who left is out of the new run too. Each hero's
own client gets its hero's numbers (full health, level 1, no experience or gold, to every
client), `SMSG_FORCE_MOVE_UNROOT` if a victory had rooted it, `MSG_MOVE_TELEPORT_ACK` to the
zone's spawn, `SMSG_COOLDOWN_CHEAT` (every cooldown over), the spellbook (`SMSG_INITIAL_SPELLS`,
which replaces the book) and the action bar, then `Ready` and its class's progress; its HUD and
meter are told anew with the next tick, and the meter starts from nothing. The hero's buffs and
the numbers line follow with the next tick. Moves the client sends from where it stood before
are ignored until it reports itself at the spawn (12 at most, as for a hero sent back into the
arena), so a late one does not start the run. A run the dev switch started at once
(`ESURV_AUTOSTART`) starts at once again. For now the whole run starts over, for every hero
in it (one hero per run until R6).

**Reset** (`reset`, the Escape menu's Reset Run) is the same fresh run
asked in the middle of a run, while it runs or is paused (or once it is over, as `again`). The
run ends there for the asker as if it left (it is out as if it fell, its time ending then) and
is paid out as a run left; the chat says what it earned and no end is told (`ESURVE`), as the fresh
run follows at once. It is a verb of its own rather than a wider `again`, which keeps the end
panel's rule (only once the run is over). Refused before the first step (nothing to reset) and
from a hero who left. A fresh run starts over for every hero in it, so a reset never ends
another hero's run: while any other hero is still in a run that is not over (playing, or fallen
and watching), it is refused, and the asker may still end its own run (`leave`); once the others
have left, or the run is over, it goes through, and those who left stay out of the fresh run.

The run **pays out** at each end (a victory, then a fall in Endless), when a hero leaves, asks
for a fresh run or resets it, and when a hero's client logs out or its connection closes. Each
payment brings what the run earned since the last one, part by part (time, kills, bosses, the
victory, the zone and run-option bonuses): leaving or resetting in the middle pays what was
earned so far, and Endless pays its extra time, kills and bosses. Each payment raises the hero's
best level and time; the run counts once as a run played (and won). On a win, once per run, the next zone opens for that hero's
class. The hero's chat says what its class earned and how many talent points that bought (when
something was paid), and the zone it opened. A run that never began pays nothing. A Challenge
Mode run (zones.md "Challenge Mode") pays only its talent experience (Ahn'Qiraj's, ×
`challenge_factor`) and its gear: no records, no run counted, no zone won or opened, and the
codex keeps nothing it took. It is a challenge run when the save's run options have the
Challenge Mode tab picked and the hero's class has won Ahn'Qiraj; then any zone may be entered.

Gear a boss drops joins the collection at once, and the hero's chat names the piece and whether
it is new.

### Pause and choices

While a choice screen is open, the run is paused or won, the clock stands, every enemy and pet
stops where it is and the hero is rooted; they move again when the run runs. A pause is only
possible while the run is running (not before the first step, not on a choice screen).

### Zones a class has not opened

A class enters only the zones it has opened: the first zone always, the next one after a win
in the last one open. Entering a locked zone is refused at the character screen with the stock
"Login for that race, class, or character is currently disabled." (the hero picker of design
step 6 says why). The refusal byte the client reads is a reason from 1 to 6 (1 world down,
4 disabled, 5 character not found), not a status code.

A new run brings the class's talent ranks and the gear it wears into the hero's numbers.

## What the client is told

| In the run | Packets |
|---|---|
| The hero enters | `SMSG_LOGIN_VERIFY_WORLD` (zone map and spawn), `SMSG_LOGIN_SETTIMESPEED` (zone hour), tutorials marked seen, the spellbook (see "The spellbook") and the action bar, the hero's player object dressed in its gear and, for its own client, the items of its worn class gear (see "What the hero wears in a run"), `SMSG_INIT_WORLD_STATES`, `SMSG_WEATHER` |
| An enemy or pet appears | an object create: entry, look, size, health, level, faction, run speed; a creature enemy or boss that holds weapons (vmangos's, [enemy-weapons.md](enemy-weapons.md); a rare its elite's) also its virtual items (`UNIT_VIRTUAL_ITEM_SLOT_DISPLAY` looks, `UNIT_VIRTUAL_ITEM_INFO` class, subclass, material, inventory type and sheath) with them drawn (sheath state 1); a pet (100 health, not attackable) names the hero who summoned it and faces the way the hero faces |
| It moves | `SMSG_MONSTER_MOVE` along what is left of its straight move, a path point every 4 yards laid on the ground. A pet that flies (`hover` in `pets.ron`) is always on a flight (`SPLINE_FLAG_FLYING`, every point whole, so the client keeps its height): `hover` yards above the ground with a bob of 0.25 yd up and down every 2 s, a point every 0.25 s; standing, one bob in place (no turn), told again 0.3 s before it runs out; while the run stands, one still flight at its height. It is created at its height, and the client keeps it upright on its flights (benilla's `LevelFlight`, [screens.md](screens.md)) |
| Its health changes | a values update |
| An enemy dies | it stops where it is, its health goes to 0 (the death) and its debuffs go; 1.5 s later `SMSG_DESTROY_OBJECT` |
| A pet leaves | `SMSG_DESTROY_OBJECT` |
| A totem is placed | after the cast's look, a unit (our totem creature, `entries::TOTEM`, "Totem"): 5 health, not attackable, made by the hero, the totem look (Magma Totem has its own); a race with a totem look of its own (the dwarf's Ironfoe) gets the composed look of its stock look, display `9 000 000 + race × 100 000 + the stock look`, which the client builds ([screens.md](screens.md#benillas-hook-composed-displays)) |
| A totem's time is up | `SMSG_DESTROY_OBJECT` (no death) |
| A pickup drops | gold, a cache, a rune or a magnet: a game object (a magnet stands in a two-part beam of light); an experience orb: a small wisp that cannot be targeted, bigger and red as it is worth more |
| An orb is pulled in | one `SMSG_MONSTER_MOVE` from where it lies to the hero |
| A pickup is taken | `SMSG_DESTROY_OBJECT`; a rune or cache plays its look or sound, a cache that needs gold says so across the screen |
| A battle standard is laid | at its spot, its faction's battle standard as a unit that cannot be targeted (`entries::BATTLE_STANDARD`, named "Warband Standard"; its look `balance.ron` `battle_standard.display`, `CreatureDisplayInfo.dbc` 14541 the Alliance Battle Standard, 14542 the Horde Battle Standard) and a soft glow under it in the faction's colour, a game object (`AuraBlueShort` 263 for the Alliance, `AuraRedShort` 1308 for the Horde, `pickups::STANDARD_GLOW`) under an entry of its own, `entries::BATTLE_STANDARDS_FROM` (400 000) + its display, that the game-object query answers; then `SMSG_SPELL_GO` of its faction's Battle Standard spell on it (`battle_standard.planted_look`, 23034, 23035: its look as it is planted) and `MSG_MINIMAP_PING` at its spot (the stock pulsing ring on the minimap, 5 s). Over it a beam of light in the faction's colour, two game objects under entries of the same kind (`pickups::STANDARD_BEAM`: `AuraBlueVeryTall` 2232 at size 1.6 and `AuraBlueTall` 298 at 1.0 for the Alliance, `AuraRedVeryTall` 1311 and `AuraRedTall` 4831 for the Horde; the magnet's beam without its sparkle). The run announces it (`lines::STANDARD_RAISED`). Taken: the standard, its glow and its beam are destroyed, and the run announces the ally ("<name> answers your call!", `lines::ally_answers`) |
| A standard's ally comes (`AllySummoned`) | its name (`SMSG_NAME_QUERY_RESPONSE`) first, then a player object (guid `0x1000_0000` + its id) shown as its hand-made character exactly as a Warsong boss shows one (`player_enemies`' `PlayerShape`: race, gender, looks, a druid form's model, its class's PvP gear under its picked gear), at `OBJECT_SCALE_X` = the event's scale (`ally_scale`, 2), its race's faction template (a friendly player), not attackable or selectable (`UNIT_FLAGS` `0x0200_0002`), 100 of 100 health, its hero's level, its weapons drawn as its class holds them. Its arrival: `Blink` plays Blink's look (1953); `Stealth` comes wearing Stealth's aura (1784, which the client draws see-through) and drops it 0.4 s later, so it fades in; `Portal` and every arrival `look_spell` play that spell's look on it. Its `yell` comes as a monster yell (`SMSG_MESSAGECHAT`) under its name |
| It moves (`AllyMoved`) | `SMSG_MONSTER_MOVE` along its segment (a run, a fast charge, or there at once for a blink or a melee step); a charge or blink plays its look on it (Charge 100, Feral Charge 16979, Blink 1953). A strafe or a jump of its rotation comes as another player's relayed move, so the client plays it as a player's (`packets::relayed_move`, its `time` the run's clock in ms): a strafe `MSG_MOVE_START_STRAFE_LEFT` or `_RIGHT` (`MOVEFLAG_STRAFE_LEFT` 0x4, `_RIGHT` 0x8: the client moves it sideways at its run speed, facing its enemy) and `MSG_MOVE_STOP_STRAFE` where it ends; a jump `MSG_MOVE_JUMP` (`MOVEFLAG_JUMPING` 0x2000 and the jump's tail: take-off speed -7.955547, the way it flies, its speed over the ground: the client flies the arc) and `MSG_MOVE_FALL_LAND` where it lands, the time in the air as its fall time. A run still going is stopped (`SMSG_MONSTER_MOVE` with no path) before a strafe or a jump, and a strafe or jump cut short ends where it is before the next move |
| It casts (`AllyCastStarted`) | a rotation spell with a cast time: `SMSG_SPELL_START` from it for the spell's cast time, at its aim (an enemy, the hero, the ground, itself): the cast bar and the cast animation; its `SMSG_SPELL_GO` follows when the cast is done (an instant's comes alone), its hits after it so they log under it |
| It acts | its steps' looks as `SMSG_SPELL_GO` from it (`caster: Unit::Ally`): missiles from it, impacts on what it struck, heals and buffs on the hero, taunts on each taunted enemy; areas, totems and pets it leaves show as the hero's (an ally's pets' hits are logged as any pet's; an ally's totem takes the ally's race's totem look). A melee `Hit` (`AllySwing`) shows its damage as its own `SMSG_ATTACKERSTATEUPDATE` swing; its other hits as `SMSG_SPELLNONMELEEDAMAGELOG` from it under the look it last cast at enemies (from the hero once it has gone: an area or bleed it left). What it puts on units (`AuraShown`: Polymorph 118, Blind 2094, a bleed's look on enemies; a buff's look on the hero) shows as their auras until the time given. An enemy it taunted striking it (`AllyStruck`): a swing absorbed whole (`HITINFO_ABSORB`, "Absorb") |
| It leaves (`AllyLeft`) | in the tick the run lets it go (it stays 15 s there, `battle_standard.assist_seconds`, or until its last planned rotation cast lands). Its `death_yell` first, then: `Fade` it wears Stealth's aura (it fades over 1 s) and goes; `Vanish` Vanish's look (1856), gone 0.2 s later; `Hearth` `SMSG_SPELL_START` of the Hearthstone (8690) for 1.5 s, its `SMSG_SPELL_GO`, then gone; `RunAway` it runs to the spot given at a hero's run speed (7 yd/s) and goes there; `Dies` health 0 (the death) and its body lies 1.5 s. Gone: `SMSG_DESTROY_OBJECT` |
| The hero's health, level or experience changes | a values update |
| The hero's run speed changes (a speed rune taken, its 10 s over, a fresh run) | its own client: `SMSG_FORCE_RUN_SPEED_CHANGE` with 7 yd/s × its move speed (the client runs that fast from then on and acks with `CMSG_FORCE_RUN_SPEED_CHANGE_ACK`, which the server takes as read); the other clients: `MSG_MOVE_SET_RUN_SPEED` with the hero where it stands. Until this was sent, a rune's +0.6 move speed reached the stat sheet but not the client, which kept the speed the hero entered with |
| The hero strays past the arena | `MSG_MOVE_TELEPORT_ACK` back inside |
| The hero takes, upgrades or empowers a ring, trinket or amulet | for its own client: the piece's new item and the slot field pointing at it, the old item destroyed (see "What the hero wears in a run") |
| The client asks | answers to the creature query (name, type, rank, look: elites rank 1, the gold dragon; rares, under an entry of their own (`entries::rare`, 300 000 + the elite's entry past 100 000), rank 4 "rare", the silver dragon; bosses rank 3; a pet: its name, "Companion" beneath it, a beast), the item query (see "What the hero wears in a run"), the name query, ping and time queries; logout goes back to the character screen |

The client's movement packets move the hero in the simulation (`Run::hero_moved`). The run is
ticked every 50 ms.

### Combat

| In the run | Packets |
|---|---|
| A hero spell goes off | `SMSG_SPELL_GO` with the spell's look (its `look_spell`), from the hero, listing what it struck: a projectile at each target, a nova or cone around the hero, an area at its spot, a bounce or melee strike at its first enemy, a buff on the hero. Auto Shot, Multi-Shot and Aimed Shot fly an arrow. |
| A proc shows its look (a milestone's extra targets, strike again, splash, line or pull; the extra strikes of an extra-strikes buff) | the same `SMSG_SPELL_GO`, marked triggered (cast flag `0x1`, benilla's `TRIGGERED_CAST_FLAG`): its missile and impact play on what it struck, but the hero plays no cast for it (no cast animation, hand glow or cast sound, no ranged weapon shot), as a triggered spell in WoW |
| It hits | `SMSG_SPELLNONMELEEDAMAGELOG`: the damage number (crit marked) and the combat log line, under the spell's `pulse_spell` when it has one, else its look (Fan of Knives' hits go under a Throw, 10277, that is no ranged basic shot: the client shows a basic shot's numbers white like a swing, a spell's yellow) |
| A damage-over-time tick | `SMSG_PERIODICAURALOG` |
| The hero's weapon swing, a melee pet's attack | `SMSG_ATTACKERSTATEUPDATE` |
| A ranged pet's shot | its spell's look from the pet; the hit logged as the hero's under that spell |
| An enemy's swing at the hero | `SMSG_ATTACKERSTATEUPDATE` with what the swing was worth after damage reduction and before any shield, at least 1; a creature with a weapon in each hand (vmangos's, [enemy-weapons.md](enemy-weapons.md)) has every other swing marked `HITINFO_LEFTSWING`, so the client plays its off-hand swing |
| Other damage to the hero | its health bar only, no number |
| A spell heals the hero | `SMSG_SPELLHEALLOG` (heals without a spell, like lifesteal, show only on the health bar) |
| A ranged enemy casts | `SMSG_SPELL_START`, then `SMSG_SPELL_GO` when the shot leaves |
| A totem pulses | `SMSG_SPELL_GO` from the totem with its `pulse_spell`: around a totem with a radius one look every pulse listing everything it reaches (even nothing); otherwise a shot at each enemy it picks, landing at the pulse spell's speed. Its hits are logged as the hero's under the pulse spell. |
| A lasting area is placed | the cast's look at its spot, then the spell's look lying on the ground (a dynamic object, scaled from the look's own size to the area's) until its time is up and 0.3 s more; its ticks show only their numbers |
| A boss's ground burst | the spot marked on the ground (the hero as its caster) until the hit lands; then the boss's impact look there |
| A boss's volley | a look from the boss at each shot's spot as it leaves; nothing where they land |
| A boss's stomp | its look on the boss, listing the hero only when in reach |
| A boss's charge | its look at the hero, then the landing look at the spot it reaches |
| Thorns, an aura striking back | a damage line as the hero's, under the aura's spell (Thorns when it has none), Nature |
| A milestone goes off | a burst, splash or death explosion shows its look from the hero at its spot (a shield burst on everything it reaches); a swept enemy or an effect jumping on death gets the spell going onto it. Its damage is logged as the hero's under the spell; extra blows after a weapon swing are more swings (a pet's when a pet's hit set them off). |
| The extra life, cheat death | the hero's comeback look and blast, with a centre-screen line |
| An effect over time lands on an enemy | its look on that enemy, and a debuff aura on it (icon, stacks, the spell's lasting effect) until its last tick |
| The hero's buffs (shields, runes, damage and haste buffs ...) | aura icons on the hero, one per look with its stack count, and, for its own client, `SMSG_UPDATE_AURA_DURATION` (slot, ms left) when one starts or renews, sent before the aura fields that name it (the order the client keys its countdown on); when the run goes on after a pause (a choice, the Escape menu) every buff's time left is told again, since the run's buffs stood still while the client kept counting. The buff frame shows the countdown (screens.md, "The stock frames in a run") |

`cargo test -p esurv-sim every_spell_shows_its_look_when_cast` checks that every class and race
spell shows its own look when cast, effects over time included.

**Area looks at their true size**. Each spell's look is drawn for its
own radius in the player's `Spell.dbc`; an area that reaches further or less shows it grown or
shrunk to match:

* A cast whose area differs from its look's own radius: just before its `SMSG_SPELL_GO`, an addon
  line with the prefix `ESURVS` (ours): `<caster guid, hex> <spell> <scale>`. The client
  (`esurv-app`) grows that cast's ground effects by it: the ring at the caster's feet, or the
  burst at the spot it was aimed at. The caster's own model and the effects on its body keep
  their size. A totem's pulse has nothing on the ground, so it plays as it is; the hit on each
  enemy it reaches shows how far it goes.
* A look lying on the ground: its size field, from the drawn radius listed for a few spells
  (`ground_looks.rs`), else from the spell's own radius.

### The buff frame

A buff's aura wears a look, a stock spell whose `Spell.dbc` text says
what that spell does, so the server tells the hero's own client what each of its buffs is
(`ESURVD`, below), in the aura slots' order (one slot per look, stacks counted, as the aura fields
show them): the look, its kind (`shield`, `damage`, `haste`, `immunity`, `aura`, `regen`,
`speed`, `berserk`), the spell of ours that cast it (the race's spell or a learned one wearing
that look; none for a rune or a milestone's), its value now as the run's rules apply it (a damage
or haste buff grown by the hero's buff power and a rune's as it is, as `refresh_stats` folds
them; what the shield still absorbs; what an aura strikes back with, grown by buff power and the
hero's damage, as `strike_back` hits; health a second), and how many stack. It goes after `hello`
and whenever one starts, ends or its value moves, at most every 0.2 s; the time left comes with
the auras' durations. The client writes each buff's tooltip from it ([screens.md](screens.md),
"The stock frames in a run").

### The starting options

A hero's run starts with the starting option its save keeps (`start`,
below; its class's first until one is picked). The clients are shown it as the run tells it
(`starting_looks`):

* the hero's power type is its option's rage or energy (else the class's), with its values in the
  player's power fields (rage in tenths, as the client counts it), and the bar's kind, name and
  count go to its own client (`ESURVY`);
* each stacking buff is an aura of its own after the hero's other buffs, with its count and time
  left, and a tooltip of the passive's (`ESURVD`, `passive`);
* a form shows its model (`UNIT_DISPLAYID`; the race's own stays native);
* a hero out of its form for a moment (`Event::FormLeft`: a caster spell from Cat or Bear Form,
  Rejuvenation or Regrowth, or Powershift) shows its race's own model at once, in the same update
  ahead of the cast's `SMSG_SPELL_GO`, so the cast plays on the caster's model; when the moment
  is up (`POWERSHIFT_SECONDS`, 0.5 s; longer if it casts another while out) it takes the form's model
  again with the form's own look; both shifts puff (the form's look aimed at the hero itself plays
  its Druid Morph impact on it). The look is `SMSG_SPELL_GO` of Cat Form 768 or Bear Form 5487 from the
  hero), `game::form_shifts`;
* a hero with a ranged swing (the Hunter) has its swing's hits logged as its shot's spell
  (`SMSG_SPELLNONMELEEDAMAGELOG` for Auto Shot, 75, as 1.12 logs it), never as a melee swing
  (`SMSG_ATTACKERSTATEUPDATE` makes the client swing the weapon in melee);
* a hero with a ranged swing (the Hunter) shoots as the ranged weapon it holds does. The client
  does it itself, the 1.12 way, for a ranged-slot spell (`Spell.dbc` `Attributes & 0x2`: Auto
  Shot, Multi-Shot, Arcane, Concussive, Aimed and Steady Shot, the stings, Volley): it fills the
  look's empty stages from the held weapon look's own shooting look (`ItemDisplayInfo` column 10:
  visual 5 on bows, the bow drawn and loosed; 743 on crossbows, the rifle's clips with the bow's
  sounds; 224 on guns, the rifle fired), the arrow's ArrowLoop flight included. So the server
  sends a second `SMSG_SPELL_GO` with no hits only where the client draws none: beside a known
  `Ranged` spell that is no ranged-slot spell (Explosive Shot looks like Explosive Trap's blast)
  the held weapon's basic shot (Shoot Bow 2480, Shoot Gun 7918, Shoot Crossbow 7919, no look of
  their own, the weapon's shown whole), and beside any shot from a weapon look with no shooting
  look of its own (the Ahn'Qiraj rifle look, display 33826) a stock shot that brings one (a bow's
  3018, a rifle's 8682). A second look on top of the client's rang a second weapon: a crossbow
  sounded like a gun. The weapon is read from the item in the ranged slot, so a weapon changed
  later shoots its own way; `game::ranged_shots`;
* a bow's or crossbow's arrow lands with ArrowThrowHit: a bow's look
  has it as its impact, a crossbow's (743) has no impact kit, so the client adds it to an arrow
  that flew with ArrowLoop and has no impact of its own (benilla-app `arrow_landing_sound`); a
  gun's shot lands silent. The bowstring is drawn light grey, its
  tips posed off their bones' pivots;
* the ghostly copy of the hero is a player object of its own, the hero's look with the Ghost Visual
  aura, that cannot be picked or attacked; its name is its hero's;
* a mark of Lightning Rod is a debuff on the enemy while it lasts;
* the options' extra looks and sounds go with the events that bring them, some a moment after
  (`SMSG_SPELL_GO`, `SMSG_PLAY_OBJECT_SOUND`; [screens.md](screens.md), "Looks").

### The spellbook

The spellbook holds exactly the race's own spells and what the hero
learned in this run. `SMSG_INITIAL_SPELLS` (at login and with a fresh run) sends, as `Spell.dbc`
ids and each once: the race spell (its ability's look), the race's passives, then the known
spells; `SMSG_LEARNED_SPELL` adds each spell a card teaches. A passive shows as the stock racial
spell of its name (`The Human Spirit`, `Hardiness` ...; `stock_spell` in `races.ron` where the
names differ: the Dwarf's Treasure Finding is the stock Find Treasure), found in the player's own
`Spell.dbc` among the rows whose rank says "Racial", a "Racial Passive" row first, then the
lowest id (the Night Elf's Nature Resistance reads as the Tauren's, the same text and icon).
Without the game data no passive is sent. The client puts the whole book in one tab, General
([screens.md](screens.md), "The stock frames in a run"). The Undead's Cannibalize and the
Dwarf's Find Treasure are active spells in `Spell.dbc` (passives in this game): clicking them
casts nothing, as every cast the client asks is ignored.

**Its tooltips**. A look's `Spell.dbc` text says what that stock spell
does, not what ours does, so the server tells the hero's own client the book as its tooltips
show it (`ESURVL`, below): every spell of the book in its order, each with the `Spell.dbc` id it
shows as, what it is (`learned`, `racial`, or a race's `passive` and its place in the race's
list), its rank (0 for the race's spell and passives) and its numbers for the hero now, worked out
by the casting rules as a card's are (`esurv-sim` `game::spell_numbers` on the spell's slot
and the hero's stats: its upgrades, talents, amulet and live buffs count). It goes after `hello`
and whenever it changes (a spell learned or upgraded, a talent or amulet, a level, a buff), at
most every 0.2 s. With each spell come its totals (`esurv-sim`
`game::spell_numbers::spell_totals`): what the upgrade cards taken for it add up to, by kind;
what the hero's talents, worn gear, jewelry, amulet and race add to it, by kind (the hero's
modifiers against the bare stat sheet of its class at its level, and the spell's own talent and
amulet modifiers; not the level's growth, nor a buff of the moment); and the hero's crit chance
and crit damage now, for a spell whose hits can crit. The client writes each tooltip from them
with the card's own templates and words ([screens.md](screens.md), "The stock frames in a
run").

### The action bar

The hero's spells sit on the first action bar: known spell 1 on button 1, and so on up to
button 7 (`present::SPELL_BUTTONS`; a spell past the seventh has no button and still casts), the
race spell on button 8, the run's bar's last ([screens.md](screens.md), "The action bar"). The bar is sent at login, when the run starts and when a card
teaches a spell (`SMSG_LEARNED_SPELL` first). After every cast `SMSG_SPELL_COOLDOWN` gives the
spell's real cooldown, so the button shows its sweep. Spells still cast by themselves.

The client starts a look's own `Spell.dbc` cooldown when the hero's `SMSG_SPELL_GO` for it
arrives, and a button shows the longest cooldown it knows of, so a look whose stock spell waits
longer than ours would show the stock wait (Divine Star looks like Holy Shock: 30 s against its
8; Frost Nova's 25 s, Holy Shock's category ...). So after every `SMSG_SPELL_GO` the hero casts (its
spells', a starting option's extra looks, a card's or milestone's casts), its own client gets
`SMSG_CLEAR_COOLDOWN` for the look, then `SMSG_SPELL_COOLDOWN` with the time left on the hero's
spell that wears it (none when it is ready: a zero there means the stock cooldown);
`game::present::bar_sweeps_after_casts`. `SMSG_SPELL_COOLDOWN` still brings the look's stock
global cooldown, as the client reads it; in a run our interface hides it (the
run has no global cooldown, so the run frames' Lua answers `GetActionCooldown` and
`GetSpellCooldown` with no cooldown for one of exactly 1.5 sec).

Each spell a hero can hold at once (its starting option's spells, its card pool, its race's
spell) wears a look of its own: the bar, the spellbook and their icons, tooltips and sweeps know
a button only by its look. Where two would share one, one wears another rank of the same stock
spell, which looks the same (Friend of the Forest: Rejuvenation 1058, against the card's 774;
Aimed Shot 20900 against Steady Shot's 19434; Explosive Trap 14314 against Explosive Shot's
13812). `cargo test -p esurv-content the_spells_one_hero_can_hold_wear_different_looks`
checks it.

### Lines

All of the run's lines are in `esurv-sim`'s `game::lines`, in our own words; the boss arrival lines
are each boss's `yell` in its zone file (`content/zones/<id>.ron`).

| Kind | Shown as | Lines |
|---|---|---|
| Shouts | the unit yells (chat bubble and chat) | a boss arriving, calling its adds, dying (rares say nothing) |
| Centre-screen | a raid-boss emote | the run's opening, a boss or elite arriving, a swarm, the extra life, cheat death, Endless (none for a rare) |
| HUD messages | a toast (`ESURVT`) for a client that has a HUD of its own; else a system line in chat | magnets, a cache that needs gold, leaving the arena |
| A level-up | a system line in the hero's chat; the hero's level field changes, and no `SMSG_LEVELUP_INFO` is sent | the damage, maximum health and health a second the level brought. The client's stock level-up block, which would say "0 hit points", is not printed in a run ([screens.md](screens.md), "The stock frames in a run") |
| Gear won | a system line in the hero's chat | the piece's name, new to the collection or not |
| The end of a run | system lines in the hero's chat | the talent experience its class earned and the points it bought, the zone it opened |

### The character pane

Whenever the hero's numbers change, the server sends an addon message with the prefix `ESURV`:
`key=value;` pairs, each key the stat's name: `health`, `damage`, `cooldown`, `crit`,
`crit_damage`, `reduction`, `regen`, `area`, `targets`, `double_cast`, `speed`, `pickup`,
`experience`, `luck`, `gold`, `heal_per_kill` and `heal_shield`, then the same with `gear_` in
front for what worn gear adds: the sums of the worn class pieces' own stats (`present::gear_adds`: flat health; damage,
cooldown recovery, crit chance and damage, damage reduction, health a second, area, extra
targets, double cast, move speed, pickup range and gold as the pieces' shares), from what the
class wears (worn gear changes only before a run). The character pane shows it in a run
([screens.md](screens.md), "The stock frames in a run").

## The game's own screens: esurv-protocol

The server and the game's screens talk over the world connection (`screens.md`, "How the app
and the server talk"), so the screens can live in any client: addon chat lines (`LANG_ADDON`), which the client hands to its interface and never
prints. `crates/esurv-protocol` holds the message set (`ToClient`, `ToServer`) and their
encoding; the server speaks it in `game/screens.rs`, the client in `esurv-app`'s
`protocol` module.

**A line** is `prefix` TAB `message`. Every prefix starts with `ESURV`; `ESURV` itself (the
character pane's numbers) and `ESURVS` (an area look's size) are the older lines above, kept as
they are. A message is tokens with one space between them: whole numbers, decimals (as Rust
prints them, which read back exactly), guids in hex, keywords, and text, where `%`, `~`, `|`,
the space and control characters are written `%XX`; an empty text, or a missing number, is `~`.
A list is its length, then its items. A hero is named by its player guid, so in a run with
others a client knows whose a message is.

**Long messages** stay within 200 bytes a line (a 1.12 chat line carries 255): a longer message
goes in pieces, one after another, every piece but the last under its prefix with `+` added
(`ESURVE+`), the last under the prefix itself; the pieces' messages joined in order are the whole
message.

**Hello.** A client says `hello` once it is in the world (the login's
`SMSG_LOGIN_VERIFY_WORLD`), with the protocol's version and the screens it has: `cards` (it
answers its own choice screens), `end` (it shows the end of its run and answers it), `hud` (it
shows HUD messages itself). From then on it is told the lines below. For each screen it lacks the
server stands in: the first offer is taken for it at once, a victory waits as it is, HUD
messages go to the chat. A client that never says `hello` sees the run as before.
`esurv-app` says `hello` with each screen it has built: `cards` and `end` so far.

### What the server tells a client

| Prefix | Message | When | Message tokens |
|---|---|---|---|
| `ESURVW` | Ready | after `hello` | hero, name, class, race, zone, zone subtitle |
| `ESURVH` | HUD | when it changes: a new phase at once, else at most every 0.2 s | hero, phase (`waiting` `running` `choosing` `paused` `defeat` `victory`), Endless 0/1, clock (whole s), kills, gold, cache price, next boss in whole s or `~` |
| `ESURVB` | Boss bar | a boss appears, its health changes (at most every 0.2 s), and once at 0 when it is gone; to every client | boss guid, health, max health, name, its health phases (each its share and label) |
| `ESURVBK` | Boss cast | a boss starts a cast or a wind-up (a mark, a rush, a ground burst, a ranged cast), or stops one (0 s); to the clients with a HUD | boss guid, label (its warning's bar label, else the spell's name), icon, seconds |
| `ESURVA` | Banner | a boss arrives or falls; to every client (a rare has none) | `boss` final 0/1 name or `bossdown` name |
| `ESURVT` | Toast | a HUD message, to a client with `hud` | text |
| `ESURVC` | Choices | a choice screen opens, or is rerolled; to its hero's client | hero, `levelup`, `cache` (bought) or `rare` (a rare's free cache), level, reroll cost, free rerolls, offers (each: kind, id, name, rarity 0–4, values as `what` and number; a new spell's head start as the gain it gives, `power` or `cooldown`; a spell upgrade's `rank` is the rank it reaches, one up for each kind it steps, and each kind's gain on what the spell has now; jewelry's `value`, and for an upgrade its `step`); then the kit: the spells known (id, rank), the rings and trinkets worn (id, name, rank, value), 1 and the amulet worn (id, name, rank, empower factor) or 0; then, for each offer, its numbers (`what` and number): a new spell's numbers at the rank it starts, for this hero (`damage`, `targets`, `range` ...: `esurv-sim` `game::spell_numbers`), which its card's description fills in; none for other offers. A line without the kit reads with an empty one, a line without the numbers with none (the card then works them out for a plain hero of the spell's class) |
| `ESURVX` | Choices closed | an offer was taken | hero |
| `ESURVG` | Gear dropped | a boss dropped a class piece for the hero | hero, item entry, slot, tier, new 0/1, name |
| `ESURVE` | Run end | the run ended for the hero (fell, won, left) | hero, `fell` `victory` or `left`, won 0/1, seconds, level, kills, elites, rares, bosses, damage dealt, damage taken, gold earned, caches; the experience this end paid (time, kills, bosses, victory, zone, run options); the class's talent experience before and after, its points before and after, experience into the next point and its cost; 1 and the zone it opened, or 0; the gear won (item entries) |
| `ESURVM` | Meter | when it changed, at most every 0.2 s while the run goes on | hero, then what it did and what it took, each a list of (source, amount): `spell` id, `swing`, `pet` name, `proc` `Spell.dbc` id, `other`; `enemy` name, `area` (no enemy) |
| `ESURVP` | Progress | after `hello` and after an armory change | class, talent experience, points earned, spent, experience into the next point, its cost, zones open, talent ranks (id, rank), worn (entries), looks (slot, tier), the class's collection (entries), then its prestige: points earned, spent, experience into the next, its cost (0 before all 51 talent points), prestige ranks (id, rank), then the zones the save has discovered (ids), its weapon loadout, news of gear and of points (0/1 each), and whether the class has Challenge Mode open (0/1: Ahn'Qiraj won; zones.md "Challenge Mode"), then whether the Challenge Mode box is due (0/1, once per save) |
| `ESURVO` | Run options | after `hello` and after a change | density, health, damage, speed (%), the reward bonus they give, then the mode: the Challenge Mode tab picked (0/1) and the challenge slider (%) |
| `ESURVN` | Refused | a request could not be done | its verb |
| `ESURVK` | Codex | after `hello`, and to every client that said it whenever a ring, trinket or amulet taken in the run is new to the codex | the ids found, as a list (a remade legendary amulet is `legendary:<spell id>`), in the save's order |
| `ESURVF` | Hero | before a run, answering an ask below | hero, runs played, runs won, best level, best seconds, the zone its next run is in (id), the starting option its next run starts with (id) |
| `ESURVV` | Save files | before a run, answering `savefiles`, `export` and `import` | the folder saves go to and come from (its whole path), then its saves newest first, each its file name and how many heroes it holds or `~` when it is no save the game can take |
| `ESURVU` | Save exported | answering `export` | the new file's name |
| `ESURVJ` | Save imported | answering `import` | the file the save now comes from, the file the replaced save was kept as |
| `ESURVL` | Spell tips | to the hero's own client after `hello`, and when one changed, at most every 0.2 s ("The spellbook", above) | hero, then the book's spells in its order, each: the `Spell.dbc` id it shows as, `learned`, `racial` or `passive` and the passive's place in its race's list, the spell's id (a passive's race's id), its rank (0 for the race's spell and passives), its numbers (`what` and number, as a card's: `damage`, `cooldown`, `range` ...); then, after the whole list, each spell's totals in the same order: the upgrade totals and the bonus totals (`what` an upgrade kind, `power` `cooldown` `targets` `area` `duration`, and its value as a card's upgrade value: a share, a cooldown's as the share it shrank by, targets as a count), then 1, crit chance and crit damage, or 0. A line without the totals reads with none |
| `ESURVD` | Buffs | to the hero's own client after `hello`, and when one starts, ends or its value moves, at most every 0.2 s ("The buff frame", above) | hero, then its buffs in the aura slots' order, each: the `Spell.dbc` id its aura wears, its kind (`shield` `damage` `haste` `immunity` `aura` `regen` `speed` `berserk` `charges`, or `passive` for a starting option's stacking buff), the spell of ours that cast it (`~` for none; a stacking buff's passive id, or its spell's for Holy Shield), its value now (a stacking buff's: how many it can stack), how many stack |
| `ESURVY` | Bar | to the hero's own client when its starting option's resource or counter changes ([screens.md](screens.md), "The power bar") | hero, the kind (the content's word: `Rage`, `Energy`, `ComboPoints`, `HolyPower` …), the name the bar shows (`~`: the client's own word for rage and energy), its value, its most |
| `ESURVZ` | Look styles | after `hello` | each styled spell: its `Spell.dbc` id, its size, tinted 0/1 and the tint's red, green and blue (0 to 1), flat 0/1, no camera shake 0/1, recoloured 0/1 ([screens.md](screens.md), "Looks") |

Offer kinds and their values: `spell` (a new spell: `power` or `cooldown`, the card's bonus by
rarity), `upgrade` (`rank` it reaches, then a step per upgrade kind: `power`, `cooldown`,
`targets`, `area`, `duration`), `jewelry` (`value`, and how many of its slot are `worn`),
`jewelryup` (`rank`, `step`, the worn `value`), `amulet` (its id and name), `empower` (the worn
amulet: `rank`, `factor`), `gold` (`gold`).

### What a client asks (`ESURVR`, verb first)

| Request | Done through | Refused when |
|---|---|---|
| `hello <version> [cards] [end] [hud]` | as above | never |
| `choose <n>` | `Run::choose` (offer `n` from 0) | no choice of its own is open, no such offer |
| `reroll` | `Run::reroll` | no choice of its own is open, no free reroll and not enough gold |
| `pause`, `resume` | `Run::pause`, `Run::resume` | not running (`pause`), not paused (`resume`) |
| `endless` | `Run::continue_endless` | the run is not won and waiting |
| `again` | a pay-out, then a fresh run (see "Again and Reset" above) | the run is not over; the hero left it |
| `reset` | `Run::leave` for the asker, a pay-out, then a fresh run (see "Again and Reset" above) | the run has not begun; the hero left it; another hero is still in a run that is not over |
| `leave` | `Run::leave`, a pay-out, then the client leaves the world | it left already |
| `options <density> <health> <damage> <speed> <challenge 0/1> <challenge %>` | `Saves::set_run_options` (kept on the sliders, the challenge slider on its 25 % steps; for the next run; a line without the mode reads as Normal Mode at 100) | never |
| `learn <talent>`, `unlearn <talent>` | `Saves::learn_talent`, `unlearn_talent` | the armory's rules say no |
| `prestige <stat>`, `unprestige <stat>`, `resetprestige` | `Saves::learn_prestige`, `unlearn_prestige`, `reset_prestige` (classes.md, "Prestige points") | no free prestige point, another class's stat or at its cap; no rank to take back; nothing spent |
| `wear <entry>`, `takeoff <slot>` | `Saves::wear`, `take_off` | not in the collection or not the class's; nothing worn there |
| `look <slot> <tier or ~>` | `Saves::set_look` (`~`: the worn piece's own) | not in the collection |

Armory changes count from the next run. No screen of a run asks them (the
armory and the codex are not reachable in a run); the server keeps answering them,
and `Progress` and `Codex`, for the hero sheet the menu rework builds before a run
([screens.md](screens.md), "The class's progress and the codex"). The damage meter is counted by
the server from the run's hits for now (what each hit took off the enemy, and what each hit took
off the hero's health; `screens.md`, "The damage meter").

A rare cache is a choice of its own (`ScreenKind::RareCache` in the run, owed apart from
the caches bought, after the level-up its kill brings), with a cache's offers. Not carried yet:
the build window's spells and jewelry with their texts on the HUD line (a choice screen carries
them in its kit).

### Before a run (`ESURVQ`, verb first)

The character screen ([menus.md](menus.md)) shows what a class has
earned, the run options, the codex and each hero's records, and changes them, before any run.
Creating, deleting and entering the world go over the world connection as the stock screens'
do (`CMSG_CHAR_CREATE`, `CMSG_CHAR_DELETE`, `CMSG_PLAYER_LOGIN`); the rest are asks of their own
(`esurv_protocol::PreRunAsk`), answered with the messages above by
`esurv_server::prerun::answer`, by the same rules as in a run (the run's armory requests go
through the same functions). A local game's launcher hands the app a carrier that calls it
straight on the embedded server's save (`LocalSave`, the app's `prerun::PreRunLink`); the asks
are lines too (`ESURVQ`), for a carrier to a remote host, which has none yet: its screens then
show only what the content says, and a locked zone is still refused at login.

| Ask | Answer | Refused when |
|---|---|---|
| `facts <class>` | `Progress`, `RunOptions`, `Codex` | no such class |
| `hero <guid>` | `Hero` | no such hero |
| `class <class> <armory request>` (`learn`, `unlearn`, `prestige`, `unprestige`, `resetprestige`, `wear`, `takeoff`, `look`, as above) | `Progress` | the armory's rules say no (the refusal names the request's own verb) |
| `options <density> <health> <damage> <speed> <challenge 0/1> <challenge %>` | `RunOptions` (kept on the sliders; the mode with them, zones.md "Challenge Mode") | never |
| `rename <guid> <name>` | `Hero` | the stock name rules, or another hero has the name |
| `reshape <guid> <race> <gender> <look ×5>` | `Hero` | a race the class may not be (the create screen's pairs), a gender past 1, or a change of faction into a full camp |
| `zone <guid> <zone id>` | `Hero` | no such zone, or one the hero's class has not opened (in Challenge Mode every zone is open) (any zone in Challenge Mode) |
| `start <guid> <option id>` | `Hero` | no such hero, or an option that is not one of its class's |
| `seen <class> <collection|talents|challenge>` | `Progress` | no such class (the view was opened, so what it holds is seen; `challenge`: the Challenge Mode box was closed, never shown again for the save) |
| `savefiles` | `SaveFiles` | asked of a remote host (below) |
| `export` | `SaveExported`, `SaveFiles` | asked of a remote host; a save kept in memory only; the file could not be written |
| `import <file>` | `SaveImported`, `SaveFiles` | asked of a remote host; a hero is in a run (the save is never replaced under one); no such file in the folder (or a name with a path in it); the file is no save of ours or breaks the save's rules ("The save file", above); the file system said no. Nothing is replaced then |

The save's export and import (design step 6, the Options panel's Saves
tab) read and write files on the save's own machine, so only the game the save is kept on may ask
them: the local game's carrier calls `prerun::answer_local`, which answers them
(`save::files`), and `prerun::answer`, the answer for any other client, refuses them. They go
through one fixed folder, `exports` beside the save, as the game has no file dialog.

## Updates

The game checks for a newer version of itself and offers it in place (`esurv-app` `update`):

1. **The check.** A player build whose folder holds the setup's `VERSION.txt` asks once per
   start, as soon as the interface's settings are in and **Check for updates** reads on (the
   CVar `esurvCheckForUpdates`, `1` by default, on the options' Interface tab;
   [screens.md](screens.md)). A thread makes one anonymous GET of
   `https://api.github.com/repos/Detroph/eternal-survivors/releases/latest` (GitHub's latest
   release, drafts and pre-releases left out) and compares its tag (`esurv-v0.5.0`) with
   `VERSION.txt`, part by part as numbers. The game never waits on it. Offline, rate-limited, a
   release without both of its files (`eternal-survivors-<version>.zip` and
   `eternal-survivors-<version>.zip.sha256`), or any other failure: nothing is shown and the game
   starts as always (the log keeps why). The request carries a User-Agent of the game's name and
   version (`Eternal Survivors/v0.4.0`) and nothing about the player. The client is ureq over
   rustls (no OpenSSL), the server's certificate checked against the system's own store, HTTPS
   only, redirects included. Dev builds never check, unless `ESURV_UPDATE_FEED` names a test feed.
2. **The offer.** A newer release shows on the warband screen, over Options, glowing and pulsing: "v0.5.0 is available"
   and **Update** ([menus.md](menus.md), [screens.md](screens.md)).
3. **The download.** Update downloads the zip (its progress shown) and its `.sha256`, checks the
   zip's SHA-256 against it, reads `Eternal Survivors Setup.exe` out of the zip, checks the setup
   carries the release's version (its payload's `VERSION.txt`), and writes it to
   `update-download\` in the game folder. A failure at any step says so in the panel ("The update
   failed: … Nothing was changed.") and leaves the game as it was; Update tries again. A game the
   setup did not install (no `VERSION.txt`, or not in a folder named `Eternal Survivors`) is told
   to update by hand.
4. **The hand-off.** The game starts the setup in a console window of its own,
   `--wow <the WoW folder> --no-pause --relaunch`, and quits. The setup waits up to 60 seconds for
   the game to close, updates it in place as any update does (saves, logs, settings and the
   player's own files kept), and starts `Eternal Survivors.exe` again
   ([design.md](design.md), step 9, item 10). The next start removes `update-download\` (trying
   again for half a minute while the setup that started it closes).

Players on v0.4.0 or older update by hand once (their game has no updater); every later version
offers its updates in the game.

## Not decided yet

* The level enemies show: for now the highest hero level.
* The faction enemies carry: for now 14 ("Monster"), hostile to every hero.
* How damage over time that a milestone adds looks when it ticks.

## Switches

| Variable | Effect |
|---|---|
| `ESURV_CONTENT` | Load the content from this folder. A dev build watches it and loads it again as its files change: new runs and the menus take the new content, a run going on keeps its own ([README.md](README.md), "Editing content while the game runs"). |
| `ESURV_SAVE` | Keep the heroes in this file. |
| `ESURV_AUTOSTART` | Start the run as soon as the hero enters (and a fresh run from the end panel), without waiting for the first step. |
| `ESURV_UNHURT` | `1`: the heroes are healed to full after every tick and every choice's first offer is taken at once (a dev aid: unattended shots of a run, the hero standing still while its passives fill). |
| `ESURV_ORBS_STAY` | `1`: the orbs a passive drops (Holy Orbs) are never taken and lie until their time is up (a dev aid: shots of an orb on the ground, `ESURV_ORB_SHOTS` in [screens.md](screens.md), "Looks"). |
| `ESURV_METER_DEMO` | `1` fills the damage meter with made-up totals ([screens.md](screens.md)). |
| `ESURV_END_PRESS` | `again`, `endless` or `leave`: an unattended client presses that button on each end panel ([screens.md](screens.md)). |
| `ESURV_ESCAPE_AT` | `<s>[,<s>...]`: presses Escape inside the game at that run clock, then that many seconds later, to pause and resume a run unattended ([screens.md](screens.md)). |
| `ESURV_MENU_PRESS` | `<button>@<s>[,...]` (`continue`, `reset`, `end`, `quit`): at that run clock presses Escape inside the game, then clicks that Escape menu button; each press waits for what the last led to. `ESURV_MENU_SHOTS=<png>` adds the game's own shots of the menu and of what each click led to ([screens.md](screens.md)). |
| `ESURV_KEY_PRESS` | `<key>@<s>[,...]` (a letter, a digit or `escape`): presses that key inside the game that many real seconds after the hero entered the world, the whole stock way; `<key>@<s>:<hold>` keeps it down that many seconds (`W@4:7` runs forward for 7 s), one key down at a time. `ESURV_KEY_SHOTS=<png>` adds the game's own shot 1.5 s after each press ([screens.md](screens.md), "The stock frames in a run"). |
| `ESURV_TOTEM_SHOTS` | `<png>`: once the hero has been in the world `ESURV_TOTEM_SHOTS_AFTER` seconds (default 12, past the loading screen) and a totem stands, holds the world camera close on the totem and writes `ESURV_TOTEM_SHOTS_COUNT` shots (default 2) `<stem>-000.png` ..., `ESURV_TOTEM_SHOTS_EVERY` seconds apart (default 0.4): a composed totem's look and how it moves, with nobody at the mouse (`esurv-app` `races`). `ESURV_TOTEM_SHOTS_OF=object` frames the nearest boss's object shown as a unit instead (a planted item such as Morbent's Bane, a troll ward), and `ESURV_TOTEM_SHOTS_DISTANCE` sets how far off the camera stands (yards, default 5; its height and aim go with it). |
| `ESURV_HERO_SHOTS` | `<png>`: once the hero has been in the world `ESURV_HERO_SHOTS_AFTER` seconds (default 12), holds the world camera close on the player's own hero and writes `ESURV_HERO_SHOTS_COUNT` shots (default 3) `<stem>-000.png` ..., each from a third of a turn further round, `ESURV_HERO_SHOTS_EVERY` seconds apart (default 0.6): what the hero wears, with nobody at the mouse (`esurv-app` `hero_shots`). |
| `ESURV_GRANT` | `<name>[,<name>...]`: every hero wears these from the start of its run, as taking their cards does: a ring or trinket of `jewelry.ron` at its base value (`<id>@<rarity>`, 0 common to 4 legendary; common without), an amulet of `amulets.ron`, or `legendary:<spell>` for that spell's remade legendary amulet; or a rune, a battle standard (the party's faction's) or a rare's cache laid on the ground, `speed_rune`, `berserker_rune`, `restoration_rune`, `battle_standard` or `rare_cache`, at the hero's feet or `:<yards>` ahead of it, where it faces (`speed_rune:20`); or the zone's first roster elite brought in, `elites:<n>` of them in a ring 12 yards round the hero, or `rare:<yards>`, a rare copy of it that far ahead (10 without); a name that is none of these is skipped (`Game::grant`). |
| `ESURV_GLOW_CHECK` | `<s>`: the client logs its frame time every 2 s from the hero's entering the world (mean and worst, with how many elites, rares and bosses glow), and turns the enemy glow option off that many seconds in: the glows' cost, on and off in one run ([screens.md](screens.md), "Glows round elites, rares and bosses"). It turns benilla's FPS journal on too (`fpsJournal`: the CPU and GPU time of each second, `benilla-config\Diagnostics\fps-journal.csv`). |
| `ESURV_STANDARDS` | `<s>[,<s>...]`: at each of these run clocks a battle standard is laid at the first hero's feet, so it is taken at once and its ally comes then (a live look at assists, [assists.md](assists.md)); each fresh run lays them again. The server's log names each ally as it comes and goes (`esurv: assist <name> comes at <s> s`). |
| `ESURV_ASSISTS` | `<id>[,<id>...]`: the characters (`player_characters.ron` ids) the run's battle standards summon, in this order, before the random picks (`Run::assists.forced`). |
| `ESURV_MOVE_LOG` | `<file>`: the server logs every hero's moves there, a line per move: real seconds since the hero entered, hero, x, y, and the run speed the server holds for it (yd/s), tab apart: how fast the hero really runs, measured. |
| `WOW_DATA` | The 1.12.1 `Data` folder, if the client does not find it by itself (beside the game, or `..\Data` when the game folder sits inside the WoW folder, as the setup makes it). Data named here is checked for its archives only, and a wrong one is logged, not refused. |
| `ESURV_ZONE` | `<zone id or name>`: every hero enters this zone, whatever was chosen for it (a dev switch; a zone its class has not opened is still refused). |
| `ESURV_UPDATE_FEED` | `<url>`: the updater asks this feed instead of GitHub's, in any build, plain HTTP allowed: a URL answering as GitHub's latest release does, its assets' download addresses where the zip and its `.sha256` are served (a test of the whole update; [Updates](#updates)). The installed version is `VERSION.txt`'s, else the crate version. |
| `WOW_CHAR` | benilla's: pick this hero without the screens. The launcher sets benilla's `WOW_REALM` to the one realm itself. |

## Built for multiplayer (roadmap R6)

The listeners take a bind address and the realm list a public address, so a host can let
other clients in. A hosted run keeps a list of players: each one's movement is passed on to
the others, every client sees every hero (each in its own gear: the visible-item fields go to
every client, the item objects only to the hero's own), and enemies chase the nearest living hero. For now
each hero who enters gets a run of their own; joining a running run comes with R6.

The Escape menu's ways out are each hero's own: End Run and Quit Game take only the asker out
(the others play on; a pause stays until one of them resumes), and Reset Run is refused while
another hero is still in a run that is not over, since a fresh run starts over for everyone in
it ("Again and Reset" above).
