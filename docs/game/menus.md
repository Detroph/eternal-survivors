# The menus before a run

This replaces the stock create screen, the stock select list and the
realm list as the zone picker. It is built after the hero picker's warband camps and the added
race and class pairs are merged. These menus leave the stock 1.12 look behind and have an art
style of their own (below); the HUD and the meter keep theirs. The
run's Escape menu, its choice screens and its end panel wear this look too (below,
"In a run").

## The look

* Near-black panels with a thin warm-gold border and rounded corners, in full-height columns.
* Headings in a gold display face with small capitals, centred between two thin gold rules.
* Small all-caps grey labels over larger white or gold values; values in rounded, bordered boxes.
* Class names in their class colour; item quality in the quality colours.
* Icons in square frames with a soft glow on the selected one; locked or not-yet-found pieces
  darkened with a "?" over them.
* Gold arrow buttons for stepping through a choice; framed text buttons, the active one filled
  dark red.

## The flow

**Built** ([screens.md](screens.md), "The menus before a run"): the launch rule, the class picker,
the warband screen and the hero sheet with its three views, in the look below, as described here.
Ours where this leaves it open: the hero sheet has Back at its top left and a hint that the class's
changes stay while a hero is made; Escape goes back a screen (and does nothing on the warband
screen); the warband panel also shows the runs played; the camera's aim moves so the camp sits in
the lower left.

0. **Launch:** if the save holds any hero, the game opens straight on character select (the
   warband screen, step 4); only a save with no hero opens the class picker.
1. **First launch, with no hero yet: the class picker.** One panel, which may fill the whole
   window:
   * a "Class" heading over a 3 × 3 grid of the nine classes (icon, name under it); the selected
     class glows and its name takes the class colour;
   * under the grid: the class name, one or two lines of our own about how it plays, three value
     boxes (health, the spell it starts with, zones open "n of" every zone), and the class's spell pool as
     a row of spell icons (each with its tooltip).
2. **Choosing a class opens the hero sheet**, on its **Character** tab:
   * left column, "Appearance": the races as portraits grouped under Alliance and Horde (only the
     races the class may be, the others dimmed), the race name, Male and Female buttons; under
     it "Equipment": the slots (head, shoulder, chest, hands, wrist, waist, legs, feet, main hand)
     with their icon and what is worn or "Empty", and a line at the foot about what worn gear
     adds;
     between the race and sex picker and Equipment, **Racials**:
     the race's spell and passives as icons in a row (the names the client's, read through the
     content's pointers; a passive's icon its stock racial spell's), each with its tooltip (name,
     racial spell or passive, what it does); it follows the race picked;
   * middle: the hero's name as a big heading (editable at creation), "sex race class" under
     it, the dressed hero standing on a lit floor, turned by dragging, with turn buttons and a
     hint at the foot;
   * right column, "Features": skin colour, face, hair style, hair colour and the race's fifth
     feature (piercings, tusks, ...), each a box with gold arrows, "n / m" and a row of dots, and
     a Randomize button; a hint at the foot;
   * confirming creates the hero (name and look saved) and lands on character select.
3. **Character, Collection and Talents.** The hero sheet has no tabs;
   Modify character opens it on Character (Back and Save changes), and the warband screen's
   **Collection** and **Talents** buttons open it straight on those views for the picked hero
   (Back returns to the warband screen).
   * **Collection**: the Equipment column on the left (its slots pick their looks); the middle is a
     "Collection Log": columns by slot, a row per zone (each zone
     drops its own pieces), each row named in its tier's quality colour with its quality and
     its count found "n/9", pieces not found dark; under it "still to find in your zones"
     (piece, boss, zone, chance); a footer with how to read it (worn and look marks) and the
     total found "n / 81"; the right column "Details" shows a picked piece: the zone it came
     from ("From Duskwood"), what it adds and which bosses drop it, or the look choices (a
     tier each) for a slot picked on the left. A double click on a piece of the log
     wears it, as picking it and pressing its Wear does (on a second row of looks, its Show This
     Look, the piece worn first when the slot wears nothing); a single click picks it as before.
   * **Talents**: the class's three trees side by side, each with its name, icon and points
     spent, on its background art; talents as icons with "rank / max" and lines between
     prerequisites; under each tree its milestone talents (10, 20 and 30 points). The right
     column "Talent Points": points to spend in big green text, earned and spent, the talent
     experience bar to the next point with a line on how runs earn it, a Reset button, and the
     picked talent (icon, name, rank, what it does) with Learn and Unlearn. Once the class has
     all 51 talent points a **Prestige** section shows under the trees
     (classes.md, "Prestige points"). A click picks a talent or prestige stat, a click on the one
     picked learns a rank, a right click takes one back.
4. **Character select, the warband screen** (the camps already built):
   * the camp fills the lower left: the campfire scene, the selected hero with a thin golden
     outline glowing softly round it and a nameplate over its own head
    : its class icon (the class picker's) in front of its name, in the
     gold heading style, on no other hero. The plate follows that figure on screen: its foot sits
     half a yard over the figure's head (the model's Head attachment in its pose, so it comes down
     with a seated or kneeling hero), clear of the heads round the fire; it never goes over a
     panel: under one above the head (the hero panel, Equipped armour, the Codex), beside one at
     its side (the Zone column), and always inside the screen. Under the fire are the buttons, in this order: **New
     character** (opens the class picker), **Modify character** (opens the hero sheet on that
     hero; race changes are always possible there) and **Delete character**, which asks first: "Are you sure you want to delete this
     character?" with **No** and **Yes** (No is the default; only Yes deletes, and the camp then
     selects its next hero);
   * a round button just under the top-left panel switches between the Alliance and Horde camps,
     with a small badge giving how many heroes the other camp holds (it follows the panel's
     height, so it never covers the panel);
   * the top-left panel: the selected hero's name, class and total wins; under it the
     **starting spell selection**: the class's starting options as
     icons, three for each class and four for the Druid, each with a tooltip of what it does and
     its passive; a click picks one for the hero's next runs, kept in the save (the first until
     one is picked); and directly under it the **weapon loadout selection** (roadmap
     R3): built as the starting spell's: the class's three loadouts as
     a row of icons of the same size and frame (each its `icon` in `loadouts.ron`, a 1.12 weapon or
     shield icon), the chosen one framed gold and named under the row; each icon's tooltip, styled
     as the starting options', gives its name, weapons, strength (green) and drawback (red); a
     click picks one for the class's next runs, kept in the save (the first until one is picked),
     and the camp's figure takes up its weapons at once;
   * the top middle, centred over the camp on the axis of its buttons:
     a row of four buttons, **Collection**, **Talents** and two dimmed placeholders reading "TBD"
     (features to come), over the selected hero's equipped armour. Collection glows (the gold
     border and soft glow of what is picked) while gear was unlocked for the hero's class since
     its Collection was last opened, Talents while talent or prestige points were earned since its
     Talents were; opening the view and coming back clears its glow. What was seen is kept in the
     save per class (`seen`); a class of an older save has nothing new to show, a class new to the
     save has seen nothing;
   * the top right, beside the zone selector, the **codex**: how many
     rings, trinkets, amulets and legendary amulets the save has found, and a button that opens it:
     the found pieces of every hero, the counts per kind, the pieces not found dark with where they
     come from;
   * the right column, the **zone selector**: a "Zone" heading, the chosen zone opened as a card
     (its name, its title, a few lines of our own about it, its enemies, and its bosses with
     their arrival times); every other zone as a row, locked ones saying which boss in which
     zone opens them. **Zones are hidden until discovered.** Every zone
     slot is listed, so the player sees how many there are, but a zone no class of the save has
     opened yet shows only "???" and a lock (a key icon, the client having no padlock): no name,
     number, difficulty, picture, enemies, bosses or rewards. Once any class opens it, it is
     discovered for the whole save and every hero sees it in full; a hero whose class has not
     opened it still sees it locked, with what opens it. The collection log hides such a zone the
     same way (its row "???" and a lock over dark unknown icons, nothing to pick), and so does
     every tooltip that would name it ("From ???"). Every zone of the content is listed, in difficulty order, so a zone added
     as a file shows in its place; when they are more than the column shows, the list scrolls
     with the mouse wheel and keeps its place when the screen is drawn anew (the collection
     log's rows do the same);
   * at the foot of the right column: **Difficulty modifiers** (the run options) and **Start
     run** (enters the world in the chosen zone).
   * the bottom-left corner of the window: **Options** over **Quit
     game**. Quit game closes the game as the Escape menu's Quit does, with no run to pay out.
     When a newer version of the game is out, a small panel over them says so ("v0.5.0 is
     available") with an **Update** button: it downloads the new version (its progress shown
     under the line), checks it, and restarts the game into it; a failure says why in the same
     panel and changes nothing ([server.md](server.md), "Updates"). The Interface tab's **Check
     for updates** box (on by default) turns the check off.
     Options opens the game's own settings: the stock options frames (Video, Sound and Interface)
     exist only in the world session's Lua interface, which is built when a hero enters the world,
     so a panel of ours in this look stands in for them before a run, reading and writing the same
     settings through the same calls they make (key bindings need the Lua interface: they are
     set in a run, from the Escape menu's Options, below). Its fourth tab, **Saves** (design step 6), exports and imports the save: the
     game has no file dialog, so the save goes to and comes from one fixed folder beside it
     (`saves/exports`), shown on the tab; **Export save** writes a dated copy there, the folder's
     saves are listed newest first with how many heroes each holds (or that it is no save the game
     can take), and **Import save** takes the one picked after asking "Replace the save with this
     one?" (No the default), keeping the save it replaces in the same folder first, so an import
     can be undone. Only the game the save is kept on may do either; a remote host refuses.

## In a run: the Escape menu

Once a run has started, Escape pauses the run and opens a menu in the same art style with five
buttons:

* **Continue**: closes the menu and resumes the run.
* **Reset run**: ends this run (it is paid out as a run left) and starts a fresh one in the same
  zone with the same hero, without leaving the world.
* **Options**: opens the warband screen's Options panel (Video,
  Sound, Interface, Saves) over the paused run, in this look; the run stays paused, and closing
  the panel (Okay, Cancel or Escape) comes back to this menu. The settings apply as on the
  warband screen; one that takes hold only when the world loads says so, and after Okay the menu
  says what waits for later. Over a run the Saves tab exports only: an import would replace the
  save mid-run, so it waits for the warband screen, and a line says so. A **Key Bindings**
  button opens the game's own key binding window (the world's Lua interface is there in a run);
  closing it (its buttons or Escape) comes back to the panel.
* **End run**: ends this run (paid out as a run left) and brings the player back to character
  select, the warband screen.
* **Quit game**: ends this run (paid out as a run left), saves, and closes the game.

Escape opens this menu, never the stock game menu, wherever the hero is in a run:
before the first step (nothing to pause yet) it opens with Continue (closes it),
Options, End run and Quit game; over the end panel with only Options, Return to main menu (out
to the warband screen, as the panel's own) and Quit game, the panel hiding under it; a second Escape
closes it. Only a choice screen keeps Escape for itself.

The armory and the codex leave this menu, and are not reachable during a run at all: no key
opens them in a run, and the end panel has no button for them. The hero sheet holds them
before a run (the warband screen's Collection and Talents buttons).

**Built** ([screens.md](screens.md), "Pausing: the Escape menu", and
[server.md](server.md), "Again and Reset"): the five buttons work as above, Reset Run through a
request of its own (`reset`), Continue, End Run and Quit Game through `resume` and `leave`, and
Options asks the server nothing (`menus::over_the_run`). The menu wears the art style above (its
look kept apart from its rules, in `escape_menu_look`) and hides under the Options panel. In a
run with other heroes, a reset waits until no other hero is still in the run (it would start
their run over too); ending or quitting takes only the asker out; and should another hero resume
the run while the Options panel is open, the panel closes as Cancel closes it.

## In a run: the end panel

The end of a run (**Triumph** or **Overrun**) wears this look:
one near-black panel with the thin gold border; the title in Morpheus small capitals between two
gold rules (gold for a victory, red for a fall) with the hero and zone under it; the run's tally
and the talent experience revealed part by part (the hero sheet's purple bar filling, each talent
point called out) under small all-caps grey labels; the zone opened and the gear won; the framed
buttons Run Again (the gold-framed call to act), Go Endless (after a victory) and Return to main menu
(after a fall as after a victory; it goes back to the warband screen).
What it shows and does is unchanged ([screens.md](screens.md), "The end of a run").

## In a run: the choice screens

The level-up choice and the ring, trinket and amulet choices (a cache bought, a rare's free cache) fit
this look in place of their stock dialog: one near-black panel over the dimmed world, the
heading between gold rules, the hero's spells and worn jewelry in columns either side (the slots
in use, as the choice's cards need them), and the cards as near-black boxes whose frame and glow
take the rarity colour, lit under the pointer and once taken, each with its slot in a value box
("Takes a spell slot, 3 of 6 in use", "Upgrades a ring you wear, Rank 1 » 2", the amulet it
replaces) and a framed Choose button; under them a framed Reroll button and the gold. Their
keys and rules stay as they were. **Built** ([screens.md](screens.md), "The card screens").

## What it needs underneath

* The save keeps each hero (name, race, sex, look, class) as now, the class progress per class,
  and lets a hero's race and look be changed later.
* The talents, collection and progress shown before the world connection come from the local
  game's save (as the picker's facts do now); changes made there (learn, unlearn, wear, take
  off, look, run options, and a hero's name, race, look and zone) reach the server before a run as
  asks of their own (**built**: [server.md](server.md), "Before a run"; a local game answers them
  straight from the embedded server's save, and a carrier to a remote host can come later).
* What the hero sheet's Character, Collection and Talents tabs and the codex show is built and
  tested already, kept from the in-run armory and codex windows that were removed:
  `esurv_app::hero_sheet` (the class's progress and its requests, the talent rules, the
  gear and drop tables, the item icons, the codex's lists and pages; [screens.md](screens.md),
  "The class's progress and the codex"). The menus draw the class's progress (**built**: the
  Collection and Talents tabs) and the codex (**built**: on the warband screen).
* The zone is picked here, so the realm list stops being the zone picker (**built**: the game
  has one realm, the realm list never shows, and each hero keeps the zone chosen for it).
* All the text on these screens is our own.
