# The game's screens

The game's own windows, which 1.12.1 does not have, live in `crates/esurv-app` and are
added to the client through `benilla_app::run_with`. They are drawn with `bevy_ui` over the game
view and the stock interface, in the stock fonts from the player's own `Data\fonts.MPQ`, and
show only while a hero is in the world, but for the menus before a run, which show at the
character select. Plain-language twin:
[guides/screens.txt](guides/screens.txt).

Each window's root node carries benilla's `OverlayWindow` marker: while the pointer is over
it, the stock interface sees no pointer, and the world's clicks, camera drags, mouse-over and
wheel zoom, and the mouse bindings, leave it alone, as for a stock frame. A window reads keys
only through benilla's two hooks, so movement and every other key binding keep working: a
screen's own keys (`OverlayKeys`, below: a modal screen's), and the game menu's place for the
HUD's Escape menu during a run (`GameMenuStandIn`, see [Pausing](#pausing-the-escape-menu)).

Every screen of a run but the damage meter, the card screens, the Escape menu and the end panel
fits the stock 1.12 interface: its frames, borders, buttons, fonts and colours,
built from the look kit below. The meter keeps the look of the Recount addon; the menus before a
run, the card screens, the Escape menu and the end panel have a look of their own ([The menus'
look](#the-menus-look)).


## The stock look kit

`esurv_app::stock` draws the stock interface's pieces with `bevy_ui`, from the stock art
in the player's own data (`interface.MPQ`), read at runtime. A screen spawns them through the
`Stock` system parameter; each piece is a bundle, and a screen's content goes in with
`.with_children` after it.

| Piece | Stock original | What it is |
|---|---|---|
| `backdrop(Backdrop::Dialog)` | `GameMenuFrame`, `StaticPopup` | `UI-DialogBox-Background` tiled every 32 units inside insets 11/12/12/11, under the `UI-DialogBox-Border` edge, 32 units wide |
| `backdrop(Backdrop::Tooltip)` | `GameTooltip` | `UI-Tooltip-Background` tiled every 16 units inside insets 5, tinted (0.09, 0.09, 0.19) (`TOOLTIP_DEFAULT_BACKGROUND_COLOR`), under the `UI-Tooltip-Border` edge, 16 wide, in white (`TOOLTIP_DEFAULT_COLOR`) |
| `tinted_backdrop(kind, colours)` | `SetBackdropColor`, `SetBackdropBorderColor` | A backdrop in other colours; change the root's `BackdropColours` and both parts follow |
| `dialog_header(title, width)` | `GameMenuFrame`'s header | `UI-DialogBox-Header`, 256 by 64 units in the stock dialogs, rising 12 units over its dialog's top, the title (GameFontNormal) 14 under its top |
| `panel_button(label, width)` | `UIPanelButtonTemplate2` | The red button, 22 units tall: `UI-Panel-Button-Up` (`-Down` while pressed, `-Disabled` when off), its 12-unit ends kept and its middle stretched; `UI-Panel-Button-Highlight` added under the pointer; the label GameFontNormal (gold), GameFontHighlight (white) under the pointer, GameFontDisable (grey) when off |
| `icon(name, size)` | `ActionButtonTemplate` | An icon (`Interface\Icons\<name>`), 36 units on the action bars, under `UI-Quickslot2` (66 units for a 36 icon, one unit low), with `ButtonHilight-Square` added while the closest button around it is under the pointer |
| `icon_at(path, size, tint)` | | The same for a texture named by its whole path (an item's icon from `ItemDisplayInfo.dbc`, a paper doll slot's picture), drawn times a tint (dark for what is not had yet) |
| `hover_glow()` | `ItemButtonTemplate`'s highlight | `ButtonHilight-Square` added over its parent while the closest button around it is under the pointer |
| `art(path)` | | A stock texture by its interface path, for an `ImageNode` |
| `tab(label, width, selected)` | `CharacterFrameTabButtonTemplate` | A window's tab, 32 units tall: `UI-Character-InActiveTab`, or `UI-Character-ActiveTab` raised 5 units for the chosen one, its 20-unit ends kept and its middle stretched; `UI-Character-Tab-Highlight` added under the pointer, 10 in from each end; the label GameFontNormalSmall (gold), white under the pointer and on the chosen tab, which takes no click (`PanelTemplates_SelectTab`). Its `StockTab` says which is chosen |
| `art_button(kind, size)` | `UIPanelCloseButton`, the spellbook's page arrows | A button drawn in its own art, 32 units: `ArtButton::Close` (`UI-Panel-MinimizeButton-Up`, `-Down`, `-Disabled`, `-Highlight` added), `PreviousPage` and `NextPage` (`UI-SpellbookIcon-PrevPage-*`, `-NextPage-*`, `UI-Common-MouseHilight` added) |
| `Tip(lines)` (a component) | `GameTooltip` | Put on a node: while the pointer is over it a tooltip box (`Backdrop::Tooltip`) shows its lines right of and below the pointer, kept on screen, over every window: the first in GameTooltipHeaderText, the others GameTooltipText, each in its own colour, wrapping past 260 units |
| `status_bar(colour, value, width, height)` | `CastingBarFrame` | `UI-StatusBar` in the bar's colour over a black background at half alpha; the root's `StatusBar` moves it |
| `text(font, value)`, `coloured_text` | `Fonts.xml` | A string in a stock font object (`StockFont`): GameFontNormal 12 gold, GameFontHighlight 12 white, GameFontDisable 12 grey, GameFontGreen, GameFontRed, the Small ones 10, the Large ones 16, NumberFontNormal 14 and NumberFontNormalSmall 12 (Arial Narrow), GameTooltipHeaderText 14 and GameTooltipText 12; every one with the black shadow one unit right and down |
| `centred(layer)` | | A layer over the whole window that centres the screen it holds; it takes no clicks, the screen it holds carries the `OverlayWindow` |

**Sizes** are in the stock interface's units, so a screen is as big as a stock frame: the stock
root is 768 units tall at every window size, scaled by benilla's `uiScale` of 0.9
(`StockScale`, one unit = window height / 768 × 0.9 pixels). The kit follows the window's
height; a screen built at one scale is rebuilt at the next.

**Buttons.** A `StockButton` (every panel button, and anything a screen marks with one) presses
and lights as the stock buttons do and sends a `StockClick` message when the mouse is let go
over it, as `OnClick` fires; a disabled one (`enabled: false`) does nothing.

**Drawing.** The edge files are eight squares in a row (left, right, top, bottom, then the
corners top-left, top-right, bottom-left, bottom-right), the top and bottom squares stored a
quarter turn from how they show (their outer side at the left and at the right). Once an edge
file loads, its squares are laid out into a 3×3 frame image (the middle clear) that the edge node
draws nine-sliced: the corners as they are, the sides tiled. The stock art loads in benilla's
sprite form (sRGB, one level, clamped) under a path of its own, so it draws in the stock colours
in benilla's gamma-composited interface. The stock interface's additive textures
(`alphaMode="ADD"`: the button highlight, the square glow) add to what is under them in gamma
bytes, as the 1.12 client blends, through a small material of the kit's (`AddImage`).

`ESURV_UI_DEMO=1` shows a sample dialog with every piece while the hero is in the world,
to check the look without a screen that needs it.

### benilla's hook: a modal screen's keys

`benilla_app::OverlayKeys` lists, by screen, the keys a crate's own modal screens take
(`set(owner, keys)`, `of(owner)`, `takes(key)`). While some screen lists a key its presses run
no key binding (as a key a shown keyboard frame ate) and come as `benilla_app::OverlayKeyPressed`
messages instead, which every screen may read; a focused edit box still takes every key, and a
held key's repeats are no presses. A modal screen lists its keys while it is open and clears them
when it closes.

## The menus before a run

Before a run the game shows menus of its own ([menus.md](menus.md))
(`esurv_app::menus`) in place of the stock select screen, the stock create screen and the
realm list: the class picker, the hero sheet and the warband screen over the camps. They have a
look of their own (below), not the stock kit's. `ESURV_MENUS=0` (a dev aid) shows the stock
screens instead (over the camps, with no camp switch).

**The flow.** When the character select comes up, a save with any hero opens on the warband
screen; a save with none opens the class picker. Create new character opens the class picker;
choosing a class opens the hero sheet on a new hero of it; confirming the sheet makes the hero
and lands on the warband screen with it picked. Escape does nothing on the warband screen; the
window's close button quits. A refusal (a name already taken, a full camp, a zone not open) is
said in our words over the screen.

**What they read and change.** The roster, the picked hero and create, delete and enter world go
through benilla's select hooks (below); the class's progress, the run options, the codex and each
hero's records and zone come from, and changes go to, the save through the app's way to the server
before a run (`esurv_app::prerun::PreRunLink`; [server.md](server.md), "Before a run"). They
are asked when a screen first needs them and kept until they change; every fact is asked again
after a run.

### The menus' look

`esurv_app::menus::look` draws every piece in code, in the stock interface's units so the
menus grow with the window; only the fonts, icons and portraits come from the player's own data.

| Piece | Look |
|---|---|
| Panel | near-black (0.035, 0.03, 0.027 at 95%), a one-unit warm-gold border at 40%, corners rounded 6 units, a soft shadow; it keeps the pointer from the camp's figures under it |
| Heading | Morpheus in gold, in small capitals (each word's first letter full size, the rest capitals at four fifths), centred between two thin gold rules that fade out away from it |
| Label and value | small all-caps grey labels (Friz Quadrata 8.5) over larger white values (12) |
| Value box | a label over a value in a rounded box, one step lighter than the panel, with the gold border |
| Icon | in a square frame; the picked one has a gold edge and a soft gold glow; a locked one is dark with a "?" over it, a dimmed one faint |
| Arrow | a gold chevron (a square's two borders, turned) |
| Button | framed, one step lighter than the panel, its border brightening under the pointer; the active one filled dark red with a red edge; the call to act (Choose, Start run) gold-framed and larger |
| Dots | a row of small rings, the current one filled gold |
| Tooltip | the stock tooltip's lines in a panel of this look (`MenuLookTip` beside a `Tip`) |

### The class picker

One panel over the camp, the world dimmed behind it: a **Class** heading over a 3 × 3 grid of the
nine classes (the stock create screen's class icons, the name under each; the picked class
glows and its name takes the class colour); under the grid the class's name in its colour, its
line from the content on how it plays, three value boxes (**Health**: its base health;
**Starts with**: the spell every run starts with; **Zones open**: `n of 9` from the save), and its
**Spell pool**: the first spell, then the rest of the pool, each with a tooltip (its name, its
school, how often it casts itself, and whether every run starts with it or a level-up offers
it). **Back** (when the save has a hero) and **Choose \<class\>**. A hint at the foot: every hero
of a class shares its talents, gear and open zones.

### The warband screen

The camps (below) fill the window; over them:

| Part | What it shows and does |
|---|---|
| Top-left panel | the picked hero's name, its race and class (class colour), **Total wins** and **Runs** (the save's records), the **Starting spell** spot, and under it the **Weapon loadout** spot. The Starting spell spot shows the class's starting options as icons, three (the Druid's four forms), the hero's pick lit and its name under them; an icon's tooltip gives the option's name, each of its level-1 spells with its numbers at level 1, how it plays, and its passive ("Passive: …", in green); a click picks it (`start`), the save keeps it for the hero's next runs (the class's first until one is picked), and the run starts with it. The Weapon loadout spot (roadmap R3) lists the class's three loadouts as rows, each with its name, its weapons ("One-handed sword, Shield"), its strength in green and its drawback in red, the one the class fights with lit (gold name, the picked row's frame); a click picks one (`loadout`), the save keeps it for the class (every hero of the class; its first until one is picked), and the next run fights with it. With no hero in the camp: "No hero waits by the Horde's campfire yet." |
| Top middle | centred over the camp, on the axis of its buttons: a row of four framed buttons, **Collection** and **Talents** (each opening the hero sheet straight on that view for the picked hero, Back returning here), a dimmed placeholder reading "TBD" (a feature to come) and **Guide** (opening the guide over the screen, below); under them **Equipped armour**: the class's slots (eight armour, then the weapons), each the worn piece's icon (tooltip: its name in its quality colour and its slot) or the slot's empty picture. **Collection** glows (gold border and the soft glow of what is picked, `ButtonStyle::TextGlow`, the glow pulsing: its strength dips to 40 % and back every 1.6 s, `look::news_pulse`) while gear was unlocked for the class since its Collection was last opened, **Talents** while talent or prestige points were earned since its Talents were (`Progress` `new_gear`, `new_points`); opening the view asks `seen <class> <collection|talents>`, so the glow is gone on coming back. The save keeps what was seen per class (`seen: Some((pieces: n, points: n))`, [server.md](server.md)); a class of a save from before it shows nothing new, a class new to the save has seen nothing |
| Right column | the **Zone** heading; once the hero's class has won Ahn'Qiraj (zones.md "Challenge Mode"), under it the tabs **Normal Mode** and **Challenge Mode** (`mode:normal`, `mode:challenge`; the picked one filled dark red), and under Challenge Mode the **Challenge** slider, its value as "1.25x" between two arrows a 0.25 step each way, 1x to 5x (`challenge-down`, `challenge-up`; each change to the save at once with the run options, `options`), with a tooltip on what the mode does; in Challenge Mode every zone is open; the zone of the hero's next run opened as a card (number and name, its title, its lines, its enemies, its bosses with their arrival times, the final one in gold); every other zone as a row: an open one picks it for the hero (`zone`), a locked one says which final boss in which zone opens it; a zone no class of the save has opened yet (not discovered: menus.md) shows only "???" and a lock. At its foot **Difficulty modifiers** (with the reward bonus when it is not 1) and **Start run** (enters the world as the hero, in its zone; off while the zone is locked) |
| Top right | the **Codex**, beside the zone column: how many rings, trinkets, amulets and legendary amulets the save has found (`n / m` each), how many in all, and **Open the codex** |
| The camp | the picked hero's name over the campfire (Morpheus, gold, kept over the flames as the camera sees them); the picked hero outlined in gold (below); a round button at the camp's top left with the other camp's emblem (`UI-PVP-Alliance`, `UI-PVP-Horde`) and a badge with how many heroes that camp holds, which switches camps (its first hero picked); under the fire, in this order, **New character** (the class picker; while the camp has fewer than nine heroes and the save fewer than 18), **Modify character** (the hero sheet on the hero) and **Delete character** |
| Bottom-left corner | **Options** (the game's settings, below) over **Quit game**, and under them the version played (`v0.5.4`, the build's `CARGO_PKG_VERSION`, small, in the label colour); Quit game closes the game through the app's own exit (`AppExit`, benilla's shutdown savers), as the Escape menu's Quit Game does once the hero is out; before a run there is nothing to pay out. Over them, while the updater has something to say ([server.md](server.md), "Updates"; `crate::update::notice_view`), a panel 200 units wide: "v0.5.0 is available" (gold) and **Update** (`update`), the panel wearing the news look of Collection and Talents (above: the gold border and a glow, pulsing; `look::NewsPulse`), the button plain; while it downloads, "Downloading v0.5.0", a progress line kept up to date in place ("42%  (17.0 of 40.4 MB)", the screen not drawn anew for it) and Update dimmed; a failure in red ("The update failed: the download failed, no connection. Nothing was changed.") with Update to try again; then "Updating to v0.5.0: the game restarts", no button. With **Check for updates** off, an offer and a failure are hidden, an update under way is not |
| Delete | asks first: "Are you sure you want to delete this character?" with the hero's name, **No** (the default, filled) and **Yes**; only Yes deletes, and the camp then picks the hero now in the deleted one's place (or the last) |
| Difficulty modifiers | a panel with each run option of `run_options.ron` (its name, its value in %, arrows a step each way, kept on its slider; its tooltip), the **Rewards** bonus the options give, a line on what harder runs pay, and **Done**; each step goes to the save at once (`options`) |

The camera's aim moves 9 yards to its right at the aim point (`WARBAND_AIM`; dev switch
`ESURV_WARBAND_AIM=<aside>,<lift>`), so the camp sits in the lower left, clear of the zone
column.

**The picked hero's outline**: a thin gold edge along the picked hero's
silhouette, its gear and weapons included (about 1.5 pixels at 1080 rows), with a soft glow
fading outward over about 7 pixels, brightest at the edge, pulsing slowly (its brightness dips by
a quarter, `sin` of 1.7 radians a second); nothing is drawn over the hero itself, and only the
picked hero has it, following its stand-up and greeting. benilla draws the hero alone into a
window-sized mask (`SceneFigureMask(0)`, below); a node over the window (`menus::outline`,
`outline_glow.wgsl`) reads round each pixel outside the silhouette and adds the gold, in gamma
bytes. The mask holds the hero alone, so the edge also shows over what stands in front of the
hero's edge (grass, the campfire); the camps' places keep the heroes clear of each other. It hides
while the codex, the guide, the options, the run options or the delete question are open.

**The codex** (`menus::codex`): over the screen, a panel with how the codex works and how much is
found in all; a tab per list with its count (**Rings**, **Trinkets**, **Amulets**, **Legendary**);
the page's name (a list's, or "\<Class\> Legendary Amulets", a class a page, the picked hero's
class first) with arrows through the pages; the pieces in two columns, a found one with its icon,
its name (an amulet in its rarity colour) and what it does, one not found dark with a "?", its
name grey and where it comes from; **Close** (and Escape). Built on `hero_sheet::codex`; what is
found comes from the save's `Codex` answer before a run.

**The guide** (`menus::guide`): the **Guide** button over the armour opens, in the codex's look, a
panel over the screen that explains the game's mechanics, a tab each: **Stats** (every hero stat
the game has, in four groups: basic stats, defences, spell stats, weapon stats; then the prestige
stats that raise a number of every spell), **Enemies** (normal enemies, elites, rares, bosses,
the final boss and enrage: when they come, glows, drops; how enemy health and damage scale is not explained), **Upgrades** (the level-up cards:
level up, new spell, spell upgrade, rarity, reroll, the gold card, and every upgrade kind) and
**Loot** (pick-ups: experience, gold, the magnet, the three runes, the Warband Standard; loot from
caches: the Battleground Cache, the free Rare Cache, rings, trinkets, amulets, legendary amulets).
Each entry has an icon (a stat its prestige stat's, else its ring's or trinket's; extra targets
Hunter's Mark's and thorns the Thorns spell's; gold the gold card's; the Berserker and Speed Runes
their buff's and the Warband Standard its Battle Standard spell's, these from the player's
`Spell.dbc`; experience `INV_Misc_Book_11`, the magnet `Spell_Nature_AstralRecal`, the Restoration
Rune `Spell_Holy_Heal`, the Battleground Cache `INV_Box_01`, the Rare Cache `INV_Box_04`; rings, trinkets and amulets the content's first one's), its
name (an enemy in its glow's colour) and a line or two. The tab's sections scroll with the wheel;
**Close** (and Escape). Every number is read from the content (`balance.ron`: an elite's
experience, the Warband Standard's count and time) or quoted
from the rule's own constant in `esurv-sim` (runes, magnets, caches, rerolls, upgrade caps, the
base stats), never copied; a test changes the balance and checks the guide follows, and a test
holds every `Stat` (read from the type itself) and every prestige stat to a Stats entry.

**Options** (`menus::options`): the game's own settings in a panel of
this look, with four tabs (the fourth, Saves, is the save's export and import, `menus::save_files`). The stock options frames exist only in the world session's Lua
interface, which benilla builds when a hero enters the world; so this panel stands in for them
before a run. It reads and writes the same settings through the very calls they make
(`GetCVar`, `SetCVar`, `SetWorldDetail`, `SetTerrainMip`, `SetBaseMip`, `SetGamma`,
`SetScreenResolution`, `SetMultisampleFormat`, `RestartGx`), on the Lua VM benilla keeps at the
character select, so a change shows in the game and in the stock frames later.

| Tab | What it holds | Applies |
|---|---|---|
| Video | the stock Video Options frame one for one: resolution (the display's list) and multisampling (the device's formats); its eighteen boxes (windowed, windowed and maximized, vertical sync, triple buffering, hardware cursor, smoother input, interface scale on, desktop brightness, film subtitles, smooth texture blending, far-off land in less detail, all shaders, shining terrain, screen glow, death effect, animated model shaders, smooth model shading, weather shaders), each greyed while the box it hangs on is off (and triple buffering where the device has none); its nine sliders with the stock ranges and steps (interface scale 0.64 to 1, view distance 177 to 777 yards, ground detail, terrain textures, texture detail, brightness −0.5 to 0.5, texture sharpness 1x to the device's top, spell effects, weather), the interface scale only with its box on, brightness only with neither desktop brightness nor windowed on. Its refresh-rate list is left out: benilla owns no exclusive display mode and offers none | on Okay, as the stock frame: each changed box and value written (All Shaders copied into `ffx`), the display rebuilt (`RestartGx`) when a display setting moved |
| Sound | the stock Sound frame one for one: sound, ambient sound and spoken errors (both on sound), music, music without pauses, hearing from the hero, emote sounds; master, effects, music and ambience volume, 0 to 100% in tens | as moved, as the stock frame; Cancel puts them back |
| Interface | the Interface Options settings that are plain CVars the game itself reads, with the game's own settings, in four headed sections in this order (each a heading between two gold rules over two columns; more than the panel shows scroll with the wheel and keep their place): **Mouse & Camera**: invert the mouse, click to move, the four camera boxes; mouse sensitivity, mouse look speed, the camera following style (`cameraSmoothStyle`: only when moving 1, always 2, never 0, the three the stock 1.12 dropdown writes and the camera does; 3 shows as never, as the camera reads it), camera follow speed, camera distance. **Names**: the five name boxes. **Eternal Survivors**: show damage numbers, **Damage numbers** (Smart or All: [Damage numbers](#damage-numbers-smart-or-all); a tooltip on its label), **Check for updates** (the game's own CVar `esurvCheckForUpdates`, `1` by default; ticked, the game checks for a newer version as soon as the setting reads on; unticked, it never asks and hides an offer: [server.md](server.md), "Updates"; a tooltip on its line), **Glow around elites, rares and bosses** (the CVar `esurvEnemyGlow`, `1` by default, `esurv_app::glow_option`: ticked, elites wear a purple outline glow, rares and the Rare Caches they drop a silver one, bosses an orange one; unticked, all of them hide at once; normal enemies never glow; a tooltip on its line); then the boss timers ([bosses.md](bosses.md)): boss warnings, warning sound (on boss warnings), boss timer bars, show bar handles and reset bar positions (both on the bars). **Misc**: chat bubbles, deselect on clicking the ground, clearing away, loading-screen tips, pet damage numbers. Not shown, their saved values kept as they are: party chat bubbles, every roll in the chat, guild arrivals, the language filter, refusing trades, tips in the game (the tutorial alerts), attack on assist, self-cast. Left out: what only the Lua interface reads (tooltips, bar text, the per-character interface settings such as combat text, party frames and action bars), Show Helm and Show Cloak (the hero's own, sent from the world), the spam filter, and the key bindings | on Okay |
| Saves | the save's export and import (design step 6): the folder the save goes to and comes from, **Export save**, **Import save**, and the saves in the folder (below) | at once; Okay and Cancel are for the other tabs |

**Export and import.** The game has no file dialog, so the save goes to and comes from one fixed
folder, named on the tab: `exports` beside the save (by default `saves/exports` beside the
game; beside `$ESURV_SAVE` when that names the save). **Export save** writes the
whole save as it stands to a new file there, named after the save and the moment in UTC
(`heroes-2026-10-13-142530.ron`; a number is added when the name is taken). Below, the folder's
saves newest first (the eight newest; "and n older ones"), each with how many heroes it holds, or
greyed with "Not a save this game can take": the server reads and checks each one (below). A
click picks one; **Import save** (on only for a save the game can take) asks first, in the menus'
look: "Replace the save with this one?", the file's name and its heroes, and that every hero,
what each class has earned, the collection, the codex and the run options are replaced, and that
the save now kept goes to the same folder first; **No** (the default, filled; Escape too) and
**Yes**. Yes has the server check the file again, write the save it replaces to the folder
(`heroes-before-import-<moment>.ron`, which the list then shows, so an import is undone by
importing that), and only then replace the save and write it; the tab says which file the save
now comes from and where the old one went, and the character list is read anew (the camp shows
the imported heroes). A file that does not read or breaks the save's rules replaces nothing.

A save is checked against the content and the save's rules ([server.md](server.md), "The save
file"): every hero of a class and a race the content has, a gender of 0 or 1, a name the stock
rules allow, no two heroes with one guid or one name, at most nine heroes of a faction and 18 in
all; a next guid at or under a hero's is raised past it. Export and import go through the pre-run
channel to the server's save (`savefiles`, `export`, `import`); only the game the save is kept on
answers them, and any other client is refused ([server.md](server.md), "Before a run").

A line says which settings take hold only after a restart (multisampling, terrain textures,
texture sharpness, smooth texture blending) or when a hero next enters the world (shining terrain,
animated model shaders), and after Okay a notice says when the changes made take hold. A hint says
key bindings are set during a run: Escape, then Options, then Key Bindings (the same panel over
a run, [Pausing](#pausing-the-escape-menu)). Escape cancels.

### The hero sheet

Over the camp, the world dimmed behind it: **Back** at the top left. No
tabs: **Modify character** (and a new hero) opens it on **Character**, and the warband screen's
**Collection** and **Talents** buttons open it on those views; Back goes back to the warband
screen. It makes a new
hero of the class picked, or changes the hero picked on the warband screen ("Modify selected
character"): its name, race, sex and look (a hero's race and look may
always change before a run). Back leaves without making or keeping anything.

**The left column** (on Character; on Collection only its Equipment; the Talents trees take its room): **Appearance**: the races as
portraits (the stock create screen's race sheet, in the sex picked) under **Alliance** and
**Horde**, the races the class may not be dimmed and not clickable, the picked one glowing; the
race's name; **Male** and **Female**. **Racials**, between them and Equipment: the
race's spell, then its passives, as icons in a row, each with its tooltip (its name, "Racial
spell: casts itself" or "Racial passive", and what it does: the spell's description with its
numbers at level 1, a passive's line in our words); the names are the client's, read through the
content's pointers, a passive's icon its stock racial spell's (`Spell.dbc`, `hero_sheet::racials`;
a question mark without the data); they follow the race picked. **Equipment**: the slots in the order head, shoulders,
chest, hands, wrists, waist, legs, feet, then the weapons, each with the worn piece's icon, its
slot and its name in its quality colour (its tooltip as the armory's), or "Empty"; at the foot
what worn gear adds stat by stat in green, or that it adds nothing yet.

**The Character tab.** In the middle the hero's name as a big heading, typed in (letters only,
up to twelve; Backspace takes one back, Enter confirms), "sex race class" under it in the class
colour, and the hero standing on a lit floor of ours (a warm glow under its feet and a soft
light from above): benilla's glue booth with no backdrop (`SelectModel`), dressed in what its
class wears (each piece in the look chosen for its slot) over the race and class's starting
outfit; dragging over it turns it (0.6 degrees a pixel, as the stock screens), and so do the gold
arrows under it (half a radian each). At the foot **Create** (a new hero) or **Save changes**,
off until the name has two letters. On the right **Features**: skin colour, face, hair style, hair
colour (horn style and horn colour for a race whose hair is horns) and the race's fifth feature
named in our words by its glue token (facial hair, piercings, earrings, tusks, horns, markings,
features, hair; none when the race and sex have none), each in a box with gold arrows that go
round its choices, `n / m` and a row of dots; **Randomize** picks every feature at random;
"Race and looks can change on any hero before a run."

**The Collection tab.** The left column shows Equipment alone (a slot picked shows its looks). In the middle the **Collection Log**: the class's
pieces in columns by slot (their names over them) and rows by quality (Common to Legendary, a
tier each), each row headed by its quality and how many of it are found (`n/9`) and the zones
that drop that tier; a piece found shows its icon, one not found is dark with a "?"; **E** marks
the piece worn, **L** the piece whose look its slot shows; each has the armory's tooltip. Under it
**Still to find in your zones**: how many pieces the zones open to the class still hold, and the
first seven (piece in its quality colour, its boss with its arrival time, its zone, its drop
chance at the run options' bonus). A zone the save has not discovered (menus.md) shows "???" and a lock in place of its name and dark unknown icons in place of its pieces. At the foot how to read the marks and the total found
(`n / 45`). On the right **Details**: the piece picked (its icon, name and slot, what it adds,
worn, in the collection or not found yet, the bosses that drop it, and **Wear** or **Take Off**
and **Show This Look** or **Show Own Look**), or, for a slot picked on the left, the looks it can
show (every piece of the slot, the one shown glowing; a click shows it) and **Show Own Look**.
A double click on a piece of the log (a second click on it within
0.45 s) does what picking it and pressing its button does: **Wear** on a zone's own row, **Show
This Look** on a second row of looks (the piece worn first when the slot wears nothing); nothing
on a piece not found or already worn and shown.

**The Talents tab.** The class's three trees side by side where the left column was, each in a
panel with its icon, its name and the points spent in it, on its stock art
(`Interface\TalentFrame\<tree>-TopLeft`, `-TopRight`, `-BottomLeft`, `-BottomRight`, in the stock
frame's shape, 320 by 384, its top 332 rows shown), dimmed a little; each talent at its place on
the stock frame's grid (its columns centred), its icon in a frame (the picked one glowing, one
not open yet faint) and its `rank/max` in green (a rank can be bought), gold (all bought) or grey
(not open yet), a line from it to the talent it needs, gold once that one is at its last rank;
under the art its **Milestones**, each with the points in the tree it needs (10, 20, 30). Each
talent has the armory's tooltip, with our click lines. On the right **Talent Points**: the
points to spend (Morpheus, big, green while any), **Earned** `n of 51` and **Spent**, the talent
experience bar in the experience purple (`into / cost`, or "Every point earned"), how runs earn it,
**Reset** (every rank taken back, deepest rows first, by the rules, then every prestige point;
on while either is spent), and the talent or prestige stat picked: its icon, name, rank, what it
does now and at its next rank, what it still needs in red, **Learn** and **Unlearn** (each off
when the rules say no).

**Clicks.** On a talent or a prestige stat, the first left click picks
it (its tooltip says "Click to pick it, click again to learn a rank."); a left click on the one
already picked learns a rank; a right click picks it and takes a rank back. Learn and Unlearn on
the right do the same. Picking a talent lets go of the prestige stat picked, and the other way
round.

**Prestige (classes.md, "Prestige points").** Once the class has all 51
talent points, a **Prestige** panel shows under the trees (the trees and it scroll with the wheel
when they are taller than the room): "Prestige", the points to spend (green while any),
`Earned n · Spent n`, the experience bar to the next prestige point, a line on how they come, and
every prestige stat the class can buy (`prestige.ron`, in its order, as many to a row as fit):
its icon (picked glowing; one at 0 that can take no rank dimmed), its name, and `rank · bonus`
("3 · +3%") in green while a rank can be bought, gold at a capped stat's last rank, grey
otherwise. Its tooltip: name, rank (`Rank 1/1` when capped), what it gives now and at the next
rank, and the click lines.

Every change on these two tabs is the class's, for every hero of it, from the next run, and goes
to the save at once (the armory's requests as asks before a run). While a hero is made, a hint
says so: they stay even if the hero is not made.

**Confirming.** Create asks the server to make the hero (`CMSG_CHAR_CREATE`); once made it is
picked on the warband screen, in its camp. Save changes asks `rename` when the name changed and
`reshape` when the race, sex or look did ([server.md](server.md), "Before a run"), then the roster
again; a hero gone over to the other faction takes the camp with it. A refusal is said over the
sheet: a name taken or not two to twelve letters, a race the class may not be, a full camp.

### The camps

The character select is a camp in the world, one per faction (`esurv_app::picker`).

| Camp | Where | Lit | In front | At the back |
|---|---|---|---|---|
| Horde | beside the road before the front gate of Orgrimmar (Durotar, map 1), on the east side of the road on flat open ground, the gate behind them | at night (23:30) | kneel | stand |
| Alliance | on the grass beside the main gate of Stormwind (Elwynn Forest, map 0), the gate and its banners behind them | at night (23:30) | sit | stand |

Each camp is the world itself, seen from a fixed camera 3 yards over the ground and 9 behind a
campfire (a camp-sized one, `ElwynnCampfire.mdx`; the Alliance's camera 5% closer along its line
of sight), looking down a little on the heroes, the gate rising behind them.

**Nine heroes a camp.** The heroes of the camp's faction (by race), in roster order: the first
five in front, in a semicircle round the campfire on its far side, 3 yards out and evenly spaced
(36 degrees apart), facing it, filled from the middle out, in the camp's front pose (Horde
kneel, Alliance sit); the other four at the back, two on each side, already standing, facing the
campfire and the camera, each in a gap of the front row as the camera sees it, filled left near,
right near, left far, right far. A front hero stands up while the mouse is over it or while it is
picked, and goes back down when neither holds; a back hero greets once when picked: the fighting
classes (warrior, paladin, hunter, rogue) salute (`EmoteSalute`), the others wave (`EmoteWave`).
The poses and the greeting rule are named in one place, `picker/scenes.rs` (`HORDE_FRONT_POSE`,
`HORDE_BACK_POSE`, `ALLIANCE_FRONT_POSE`, `ALLIANCE_BACK_POSE`, `greeting`). Clicking a hero in
the camp picks it.

A save holds at most nine heroes of either faction (18 in all): making a tenth of one faction is
refused. Each hero wears its class's worn gear (the character list carries it, see
[server.md](server.md)); a slot no class gear fills shows the race and class's starting outfit
piece, as the stock create screen dresses a new hero (a presentation rule, no numbers). The
heroes by the fire hold their weapons drawn, standing or kneeling, as the stock select screen
shows a character; a starting-outfit weapon a figure puts away rests where `outfit_weapons.ron`
says, as in a run (staves on the back, one-handers on the hip;
benilla's `OutfitWeaponSheaths` hook).

### Checking the menus unattended

`ESURV_MENUS_SCRIPT=<step>;<step>...` (a dev aid) presses the menus' buttons inside the
game, as a click does, with no input of the operating system, one step after the other, each
1.5 s after the last: `ready` (the select screen up, its camp loaded, a screen shown; then 6 s for
the figures), `wait:<s>`, `size:<w>x<h>` (the game's own window, resized from inside),
`shot:<name>` (the game's own shot, `<name>.png` in the folder `ESURV_MENUS_SHOTS`; the window
is never raised nor pinned), `quit`, or a press: `class:<id>`, `choose`, `back`, `create-new`,
`hero:<name>`, `camp`, `modify`, `delete`, `yes`, `no`, `zone:<id>`, `run-options`, `more:<n>`,
`less:<n>`, `done`, `mode:<normal|challenge>`, `challenge-up`, `challenge-down`, `challenge-okay` (closes the Challenge Mode box), `start`, `codex`, `codex-list:<rings|trinkets|amulets|legendary>`,
`codex-page:<next|prev>`, `close-codex`, `guide`, `guide-tab:<stats|enemies|upgrades|loot>`, `close-guide`, `game-options`, `options-tab:<video|sound|interface>`,
`option:<cvar or getter>:<+|->`, `okay`, `cancel`, `options-tab:saves`, `export-save`,
`save-file:<name, or its place in the list, 0 the newest>`, `import-save`, `import-yes`,
`import-no`, `quit-game`, `open:<collection|talents>` (the warband screen's button), `end-panel:<victory|defeat>`
(a made-up end panel over the run, once the hero is in it: sample numbers, kept until a button or a fresh run closes it), and on the hero sheet `tab:<character|collection|talents>`,
`double-piece:<slot>:<zone>[:<look>]` (two clicks on a piece in one frame, a double click),
`race:<1.12 id>`, `sex:<0|1>`, `feature:<n>:<+|->`, `randomize`, `turn:<left|right>`,
`name:<text>`, `confirm`, `slot:<n>`, `piece:<slot>:<zone>` (`piece:<slot>:<zone>:<look>` in another row of looks), `look:<slot>:<look>` (shows a look from the collection; `look:<slot>` the worn piece's own), `talent:<id>`, `learn`, `unlearn`, `reset`. On the warband screen `start:<option id>` picks a starting option, `loadout:<loadout id>` picks the class's weapon loadout (its id in `loadouts.ron`), `tip-start:<option id>` and `tip-loadout:<loadout id>` show that icon's tooltip as the pointer would and `tip-off` takes it away. `ESURV_MENUS_DEMO=1` shows a sample of the look's pieces in
place of the first screen.

### benilla's hooks: a select screen over the world

`benilla_app::SelectScene` (a resource, `SceneView`: map, eye, look-at point, minute of the day)
draws the world behind the select screen while it is up and no hero is entering the world:
the world streams around the look-at point, the world camera looks from the eye once the place
has loaded (the screen's own backdrop stays until then), the clock reads the scene's minute,
and the select screen drops its painted race backdrop, its model and its rotate buttons.
`benilla_app::SceneFigure` puts a hero (from its roster row, with its stand state) or a game
object (by its `GameObjectDisplayInfo` id) in that world with no wire object behind it, set on
the terrain under it: benilla's own unit pipeline draws, dresses, lights and poses it, so a
change of stand state plays the stand-up. `benilla_app::SelectScreen` mirrors the roster, the
picked row and the realm; `benilla_app::SelectCharacter(guid)` picks a row.
`benilla_app::SelectListFilter` lists only the rows of the guids it names (in roster order);
the list's clicks and arrow keys walk the rows listed. `SelectScreen::hovered` names the hero figure
under the mouse (from the window's cursor events), `benilla_app::FigureEmote` plays an emote
animation once on a figure, and `benilla_app::SelectCreateAllowed` sets when Create New Character
is offered (the stock rule: below ten characters). Stock benilla never
sets a scene, and then nothing changes.

**Checking the camps unattended.** `ESURV_PICKER_SHOT=<png>` (a dev switch) holds at the
character select (leave `WOW_CHAR` unset), waits until the camp has loaded and
8 s more (`ESURV_PICKER_SHOT_AT` seconds), writes the window and closes the game.
`ESURV_PICKER_FACTION=horde|alliance` picks the camp shown first, benilla's
`WOW_CHARSELECT_PICK=<name>` the hero, and `ESURV_PICKER_PICK=<name>` keeps that hero picked
whatever is clicked or hovered. `ESURV_PICKER_CLICK=<name>` clicks that hero's figure
once the camp has loaded, by writing the window's cursor-move and mouse-button events inside the
game (no input of the operating system), with a shot just before it (`<shot>_before.png`). To try a spot out: `ESURV_PICKER_CAMPFIRE=<x>,<y>,<z>`,
`ESURV_PICKER_BEARING=<degrees>` and `ESURV_PICKER_FRAMING=<back>,<up>,<beyond>,<look
up>` (yards from the campfire) move the camp shown.

### benilla's hooks: a select screen of the embedder's own

`benilla_app::SelectScreenReplaced(true)` puts a crate's own select screen in place of the stock
one: the stock tree is not built, its keys (Enter, Escape, the arrows) do nothing and its delete
dialog never opens, while the session stays parked at character select and the scene, its
figures and `SelectScreen` work as before. The crate's screen asks through messages:
`SelectCharacter(guid)` picks a row, `SelectEnterWorld(guid)` enters the world as Enter World
does, `SelectCreate(CharCreateReq)` creates a character (`CMSG_CHAR_CREATE`; on success the fresh
roster comes first with the new row selected), `SelectDelete(guid)` deletes one
(`CMSG_CHAR_DELETE`; asking first is the crate's), and `SelectRefresh` asks the server for the
roster again (`CMSG_CHAR_ENUM`) keeping the selected row, after the crate changed a character
another way. `SelectActionDone { action, code }` answers each create and delete with the server's
result byte. `SelectModel` shows a character (a `FigureHero` and a facing) in the glue booth with
no race backdrop, its picture clear behind the body, and `SelectModelImage` is that picture for an
`ImageNode`; the booth renders on while it shows, as it does while a glue scene stands, so the
character breathes. `CreateChoices` says what the create screen would offer: which race may be
which class (the stock pairs and the added ones), how many choices each look feature has per race
and sex, and the create screen's glue tokens for the fifth feature (`NORMAL`, `PIERCINGS`,
`TUSKS` ...) and the hair (`NORMAL`, `HORNS`). `SceneFigureMask(mask)`, put on a figure, draws its body, gear
and weapons alone, through a camera that follows the world camera's eye and optic, into mask
`mask` of `FigureMaskImage`, a window-sized picture clear but where its figures are, for drawing
round their silhouettes (only the model batches take the mask's own layer, never their particles);
there are `FIGURE_MASKS` of them, and they take a run's units and game objects too
([Glows round elites, rares and bosses](#glows-round-elites-rares-and-bosses)). Stock benilla never sets them, and then nothing changes.

## The card screens (level-up, cache and rare cache)

When the server opens a choice for the hero (`ESURVC`: a level-up, a cache bought, or a rare's
free cache opened), the run waits frozen (the server's choosing phase) and a panel shows the offers, one
card each, side by side (`esurv_app::cards`). The client's `hello` says it has `cards`, so
the server waits for the player's pick instead of taking the first offer.

**The look.** The card screens wear the menus' look
([The menus' look](#the-menus-look); [menus.md](menus.md), "The look") in place of the stock
dialog: the world dimmed behind one near-black panel with the thin warm-gold border and rounded
corners, the heading in Morpheus small capitals between gold rules, small-caps grey labels,
value boxes, framed text buttons, and a glow on what is under the pointer or taken.

| Part | What it shows |
|---|---|
| Heading | Level-up: **Level** and the new level. Cache: **Treasure Cache**. Rare's cache: **Rare Cache**. |
| Line under it | Level-up: choose one, the rarer the card the stronger. Cache: choose a ring, a trinket or an amulet. Rare's cache: free to open, choose a ring, a trinket or an amulet. |
| Left column, **Spells** | The spells known, `n / 6`: each its icon, name and rank (its tooltip: school, how often it casts itself), then the empty slots. |
| Cards (up to 4) | A near-black card whose border and glow take its rarity colour (`ITEM_QUALITY_COLORS`), dim at rest, full under the pointer, strongest once taken, with that colour washing down from its top: the key that takes it (top left, in a small box), its badge (top right, below), the icon framed in the rarity colour (its tooltip: a spell's school and cooldown, an empowered amulet's effects), the rarity in spaced small capitals in its colour, the name in Morpheus in the rarity colour, the line under the name in a muted gold (below), a gold divider, what it says (below: its numbers in green, as an item's `Equip:` lines, its words in white, how things stand now in grey; nothing else in the card's middle), and its footer: a strip across the card's foot under a thin rule, in small quiet grey, saying what taking it does to the hero's slots (below). No button: a click anywhere on the card takes it. |
| Right column, **Jewelry** | **Rings** `n / 2`, **Trinkets** `n / 2` and **Amulet** `n / 1`: each worn piece's icon, name and rank (its tooltip: its text at its value now; the amulet's at its rank), then the empty slots. |
| Reroll button | A framed button: **Reroll (n free)** while free rerolls are left, else **Reroll (cost gold)**; dimmed and dead when neither a free one nor the gold is there (the gold from the HUD line; until one came, the server decides). |
| Gold | The hero's gold in a small value box, once the HUD line said it. |
| Last line | The keys that work: 1 to n to choose, R to reroll. |

### What a card says

A card says what taking it changes, and no more: a spell the hero has
never had is written out in full, a spell it knows says only its improvement, a piece of jewelry
says its stat lines. Every number a card brings reads **green and comes first** ("+34% damage",
"+12% movement speed"), its words white; a number inside a spell's description stays white, the
description being one plain paragraph. Every sentence is our own; the texts are content, not
code (design step 9, [design.md](design.md)):

* a spell's **description** is a template in `abilities.ron` (`description`), its `{name}`
  placeholders filled with the spell's numbers at the rank offered, for the hero taking it (the
  server works them out with the run's own rules: `esurv-sim` `game::spell_numbers`, sent
  with the offer, [server.md](server.md)): what it hits and whom it prefers, how far, its damage
  and school, what it does to what it hits and for how long, what bosses shake off. The names a
  template may use and how each reads are `esurv_content::text::SPELL_NUMBERS`;
  `{targets:one|# many}` picks the words by the number (`#` is the number). A test fills every
  spell's description at ranks 1 to 10 for a plain and a strong hero: a placeholder a spell has
  no number for fails it, and the loader refuses a template that names an unknown number;
* the words after an upgrade's number, what power raises on a heal or a shield, and every stat's
  line are one table, `content/words.ron`; a ring's or trinket's line is its own `text`.

| Offer | Badge, under the name, footer | Lines |
|---|---|---|
| New spell, common | **NEW**; New spell; Takes a spell slot · `n/6` | its description, filled at rank 1 |
| New spell above common | as above | its description (the head start counted in its numbers), then the head start: **+n% damage** (healing, absorb) or **−n% cooldown** |
| Spell upgrade | **RANK b**; Rank a » b; Improves a spell you know | only what it improves, each kind a line: **+n% damage**, **−n% cooldown**, **+n targets**, **+n% area**, **+n% duration**; no description |
| New ring or trinket | **RING** (**TRINKET**); Ring (Trinket); Takes a ring (trinket) slot · `n/2` | its line at the offered value, the value green: **+19%** critical strike |
| Ring or trinket upgrade | **RANK b**; Rank a » b; Upgrades a ring (trinket) you wear | its line with the worn value and the value after the step: **+70% » +114%** critical strike damage |
| Amulet | **AMULET**; Amulet; Takes the amulet slot, or **Replaces** the worn amulet (below) | its rules first, each a sentence with its numbers green, then its stat lines (**+15%** max health) |
| Legendary amulet (`legendary:<spell>`) | as an amulet | "Remakes your \<spell\>:", then a line for every effect its recipe gives that spell (below) |
| Empowerment | **RANK b**; Rank a » b; Empowers your amulet | its effects' new strength (the factor as a percentage); "Now:" the strength worn |
| Gold | none; Gold; Nothing left to learn | **+n** gold |

**The badge.** A small framed label at every card's top right says what
is new about it (**NEW** for a spell the hero does not know, **RING**, **TRINKET** or **AMULET**
for jewelry not worn) or, for anything taken again (a spell upgrade, a worn piece's upgrade, an
empowered amulet), the rank it reaches (**RANK 3**). A new thing's badge is filled with the rarity
colour (a light grey for common) with dark letters; a rank is outlined in that colour instead.

**Under the name**: "New spell", the slot's word for new jewelry
("Ring", "Trinket", "Amulet"), or "Rank a » b" for anything taken again; the rarity's word stays
above the name.

**The footer**: what taking the card does to the hero's slots moves
out of the card's middle to its foot, as small quiet grey text in a strip under a thin rule.

An upgrade's gain is what the step does to what the spell has now: power by the step (a heal or a
shield on the hero grows with the square root of power, so its gain is shown as that), a cooldown
shorter by the step's share of the cut it has, area and duration by the step's share of what they
have, targets by the count. The rank bonus every step adds stays unsaid.

**Replaces.** An amulet that would replace the worn one says so in its
footer, in place of "Takes the amulet slot", in the same quiet type: **Replaces** in grey, the
worn amulet's name a shade lighter, on one line; no icon, no heading, no box. Its tooltip shows the worn amulet's
effects, "Worn now" and "Lost if you take this one". A click on it takes the card too.

**No Choose button.** A card has no button; a click anywhere on it
takes it (the line naming the amulet it replaces included). Its glow under the pointer says it can
be clicked; the taken card stays lit the strongest until the server answers. Keys 1–4 still take
cards.

`a` is the rank the kit says now, `b` the rank the offer reaches (a legendary spell card steps
two kinds, so two ranks).

**Legendary amulets' text.** A remade legendary amulet has no text of its own in the content: the
card builds it as the run does (`legendary_for`, its delivery kind's recipe in
`legendary_amulets.ron`, the parts whose conditions the spell meets) and says each effect in our
words with the recipe's numbers (green), empowered by the worn factor in the tooltip of a worn one,
its rules first as sentences, then its gains: +damage, −cooldown, +targets, +area, +duration;
a hit leaping to more enemies, a chance to strike again,
what it hits stunned (or melee attackers stunned by chance), damage again over time, kills
exploding, chains losing no strength, an effect over time spreading on death, a kill readying it
again, a splash round its target, a breaking shield bursting, every totem pulse healing; for a plain amulet, a killing blow survived and
melee attackers struck back too. A test
reads every recipe for every spell it can remake, so an effect without words fails.

**The kit.** A choice screen carries what the hero has as it opens (`Kit`: the spells known with
their ranks, the rings and trinkets worn with their ranks and values, the amulet worn with its
rank and empower factor; [server.md](server.md), "What the server tells a client"), for the slot counts, the
ranks, the amulet replaced and the side columns.

**Input.** Keys 1–4 and the keypad's 1–4 take that card, R rerolls, a click anywhere on a card
takes it, the reroll button rerolls. Input is ignored for 0.45 s after the screen opens
or its cards change, and one answer goes per screen: after a take or a reroll nothing more goes
until the server answers (new cards, the screen closing, or a refusal, which lets the screen
answer again). While the screen is open those keys and Escape are the screen's: no key binding
runs for them (the action bar's 1–4 casts nothing, Escape opens no menu and pauses nothing),
through benilla's `OverlayKeys` (above), under the name `cards`. A focused chat box still takes
every key first.

The screen closes when the server says the choice closed (`ESURVX`), the run ended or restarted,
the HUD shows the run stopped, or the hero left the world. `CardScreen::is_open` tells other
screens it is up: a screen that pauses on Escape leaves the run alone then.

**Checking it unattended.** `ESURV_CARDS_SCRIPT=<step>;...` (a dev aid) drives the screen
inside the game, with no input of the operating system: `screen` (waits for a choice from the
server), `show:<kind>` (a made-up screen from the content's pieces, its numbers from the run's own
rules, its answers going nowhere, kept over any screen the server opens until `close`: `levelup` with a common new spell, a new spell with a head start and an
upgrade; `cache` with a new ring, trinket and amulet; `rare` with upgrades of the worn trinket
and ring and the amulet empowered; `amulet` with one that replaces the worn amulet, a legendary
one and a trinket upgrade; `legendary`), `lit:<n>` and `unlit` (card n shown as under the
pointer), `key:<1-4|r>` (the screen's own key message), `click:<n>` (card n clicked, the
screen's own click message), `take:upgrade` (the first card that upgrades a known spell
clicked, the cards rerolled while a free reroll is left and none does, else the first card), `close`, `wait:<s>`, `shot:<name>` (the game's own shot, into `ESURV_CARDS_SHOTS`)
and `quit`.

## The damage meter

A small window listing the run's damage, one bar per entry, in the style of the Recount meter
(our own drawing: plain shading and drawn buttons, with the stock 1.12 page arrows for the
list arrows, loaded as the look kit loads the stock art: `stock::load_stock_art`, in benilla's
sprite form, so they draw in the stock colours).

| Part | What it does |
|---|---|
| Title bar | The list's name: **Damage Done** or **Damage Taken**. Drag it to move the window. |
| `<` `>` | The previous and next list. |
| `R` | Clears the meter (every list, and its clock). |
| `–` `+` | Folds the window to its title bar, and back. |
| Rows | `rank. name` on the left, `total (per second, share%)` on the right; the bar's length is the entry's total against the largest. Largest first, ties by name. |
| Scroll bar | Shows when the rows do not all fit; the wheel over the window scrolls, and the thumb drags. |
| Corner grip | Drag to resize; the height snaps to whole rows (2 to 40). |

A hero's bar takes the stock class colour (`RAID_CLASS_COLORS`); any other entry (a pet, a
spell, an enemy, `area`) takes one of ten colours of our own, kept by its name. Totals from
100 000 read in thousands (`153.2k`), from a million in millions (`2.35M`). The share is of the
list's own total. Per second is the entry's own figure when the sender gives one, else its
total over the run's elapsed seconds; nothing shows in the run's first second.

The window first sits on the right, its bottom 45% of the window's height up, above the corner where the stock tooltips grow (the meter never covers a tooltip). Where it was moved, its size,
the list shown and the fold are kept in `meter.ron` in benilla's state folder
(`benilla-config/`), written when a drag ends, the list changes or it folds; so is whether the
minimap's meter button hid it (see the HUD's minimap buttons):

```ron
(position: Some((1210.0, 604.0)), size: (250.0, 167.0), mode: DamageDone, collapsed: false, hidden: false)
```

A missing or unreadable file starts over at the default. The window is kept on screen when the
game window is smaller than where it was left.

### Its feed

`esurv_app::meter::MeterFeed` (a Bevy resource) is the window's only input; the protocol
fills it, the window only reads it, and `R` clears it.

| Field | Holds |
|---|---|
| `elapsed` | Seconds the run has gone (the per-second divisor). |
| `damage_done`, `damage_taken` | One `MeterEntry` per line, in any order. |

`MeterEntry`: `name`, `amount` (the total so far), `per_second` (optional, the sender's own
figure), `class` (optional, a class id from `classes.ron`, for the class colour). Helpers:
`entries(mode)`, `entries_mut(mode)`, `add(mode, name, amount)` (sums by name), `clear()`,
`rows(mode)` (what the window draws). Pressing `R` also sends a `MeterReset` message, for a
sender that keeps totals of its own. A writer to the feed runs before the window's
`MeterSystems`.

In play the server's meter lines (`ESURVM`, the run's totals, at most every 0.2 s) fill it, with
the run clock from the HUD line (`esurv_app::meter::server`). Damage done shows one row per
spell (its id as words: `shadow_bolt` is "Shadow Bolt"), one row per assist character
("Assist: <name>", the meter line's `assist <name>` source: its own hits, the areas and bleeds
it left, and the pets it summoned), and one row each for all pets ("Pets"),
Thorns, the weapon swing ("Melee") and the rest ("Other"). A
proc counts under what gave the hero it, by the name the content gives it — a talent
by its talent name (from the client's names, never written by us), a ring or trinket by its item
name (the Major Soulstone's extra life blast), an amulet by its name, a legendary amulet by its
own ("Light's Blessing"), a weapon loadout's proc by the loadout's name, and an aura striking
back (Lightning Shield) on its spell's row; the ticks of an effect over time a milestone put on
an enemy count under that milestone's origin; rows of one name are one row, and a proc of no known
origin counts as "Other". The sim's proc hits carry that origin (`HitSource::Proc`'s `origin`,
`ProcOrigin`, presentation only: the golden run's event log leaves it out); the meter line
sends it after both lists (`proc <spell>` as before, then the procs' origins in order), so an
older client still reads the line and an older server's procs read with no origin. Gear bonus
pairs give only stats, so no proc comes from them. Damage taken shows one row per enemy and
"Area" for what no enemy dealt. A new run starts the meter from nothing; `R` keeps the totals of that
moment and shows what came after.

`ESURV_METER_DEMO=1` fills the feed with made-up totals that grow instead, for a look at the
window without a run.

The meter window's place is kept in `meter.ron`.

## The end of a run

When the hero falls or wins, a panel opens over everything else (`esurv_app::run_end`). It wears the menus' own look (it
fits the warband screen, the hero sheet and the Escape menu; [The menus'
look](#the-menus-look), `menus::look`): one near-black panel with the thin warm-gold border and
rounded corners, Morpheus small-capital headings between gold rules, small all-caps grey labels,
the look's framed buttons, in the stock interface's units, so it grows with the window. Every
text on it is ours.

| Part | What it shows |
|---|---|
| Title | **Overrun** in the menus' red after a fall, **Triumph** in the headings' gold after a victory: Morpheus small capitals (34 units) between two gold rules, as the menus' headings |
| Subtitle | under the title, white (Friz Quadrata 11): the hero's name, race and class, then the zone: `Probe, Human Warlock – Elwynn Forest` |
| Window | the menus' panel, 480 units wide; a gold line under the subtitle and between the parts below. Dragging the title or the subtitle moves the whole panel (always on screen; where it was put is kept between sessions, below) |
| Tally (left) | under the label **RUN REPORT**: Time lasted (`M:SS`), Level reached, Enemies, Elites, Rares and Bosses defeated, Damage done, Damage suffered, Gold collected, Caches looted; labels grey (Friz Quadrata), numbers white (Arial Narrow), thousands marked (`4,812,331`) |
| Talent Experience (right) | under the label **TALENT EXPERIENCE**: the parts the run paid, appearing one after another: Time endured, Enemies, Bosses, Victory, Zone difficulty, Challenge settings (a part that paid nothing is left out), each `+n` in green; **Earned** (gold) counts up through each; the class's bar (the hero sheet's: a dark rounded box with the gold border, filled in the experience bar's purple) fills from where it stood before the run, its text `into / cost` of the next talent point (or that every point is earned); **Talent points: n (+k)** in gold below, and **New talent point!** flashes green each time one is crossed. "Nothing earned this time" (a quiet grey hint) when the end paid nothing |
| New zone | `The way to Duskwood lies open`, in green, when the run opened a zone to the class |
| Gear won | under the label `GEAR WON: 3 PIECES, 2 NEW`, a row per piece: its name in its quality colour (its tier: common to legendary), its slot as the stock interface names it (grey), **New** (green) or **Already owned** (grey); the first six, then "and n more" |
| Buttons | the look's framed buttons, 124 units wide: **Run Again** (gold-framed, the call to act); **Go Endless** after a victory only; **Return to main menu** (150 units; after a fall as after a victory). The class's progress and the codex are not reachable from a run: the hero sheet before a run shows them |

The reveal's timing is ours: the first part 0.8 s after the panel opens, then one every 0.6 s,
each counted up over 0.45 s; a talent point's line shows for 1.6 s.

| Button | Asks | Then |
|---|---|---|
| Run Again | `again` | a fresh run for the same hero in the same zone with the same run options; the panel closes when the server says `Ready` |
| Go Endless | `endless` | the run goes on into Endless; the panel closes when the HUD shows the run going on |
| Return to main menu | `leave` | the hero leaves the run and the world, back to the warband screen |

A button acts when the mouse is let go over it (`StockClick`). While a button's answer is
awaited the buttons are disabled (dimmed, their labels grey) and do nothing; a refusal lets them
work again. After an Endless run falls, a new panel opens with what the extra time paid.

**Its data.** `RunEnd` (the tally and what this end paid), the last `Ready` (who and where, for
the subtitle) and each `GearDropped` of the run (a piece's name and whether it was new; a piece
the client was not told of is named from the content, by its item entry). A run ended or reset
from the Escape menu shows no panel: its client is on its way out of the world, or a fresh run is
up at once. A `Ready` (a fresh run, or entering the world) closes the panel and forgets the run's
drops; so does leaving the world.

The client says it has this screen (`hello ... end`), so the server waits at a victory for the
player's choice instead of letting the run stand.

Escape on the end panel opens the run's Escape menu with only Options, Return to main menu and Quit Game
([Pausing](#pausing-the-escape-menu)); the panel hides while that menu,
or the Options panel opened from it, is up, and shows again as it was once they close.

Ours: the labels and the reveal's timing; the button to leave is always **Return to main
menu**; while the panel shows, the stock death popup
("release to the graveyard", which the client raises when the hero's health reaches 0) is kept
hidden, with no answer to the server (a run has no graveyard).

A fresh run sets the hero's level back to 1; the client takes any change of its level as a
level-up (benilla fires `PLAYER_LEVEL_UP` on demotions too), so the stock chat would say it
"reached level 1". In a run the stock level-up block is not printed at all ([The stock frames in
a run](#the-stock-frames-in-a-run)), so no such line shows. The level-up glow still plays on the
hero (the client's own look on any change of a unit's level); left as it is.

**Its place.** Where the player moved the panel (how far from its default place, in logical
pixels) is kept in `end_panel.ron` in benilla's state folder, written when a drag ends, as the
meter keeps its own (`meter.ron`): `(moved: (-312.5, 140.0))`. A missing or unreadable file is the
default place; the panel is kept on screen whatever the file says.

**Checking it unattended.** `ESURV_END_PRESS=again` (or `endless`, `leave`; a dev switch,
off by default) presses that button on each panel 2 s after its reveal is over, so a run with no
hand on the mouse goes through the whole loop (a warlock left alone falls in about 15 s). The
live shot refuses to shoot while the hero is dead (the death filter); `WOW_LIVE_SHOT_DEAD=1`
(benilla's, off by default) shoots anyway, for the panel shown on a fall.

## Damage numbers: Smart or All

In `esurv_app::smart_numbers`, the floating numbers of the hero's damage
over enemies (its pets' and procs' too, logged as the hero's) hide the small hits while
**Damage numbers** is **Smart**, so the big ones read in a busy fight. The game keeps every
damage number the hero floats in two running totals for the current 30 s window, the sum and the
sum of squares (no list: each floating number is one hit, so each target of a many-target spell
and each damage-over-time tick counts on its own). At each window's end it sets a threshold anew:
60 % of that window's average weighted by damage, `sum of squares / sum` (rounded up), so each
hit counts as much as it deals and a flood of small ticks and procs cannot pull it down; the
totals start again, and a number under the threshold does not float. A crit is held to the same
threshold, but crits are left out of the totals, unless the hero's crit chance is over 50 % (then
they are its usual hits and count too). The crit chance is the one the server tells the client
with the spellbook's tooltips (`SpellTips`, on each spell that can crit; none told is 0). The
threshold holds the whole next window. The first is set 30 s after the first hit (until then
every number floats), then every 30 s; a refresh with no hit in the window clears it, and every
number floats until the next one. **All** floats every number, as the stock client does. Only
the floating numbers change: the damage, the meter and the combat log are the same either way,
and numbers over the hero (damage taken; heals never float in 1.12) always show. Words (Miss,
Resist, Absorb ...) and XP always float.

The option is our own CVar `esurvSmartDamageNumbers` (`1` Smart, the default; `0` All), a row of
the Options panel's Interface tab (under Show damage numbers, the stock `CombatDamage`), kept
with the game's other settings. Its tooltip, over the row's label, says the rule in plain words.

### benilla's hook: damage numbers

benilla-app's floating text (`combat_text`) writes `DamageNumberSpawned { amount, crit }` for
each damage number it is about to float (the player's or its pet's over another unit, every one,
hidden or not), and reads the resource `DamageNumberFloor(Option<u32>)`: a number under the floor
does not float and takes none of the unit's four slots. `None`, the default, floats every one,
as the 1.12 client does; benilla itself never sets it.

## Glows round elites, rares and bosses

In a run (`esurv_app::enemy_glow`), an elite wears a **purple** outline glow, a rare and the
Rare Cache it drops a **silver** one, and a boss an **orange** one (the legendary quality's
colour); the other enemies have none. The look is the
picked hero's outline on the warband screen (above), in another colour and from the same shader
(`outline_glow.wgsl`): a thin edge along the silhouette, gear and weapons included (about 1.5
pixels at 1080 rows), a soft glow fading outward over about 7 pixels, and the same slow pulse.
Purple is `(0.74, 0.36, 1.0)` at 0.6 strength, silver `(0.92, 0.95, 1.0)` at 0.7, orange
`(1.0, 0.5, 0.0)` (`ff8000`) at 0.7, in gamma bytes.
An elite, a rare or a boss is told by the rank the server gives it in the creature query (1 elite,
3 boss, 4 rare: the portrait's gold and silver dragons and the skull), read through benilla's
`CreatureRanks`; a Rare Cache by its game-object entry (the server's `entries::RARE_CACHE`). A
player-looking enemy (Warsong Gulch's, [zones.md](zones.md)) has a player's guid, which no
creature query answers: the server tells its rank in the same numbering (`ToClient::EnemyRanks`,
`ESURVER`, as each ranked one appears and all of them after `hello`), and the client keeps the
ranks told until the next run's `Ready` (`enemy_glow::ToldRanks`). A dead one stops glowing.
Every one in sight glows, with no limit (its cost, below, is light).

Each colour has its own mask (elites mask 1, rares and caches mask 2, bosses mask 3; the warband hero has mask 0)
and a node over the window, over the world and under the run's own windows (the meter, the HUD,
the card screens). The mask holds the glowing figures alone, so a glow also shows over what stands
in front of the figure's edge (another enemy in the swarm).

The glows follow one option, **Glow around elites, rares and bosses** (the CVar `esurvEnemyGlow`, `1`
on, the default; `0` off; `esurv_app::glow_option`), on the Options panel's Interface tab:
off, every glow hides at once, as the masks are taken off and their cameras stop. The warband
hero's gold outline does not follow it.

**Its cost**: with 40 elites round the hero, measured with `ESURV_GLOW_CHECK` (the frame time on
and off in one run, [server.md](server.md), "Switches"): the frame held at the screen's refresh either way (16.67 ms; 17.6 and 17.5 ms at the 95th percentile), the main thread at 5.05 ms a frame on and 4.69 ms off, every thread's CPU together 11.4 and 9.9 ms, and the GPU's passes 2.3 and 1.8 ms, the two colour nodes' share 0.5 ms of it (benilla's FPS journal; RTX 4080 Super, Vulkan, 1600 x 900).

### benilla's hook: several masks

`SceneFigureMask(mask)`, put on a select-scene figure or on any streamed object's entity (a unit,
a game object), draws its model batches alone into mask `mask` (below `FIGURE_MASKS`, 4), each
mask through a camera of its own on a layer of its own, following the world camera's eye and optic;
`FigureMaskImage` holds each mask's window-sized picture (`None` until a figure first asks for
it). Only the masked figures are walked each frame, and those whose mask was just taken off, whose
batches go back to the world's layer alone. A mask's camera renders only while a figure is in it.
`CreatureRanks::rank(entity)` reads a unit's rank as the stock portrait does (`gated_rank`: 0
for a pet or a charmed unit and until the creature query answers, which it asks once on a miss);
`creature_rank` names the values. Stock benilla never sets a mask, and then nothing changes.

## The HUD

The run's own readouts over the stock interface (`esurv_app::hud`), every text in our
words. Laid out in the stock
interface's units (the kit's scale: a screen 768 units tall at benilla's UI scale of 0.9), so
it is as big as the stock frames at every window size, built from the stock look kit, clear of
the stock unit frames, the minimap, the action bars and the damage meter. Positions below are
units from the top of the screen.

| Part | Where (units from the top, centred) | Shows | Fed by |
|---|---|---|---|
| Clock | 8 | the run clock, `M:SS` (minutes go on past the hour), white, `GameFontNormalHuge`'s size (Friz Quadrata 20) | `Hud` clock |
| Next boss | under the clock | "Boss arrives in `M:SS`" while one is to come; "Endless mode" once Endless is on; nothing when no boss is left to come. Gold, 12 (`GameFontNormal`) | `Hud` next boss, Endless |
| Kills | left of the clock | label "Foes slain" (gold, 10), the count (white Arial Narrow 14, `NumberFontNormal`) | `Hud` kills |
| Gold | right of the clock | label "Gold", the gold (`NumberFontNormal`), and "Next cache: n" (`GameFontDisableSmall`, grey) | `Hud` gold, cache price |
| Boss bars | 62 | every boss up, oldest first (`hud::boss_stack`): the first on a wide bar (370 × 18) in the kit's status bar, a dark red, inside the stock skill bar border (`UI-Character-Skills-BarBorder`), the target frame's boss skull (`UI-TargetingFrame-Skull`) and its name (`GameFontHighlight`) at the left, `health / max` (`NumberFontNormalSmall`) and its share (`58%`) at the right, a gold tick at each health phase of its script with the phase's name over it (`GameFontNormalSmall`, gold: the `soon` line without " soon"). Under it, 16 apart, its cast or wind-up (200 × 11, the stock casting bar's colour and border, its icon at the left, `label` and time left); the room is kept while it does not cast. Every other boss below on a thinner bar (310 × 13) of the same look. A bar goes as its boss dies. The boss warnings move down under the stack (`boss_alerts::warnings_top`) | `BossBar`, `BossCast` |
| Banners | 292, two lines, newest on top | a boss arrives (orange, the stock raid warning's colour), the final boss arrives (red), a boss falls (gold); a rare comes and falls with none. Friz Quadrata 20 (`GameFontNormalHuge`). Each holds 10 s, then fades over 3 s: a stock message frame's timing, as `RaidWarningFrame` keeps it | `Banner` |
| Toasts | 352, three lines, newest on top | the server's HUD messages (magnet, runes, a cache that needs gold, leaving the arena) in the stock info yellow, Friz Quadrata 16 (`ErrorFont`). Each holds 5 s, then fades over 3 s: `UIErrorsFrame`'s timing | `Toast` |
| Start hint | 292 | before the first step: "Take a step to begin the fight.", in gold, pulsing (strength \|sin(4.5 t)\| × 0.45 + 0.55) | `Hud` phase `waiting` |
| Escape menu | the middle of the screen | see below | `Hud` phase `paused` |

The stock centre lines keep their places between the readouts and the banners: the error and
info line (`UIErrorsFrame`, 122 to 182, where the cache's "needs gold" notice also shows, as the
server sends it both ways) and, below it, the raid warning and the centre-screen lines
(`RaidWarningFrame`, `RaidBossEmoteFrame`, 192 to 282) where the run's centre-screen lines show
(a boss's arrival, an elite's, a swarm ...). Banners and toasts sit below them.

A line that comes again while it is still up moves to the top and starts over rather than
showing twice (ours: the server repeats a cache's "needs gold" message every few seconds while
the hero stands on it).

The HUD shows while the hero is in the world and the server has said `Ready` (a server that
does not speak the protocol shows none of it); `Ready` starts it afresh (no boss bar, no
banners), and leaving the world forgets the run. The server sends the HUD when it changes, a new
phase at once and otherwise at most every 0.2 s; the HUD shows the newest.
A HUD line for another hero is not ours (a run with others: each client is told its own).

**Chat bubbles keep off the readouts**. A boss's shout raises a chat
bubble over its head, which can rise to the top centre of the screen. The HUD draws over the
bubbles already (`bevy_ui` comes after benilla's interface pass), but a bubble's own text behind
the clock made it hard to read. So while the readouts and the boss bar show, their screen areas
(each with 2 pixels of room) are handed to benilla's `BubbleKeepClear`, and a bubble that would
overlap one is moved straight down until its top sits under it, its tail with it: the readouts
first, then the boss bar, so a bubble moved under the clock goes on under the bar. The bubble
still follows its speaker; only its height on the screen changes while it would cover them.

### The minimap buttons

Two round buttons on the stock Minimap's rim in a run, built as an
addon's minimap icon is (`esurv_app::minimap_buttons`, its Lua `minimap_buttons.lua` in the
interface VM): a 20 × 20 icon (its edge trimmed, `SetTexCoord` 0.075 to 0.925) under the stock
tracking button's ring (`Interface\Minimap\MiniMap-TrackingBorder`, 52 × 52), the zoom
buttons' highlight (`UI-Minimap-ZoomButton-Highlight`) under the pointer, 80 units from the
minimap's middle on its lower left (200° and 225°, counting anticlockwise from the right),
clear of the stock tracking, mail, clock and zoom buttons. They are stock frames, so the stock
mouse handling clicks them as it clicks the minimap's own buttons, and a click on them never
reaches the world.

| Button | Icon | Tooltip | A click |
|---|---|---|---|
| Boss timer bars | `INV_Misc_PocketWatch_01` | "Boss timer bars: show/hide drag handles", then "Drag handles shown" or "Drag handles hidden (hold Shift to drag)" | flips `esurvBossBarHandles`, the same setting as Options, Interface, "Show bar handles": show the handles, drag the bars, hide them again. Kept as that CVar is |
| Damage meter | `INV_Sword_04` | "Damage meter: show/hide", then "Shown" or "Hidden" | shows or hides the meter window; kept in `meter.ron` (`hidden`) |

A click takes hold the same frame (the game reads the buttons' clicks every frame, not only on
its once-a-second read of the Options CVars). `ESURV_MINIMAP_SHOTS=<stem>` is the unattended
check: it points at each button and clicks it twice through the interface's pointer calls,
with a shot of each step.

### benilla's hook: areas a chat bubble keeps out of

`benilla_app::BubbleKeepClear(Vec<Rect>)`: screen areas in logical pixels from the window's top
left. Each frame, a bubble whose frame overlaps an area is moved down by whole pixels until its
top sits on that area's bottom, the areas tried in their order. Empty by default, so benilla's
bubbles are untouched unless a crate on top fills it (`chat_bubble.rs`, `keep_clear_of`).

### benilla's hook: looks that fly level

`benilla_app::LevelFlight(HashSet<u32>)`: creature looks (`CreatureDisplayInfo` ids) that keep
upright on a flying path. The stock client tilts a unit on a flying spline into its flight
attitude (pitched by the climb, banked by the turn ahead), as a taxi flies; a listed look only
turns to its path's facing. Empty by default, so stock flights are untouched. The game lists
every pet that flies (`hover` in `pets.ron`, `esurv_app::level_flight`, read once at launch): the
server keeps such a pet on short flights that bob it at its height ([server.md](server.md)), and
the stock attitude pitched it with each bob, nearly straight up or down on a flight that barely
moves ahead, so it showed flipped over.

### Pausing: the Escape menu

See [menus.md](menus.md), "In a run: the Escape menu". Once a run has
started, Escape pauses it and opens a menu of five buttons. In a run the menu is the game menu,
and the stock game menu never shows: before the first step and on the
end panel Escape opens our menu too, with only the buttons that fit there (below). benilla's
`GameMenuStandIn` (a resource the HUD sets while the hero is in a run, but over a choice screen)
stands in for the stock one: Escape first does what the stock Escape does (a focused chat line
lets go of it; a popup, the options, a dropdown, a cast, spell targeting or an open window is
closed or stopped, one per press), but where the stock ladder would go on to clear the target or
open `GameMenuFrame`, the HUD gets a `GameMenuAsked` instead; the main menu button closes the
dropdowns and windows and asks too. The target is not cleared: the stock client selects whoever
hits a hero that has no target, so in a swarm the hero nearly always has one, and Escape would
never reach the pause. The Lua half is the `ToggleGameMenu` wrapper in benilla's
`GameMenuAdapters.xml`, which reads `BENILLA_GAME_MENU_STAND_IN` and counts the asks in
`BENILLA_GAME_MENU_ASKED`.

| The run | Escape (or the menu button) |
|---|---|
| waits for the first step | opens the menu on the client alone, headed "Game Menu" (there is no run to pause: the server refuses `pause` before the first step): **Continue** (closes it), **Options**, **End Run**, **Quit Game**; a second Escape closes it. A step taken under it starts the run and the menu goes (with the Options panel, as Cancel) |
| runs | asks `pause`; the menu opens once the server says the run is paused |
| is paused (the menu is open) | asks `resume`, as Continue does |
| has the Options panel open over the menu (any of its kinds) | closes the panel as its Cancel does, back to the menu; the run stays paused (with the stock key binding window open from the panel, that window takes Escape first and closes, back to the panel) |
| has a choice screen open | no pause: the stand-in is off (the choice screen claims Escape for itself through `OverlayKeys`, and does nothing with it) |
| is won or lost (the end panel) | opens the menu on the client alone, headed "Game Menu", over the end panel (which hides under it): **Options**, **Return to main menu** (`leave`, as the panel's own) and **Quit Game**; no Continue or Reset Run, which would cross the panel's Run Again and Go Endless. A second Escape closes it, back to the panel. Go Endless pressed elsewhere (another hero) or a fresh run closes it |

The rules are pure functions in `hud::escape_menu` (`answer_escape`, `menu_kind`,
`MenuButton::press`): the menu opened on the client is the phase it was opened in (`MenuHere`),
forgotten once the phase moves on, a fresh run comes or the hero leaves the world.

One ask at a time: a second Escape or button before the server answered (a new phase, a fresh
run, the hero out of the world, or `ESURVN` refusing it) does nothing.

The menu shows while the server says the run is paused (or while it is open on the client, above),
centred on the screen, over the HUD and the end panel. Its look is
apart from its rules (`esurv_app::hud::escape_menu` holds the buttons and what they ask,
`escape_menu_look` how they are drawn), so the menu rework's art style replaces only the look.
It wears the menus' own look ([The menus' look](#the-menus-look)): a
near-black panel 230 units wide with the thin gold border, the heading "Run Paused" in Morpheus
small capitals between its gold rules, the hint "Press Escape to fight on.", and five of the
look's framed buttons 170 units wide (Continue the gold-framed call to act), the ways on and
Options above, the ways out set apart below:

| Button | Asks | Then |
|---|---|---|
| Continue | `resume` | the run goes on; the menu goes |
| Reset Run | `reset` | the run ends here, paid out as a run left (the chat says what it earned; no end panel), and a fresh run in the same zone with the same hero starts without leaving the world, as **Run Again** starts one ([server.md](server.md), "Again and Reset"). Refused while another hero is still in the run: a toast says "The run cannot be reset while another hero is still in it." |
| Options | nothing (the client's own) | the Options panel of the warband screen opens over the paused run (below); the menu hides under it and comes back when it closes |
| End Run | `leave` | the run ends here, paid out as a run left, and the hero leaves the world: back to character select (the warband screen, with its camp once the hero picker is in) |
| Quit Game | `leave` | the same, then the game closes once the hero is out of the world (at once if the server says it had left already): the server has paid out and written the save before it lets the hero out, and the client closes through the app's own exit (`AppExit`), which runs benilla's shutdown savers |

The armory and the codex are not on the menu, nor reachable in a run at all:
the hero sheet before a run will show them ([The class's progress and the
codex](#the-classs-progress-and-the-codex)).

**Options over the run** (`esurv_app::menus::over_the_run`):
the Options button (on each of the menu's kinds) opens the very panel of the warband screen ([The warband
screen](#the-warband-screen), Options: Video, Sound, Interface, Saves; `menus::options`) over
the paused run, in the menus' look, drawn by the menus at their layer over the world and the
stock interface. The run stays paused under it (the panel asks the server nothing), and the
Escape menu hides until it closes, as the stock game menu hides under its options windows. It
reads and writes the settings through the same calls as before a run, on the world session's
Lua VM. What differs over the run:

* Okay, Cancel and Escape close it, back to the Escape menu. What Okay leaves for later the
  menu says in a gold line under its buttons until the run goes on ("Some changes take hold
  when a hero next enters the world (End Run, then a new run)."; `Menus::run_notice`); the lines
  of the settings that take hold when the world loads say "next time in the world".
* The Saves tab exports only: an import would replace the save the run is paid out into, so
  **Import save** is greyed and a line says "Import works from the warband screen, before a
  run." (`SavesPage::in_run`; an import press is refused there too).
* **Key Bindings** (beside Okay and Cancel) opens the stock key binding window, loaded on demand
  the stock game menu's way (`KeyBindingFrame_LoadUI`, `ShowUIPanel(KeyBindingFrame)`; the run's
  frames, `run_frames.lua`, do not block it). The panel waits hidden under it and comes back when
  it closes (its Okay, its Cancel, or Escape, which the window takes for itself). The window's
  own `OnHide` would open the stock game menu, which a run does not have: while it was opened
  from the panel, that one step is left out (its line still cleared and the micro buttons
  updated); opened any other way, its `OnHide` is the stock one.
* Should the run go on under the panel (another hero resumed it, a step started it) or the hero
  leave the world, the panel closes by itself as Cancel closes it.

With the menus off (`ESURV_MENUS=0`, a dev aid) the menu has no Options button.

**Checking it unattended.** `ESURV_ESCAPE_AT=<seconds>[,<seconds>...]` (a development aid)
presses Escape inside the game, a key event of the game's own and no key of the operating
system: the first when the run clock reaches the first time, each later one that many real
seconds after it (the clock stands while paused). The press goes the whole stock way, so a live
check can pause and resume a run with nobody at the keyboard: `ESURV_ESCAPE_AT=15,21`
pauses at 0:15 and resumes 6 s later.

`ESURV_MENU_PRESS=<button>@<seconds>[,...]` (a development aid; `continue`, `reset`,
`options`, `end`, `quit`) presses a menu button: for each item in turn, once the run clock of a running run
reaches its time, Escape is pressed the same way, and once the menu has shown 1.5 s its button
is clicked through the kit's own `StockClick`, as the mouse would. The next item waits for what
the last led to (the run going on, the fresh run, the hero out of the world, the Options panel
open over the paused run), so
`reset@20,end@15` resets the run at 0:20 and ends the fresh one at 0:15. If the menu does not
come within 2 s (the loading screen still up, a choice screen came between), Escape is pressed
again once the run runs.
`ESURV_MENU_SHOTS=<png>` adds the game's own shots of each step, never raising or pinning
the window: `<stem>-<n>-menu.png` just before the click, and `<stem>-<n>-<button>.png` after
what it led to (1.5 s into the run going on or the Options panel open, 4 s into the fresh run,
8 s after leaving the world for character select). The menu shot of an Options press already
shows the panel: the click and the shot fall in one frame.

The menus' script (`ESURV_MENUS_SCRIPT`, [Checking the menus
unattended](#checking-the-menus-unattended)) goes on over the run: `over-run` waits until the
Options panel is open over the paused run, `escape` presses Escape inside the game the same
way, and the panel's presses (`options-tab:<tab>`, `option:<setting>:<+|->`, `okay`, `cancel`,
`export-save`, `key-bindings`, and `game-options` to open it again) work there as before a run.
A run starts unattended with `ESURV_KEY_PRESS=w@8` (a step, 8 s into the world). For
example `ESURV_MENU_PRESS=options@3` with
`ESURV_MENUS_SCRIPT=ready;hero:Grumak;start;over-run;shot:video;key-bindings;wait:2;shot:keys;escape;wait:2;cancel;wait:2;shot:menu;quit`.

## The class's progress and the codex

The armory (what a class has earned between runs: talents, gear and
looks) and the codex are not reachable during a run; the hero sheet before a run shows the
class's progress ([The hero sheet](#the-hero-sheet): its Character, Collection and Talents views).
The codex is on the warband screen ([The warband screen](#the-warband-screen)).
There are no in-run windows and no buttons for them on the pause and end panels (in a
run N opens the stock talent frame, read-only, and L nothing: [The stock frames in a
run](#the-stock-frames-in-a-run)); their drawing code stays in `esurv_app::hero_sheet`,
unit-tested, for the hero sheet to draw:

| Module | What it holds |
|---|---|
| `progress` | `HeroSheetState`: the server's `Progress` (sent after `hello`, after each change and with a fresh run), `Ready` and the run options' bonus; the tab, the piece and the zone picked; the changes asked (`learn`, `unlearn`, `wear`, `takeoff`, `look`), one at a time until the server answers (a new `Progress`, or `Refused`), and the refusals in our words |
| `talents` | a class's three trees under `esurv_sim::game::talents` (the server's own rules): each talent's rank, whether a rank can be learned or taken back, how it shows (open, at its last rank, not open yet), its tooltip |
| `gear` | the class's pieces by slot and zone (each zone's own names and looks, Blackrock's two rows), what is worn and which look each slot shows, what worn gear adds stat by stat, the drop tables (each boss's pieces per zone, at the tier's chance times the run options' bonus), a piece's tooltip |
| `item_icons` | a piece's icon from the player's own `ItemDisplayInfo.dbc`, else its slot's empty picture |
| `codex` | the codex's lists (rings, trinkets, amulets, and a remade legendary amulet for every class spell whose delivery kind has a recipe in `legendary_amulets.ron`), how many of each are found, the pages (12 pieces a page; the legendary amulets a class a page, the hero's class first), and `CodexState`: the server's `Codex` (`ESURVK`) with the list and page shown |

The codex keeps every ring, trinket and amulet offered during play (seen on a card, taken or
not, as the reference's codex), to any hero of the save, for
good. Before a run the hero sheet
reads and changes them through the app's `prerun::PreRunLink` (the same `Progress`, `Codex` and
armory requests, as asks of their own: [server.md](server.md), "Before a run").

What the in-run windows showed, kept as the spec of what the hero sheet shows (its layout is the
menu rework's), the class's progress by tab:

| Tab | Shows | Does |
|---|---|---|
| **Character** | The paper doll as the stock character pane lays it out: head, shoulders, chest and wrists on the left, hands, waist, legs and feet on the right, the weapons below. Each slot shows the worn piece's icon (its item display's icon from the player's `ItemDisplayInfo.dbc`; the stock `UI-PaperDoll-Slot-*` picture when nothing is worn), its name in its quality colour (its tier, common to legendary) and the look it shows ("Look: its own", the chosen piece, or "your own clothes"). Between the columns: the class's name in its class colour, what all worn gear adds stat by stat in green (or that nothing is worn yet, and that bosses drop the class's gear for good), pieces collected, talent points spent of earned, zones open | the slots' tooltips: the piece, its slot, what it adds, worn or not, the look shown |
| **Collection** | "n of m pieces collected · kept for every Warrior hero", then a grid: a column per slot, a row per zone's look (its name in its quality colour; Blackrock a second row, "Second look"; Stratholme a second row by its name, "Scholomance sets"); a won piece's icon in its row's look in colour, one not won yet dark; **E** on the worn piece (its zone's first row), **L** on the look its slot shows | a click picks a piece in its row's look: its icon, name and slot, what it adds (a weapon also the three loadouts' weapons in the look), worn / in the collection / not won yet, the bosses that drop it, and the buttons **Wear** (`wear`) or **Take Off** (`takeoff`) and **Show This Look** (`look <slot> <look>`, a place in the class set's looks) or **Show Own Look** (`look <slot> ~`); a button that cannot act is disabled |
| **Drop Tables** | One zone a page (the page arrows go through them; the first page is the hero's zone): its name, its number, its gear tier and each piece's drop chance (the tier's chance times the run options' bonus); a locked zone says which zone to win, and its final boss, to open it. Then each boss in arrival order (its time, the final boss marked) with the class's pieces it can drop: icon, name in its quality colour, **Owned** in green or **Still to find**. At the foot: how many pieces the open zones still hold for the class, or that they hold none and the next zone waits for a win | the pieces' tooltips |
| **Talents** | Talent points to spend, earned of 51, spent; the stock status bar in the experience bar's purple toward the next point ("Next point: into / cost", or every point earned); how to learn and take back. Then the three trees side by side, each with its name and points spent, its stock art (`Interface\TalentFrame\<tree>-TopLeft`, `-TopRight`, `-BottomLeft`, `-BottomRight`, in the stock frame's shape, 320 by 384, at 250 by 300 units; its top 332 rows show, as the stock frame shows them) and its talents on a grid of three columns and seven rows, drawn as the stock talent frame draws them: the icon on `UI-EmptySlot-White`, the rank in `TalentFrame-RankBorder`, green while a rank can still be bought, gold at the last rank, the icon dimmed and the slot grey while its row or the talent it needs is not there yet | a click learns a rank (`learn`) when one can be bought; a right-click takes one back (`unlearn`) when every rank left still meets the rules; both under `esurv_sim::game::talents`, the server's own rules. The tooltip, as the stock talent tooltip: name, rank n/m, what it still needs in red ("Requires 10 points in Arms Talents", "Requires 5 points in Deep Wounds"), what it does at its rank in gold, the next rank's, a line for a milestone, and what a click and a right-click do |

A talent's text shows each `{v}` with the number of the effect in its place (the first `{v}` the
first effect's, and so on), times the rank (rank 1 before any is bought), as a percentage where a
`%` follows it; a milestone's `{v}` is the number its rank grows, or its count where nothing grows.

The codex, by list:

| Part | Shows |
|---|---|
| Under the header | how many pieces of every list are found in all |
| Tabs | **Rings**, **Trinkets**, **Amulets**, **Legendary**, each with how many of it are found (`Rings 3/9`) |
| A list's head | its name, how many are found, the page (12 pieces a page, in two columns); the legendary amulets go a class a page (the hero's class first): "Warrior Legendary Amulets", found of the class's and of every class's |
| A piece found | its icon, its name (an amulet in its rarity colour, a ring or trinket in white) and its text from the content (a ring's or trinket's at its first value; a remade legendary: "Remakes Whirlwind, far stronger.") |
| A piece not found | its icon dark, its name grey, and where it comes from: caches (bought ones and rares' free ones); for a legendary amulet, a cache's legendary pick for a spell the hero knows |

## The stock frames in a run

This is design step 6's patch of the stock interface. While the hero is in
the world, which in this game is always a run (waiting, running, paused, or over on the end
panel), the stock interface keeps three of its windows and nothing else; character select is
untouched.

| Stock window | Opens with | In a run |
|---|---|---|
| Character pane | C, its micro button | on its Character tab only (the Pet, Reputation, Skills and Honor tabs are hidden and their keys open nothing); its stat rows show the run's numbers (below); its slots hold the class gear the hero wears, each piece an item of the hero's own with its icon (the look shown) and, in its tooltip, its name in its quality colour; the neck, finger and trinket slots hold the amulet and the first two rings and trinkets the run gave (their icons, names, rarity colours and what each does now, in the tooltip's quoted line), each replaced as it is upgraded or empowered ([server.md](server.md), "What the hero wears in a run"); the model wears what the hero shows in the world |
| Spellbook | P, its micro button | one tab, **General**, holding exactly the spells the server says are known: the race spell, the race's passives (under their stock racial spells) and the spells the hero learned in this run ([server.md](server.md), "The spellbook"); no other side tab, no pet tab; its own look |
| Talent frame | N, its micro button (shown at any level) | read-only: the class's bought talents of our own trees (`talents.ron`, the ranks from the class's progress), each in its place on its tree's stock art, with the stock branch arrows between bought talents, green while a rank is left, gold at the last; "Talent Points: 0"; a click learns nothing; the tooltip is ours: name, rank, what it does now (gold), the next rank's, a milestone's line, and "Talents are learned and taken back on the hero sheet, before a run." |

**The buff frame** (top right) shows each buff's time left under its icon ("10 s", "1 m") and
blinks it near the end: the stock option "Show Buff Durations" (`SHOW_BUFF_DURATIONS`, off in the
stock interface) is turned on as the hero enters the world
(`esurv-app` `buff_frame`). The time is the server's (server.md, "The hero's buffs"), read
by the stock frame through `GetPlayerBuffTimeLeft`. A buff's tooltip is still the stock one of
the spell whose look it wears.

**Blocked**, keys, buttons and every other way in (a slash command, an event): the quest log
(L), the social window (O, and its Friends, Who, Guild and Raid keys), the world map (M), the
reputation (U), skills (K), honor (H) and pet (Shift-P) tabs, the bags (B, F8 to F12, Shift-B)
and the keyring, the pet spellbook (Shift-I), the battlefield minimap (Shift-M) and the
battleground scores (Shift-Space), the macro window (`/macro`, and the stock game menu's Macros
button on the end panel) and the help window. The stock game menu never shows in a run: the
Escape menu stands in for it from the first moment in the world to the end panel
([Pausing](#pausing-the-escape-menu)), and the stock options frames
are reached only through our Options panel (its Key Bindings button opens the stock key binding
window).

**Hidden**: the micro buttons for the quest log, the social window, the world map and help; the
backpack, bag and keyring buttons; the main bar's buttons 9 to 12 and its latency bar
([The action bar](#the-action-bar)); the minimap's world map button and its battleground button.
The main menu's micro button stays: a click on it opens the Escape menu, as Escape does. The
minimap's zoom buttons, its
tracking icon, its day and night dial and its own close button stay: they open no window.

**The run's numbers on the character pane** (design step 6). Once the server's numbers line has
come (`ESURV`, [server.md](server.md), "The character pane"), read through the addon message
event, the pane's twelve stat rows show the hero's run numbers in place of the stock attributes,
in our words, and the resistance icons give way. A value worn gear adds to shows green (as the
stock pane shows a bonus); each row's tooltip says what the number does and, in green, what worn
gear adds to it.

| Stock row | Label | Value | Its tooltip adds |
|---|---|---|---|
| Stat 1 | Health | max health | |
| Stat 2 | Damage | `+n%` over the start (damage − 1) | it grows with levels, talents, gear, jewelry, buffs |
| Stat 3 | Cooldowns | `−n%` (the cooldown factor − 1) | every spell is ready again after n% of its cooldown |
| Stat 4 | Crit | crit chance, one decimal | past 100% it turns into crit damage |
| Stat 5 | Crit Power | crit damage, `n%` of a hit | |
| Armor | Reduction | damage reduction | its cap (the sim's, 60%) |
| Melee attack | Regen | health a second, one decimal | health a second to two decimals, heal per kill when there is one |
| Melee power | Area | `+n%` (area − 1) | |
| Melee damage | Targets | `+n` extra targets | |
| Ranged attack | Double Cast | double cast chance | past 100% every spell goes twice |
| Ranged power | Speed | `+n%` (move speed − 1) | |
| Ranged damage | Pickup | pickup range in yards | experience, gold and luck as `+n%`, and the gold worn gear adds |

Our layout and texts. Outside a
run, or before the line comes, the pane is the stock one.

**The run's level cap.** The stock interface's level cap (`MAX_PLAYER_LEVEL`, 60), which the
experience bar and the reputation bar read, is the run's own in a run (`esurv_sim`'s
`level::MAX_LEVEL`, 99), so the experience bar keeps filling past 60 and gives way only at the
run's cap; the stock one again outside a run.

**The stock level-up block.** The client prints the stock level-up lines ("Congratulations, you
have reached level n!", the hit points gained ...) on every change of the hero's level field.
The server sends no `SMSG_LEVELUP_INFO`: what a level brought comes
as the server's own system line ([server.md](server.md), "Lines"), and the stock block would say
"0 hit points"; and a fresh run (Run Again, Reset Run) sets the level back to 1, which the stock
block would call a level-up. So in a run the chat frame does not print the stock block at all;
everything else that listens for the level change (the experience bar) still hears it.

**Right-click menus** of the player, target, party and pet frames open nothing (their entries,
loot, party, trade, duel, inspect and raid marks, have no place in a run); a left click still
targets. The chat's own menus stay.

**Spell tooltips**. The action bar and the spellbook hold each spell
as the stock spell it looks like, whose stock tooltip would say what that stock spell does. In a
run, hovering a spell on the action bar or in the spellbook shows ours in the stock tooltip frame
(`GameTooltip`, its look unchanged), laid out as the stock spell tooltip lays one out:

| Line | Left | Right |
|---|---|---|
| 1 | the spell's name (white) | its rank in this run, `Rank n` (grey); `Racial` for the race's spell |
| 2 | its reach: its range, `30 yd range`, else the radius it covers, `9.7 yd radius` (none: the school alone on the left) | its school, `Fire spell` |
| 3 | `Casts itself` | its cooldown for the hero now, `1.8 sec cooldown` |
| 4 | its description, gold, wrapped: the spell's template in `abilities.ron` filled with the hero's numbers now, as a card fills it (`esurv_content::text`; the card's own `spell_shown`) | |
| 5 | the race's spell only, green: `<Race> racial: it casts from a slot of its own, and cards never upgrade it.` | |
| | an empty line, when any of the three below follows | |
| 6 | white: `Upgrades this run: +8% damage, +18% area, +15% duration`: what the upgrade cards taken for the spell add up to, by kind (the small power each rank brings counts with its power) | |
| 7 | green: `Your bonuses: +31% damage, -4% cooldown, +3% area`: what the hero's talents, worn gear, jewelry, amulet and race add to the spell's numbers | |
| 8 | white: `7% chance to crit for 204% damage`: the hero's crit chance and crit damage now, for a spell whose hits can crit | |

Lines 6 to 8 show only when they have something to say: a kind only when it moved the spell's
numbers (by half a percent, or a target). Their parts are the cards' own upgrade lines
(`words.ron`'s upgrade words, `cards::lines::upgrade_line`: "+34% damage", "-7% cooldown", "+1
target", a heal's or a shield's power in their words), joined with commas. The totals are the
sim's (`esurv_sim` `game::spell_numbers::spell_totals`, sent with the tooltip): each is how
much a kind moved the spell's own numbers (`spell_numbers`), the upgrades measured on the slot
without and with its cards, the bonuses on the slot with its cards and the bare stat sheet of the
class at the hero's level (no modifier), against the slot with its talent and amulet modifiers
and the stat sheet with the hero's modifiers (talents, gear, jewelry, amulet, race; no buff of the
moment, nor the level's own growth). Power is read on one tick of what lasts, else the damage,
heal, shield or aura; the cooldown is the share it shrank by; area the radius; targets how many
more. Hits crit on every damaging delivery (`roll_crit`), but an effect over time's ticks and
what a spell does on the hero (heals, shields, buffs, an aura's strikes back) never do, so those
have no crit line (`can_crit`).

A race's passive (under its stock racial spell) shows its name with `Passive` (grey), `<Race>
racial` (green) and the race's line for it (`races.ron`, gold: "Maximum health +10%."). The
spellbook writes our name under each icon and, under it, `Rank n`, `Racial` or `Passive`. The
numbers follow the run (a rank, a talent, the amulet, a level, a buff): the server sends them
whenever they change (`ESURVL`, [server.md](server.md), "The spellbook"), and a tooltip left up
refreshes itself as a stock one does. A spell the server has said nothing of, the pet's book,
items and macros keep the stock tooltips. Elsewhere a run's spell names are ours already (the
meter, the chat lines, the cards); the cast bar shows the look's name.

**Buff tooltips**. Each of the hero's buffs wears the look of a stock
spell's aura, whose stock tooltip would say what that stock spell does. In a run, hovering a buff
on the buff frame at the top right (and on the target frame while the hero targets itself) shows
ours in the stock tooltip frame:

| Line | Left | Right |
|---|---|---|
| 1 | its name: the spell of ours that cast it (the race's spell or a learned one), else the word table's for its kind (`words.ron` `buffs`: "Speed Rune", "Berserker Rune", "Untouchable" for a milestone's immunity ...) | `Racial` for the race's spell, `Rank n` for a learned one (grey) |
| 2 | what it does now, the word table's line for its kind with its value: "+60% movement speed", "+42% damage" (grown by the hero's buff power), "Absorbs 54 more damage" (what the shield has left), "+3.5 health per second", "Nothing can hurt you", "Whatever hits you takes 18 damage back" | |
| 3 | the casting spell's description with the hero's numbers now, gold (from its spellbook tooltip) | |
| 4 | the race's spell only, green: `<Race> racial` | |
| 5 | the buff frame only: its time left, gold: "8 sec left", "2 min left" (`GetPlayerBuffTimeLeft`, from the aura's duration) | |

An effect of one of the hero's own spells on an enemy (an effect over time; a slow shows no aura),
hovered on the target frame's debuffs, shows that spell's name and school and its description with
the hero's numbers. An aura of a spell the hero does not know (another hero's, an enemy's) keeps
the stock tooltip.

How: all of it is Lua of ours in the interface (`esurv_app::run_frames`, `run_frames.lua`),
run once in each interface VM once the stock FrameXML is loaded, acting only while
`ESURV_RUN_FRAMES_ON` is set (the module keeps it to whether the hero is in the world). It
wraps the stock toggles the key bindings and buttons call (`ToggleQuestLog`, `ToggleCharacter`,
`ToggleSpellBook`, `ShowUIPanel` ...), keeps the hidden buttons hidden through their `OnShow`,
gives the spellbook one tab (`GetNumSpellTabs` 1, `GetSpellTabInfo(1)` from the first spell over
all of them: the book's tabs follow one another, so its order stays), and answers the talent
frame's questions (`GetTalentTabInfo`, `GetTalentInfo`, `GetTalentPrereqs`, `LearnTalent`,
`UnitCharacterPoints`, each talent's tooltip) from the pages the module pushes: the class's
trees from the content with the ranks from the server's `Progress`. Our trees fit the stock
frame (three columns of its four, seven tiers, the stock tree art by name), so the stock frame
shows them as they are; no benilla change. For the character pane it follows the stock pane's
own fills (`PaperDollFrame_OnShow`, `PaperDollFrame_OnEvent`) with ours, gives each row its own
`OnEnter`, and listens for `CHAT_MSG_ADDON` with the prefix `ESURV` on a frame of its own; the
module also hands it the last numbers line when it is installed (the first can come before it)
and the run's limits (`set_limits`: the level cap, the most damage reduction). The level cap is
`MAX_PLAYER_LEVEL` set while the run is on (the bars told at once), and the level-up block a wrap
of `ChatFrame_OnEvent` that drops `PLAYER_LEVEL_UP` while the run is on. The spell tooltips
(`run_frames::spell_tips`) are the module's lines for each spell of the server's `SpellTips`,
handed over by `Spell.dbc` id (`set_spell_tips`) with which spell each action slot and each
spellbook slot holds (`set_spell_slots`); the Lua answers `GameTooltip:SetAction` (the action
buttons' `ActionButton_SetTooltip`), `GameTooltip:SetSpell` (the spellbook's
`SpellButton_OnEnter`, the player's book only) and `GetSpellName` (the spellbook's name and rank)
for those spells while the run is on, and passes everything else to the stock ones. 1.12's Lua
has no call that names an action slot's spell, so benilla has one small hook for it, on the Rust
side only (`UiScript::action_spell`: the spell a slot holds, as `SetAction` renders it; no Lua
global); the spellbook's slots come from `UiScript::spellbook`. The buff tooltips
(`run_frames::buff_tips`) are the module's lines for each buff of the server's `Buffs` (`ESURVD`,
[server.md](server.md), "The buff frame") and for each of the hero's spells as it shows on an
enemy, handed over by the aura's `Spell.dbc` id (`set_buff_tips`) with which aura each of the
player's buff positions, each of its buffs and each of the target's debuffs is
(`set_aura_slots`); the Lua answers `GameTooltip:SetPlayerBuff` (the buff buttons),
`GameTooltip:SetUnitBuff` (while the unit is the hero) and `GameTooltip:SetUnitDebuff` (the
target's, while it is not the hero) for those auras while the run is on. The player's buff
positions come from a second small benilla hook on the Rust side (`UiScript::player_aura_spell_ids`:
the player's auras' spells by the position `GetPlayerBuff` answers), the target's from
`UiScript::aura_spell_ids`.

**Checking it unattended.** `ESURV_KEY_PRESS=<key>@<seconds>[,...]` (a development aid; a
letter, a digit or `escape`) presses keys inside the game, key events of the game's own and no
key of the operating system, that many real seconds after the hero entered the world; each goes
the whole stock way (the interface, the key bindings); `<key>@<seconds>:<hold>` keeps it down
that many seconds (`W@4:7` runs forward), one key down at a time. `ESURV_KEY_SHOTS=<png>` adds the
game's own shot 1.5 s after each press, `<stem>-<n>-<key>.png`, never raising or pinning the
window.

`ESURV_TOOLTIP_SHOT=<where>:<n>@<when>[,...]` (a development aid) shows a spell's tooltip
with nobody at the mouse, the stock way the pointer would (`ActionButton_SetTooltip`,
`SpellButton_OnEnter`, a buff button's `SetPlayerBuff`), and leaves it up: `action:<n>` is button
`n` of the main action bar, `book:<n>` the spellbook's `n`-th spell (the book opens first),
`buff:<n>` the buff frame's `n`-th buff; `<when>` is real seconds after the
hero entered the world (`@34`), or `rank<r>` (`@rank2`), once the spell that button holds has
reached rank `r`. The items go in turn. `ESURV_TOOLTIP_SHOTS=<png>` adds the game's own
shot 1 s after each, `<stem>-<i>-<where><n>.png`. With the card script's `take:upgrade` (the
first card that upgrades a known spell, [The card screens](#the-card-screens-level-up-cache-and-rares-cache)),
`action:1@34,book:1@38,action:1@rank2` shows the first spell at rank 1, the race's spell, then
the first spell again once a level-up upgraded it. With `ESURV_GRANT=speed_rune,berserker_rune`
(both runes at the hero's feet as the run starts) and `ESURV_AUTOSTART=1`,
`buff:1@6,buff:2@8,buff:3@10` shows the runes' buffs and the race spell's.

### The action bar

In a run the stock main bar (`MainMenuBar.xml`, 1024 units wide) is laid out compact and
centred, 494 units wide, from its own art (`esurv-app` `run_frames::action_bar`,
`action_bar.lua`, switched with the run frames); out of a run every piece goes back where the
stock file put it. Left to right:

| Part | In a run |
|---|---|
| Gryphons | the stock end caps, one over each end, overlapping it by 32 as in the stock bar |
| Buttons 1 to 7 | the hero's spells, learned spell `n` on button `n` (keys 1 to 7): seven, as rogues and druids need; a spell past the seventh has no button and still casts |
| Button 8 | the race spell (key 8), the server's `RACE_SPELL_SLOT` ([server.md](server.md), "The action bar") |
| Page arrows and number | as the stock bar has them, though only page 1 holds spells |
| Micro buttons | Character (C), Spellbook (P), Talents (N) and the main menu's (the 1.12 computer button), which opens the run's Escape menu as Escape does ([Pausing](#pausing-the-escape-menu)) |

Left out: buttons 9 to 12 (hidden, whatever shows them), the bag slots' end of the art (the
art's last quarter) and the latency bar. The art is the stock strips cut together: the first
quarter whole (the bar's left end, buttons 1 to 6), the second up to button 8 and its last 6
units (the pillar before the arrows), then the third from its start (the arrows, the page
number, the micro buttons) to the bar's end, under the right gryphon.

The experience bar over it (`MainMenuExpBar`, and the rail that stands in for it at the level
cap) is the bar's width, in **ten segments** of a tenth each, not the stock twenty of 5 %: the
stock art's first and last strips (five segments each, the bar's two ends), half the width
each. The exhaustion tick reads the bar's width, so it stands at the right place.

### The power bar

The player frame's power bar shows what the hero's starting option
spends or counts, with only its count written on the bar ("3 / 5", as the health bar writes its
own) and its name on a small tab under the bar ("Holy Power"; rage and energy take the client's
own word): a tooltip's dark ground and thin edge, the name in the small gold font, under the bar's
middle, or under its right end when combo pips take its left. The server tells it as it changes
(`ESURVY`, [server.md](server.md)):

| Bar | Shows |
|---|---|
| Rage (Warrior, Bear Form), Energy (Rogue, Cat Form) | the client's own rage and energy bars, in their own colours: the server sends the hero's power type and its values; energy comes in ticks of 20 every sec (1.12's ticks at twice their pace), so the bar fills in steps |
| Every counter | the mana bar, painted the counter's colour by our interface (`ManaBarColor[0]` and `SetStatusBarColor`, `run_frames::class_bar`): gold for Holy Power, Holy Shield's stacks and Zeal; fiery orange for Inner Fire; deep purple for Insanity; teal for Maelstrom Weapon; pale spirit blue-green for Call of the Ancestors; pink-violet for Arcane Knowledge; flame orange-red for Hot Streak; icy light blue for Water Elemental; violet for Soul Shards; fel green for Demonic Souls; dark coal red for Burning Embers; silvery moonlight blue for Lunar Power (Moonkin Form's bar, its passive being Starshards); emerald green for Emerald Slumber. The colours sit in `esurv-content` `option_looks` |
| Combo points (Rogue, Cat Form) | five pips under the player frame's mana bar, lit as the points build (the stock combo frame's art), never on a target |

**Burning Embers** also float over the hero's head: four embers (the client's ember art), each
lit as the counter fills it, half lit when half filled (`embers`).

**Stacking buffs** (Zeal, Quick Shots, Frenzy, Lock and Load, Holy Shield, Inner Fire, Arcane
Knowledge, Hot Streak, Maelstrom Weapon, Burning Embers, Thick Hide) show in the buff frame, each
an aura of its own with its count; where the aura's spell has another icon than the buff's, our
interface shows the buff's (Maelstrom Weapon's hammer, Arcane Knowledge's essence, Hot Streak's
Pyroblast).

**Counter buffs** (`esurv_content::legendary_counters`, the server's
`game::counter_buffs`): a worn legendary amulet whose milestone goes off every n-th cast or hit
(an `Every` of a count above 1, a `LineHit` every n-th, and the like) shows a buff in the buff
frame from its first count on, in the legendary's own icon, its stacks the count toward the next
time (Stampede: 0 to 8, back to 0 as the stampede goes; the stock frame writes no number under 2),
no time left; its tooltip names the legendary, "4 of 9" and its text. Each counting legendary's
buff is an aura of its own, the legendaries' order picking one of `COUNTER_AURAS` (old
`Spell.dbc` rows 2186 to 2205 with no look of their own, the "zzOLD ... Damage Bonus" ones), its
icon swapped for the legendary's as the stacking buffs' are. Thirteen legendaries count: Gruul's
Shatter, Hateful Strike, The Ashbringer, Thori'dal, Stampede, Penance of Light, Tornado, Arcane
Bomb, Frostbolt Volley, Magnetic Pull, Living Bomb, Shadow Bolt Volley, Starfall. Those that count
on each enemy apart (`per_enemy`: Flash Freeze, Silithid Swarm) show none, and neither do those
that go off every time. A look only: no number changes.

**Spell icons.** The action bar and the spellbook hold each spell as the stock spell it looks like,
whose stock icon can be another's (Arcane Blast's look is a fireball). In a run they show each
spell's own icon, the one its card wears (`abilities.ron` `icon`; `run_frames::spell_tips`
hands them over by `Spell.dbc` id, the Lua answers `GetActionTexture` and `GetSpellTexture`); a
race's passive keeps its stock racial spell's. Their tooltips name the passive (or the spell, for Holy Shield), how many stack of how
many, and what it does.

**Berserk** (a Cat card) shows in the buff frame as its sweeping aura (Sweeping Strikes', spell
12292) with the **Idol of Ferocity's icon** (`INV_Relics_IdolofFerocity`; `option_looks::BUFF_ICONS`),
its time left and its tooltip ("Your attacks also strike 2 more enemies"); each set of extra hits
its attacks make shows a small sweeping slash (the run's `EXTRA_STRIKE_LOOK`, spell 12723, at 0.4).

### Looks

What the options look and sound like, all from the player's client:

* **Look styles** the server sends at `hello` (`ESURVZ`) and benilla applies to every play of a
  spell's looks, whoever casts it (`SpellLookStyles`): a colour on the effects' particles,
  ribbons and beams (Inner Fire's burn on the Priest is Evocation's whirl in red and white;
  Maelstrom Lightning is Chain Lightning in teal), or a recolour: each fragment keeps its
  texture's brightness (its brightest channel) and takes the style's colour, its brightest channel
  raised to 1, as its hue, so fire turns a bright colour rather than a dark one under the multiply
  (the Restoration Druid's lashers burst in a poison green Goblin Sapper Charge blast; it covers
  the effect models' own mesh parts, their ground decals, particles, model particles and child
  pools included, and ribbons, and effect models spawn no M2 lights), effects laid flat on the
  floor (Arcane Knowledge's rune, Dampen Magic's), a size for every effect model and missile, their particles
  with them (an emitter that does not size its particles by its model's scale has its sizes
  multiplied by the style's), which also widens a chain's beams (Hot Streak's flames and
  Maelstrom Weapon's tornado grow with their stacks, the tornado round the hero from 0.3 to 0.8
  of its size over its five stacks; Denounce's holy bolts, chain and bursts at half; Wellspring's
  geyser at two thirds; Fan of Knives' thrown knives and Divine Star's holy bolt at twice their
  size), and no camera shake. An area's size per cast (Hand of
  Gul'dan's meteor) goes with each cast as before (`ESURVS`) and grows the models only.
* **Assists** (a battle standard's ally, [assists.md](assists.md)): the standard lies as its
  faction's battle standard (the Alliance or the Horde Battle Standard) over a short aura glow in the
  faction's colour (blue, red), planted with its Battle Standard look, and
  pings the minimap where it lies; the ally is a player of its hand-made character drawn at twice
  its size, a friendly player that cannot be selected, its name over it and its yells in the chat;
  it comes in its arrival's look, casts its steps' looks from itself, swings at what it strikes in
  melee, then loops its rotation for the rest of its 15 s (cast bars and cast animations, a rogue's
  strafes and jumps, a frost mage's jump back), and goes its leaving's way ([server.md](server.md), "What the client is told").
* **Summons** are the run's pets with their models (`pets.ron`); the ghostly copy of the hero is a
  copy of its player, see-through and blue (the Ghost Visual aura), standing where it appeared;
  it is the one seen casting its lightning, flame and heal, with their cast animations (the run
  shows the looks of a summon's casts for its hero from the summon: `Run::cast_by_summon`). The
  Restoration Druid's plants (Lasher, Thornling) are twice their size (1.2, `pets.ron`), and
  each bursts as a Goblin Sapper Charge's blast (13241) recoloured poison green by its look style
  (`LASHER_POISON_GREEN`) at half its size, with the poison cloud sound, cast by a look left where it stood
  (`extras::LASHER_BLAST`); the
  Insanity tentacles rise anywhere 4 to 8 yards round the hero, never on it. A
  beast with a `family` (the Hunter's Cat, Timber Wolf and Bear, the talents' Wolf and Raptor)
  grows with its hero's level as 1.12 grows a hunter's pet with its own: its family's size at
  level 1 to its size at level 60 in even steps (`PetFamily::scale_range`, from the player's
  `CreatureFamily.dbc`: Wolf 0.7 to 1.0, Cat 0.7 to 1.1, Bear 0.6 to 1.0, Raptor 0.5 to 0.8). The
  server sends the size with the pet and again each time its hero goes up a level.
* **The Holy Orb** on the ground is a dynamic object of its own look (`option_looks::HOLY_ORB_LOOK`)
  that the client draws as a glowing ball of holy light, `option_looks::HOLY_ORB_MODELS`, floating
  a little over the ground (benilla's hook `GroundLookModels`: the models a crate draws for a
  spell's dynamic object in place of the spell's own area look, each with its size, lift, a
  colour on its particles, and whether it loops or plays once). The models are a weapon
  enchantment's round golden glow (`Spells\Enchantments\YellowGlow_High`, picked among five
  glowing orbs) with a holy bolt's ball inside it (`Spells\Holy_Missile_Low`) at half its own
  size and dimmed to 0.55. `ESURV_HOLY_ORB_MODEL=<M2 path>[,<scale>[,<lift>]]` draws one
  other model in their place, and `ESURV_ORB_SHOTS=<png>` (with `_AFTER`, `_COUNT`,
  `_EVERY`) holds the camera close on the first orb lying and writes shots of it (`_LOOK=<spell
  id>` frames another look on the ground instead, `26093` Slam's burst; `esurv-app`
  `ground_models`; with the server's `ESURV_ORBS_STAY=1` so the orb is not taken): dev
  aids, for comparing looks.
* **Slam's bursts** cover its whole cone on the floor: short dynamic objects of their own look
  (`option_looks::SLAM_BURST_LOOK`, 2.5 s) in rows every 2.5 yards out to the cone's end, each
  row spread across the cone with one every 2.5 yards of its arc (`starting_looks::cone_spots`,
  27 for a 15-yard, 60° cone), which the client plays once as the first of
  `option_looks::SLAM_BURSTS`; the enemies it hits show Slam's own hit look. Quake's earth burst
  (`Spells\BurrowEarth_Brown_Missile`) at 0.3, its rocks and dust with it. The
  others are kept for comparing (screens/level1 `warrior-slam-alt-*`): Dust
  Field's ring of fine dust at half size, a charge's landing dust at twice its size, War
  Stomp's shock ring at 0.35; `ESURV_SLAM_BURST=<name or number from 1>[,<scale>]` on the
  client draws another (`dust_field`, `2`, `dust_field,0.8`). No camera shake.
* **The tips** (1.12's tutorial alerts, the "!" buttons over the action bar) are off for a new
  save: the save keeps the tips seen (`tips_seen`, every tip seen to start) and the server sends
  them at each entry, so none shows. The options panel does not show the tips' box (Tips in the
  game); the server keeps what the client tells it (`CMSG_TUTORIAL_RESET`, `_CLEAR`,
  `_FLAG`). benilla copies a bank sent after the login onto the banks its world entry zeroed, so
  the bank holds.
* **Forms** show their model over the race's own: Cat, Bear and Moonkin, the night elf's and the
  tauren's (another race takes its faction's).
* **Channel beams** (`channels`): while the hero's Drain Soul drains an enemy, and while each
  Insanity eye tentacle flays one (the claw tentacle claws an enemy within 5 yd of it with its
  own melee attack instead, no beam), the server sets the caster's channel fields (`UNIT_CHANNEL_SPELL`,
  `UNIT_CHANNEL_OBJECT`) to the 1.12 spell and the enemy, so the client draws that spell's own
  beam between them (Drain Soul's, Mind Flay's: their channel kits' chains) and the caster holds
  its channel pose. A beam that moves to another enemy is cleared on one mirror and set again on
  the next, as the client draws a beam on the channel's rising edge.
  Mind Flay, Drain Soul and Drain Life (every rank, an ally's too) play no looping sound, so no
  channel hum (our rule, the owner's; `MUTED_CHANNEL_HUMS`, benilla's `MutedLoopSounds`); their
  cast and hit sounds play.
* **Debuff looks** on enemies: Deadly Poison shows as Poison State (spell 12881: the green poison
  cloud on the afflicted, its body tinted green, and the poison cloud's sound as it lands), with
  Deadly Poison's own icon in the target frame (`option_looks::DEBUFF_ICONS`; `run_frames`
  answers `UnitDebuff` for the target); Starshards' stars (its own look's lasting part, spell
  10797) fall on each enemy its novas strike while they go on striking it (1.25 s after each).
* **Spells shown as another's look**: Windfury Weapon's extra strikes show Thunderfury's proc
  (spell 21992, its lightning on the enemy struck). Divine Star is a holy bolt (spell 10319,
  Holy Wrath's bolt and trail at twice its size) that flies out to its range and back to the
  priest: the server leaves a look of its own where the first lands and casts the second from it
  at the priest as the first arrives; its hits on the way out and back land as it passes (it
  flies at its look's speed, 20 yards a second).
* **Extra looks and sounds** the server adds to the options' own (`starting_looks::extras`):
  bursts of earth over Slam's cone (above); Explosive Shot's dynamite blast, again half a second
  later, and dynamite blowing up at the feet of each enemy its blast hits (Ez-Thro Dynamite
  Backfire's look, spell 8332, cast by each enemy so it plays at its feet); Fan of Knives' spin,
  then fourteen throwing knives flying out all around with a throw and a dagger's strike; a dagger in flesh for each Mutilate hit; Envenom's poison cloud;
  Radiance with Lay on Hands' sound; Oil of Immolation's sound with Inner Fire's burn; Holy
  Shield's strike on a block; the Water Elemental's water sound; Lightning Rod's crackle on a
  marked enemy; Shred's sound (the cat's attack, sound 4121) on each enemy Lacerate's bleed
  lands on, with Rend's look.
* **Legendary looks**: each legendary amulet's signature moments wear
  a 1.12 spell look of their own, the boss's or hero's own where the player's client has it, the
  class fantasy kept. `content\legendary_looks.ron` names, per legendary (by its main spell), each
  moment (a milestone kind, or for an `Afflict` the effect: `Stun`, `Fear`, `Silence`...) and the
  `Spell.dbc` id it shows; the content puts each into the matching milestones of that legendary
  as it loads (`legendary_looks::apply`, over a look `legendary_amulets.ron` gives), so the numbers
  and words stay in `legendary_amulets.ron`. How each kind shows:
  * a look at a spot (a burst, a splash, a death explosion, a dot's burst, a summon's arrival)
    plays from a short look of its own put down there for 2.5 s (`Game::spot_look`), sized to
    the burst as an area look is, that casts the spell on itself: its cast and its ground model
    play at the spot, not at the hero (a look with a missile still flies from the hero);
  * a lasting area (`LeaveArea`, `FireTrail`, a growing Consecration) shows the look's ground
    model; on an enemy a sweep, a jump, a strike again or a second strike shows it as a hit;
  * a lasting state (a stun, fear, silence or mark on an enemy, the turned enemy, a burst's mark
    until it goes off; a spree, stacks or a buff on the hero) is worn as an aura for as long as
    it lasts (`worn_looks` on the enemy's and the hero's kit), so its state look plays on the
    body: stone (Petrify's grey and stillness), ice (Sapphiron's Icebolt, Kel'Thuzad's Frost
    Blast), Bladestorm's whirl, Gorehowl's Bloodlust; a heal plays on the hero.
  Picks: Gruul's Shatter turns enemies to stone (11020) then rock bursts (26271); Bladestorm
  (9632); Hateful Strike (28308); the Mountain King's Storm Bolt (20685, also Storm Bolt's jumps);
  Gorehowl's Bloodlust (6742); Onyxia's Bellowing Roar (18431); Hakkar's Corrupted Blood (24328)
  and Blood Siphon's bursts of blood (16141); the Ashbringer's holy burst (8913); Sanctified
  Ground grows as Death and Decay's ground (11433) recoloured holy gold (`HOLY_GOLD`); Hand of
  Ragnaros a meteor (24340), Wrath of Ragnaros Ragnaros's own fire nova (21154) and heated ground
  (22191, also Lava Bomb and Legion Flame); a Mimiron-like rocket blast (1940); Grobbulus's poison
  cloud (28158); VanCleef's Whirling Blades a cyclone (11568);
  Living Bomb: Baron Geddon's bomb burns on the enemy (20475), then his explosion (20476);
  Starfall's stars (20687); Thaddius's Magnetic Pull (28337); Kel'Thuzad-like volleys (Frostbolt
  Volley 8398, Shadow Bolt Volley 9081); Mind Control on Dominate Mind's turned enemy (605); Sleep,
  Blind and Silence on the enemies those legendaries hold; the rest as `legendary_looks.ron` says
  beside each. The legendaries that only make their spell bigger (Blizzard, Rain of Fire,
  Hurricane, Shade's Missiles...) have no moment of their own: their spell's own look shows, at
  its new size.
  Every moment of every legendary has a look (`legendary_looks::moments_without_looks`, tested;
  Gruul's Shatter's delayed burst needs no mark, its enemies stand in stone). The
  moments that showed none:
  * **smaller looks** (`small` in `legendary_looks.ron`): a rank of the spell no hero casts,
    styled at 0.6 (`SMALL_LOOK_SCALE`, sent with the look styles), so the spell's own look keeps
    its size: on each further enemy of a sweep or a jump (`ExtraTargets`: Death Coil 705, Feast
    22827, Hammer of Uther 5588, Holy Wrath 10318, Silithid Swarm 24974, Thrall's Doomhammer 548,
    Static Charge 325, Eye Beam 8102, Windrunner's Volley 14288, Pounce 6787, Saber Lash 6800,
    Penance of Light 598, Doomwinds 27648), on an enemy struck again (`StrikeAgain`: Mirror Image
    and Wind Walk Sinister Strike 1757, Endurance Aura Heroic Strike 284, Light's Barrage Holy
    Shock 25902, Frenzy Shred 6800, Wrath of Cenarius Wrath 5177) and on each enemy an effect
    spreads to on a death (`SpreadOnDeath`: Magma Blast 8052, Silent Strangler 8631, Blood Siphon
    17347, Brutal Swipe 6546, Moonfire 8924, Rend Flesh 1823);
  * **pulsing looks** (`pulsing`): played on the enemy at once and again every 2 s
    (`PULSE_EVERY`) while the effect lasts, never worn: an exposed enemy (`Vulnerable`, fourteen
    legendaries) shows Sunder Armor's purple strike, small (7405); Righteous Fire's and Holy Fire's
    spreading burn (`DotSpread`) Exorcism's holy burn (5614);
  * **heals on the hero** (`HealOnTrigger`): Holy Light's glow (639) for Light of the Naaru, Holy
    Nova and Lifebinder's Gift; a short splash of blood (Hemorrhage, 17348) for Blood Siphon and
    Feast; Healing Touch's nature glow (5186) for Silithid Swarm;
  * **lines** (`LineHit`): Thori'dal's arrow of light is a glowing arcane arrow (14281), Netherstrand
    Longbow's an aimed shot (20901), both flying the line; Stampede's are its runners (below);
  * Rocket Strike's rocket marks the enemy with Hunter's Mark (14323) until it lands; Prophet's
    Infusion wears Holy Strength's glow (20007) while Power Infusion lasts;
  * **Blink Strike's reflection** (`StrikeOne` `reflection`; `game::reflections`): the hero no
    longer blinks. A see-through copy of the hero (a clone's look, Ghost Visual 9617) appears 1.5
    yd from the further enemy on the hero's side, facing it, plays Ghostly Strike (14278) on it and
    goes 1 s later; the damage is the milestone's as before;
  * **Sprint of the Wild's wild spirits** (`LeaveArea` `spirits`; `game::spirits`): two
    see-through cats in the druid's cat form of the hero's faction (892 Alliance, 8571 Horde), at
    0.6 of their size, run 2 yd left and right of the hero (`SMSG_MONSTER_MOVE` at the hero's
    run speed whenever their place moved half a yard) while the area lasts; each time it strikes,
    each plays a small Shred (6800) on one enemy it struck. They go when it ends; the damage is the
    area's (8% of max health a second within 5 yd).
* **The Phoenix** (Fireball's legendary) burns while it lives, half see-through: a pet may wear
  lasting `auras` (`pets.ron`) from the moment it appears, sent in its first aura slots in order;
  the Phoenix wears 20564 (Elemental Fire: flames at its feet), 66 (Lesser Invisibility's look: half
  see-through, no tint), 830 (Fearless's look: a fire-cast glow in each hand, which on the carrion
  bird are its wing tips, so bright yellow fire streams from both wings), 23097 (Fire Reflector's
  look: a ring of yellow-orange flames and a fiery glow round its body) and 28330 (an Immolate
  look: flames over its body and wings, a flame-red tint; last, so its tint is the one shown), and
  it rises from a great burst of fire (16049). Its fire burns at half strength too, as a kit's
  particles take their unit's see-through alpha.
* **A line's runners** (`LineHit`'s `runners`, [classes.md](classes.md); `game::line_runners`):
  as the line strikes, the simulation says so (`Event::LineRunners`: the hero's spot, the line's
  end, its width) and the server puts a row of units across the line's whole width, each in a
  lane of its own, 3 yd behind the hero (each up to 1 yd before or after, so the row is ragged as
  a herd is), sends each running straight to the line's end at the runners' speed
  (`SMSG_MONSTER_MOVE`, its path on the ground, run speed set to match) and takes each away as it
  gets there. How many and each one's look are picked fresh at random for each row, at most 3 of
  any one look in a row. A runner is a unit as a bunny is
  (entry `LINE_RUNNER`): no name, unselectable, unattackable, no room taken, no enemy of the
  simulation; the damage stays the line's. **Stampede** (Kill Command's legendary, every 9th
  command) sends 5 to 7 beasts at 14 yd/s, each one of the 1.12 Timber Wolf (604), Stonetusk Boar
  (503), Saltwater Crocolisk (2548), Sunscale Lashtail (1744), Savannah Prowler (1973),
  Mountain Lion (1058) and Elder Shardtooth (8837) looks (`CreatureDisplayInfo.dbc`), each at a
  scale of its own that draws it at a Timber Wolf's size (its model's size, the cube
  root of the M2 vertex box as zones.md measures it, times the scale comes to 1.98, the wolf's 3.60
  at its own 0.55; the crocodile model, 8.49, at 0.23, the raptor, 6.42, at 0.31). Any other
  line can take runners the same way.

## The create screen

Heroes are made on the hero sheet ([The hero sheet](#the-hero-sheet)); the stock 1.12 create
screen (benilla's `char_create`) shows only with the stock screens (`ESURV_MENUS=0`), and the
hero sheet offers the same pairs (through `CreateChoices`). The create screen offers each race the
classes 1.12's `CharBaseInfo` lists for it. It also offers the
six pairs `races.ron` adds ([classes.md](classes.md), "Race and class pairs"): Dwarf Shaman, Human
Hunter, Gnome Priest, Orc Mage, Troll Warlock and Undead Paladin. Each shows like a stock pair: the
class button among the race's (ascending class id, as the stock list orders them), its stock
class tooltip and description, and the preview body in a borrowed stock starting outfit (the
outfit rule in classes.md). The dead Dwarf Mage row stays unoffered. Human and Troll now offer seven
classes, one more than any stock race: past six, the class buttons go four to a row across the
dial rows' width (x 4 to 202) instead of the authored three, so a third row never runs into the
dials (stock races keep the authored three).

### benilla's hook: added race and class pairs

`benilla_app::AddedRaceClassPairs` lists the pairs a crate adds, each a `benilla_app::AddedPair`
(1.12 race and class ids, and the races whose stock outfit for the class it may borrow, the first
with one winning). The crate inserts it in `run_with`'s extend; benilla adds the pairs to its
create catalog (`CharCreateCatalog::add_pairs`) as the catalog loads, so the create screen, its
preview and the character list's bodies read one catalog. Stock benilla adds none. The game's
list comes from `Content::added_pairs` (`esurv-app` `races`).

### benilla's hook: composed displays

`benilla_app::ComposedDisplays` lists displays a crate composes from two of the player's own
models: an id the server sends (one `CreatureDisplayInfo` lacks), a base display, a part
(`PartModel`: an M2 path, or a weapon's `ItemDisplayInfo` display, whose
`Item\ObjectComponents\Weapon` model wears the display's own texture) and how it stands
(`PartPlacement`, in yards of the base's model space at model scale 1: its scale; a tip about its
own Y axis in degrees, -90 standing a weapon head up; its turn about the vertical in degrees; how
deep its foot goes below the ground; how far the base's particle effects on moving bones rise;
and whether the base's body is hidden). The id answers every display lookup as its base
(`CreatureCatalog::alias_display`); its model is the base model with the part's batches tipped,
turned and scaled, centred on the base's origin across with their lowest point `sink` below the
ground in the first frame of the base's stand (animation 0), and skinned whole to the bone most
of the base's solid body rides, so it moves as the body does while the base animates: its
vertices are put back from that frame to the bone's rest pose, so the stand's first frame shows
them where they are placed. A base without a body bone posed in a stand takes the part on a
root bone no animation moves (keyed in no sequence or global sequence; the one nearest the
origin), standing still.
A hidden body drops the base's solid batches (opaque or alpha-tested, no glow card) and the
cards laid on their bones, keeping its glow cards, particle emitters and bone animation; the
part fades in and out as the body did. A particle emitter on a bone with a parent (the base's
moving body) rises by `raise_effects`. The game composes the Dwarf Shaman's Ironfoe totems this
way (classes.md).

| Switch | Effect |
|---|---|
| `WOW_CHARCREATE_SHOT=1` | Opens the create screen a few seconds after the character list is up |
| `WOW_CHARCREATE_PICK="race,sex,class"` | Picks that race, sex and class on it (an added pair too), e.g. `3,0,7` for a male Dwarf Shaman |
| `WOW_CHARCREATE_SHOT_OUT=<png>` | Writes one shot of it once it has settled |
| `WOW_CHARCREATE_NAME=<name>` | Fills the name and presses Create once |
