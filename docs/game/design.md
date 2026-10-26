# Eternal Survivors: design and build plan

## 1. Goal

Build a survivors game on top of benilla: zones, enemies, bosses, timings and rules, all driven
by data. It must also be **structured for growth**, so that new zones, enemies, bosses, classes, abilities and talents can be added later
mostly by editing data.

Success means:

1. A run in a given zone, with a given class and run options, follows the zone's data: its
   enemy roster and unlock minutes, boss schedule, damage, scaling and rewards.
2. Every system is documented: what it does, which data drives it, and how to extend it.
3. Adding a zone or an enemy needs no code change. Adding an ability or talent needs code only
   when it introduces a new kind of behaviour.

## 2. Ground rules

| Rule | Meaning |
|---|---|
| **Numbers by design** | Numbers change on purpose, one category at a time, measured with the balance check (step 7b); never tweaked on the side. |
| **Own prose** | All player-facing text (zone and class descriptions, talent and tooltip wording) is written for this project. WoW names, IDs and boss quotes stay. |
| **No Blizzard assets** | Like benilla, the game reads the player's own 1.12.1 data through `WOW_DATA`; nothing from the MPQs is committed. |
| **Separation** | Self-contained repo with its own benilla copy. Crates use the `esurv-` prefix. Nothing is shared with a separate benilla checkout. |
| **benilla stays benilla** | Changes to `crates\benilla-*` are kept minimal and isolated: embedder hooks only, each one documented, so upstream benilla can be merged in later. |

### Built for the roadmap

The game's insides are built for the changes planned in `roadmap.md`: one stat-modifier pipeline shared by every source of power, an event system
for procs and mechanics, and reusable choice screens. These shape how steps 2, 5 and 6 are
built.

## 3. Architecture

One executable, two halves in one process:

```
esurv.exe
├─ thread: esurv-server (game server)
│   ├─ realmd   SRP6 logon; one realm (the zone is chosen with the hero)
│   ├─ world    1.12.1 world protocol: heroes = characters, the run = the world
│   └─ esurv-sim  the game itself: fixed-step simulation, no networking, no Bevy
└─ main: benilla client (Bevy) + esurv-app screens
    └─ connects to 127.0.0.1 like to any 1.12 server
```

Why this shape:

* **The stock 1.12 UI just works.** Enemies, spells, damage numbers, auras, XP and loot reach the
  client as normal WoW packets, so benilla renders them with no game-specific client code.
* **The simulation is testable on its own.** `esurv-sim` is plain Rust: deterministic,
  seedable, unit-tested, runnable headless at many times real speed.
* **Content is data.** `esurv-content` loads RON files at startup; the simulation reads only those.

### Crates

| Crate | Job | Depends on |
|---|---|---|
| `esurv-content` | Data model (zones, enemies, bosses, classes, abilities, talents, jewelry, gear, run options), RON loading and validation | serde, ron |
| `esurv-sim` | The rules: run clock, spawning, scaling, movement, combat, XP, cards, loot, run end | esurv-content |
| `esurv-server` | realmd + world server; turns sim events into 1.12 packets and client packets into sim inputs | esurv-sim, esurv-protocol, benilla-protocol, benilla-srp |
| `esurv-protocol` | The messages between the server and the screens (addon chat lines our own): what the screens are told, what they ask | none |
| `esurv-app` | Bevy screens over the client: hero picker, armory, level-up cards, HUD, meter, codex, options | benilla-app, esurv-protocol, esurv-content |
| `esurv` | Launcher binary: starts the server thread, then the client pointed at it | all of the above |
| `esurv-installer` | Builds a playable folder from the player's own 1.12.1 client (step 9) | benilla-mpq |

### Repository layout

```
esurv\
├─ crates\benilla-*          benilla (fork)
├─ crates\esurv-*              this game's libraries
├─ crates\esurv\ the game executable
├─ content\                  RON data: zones\<id>.ron (a file per zone), enemy_kinds.ron, classes.ron, ...
├─ docs\game\                this doc, per-system references; guides\ has plain-language versions
├─ docs\BENILLA.md           the upstream benilla README
├─ scripts\run-game.ps1      build and run a dev build
└─ logs\                     build and run logs (not committed)
```

## 4. Build steps

Each step has a **goal**, **deliverables** and a **done when** test.

### Step 0: Foundation

* **Goal:** a building, running base.
* **Deliverables:** pinned toolchain (1.98.1) building the benilla fork; empty `esurv-*` crates in
  the workspace; `cargo test -p esurv-*` runs; a `run-game.ps1` that starts benilla against
  `WOW_DATA`.
* **Done when:** the unmodified benilla client starts from this repo and reaches the login
  screen.

### Step 1: Content model (`esurv-content`)

* **Goal:** every game table as editable data, with validation.
* **Deliverables:**
  * Rust types for: Zone (map, area, spawn point, facing, radius, hour, weather, difficulty
    multiplier, enemy roster, boss schedule), Enemy (display, scale, type, HP, damage, swing,
    speed, rewards, unlock minute, elite, ranged attack), Boss (spawn minute, HP, damage, speed,
    scale, abilities, yell, final), Class (base health, power, auto-attack, ability list, starting
    ability), Race (passives, race spell, faction), Ability (spell visual, school, delivery kind, damage, cooldown, range, radius,
    targets, duration, ticks, projectile speed, summon, on-hit effect, categories, upgrades), Talent tree
    and Talent (row, column, ranks, prerequisite, effects as the Stat / AbilityMod / Milestone
    union), Ring, Amulet, Trinket, Gear tier, Run option.
  * Validation: every id resolves, every class has 3 trees, one final boss per zone.
* **Done when:** all the content loads and validates; a test checks spot values.

### Step 2: Simulation core (`esurv-sim`)

* **Goal:** a complete, headless run that plays by the game's rules.
* **Deliverables**, as separate modules (`game/enemy`,
  `game/abilities`, `game/rares`, `game/upgrades`, `game/talents`):
  * **Run clock and world:** fixed tick, arena (centre, radius), run phases (waiting for the
    first move, running, paused, victory, defeat).
  * **Hero:** class base stats plus a stat sheet (the 26 stats the talents and jewelry use),
    health, regen, damage reduction, shields, cheat-death, extra lives.
  * **Spawn director:** roster unlock by minute, spawn rate and pack size over time, density
    option, elites, rares, boss schedule, cap on live enemies.
  * **Enemy scaling:** zone difficulty multiplier × time growth × run options (health,
    damage, speed).
  * **Enemy behaviour:** chase, melee swing timer, ranged attacks (range and cooldown),
    knockback and leap paths, crowd-control timers.
  * **Abilities:** auto-cast on cooldown, by delivery kind: projectile, instant, nova around
    self, ground area, melee strike, damage-over-time, channel, self buff or heal, totem, pet.
    Targeting, chaining, extra targets, schools.
  * **Damage pipeline:** in this order: ability base → hero multipliers → crit →
    per-target bonuses (vs crowd-controlled, below 30 %, vs afflicted) → `max(1, …)` → health
    → meters. Double-cast, lifesteal, thorns, DoT stacking.
  * **Progression in the run:** XP from kills, level curve, level-up stat gains, gold, pickups
    (Gold, Battleground Cache, Speed / Berserker / Restoration Rune, Magnet).
  * **Run end:** final-boss victory, death, tally (time, kills, elites, rares, bosses,
    damage dealt and taken, gold, caches).
  * Seeded RNG so any run can be replayed exactly.
* **Done when:** a scripted headless run in Elwynn spawns the right enemies at the right minutes
  and the five bosses at 3 / 6 / 9 / 12 / 16 min with their set HP; damage numbers match the
  formula tests; a run can be won or lost.

### Step 3: Embedded server (`esurv-server`)

* **Goal:** the benilla client logs into the game with no external server.
* **Deliverables:**
  * realmd: SRP6 server side (any username and password accepted locally), realm list
    of one realm; the zone is chosen with the hero on the character screen.
  * world: auth challenge, header crypt, character enum (heroes from the save), create
    and delete, login to the zone's map and spawn point, time of day and weather.
  * Object mirror: hero and creatures created, moved and destroyed through update packets;
    movement from the client fed into the sim.
  * Server tick: the sim runs at a fixed rate, and its events are turned into packets.
* **Done when:** start `esurv.exe`, pick a zone and a hero, and you stand in Elwynn at the
  zone's spawn point while enemies walk at you.

### Step 4: Combat presentation

* **Goal:** combat looks and reads like WoW.
* **Deliverables:** spell visuals for every ability (WoW spell IDs), damage and heal
  numbers, crits, auras and debuffs, deaths and corpses, the XP bar and level-ups, boss yells
  (the boss health bar itself is part of the HUD, step 6), combat log lines, the run-stats
  line read by the character pane (an addon message of our own, prefix `ESURV`).
* **Done when:** every ability of every class fires visibly with the right visual and
  numbers; bosses yell on arrival.

### Step 5: Progression and rewards

* **Goal:** the full loop between runs.
* **Deliverables:** level-up cards (3 offers, spell pool, rarity odds and strength, reroll
  and its growing cost, empowering legendaries up to 3×); loot (boss class gear, caches →
  ring / trinket / amulet, gold, luck); talents (talent XP from runs, points, respec, milestones
  and capstones); run options and their reward bonus; zone unlocks per class; the codex; saves
  in an open, readable format (RON).
* **Done when:** a hero can play Elwynn, earn talent points, unlock Duskwood, and keep gear,
  talents and codex across restarts.

### Step 6: Screens (`esurv-app`)

* **Goal:** the game's own interface.
* **Deliverables:** hero picker (class, race, name, race perks), armory (gear and talents),
  level-up card screen with reroll, reward pop-ups, HUD (clock, kills, gold, next boss, boss
  bar, pause and end panels), damage meter, codex, options (save export and import), run
  options (gamepad and touch input moved to the roadmap, R7). Lua changes to the stock interface hide the talent
  frame, spellbook and quest log, and add the run's level cap; the run stats on the character pane.
* **Done when:** a full session needs no stock WoW menu beyond the ones the game keeps.

### Step 7: Play check

* **Goal:** prove the game plays as designed.
* **Deliverables:** a checklist per system with the designed value, the measured one, and how it
  was checked; headless runs whose measurements (time-to-kill per boss, damage taken per minute)
  are compared against the design.
* **Done when:** every checklist row matches. A mismatch is a bug in the implementation, fixed in
  the code. **No balance passes:** numbers are not tuned in this step.

### Step 7b: Tuning

* **Goal:** every value and selection the game ships is tuned on purpose, and the game plays
  well.
* **When:** right after step 7. The play check first proves the systems work, so every change
  after it is a design decision, never a bug in disguise.
* **What it covers:**
  * Every tuned number: health, damage, cooldowns, ranges, radii, durations, speeds, the
    constants of the scaling and experience curves, drop chances, prices, talent, gear,
    jewelry and amulet values, run-option bonuses, spawn-director budgets, boss arrival
    minutes, enemy unlock minutes.
  * Every selection: which spells each class gets and its
    starting spell, card upgrade steps, the enemies and bosses of each zone and their order,
    boss abilities, gear sets, the jewelry and amulet lists, talent trees.
  * WoW's own data references (spell, model, creature, map and area ids used for looks) may
    stay or be re-picked: they point at the player's own client files.
* **How:** one category at a time. A direction is set first (for
  example "bosses tougher but shorter fights", or a curve of our own); the change is applied
  consistently, by formula where one fits rather than value by value, with the tests, docs and
  guides kept in step.
* **The balance check:** `cargo run --release -p esurv-sim --example balance_check` shows,
  for every zone and run minute, how long enemies last against the hero we expect a player to
  bring there. The plain check takes seconds, so run it after any change to spells, gear, talents
  or enemy scaling. The hero arrives with the talent points of one won 20-minute run in each
  earlier zone (every boss slain, the mean kill count the check measured there), all spent (first
  tree, deepest talent first, so the capstone comes at 31 points; then the second and third
  trees), and the whole class set of the zone before. It stands still, takes the first offer and
  cannot die (a cheat death with no cooldown, counted each time it fires). Per minute it prints
  the mean hero level and DPS; the normal enemies' mean health, damage per hit and **kill
  seconds** (health ÷ DPS); the elites and rares that appeared, with their health, kill
  seconds and damage per hit as a share of the hero's max health; and the cheat deaths; then each
  boss the same way. Options: `--seeds N` (at most N runs per zone and class, default 2; every mode
  stops early once its results are clear, `--fixed-seeds` plays the full count: AUTHORING.md
  section 16, "Early stop"); `--targets <csv>` with our target kill
  seconds (`zone,minute,kill_seconds`) and `--band LOW,HIGH` (default 0.8,1.25) to flag measured
  ÷ target outside it; `--milestones` (each class, tree and milestone row with its milestone and
  without, in the first zone whose hero reaches the row, against the row's power budget;
  `--talents id,...` for a few); `--gear` (each zone's set against the one before, against the
  gear budget); `--check-seeds N` (runs per case for those two at most, default 200, gains printed with
  their standard error); `--csv <file>`; and, to try a new scale without touching content,
  `--d zone=D,...` (new difficulties for real zones) and `--add-zone new_id=template_id:D` (a
  synthetic zone: the template's roster and bosses at D, its gear by the same rule as every
  zone's), both slotted into the difficulty order in memory.
* **Done when:** every category has been through this pass, every zone plays through well, and
  the tests pass.

### Step 8: Extensibility

* **Goal:** easy to grow.
* **Deliverables:** `docs\game\AUTHORING.md` (how to add a zone, enemy, boss, class, ability,
  talent or item, with worked examples); content hot-reload in dev builds; behaviour kinds as
  open enums with one module each, so a new ability or boss behaviour is one new file plus data.
* **Done when:** a new test zone with its own roster and bosses is added with data alone.

### Step 9: Clear names and release builds

* **Goal:** the data model reads clearly from end to end, and neither the repository nor a
  release holds any Blizzard file or WoW client text: the playable game is put together on the
  player's machine from their own 1.12.1 client.
* **What is distributed:** one archive holding
  `Eternal Survivors Setup.exe`, which carries our own files inside it (the game executable, the
  `content\` RON files, the README and licenses), plus the README and licenses beside it. The
  licenses are ours and `THIRD-PARTY-LICENSES.txt`, every crate compiled in with its license
  texts and copyright notices (`scripts\third-party-licenses.ps1`, run by the release build). Never an
  MPQ, texture, model, sound, DBC or FrameXML file, and no patch MPQ either: the game reads the
  player's client where it is.
* **WoW client text as pointers:** every name the 1.12 client holds
  (spell, talent and race passive names; zone, class and race names) is stored as a pointer (its
  spell, area, class or race id) and read from the player's client when the game loads. Names
  the client does not hold (a custom talent's, enemies', bosses', their yells, gear items') are
  our content and stay as text. (Done in content, tests and fixtures: `name: Spell(12294)`,
  `Area(12)`, `Class(1)`, `Race(1)`, `TalentTab(161)`, and `{Spell(772)}` inside our prose,
  read by `esurv_content::ClientNames`; `cargo run -p esurv-content --features client
  --example scan_client_names` lists every client name still written as text; comments, which
  the game never shows, are listed apart, and the few that stay text by review, such as test
  messages, are in `scan_client_names.allow` with their reasons.)
* **Already done (step 6):** what the cards say is content, not code: every spell's
  `description` in `abilities.ron` is a template in our words filled with its numbers by name
  (`esurv_content::text`, the names in `SPELL_NUMBERS`, worked out by `esurv-sim`
  `game::spell_numbers`), and the words around a card's numbers (upgrade words, stat lines) are
  one table, `content/words.ron` (screens.md, "What a card says"). The renames below keep those
  names.
* **In this order:**
  1. **List what needs a clear name:** numbered kinds and tags, terse field names, the class-gear
     item-entry scheme (done: by zone, gear.md section 3), and unclear test and function names.
  2. **Record a golden run:** a headless run with a fixed seed, its whole event log hashed and
     kept, so any change of behaviour, down to one event, shows. (Done:
     `cargo run --release -p esurv-sim --example golden_run` plays every zone with every
     class and prints one hash; run it before and after a change that must not alter play;
     `-- --log <file>` also writes every event. The current hash, 6 minutes a run:
     `9f0549729371920e`. A change meant to alter how runs play records its new hash here.)
  3. **Rename:** named kinds in the RON instead of numbers (`delivery: Nova`,
     not `delivery: 2`), clear property names, an item-entry numbering (with room for
     three weapon sets of two slots per class, roadmap R3), a one-time
     migration of existing saves (collection, worn gear), and the templates, docs and guides in
     the same pass.
  4. **Check nothing broke:** every test passes, the content loads, the golden run's hash is
     unchanged, and a live run plays as before.
  5. **Fix the path remapping (done):** the machine-local cargo config lists its
     `--remap-path-prefix` rules from most general to most specific (rustc applies the *last* rule
     that matches), the project rule naming the current folder. The release recipe (item 7) sets
     its own rules and does not depend on it.
  6. **The leak scan (done):** `scripts\leak-scan.ps1 <file | folder | .zip> ...` reads every
     file (a folder's, a zip's entries, and the files a setup carries in its payload) and finds this
     machine's user name, absolute paths into a user folder (`C:\Users\<name>\`, `/home/<name>/`,
     `/Users/<name>/`), e-mail addresses off its allow-list (noreply addresses), and Blizzard files
     by type (MPQ, BLP, M2, WMO, ADT, WDT, DBC, WAV, MP3, OGG, TTF, `.xml`/`.lua` from a client's
     `Interface`, `FrameXML` or `GlueXML`) and by their first bytes, whatever their names. Strings
     are searched as bytes and as UTF-16. Its exit code is every later item's pass/fail check.
  7. **The release build recipe (done):** `scripts\release-build.ps1` builds through the junction
     `C:\esurv-build` → the repo (compile-time `CARGO_MANIFEST_DIR` strings cannot be remapped),
     with its own rustflags (the remap rules, general to specific, and the junction's), the player
     feature set (`esurv --no-default-features`: no `dev`, so no build stamp of the project folder,
     no content watcher, no console window) and the release profile, into `target\player`, then
     scans `esurv.exe` and `esurv-setup.exe`.
  8. **How our UI ships:** compiled into the game executable (benilla's `assets/ui` and our
     `esurv-app` frames); an addon folder of ours, once there is one, goes in the game folder's
     `benilla-config\AddOns\`, where benilla loads addons.
  9. **The client check (done):** `esurv-client-check`, a crate with no dependencies: `WoW.exe`'s
     file version is 1.12.1.5875, `Data\` holds the ten base archives, `patch.MPQ` and
     `patch-2.MPQ`, and the locale is enUS (`WTF\Config.wtf` sets no other `locale`, and `Data\` has
     no other language's folder). The setup checks its own folder with it; the game checks the
     folder its data comes from at start, and a player build stops on a wrong or missing client
     with a message box that says what is wrong (a dev build, or data named by `WOW_DATA`, only
     logs it).
  10. **The setup:**
      `Eternal Survivors Setup.exe` (crate `esurv-setup`), one file. The player puts it in the WoW
      1.12.1 folder and runs it there: it checks the client in its own folder (item 9) and says
      clearly what is wrong if it is not one; then it makes or updates `Eternal Survivors\`
      inside the WoW folder with `Eternal Survivors.exe`, `content\`, the README, `NOTICE.txt`
      and the licenses (benilla's MIT and Apache-2.0), `VERSION.txt` (`v0.2.0`), and empty
      `saves\` and `logs\`. **The setup is also the patcher (v0.2.0):**
      it reads the `VERSION.txt` of a `Eternal Survivors\` already there and says what it does
      before it changes anything (`esurv_setup::Plan`): "Installing Eternal Survivors v0.2.0"
      (none there), "Updating Eternal Survivors from v0.1.0 to v0.2.0" (an older one, or "from an
      earlier version" for a game with no readable version file), "Reinstalling Eternal Survivors
      v0.2.0" (the same), and for a newer one found it warns and asks (`yes` goes back, anything
      else changes nothing); with `--no-pause` it asks nothing and refuses the downgrade (exit
      4). While the game runs from that folder (Windows holds its executable) it says so and
      changes nothing (exit 5). An update writes every carried file, removes only the files an
      older version carried and this one does not (`setup-files.txt` lists what it wrote; folders
      they leave empty go too), and never touches `saves\`, `logs\` or any file outside its
      lists (the player's own, `benilla-config\` among them; `content\` is no longer cleared
      whole). A save from an older release loads in the newer one (the save's added fields have
      defaults; `save.rs`, `a_v0_1_0_save_loads_with_everything_it_held`). The files ride inside the setup as a payload appended to its
      executable (`esurv_setup::pack`, made by the `pack_setup` example), stored as they are so the
      leak scan reads them. No Blizzard file anywhere, no patch MPQ: the game reads `..\Data`, the
      WoW folder's (`benilla_formats::wow_data`'s last rung; `WOW_DATA` still wins), and keeps its
      saves in `saves\`, its log in `logs\esurv.log` and its settings in `benilla-config\`, all in
      its own folder. `--wow <folder>` installs into another folder, `--no-pause` closes at once.
      `--relaunch` is how the game's updater starts it (server.md, "Updates"): it waits up to 60
      seconds for the game to close (`esurv_setup::wait_until_closed`; still running after that,
      it says so and changes nothing, exit 5), and once the update is in it starts
      `Eternal Survivors.exe` again; a failure then waits for Enter even with `--no-pause`, so the
      reason can be read in the console window the game opened for it.
  11. **The release archive (done):** `scripts\release-archive.ps1 [-Version v0.2.0]` makes
      `dist\eternal-survivors-<version>.zip` (the version defaults to the workspace version in
      `Cargo.toml`, the one the game's build line shows; `-Junction` as `release-build.ps1`'s):
      the setup, the README, `NOTICE.txt` and the
      licenses. The leak scan of the archive is its gate: one that fails it is deleted, never
      handed out. Beside a passing archive it writes `eternal-survivors-<version>.zip.sha256`,
      the archive's SHA-256 then its name, as `sha256sum` writes it
      (`<hash>  eternal-survivors-v0.5.0.zip`), which the game's updater checks the download
      against. The leak scan lets the user-name check pass only inside the public repository's
      slug, `Detroph/eternal-survivors`, written exactly so: the updater's feed address holds it.
  12. **The local check (in place of CI):** `scripts\local-check.ps1 [-GoldenHash <hash>] [-Junction <path>]` runs the game crates' tests
      (`cargo test --release` for `esurv`, `esurv-app`, `esurv-content`, `esurv-protocol`,
      `esurv-server`, `esurv-sim`, `esurv-client-check`, `esurv-setup`; benilla-app's own tests do not
      build on Windows), the golden run, the client-name scan, then the release build and archive
      with their scans, and prints a pass/fail line for each.
* **Done when:** the golden run is unchanged after the renames, the setup makes a game folder
  inside the WoW folder (game, content, licenses, empty saves and logs) that plays on a machine
  with only a 1.12.1 client, the repository holds no WoW client text (a scan against the client's
  name tables finds only our own names), and the scan of the release archive finds no Blizzard
  file and no machine-identifying strings.

### Step 10: Release (last)

* **Goal:** an open-source repository and a release archive built from a tagged commit.
* **Benilla:** Eternal Survivors stays built on benilla; its copyright, `docs/BENILLA.md` and its dual
  MIT OR Apache-2.0 license stay in the tree and go into the release archive with our notice.
* **In this order:**
  1. **Freeze:** steps 7b and 9 are done, main is green, the release commit is tagged.
  2. **The release archive:** built from that exact commit with step 9's recipe
     (`scripts\release-archive.ps1`): `Eternal Survivors Setup.exe`
     carrying the game executable, content, README and licenses (benilla's and ours), and beside
     it the player README with the "not affiliated with Blizzard Entertainment" note and the
     licenses, as `eternal-survivors-<version>.zip` (`v0.1.0` the first, `v0.2.0` the next).
     The GitHub release, tagged `esurv-v<version>`, attaches both files the script writes: the
     zip and its `eternal-survivors-<version>.zip.sha256`. The game's updater offers a release
     only with both, under exactly those names.
  3. **Its scans:** the archive holds no Blizzard file and no machine-identifying strings (step 9,
     item 6), and the repository holds no WoW client text in its content, code and tests (step 9:
     names are pointers, a few exceptions aside). Docs stay readable and never ship.
     `scripts\local-check.ps1` runs them all, with the tests and the golden run.
  4. **A clean machine:** the setup, run in the WoW folder of a machine with only a 1.12.1
     client, makes a game folder that plays.
* **Done when:** the tagged release archive passes its scans and plays on a machine with only a
  1.12.1 client.

## 5. Order and dependencies

```
0 Foundation ─► 1 Content ─► 2 Simulation ─┬─► 3 Server ─► 4 Presentation ─┐
                                           └─► 5 Progression ──────────────┼─► 6 Screens ─► 7 Play check ─► 7b Tuning ─► 8 Extensibility ─► 10 Release
                                                                            └─► 9 Clear names, release builds (after 3) ───────────► 10 Release
```


## 6. Risks and open questions

| Risk | Plan |
|---|---|
| Writing a server-side 1.12 protocol is large | Build only the packets the game needs, in the order the steps above require. |
| Upstream benilla changes | Keep benilla edits to documented hooks; merge upstream at milestones. |
| A Blizzard file slipping into a release | The installer is the only path assets take; CI-style release check rejects any archive containing MPQ/BLP/M2/WMO/ADT/DBC/WAV/MP3 or FrameXML files. |
| Different client builds or locales | The installer checks build 5875 and enUS up front and refuses others. |
