# Achievements

WoW-style achievements for the whole warband (v0.6.0, step 1A: the data, the tracking, the save
and what the screens are told; step 1B: the Achievements window, the in-run banner and the
tracker, [screens.md](screens.md) "The Achievements window" and "Achievements in a run"; step 2:
the shop and the companions, sections 10 and 11; step 3: titles, tabards, weapon glows and camp
decorations, sections 12 to 15). How to add one with data alone:
[AUTHORING.md](AUTHORING.md), section 13b, and [templates/achievement.ron](templates/achievement.ron);
a companion: section 13c and [templates/companion.ron](templates/companion.ron); a tabard, a weapon glow and
a camp decoration: section 13d.

## 1. Rules

* **Shared by the whole warband.** Every hero counts towards the same achievements, and an
  achievement is earned once for the save.
* **Points.** Each achievement gives 0, 5, 10, 25 or 50 points (Feats of Strength 0).
* **Two numbers.** The **achievement score** is the sum of the points of every achievement earned
  (one the content no longer has counts for nothing); it never drops. The **points to spend** are
  the score less what the shop took (`spent` in the save). The shop (section 10) spends only them.
* **Rewards** besides points: `Title("{name} the Eternal")` (the text, `{name}` where the hero's
  name goes; section 12), `Companion("tiny_hogger")`, `Trophy("onyxia_head")` (a trophy of
  `decorations.ron`, section 15), `Unlock("death_knight")`. An achievement may give several; what
  it gives is the warband's the moment it is earned. Unlocks are read by a later step.
* **Series.** Achievements with the same `series` are the steps of one series, in the file's
  order (Slayer I, II, III). The window shows the next step with the earlier ones under it.
* **Metas** (`AllOf`) are earned the moment their last part is.
* **Hidden** achievements show as "???" until earned (the Death Knight's Feat of Strength, which
  is `Never` for now).
* **Challenge Mode counts** for every achievement, and Endless counts its minutes.
* **Multiplayer:** heroes are a list. The tracker keeps every hero apart; whatever one hero does
  earns the achievement for the warband, and the chat names that hero.

## 2. Where it lives

| What | Where |
|---|---|
| The achievements | `content\achievements.ron` (54 shipped, 935 points) |
| The schema and its checks | `crates\esurv-content\src\achievements.rs` |
| The run's tracker (events in, achievements out) | `crates\esurv-server\src\game\achievements.rs` |
| The save's section, its counting and the grant for older saves | `crates\esurv-server\src\save\achievements.rs` |
| The messages | `crates\esurv-protocol\src\to_client.rs` (`ToClient::Achievements`, `ToClient::AchievementEarned`) |
| The companions and the shop's other wares | `content\companions.ron`, `content\shop.ron`; schema and checks `crates\esurv-content\src\companions.rs` |
| A companion in a run | `crates\esurv-sim\src\game\companions.rs` (fetching, carrying, handing over); `crates\esurv-server\src\game\companions.rs` (its unit) |
| The Shop page, the hero sheet's Companions, the camp | `crates\esurv-app\src\achievements\shop.rs`, `crates\esurv-app\src\menus\companions.rs`, `crates\esurv-app\src\picker.rs` |
| Tabards, weapon glows, camp decorations | `content\tabards.ron`, `content\weapon_glows.ron`, `content\decorations.ron`; schema and checks `crates\esurv-content\src\cosmetics.rs` |
| What heroes wear and the camps' layouts in the save | `crates\esurv-server\src\save\cosmetics.rs` |
| The new Shop tabs, the hero sheet's picks, Decorate camp, the camp's pieces | `crates\esurv-app\src\achievements\cosmetics.rs`, `crates\esurv-app\src\menus\wears.rs`, `crates\esurv-app\src\menus\decorate.rs`, `crates\esurv-app\src\picker\decor.rs` |

## 3. An achievement, field by field

| Field | |
|---|---|
| `id` | unique, lowercase; the save keeps it, so it never changes once shipped |
| `name`, `text` | our own words: its name and what to do |
| `points` | 0, 5, 10, 25 or 50 |
| `icon` | a texture name from the client's `Interface\Icons` |
| `category` | `General`, `Class("<class id>")` (the class's own page) or `Feats` (the creative feats and the Feats of Strength) |
| `rewards` | optional list: `Title(text)`, `Companion(id)`, `Trophy(id)`, `Unlock(id)` |
| `series` | optional: `Some("<series id>")` |
| `hidden` | optional: `true` shows it as "???" until earned |
| `criteria` | what earns it (section 4) |

## 4. Criteria

| Kind | Earned |
|---|---|
| `Win([conditions])` | when a run is won, for each hero standing (not fallen for good) for whom every condition held; `Win([])` is any win |
| `Fight((boss: (zone, boss), ...))` | when that boss falls, for each hero standing, won run or not (section 4b) |
| `Counter(counter: ..., goal: Count(n) \| All)` | when a warband counter reaches its goal (section 4c); `All` is every one the content has |
| `Checklist(EveryStartingOption(class) \| EveryLoadout(class))` | when every item has been won with (section 4d) |
| `AllOf([ids])` | when every one of these achievements is earned (never itself, never a hidden or `Never` one) |
| `Level(n)` | the moment a hero reaches level `n` in one run, Endless too (Peak of Power: 99, the most) |
| `Never` | not yet: kept for something still to come |

### 4a. Won-run conditions

Each is checked for one hero, as the run is won.

| Condition | Holds when |
|---|---|
| `Zone("brm")` | the run is in this zone |
| `Class("mage")` | the hero is of this class |
| `Start("fireball")` | the hero plays this starting option (ids are unique among all classes) |
| `Loadout(class: "mage", id: "staff")` | the hero is of this class and fights with this weapon loadout |
| `ChallengeAtLeast(200)` | the run is in Challenge Mode with its slider at this or more (whole percentages: 200 is 2x) |
| `RunOptionsAtMax` | every slider of `run_options.ron` is at its highest |
| `NoGoldSpent(collected_at_least: 1000)` | the hero spent no gold all run (it holds all it picked up: caches and rerolls spend) and picked up at least this much |
| `NoBattleStandard` | the hero took no battle standard all run |
| `HealthNeverBelow(0.5)` | the hero's health was never under this share of its most all run |
| `DroppedBelow(share: 0.2, back_to: 0.3, times: 9)` | the hero's health fell under this share at least this many separate times; a time ends once its health is back to `back_to` (under 20 %, back to 30 %), so hovering around the line counts once |
| `HealthLostAtLeast(50000.0)` | the hero lost at least this much health over the run (health lost, after shields) |
| `KillingBlows(by: ..., at_least: n)` | these sources landed at least this many killing blows |
| `TopDamage(...)` | these sources' lines on the hero's damage meter are together its top line: above 0 and at least every other line |
| `Boss((boss: ..., ...))` | a boss fight in the run went this way (section 4b) |

**Sources** (`by` and `TopDamage`):

* `Spells(["mutilate", "deadly_poison", "envenom"])`: these spells, their damage over time ticks
  included (a tick an aura of a milestone puts on is that milestone's, as on the meter).
* `Pets`: the hero's own pets (a battle standard's ally's pets are the ally's).
* `Swing`: the hero's weapon swing.

A **killing blow** is the last hit an enemy took before it died, as the balance check's `--feats`
counts it; a hit from a source no achievement names (a proc, an ally) leaves the enemy with no
killing blow of a named source.

Health is read after every tick of the server, as a share of the hero's most health then.

### 4b. Boss fights

`(boss: (zone: "elwynn", boss: 0), within_seconds: Some(20.0), lost_at_most: Some(0.05), never_below: Some(0.5))`:
the boss is named by its zone and its place in the zone's `bosses` list (0 first). Every condition
given must hold for a hero standing when the boss falls:

* `within_seconds`: it fell within this many seconds of its arrival.
* `lost_at_most`: the hero lost at most this share of its most health from the boss's arrival to
  its fall (all health lost in that time, whatever hit it).
* `never_below`: the hero's health was never under this share in that time.

### 4c. Counters

| Counter | Counts | `All` |
|---|---|---|
| `Kills` | enemies killed by every hero, every run | (a number only) |
| `ZonesCleared` | different zones won, any mode | every zone |
| `BossesDefeated` | different bosses that fell in a run, each zone's own, Warsong Gulch's flag carriers too | every boss (54) |
| `LegendaryAmulets` | different legendary amulets in the codex (seen on an offer card counts, as the codex counts) | every legendary |
| `EndlessMinutes` | the most whole minutes of Endless one hero survived in one run (never a sum) | (a number only) |
| `ClassesWon` | different classes that won a run | every class |
| `GoldCollected` | gold collected by every hero, every run (a companion's once handed over, the gold card's too), added as the run pays out (Hoarder) | (a number only) |

### 4d. Checklists

* `EveryStartingOption("warrior")`: one mark for each of the class's starting options won with.
* `EveryLoadout("warrior")`: one mark for each of the class's weapon loadouts won with.

## 5. When an achievement is earned

* **A boss fight:** the moment the boss falls.
* **A kill counter's step:** the moment the kill that reaches it lands (the run's kills go into
  the save then, and at every pay-out: the end of the run, leaving it, logging out).
* **A gold counter's step** (Hoarder): the moment the gold that reaches
  it comes in, picked up, from a gold card or handed over by a companion (the run's gold goes into
  the save then; the pay-out adds only what is not in it yet, so nothing counts twice).
* **The tracker moves live:** while a kill or gold achievement is tracked, the list is told again
  in a run, at most once a second when either moved, its counters counting what the save does
  not hold yet.
* **Endless minutes:** as each whole minute passes.
* **Won runs, checklists, zones, classes:** the moment the run is won (once per run).
* **Legendaries:** the moment a legendary amulet's card is shown (the codex keeps it then).
* **Metas:** with their last part.
* **An achievement added to the content later:** the next time anything is counted (the game
  counts once as it starts), from what the save has kept.

## 6. What the save keeps

A warband-wide section `achievements` in the save (sealed like the rest, written after each
change):

| Field | |
|---|---|
| `earned` | each achievement earned, by id, with when (Unix seconds) |
| `kills` | enemies killed by every hero |
| `gold_collected` | gold collected by every hero, counted from the version that kept it (0 in an older save) |
| `zones_cleared`, `classes_won`, `starts_won` | zones, classes and starting options won with, by id |
| `loadouts_won` | loadouts won with, as `class/loadout` |
| `bosses_defeated` | bosses that fell, as `zone/index` |
| `longest_endless` | the most seconds of Endless survived in one run |
| `spent` | the points the shop took |
| `bought` | what the shop sold, as its items (`companion:<id>`, `companion_rank:<n>`, `banish_rank:<n>`, `skip_rank:<n>`), in the order bought |
| `tracked` | the achievements tracked in runs, by id, in the order ticked (at most three; one earned drops out as it is told) |
| `seen` | how many achievements were earned when the Achievements window was last opened (more now makes its button glow) |
| `camps` | each camp's decorations (`horde`, `alliance`): the piece on each spot, by id (section 15) |

The counters, checklists and metas are counted from these facts, never kept apart, so a new
achievement counts what was kept before it.

Each hero keeps its own `companion` (an id, or nothing) beside its zone and starting option, and
`wears`: its `title` (the id of the achievement giving it), `tabard` and `glow` (ids), each only
when picked.

## 7. What counts from a save from before achievements

A save written before v0.6.0 has no `achievements` section. The first time the game reads it (at
start), it is given one from what it can prove, every grant dated that moment:

* **Zones cleared:** every zone any class has won (the save keeps Normal Mode wins only).
* **Classes won:** every class that has won a zone, and every class whose hero has a win in its
  records.
* **Bosses defeated:** each cleared zone's final boss (a win proves it fell); the other bosses
  start at nothing.
* **Won-run achievements** that ask only for zones and classes a class's won zones show: First
  Victory.
* **The codex** as it is: a legendary amulet in it counts for First Legend.
* **What follows from these:** Wanderer, Pathfinder, The Eternal Road and Of Nine Paths when the
  zones and classes are there.
* **Never kept, so from nothing:** kills, Endless minutes, the starting options and loadouts won
  with (the save kept only the one picked last), boss times, gold, Challenge Mode clears.

The game's log says each achievement granted this way (`esurv: achievement earned from the save:
<id>`). Talents and prestige earn nothing (no achievement asks for them).

## 8. What the screens are told

Both are addon lines of esurv-protocol (a long one goes in pieces).

**`ToClient::Achievements`** (`ESURVAL`), the warband's achievements:

| Field | |
|---|---|
| `score` | the achievement score |
| `balance` | the points to spend |
| `earned` | each achievement earned: `(id, when)`, when in Unix seconds, oldest first |
| `progress` | for each achievement not yet earned (and not hidden) that counts something: `id`, `current` of `goal` (a counter's number; a checklist's or a meta's items done), and `done`, the items done by id (a checklist's starting option or loadout ids, a meta's achievement ids; empty for a counter) |
| `tracked` | the achievements tracked in runs, by id, in the order ticked (none earned) |
| `news` | whether anything was earned since the Achievements window was last opened |
| `bought` | what the shop sold the warband, as its items, in the order bought |
| `companion_rank` | the companion upgrade's rank the warband has (1 from the start) |
| `banish_rank`, `skip_rank` | the banish and skip upgrades' ranks the warband has (0 until bought) |
| `zones_cleared` | the zones the warband has cleared (the pennant line, the zone trophies) |
| `camps` | each camp's layout: `camp` and its `(spot, piece)` pairs, those that may still stand (owned, fitting, the spot open) |

Told before a run with a class's facts (`PreRunAsk::Facts`: after the codex) and as the answer to
`achseen` and `track <id> <on>` (the window opened; an achievement tracked or no longer), and in a run after
`hello`, after every achievement earned, and after a pay-out that counted kills. In a run its
counters for kills and gold collected include what the save does not hold yet (section 5).

**`ToClient::AchievementEarned { hero, id }`** (`ESURVAG`): in a run, to every client with
screens, the moment hero `hero` (its player guid) earns achievement `id` for the warband: the
banner's moment. For what one tick earns, the chat lines go first, then one `AchievementEarned`
for each achievement, then one `Achievements` with them all.

**Chat:** every player's chat gets a system line `[Hero] has earned the achievement [Name]!`.

**What the window takes from the content** (`content\achievements.ron`, which the app loads):
each achievement's name, text, points, icon, category, rewards, series and hidden flag; a
checklist's items in order (the class's starting options or loadouts, with their names); a meta's
parts. An earned hidden achievement shows as itself.

## 9. Checks

* `cargo test -p esurv-content`: the shipped 54 load with their points (935: classes 405, general
  350, feats 180), the template is valid, and every broken reference is refused (unknown zone,
  class, starting option, loadout, spell, boss, meta id; points off the scale; a goal of 0; `All`
  on a plain count; a meta of itself or of a hidden one; a series of one step or over two
  categories).
* `cargo test -p esurv-server achievements`: every won-run condition, killing blows, the meter's
  top line, drops below a share, boss fights, the game earning and telling them as a run goes
  (fight, kill step, win once, Endless, kills at the pay-out), the counters, checklists and metas,
  the older save's grant once and sealed, a new save across a restart, and every icon in the
  player's client (when the client is there).
* `cargo test -p esurv-protocol`: both messages read back, in pieces when long; `buy` and
  `companion` read back.
* `cargo test -p esurv-content --test companions`: the 16 achievement companions are listed with
  no price, 26 sell at 25 and 20 at 50, the upgrade has three ranks, the template is
  valid, and a broken companion is refused (unknown pet npc, size 0, no price and no achievement,
  both, price 0, an achievement that does not give it, listed twice, an achievement's companion
  missing or sold, ranks that do not grow).
* `cargo test -p esurv-server shop`, `... companions`: the shop spends points to spend and never
  the score, sells a companion once and the ranks in turn, never sells an achievement's
  companion, a hero picks only what the warband owns, kept across a restart; a companion's unit is
  friendly and untouchable, the Eye glides; every companion icon is in the player's client.
* `cargo test -p esurv-sim companions`: a companion carries three piles and the gold counts only
  as it hands them over (with the coin sound), brings back what it has when nothing is left in
  reach, reaches farther and carries more at rank 3, leaves the hero's own pickup range alone, and
  hands over what it carries when the run ends.
* `cargo test -p esurv-app shop`: the Shop's sections and tiles, and what is owned.
* `cargo test -p esurv-content --test cosmetics`: the titles read as intended, the tabards
  swap by faction and a faction's own stays with it, the damage glows are a ladder and glows skip
  shields, trophies are earned and the throne climbs, sets give their signposts, and a trophy for
  sale or one an achievement names but the file lacks is refused.
* `cargo test -p esurv-server wears`, `... gold`, `... foal`: heroes wear only what the warband
  owns (a faction's own tabard only for its heroes), each camp is decorated apart and kept across
  a restart, a run's name, tabard and glow; Hoarder earned the moment the gold comes in and the
  payout adding only the rest, the tracker told live; the foal's kits as it appears and hands
  over; every cosmetic icon in the player's client.
* `cargo test -p esurv-app cosmetics`, `... decor`: the new Shop tabs' tiles and states, the
  hero sheet's picks, and what each camp shows.

## 10. The shop

The warband screen's **Shop** button opens the shop alone ([screens.md](screens.md), "The Shop"). Everything bought
is the whole warband's and is bought once.

* It spends the **points to spend**, never the score; a price above them is refused.
* **Companions** are sold at their `price` (`companions.ron`). One that an achievement gives has
  no price and is never sold: it shows dark with its achievement's name until earned.
* **The companion upgrade** (`shop.ron`, `companion_ranks`), one for every companion of the
  warband: rank 1 from the start, rank 2 for 50 points, rank 3 for 100, each only after the one
  before. The prices and reaches are the plan's drafts; the speeds and piles carried are fixed.


| Rank | Price | Reach past the pickup range | Run speed | Piles carried |
|---|---|---|---|---|
| 1 | owned | 8 yd | 9 yd/s | 3 |
| 2 | 50 | 12 yd | 15 yd/s | 6 |
| 3 | 100 | 16 yd | 21 yd/s (3x the hero's 7) | 9 |

* **Banish** and **Skip** (`shop.ron`, `banish_ranks`, `skip_ranks`),
  each three ranks bought in turn, for every hero in every run. One shared count of banishes and
  one of skips per run, spent on level-up screens and caches alike (`esurv-sim` `Run::banish`,
  `Run::skip`; the server reads the ranks into `HeroSetup` at the run's start):

| Rank | Price | Banishes (Banish) or skips (Skip) per run |
|---|---|---|
| 1 | 25 | 1 |
| 2 | 50 | 2 |
| 3 | 75 | 3 |

  A banished card (a spell, a ring or trinket, an amulet, the empower, the gold card: one key each,
  `choices::banish_key`) never comes back that run, rerolls included.

* Asked with `buy <item>` (`PreRunAsk::Buy`); `ToClient::Achievements` then tells `bought` and
  `companion_rank`, `banish_rank` and `skip_rank` too. Step 3 sells `tabard:<id>`, `glow:<id>`,
  `decoration:<id>` and `camp_spot:<id>` the same way (sections 13 to 15); a damage glow and the
  Great Anvil only once what they need is owned (`Content::shop_needs`).

## 11. Companions

A small pet that follows its hero and fetches gold. **2026-10-27):**

* **Reach on top of the pickup range.** Gold within the hero's own pickup range is the hero's as
  ever. The companion goes for the piles past it, as far as the upgrade's reach goes beyond it
  (rank 1: pickup range + 8 yd), the nearest to it first, never one flying (a magnet's) or one
  another companion is fetching.
* **It carries, then hands over.** It runs out at its rank's speed, picks the piles up one after
  another and carries them; with as many as its rank carries (3, 6, 9), or none left in reach, it
  runs back. Within 2 yd of its hero it hands them over: only then does the gold count (times the
  hero's gold bonus, as a pile it took itself) and the coin sound plays (`SoundEntries.dbc` 120,
  the stock loot window's coin), never on picking a pile up.
* Following or running back it is never slower than its hero's run speed x 1.2, so a hero made
  faster is not left behind; with nothing to fetch it keeps 2.5 yd to its hero's back right. Over
  60 yd away (a teleport) it is put back beside it.
* **When its hero is down** it waits beside it, carrying what it has. **When the run ends** (won
  or lost) what it still carries is handed over, so nothing is lost.
* **It never fights and never blocks:** it is no unit of the fight (`Run::companions`, apart from
  the pets), and on the clients it is a friendly unit of its look that can be neither attacked
  nor selected, its entry its own (`entries::companion`, from 500 000) so the creature query names
  it, a critter. A look with no walk glides `hover` yards up, bobbing, on flights as a flying pet
  does (the Eye of C'Thun).
* **Its size** is its look's own (`CreatureModelData` x `CreatureDisplayInfo` scale, read from
  the player's data) times its `scale`, as a server folds it into `OBJECT_FIELD_SCALE_X`; a pet
  look keeps `scale` 1.0, a creature look shrinks it (King Gordok 0.15).
* **Each hero picks its own** (or none) on its hero sheet ([screens.md](screens.md), "The
  Collection tab"), kept in the save (`companion <guid> <id>`, `~` for none; told back in
  `ToClient::Hero` `companion`). A run brings the hero's pick while the warband owns it, at the
  warband's upgrade rank.
* **At the camp** the picked companion stands beside its hero at the camp fire, a step to its
  left as the camera sees it.
* **Spell looks** (`kits`): a companion may play `SpellVisualKit.dbc` kits on itself, at its own
  size (`SMSG_PLAY_SPELL_VISUAL`), as it appears in a run and as it hands gold over. The Warhorse
  Foal (in place of the plan's glow) plays its counterpart's, Summon
  Charger's (spell 23214, SpellVisual 3339): its cast kit 165 (Holy_Precast_Uber_Base and the
  hands' light) as it appears, its lighter precast kit 193 (Holy_Precast_High_Base) as it hands
  gold over. Its model's own ground light shows as a soft square under it (the Charger's, not ours).
* The Eye of C'Thun's model has no blink of its own, so it only bobs.

## 12. Titles

* **What gives one:** an achievement's `rewards: [Title("...")]`, its text with `{name}` where the
  hero's name goes. The WoW-like mix: a suffix stays after the name,
  a noun takes a comma, an adjective goes first.

| Achievement | Title |
|---|---|
| The Eternal Road | Lumi the Eternal |
| Bossbane | Lumi, Bossbane |
| Of Nine Paths | Lumi of Nine Paths |
| Spellbound | Spellbound Lumi |
| Well-Armed | Lumi the Armsmaster |
| Gnoll Means No | Lumi, Gnollslayer |
| Lord of Nothing | Lumi the Fivefold |
| Eye Exam Passed | Lumi, Eye of the Old God |
| Tortoise Wins | Lumi the Unending |

* **Each hero picks one or none** on its hero sheet (section 13's picks), by the achievement's
  id (`dress <guid> title <achievement id>`, empty for none), once the warband earned it.
* **Shown** with the name over the camp fire at the camp, and in runs: the server names the hero
  with its title (`SMSG_NAME_QUERY_RESPONSE`, also sent as the hero joins, since the character
  list named it without), so it shows over its head when names are shown (decision 4), on
  its unit frame and in the chat's lines.

## 13. Tabards and weapon glows

* **Bought warband-wide** in the Shop (its Tabards and Enchants tabs), for points to spend;
  prices are the plan's drafts (`tabards.ron`, `weapon_glows.ron`, each marked).
* **Each hero wears one of each, or none**, picked on its hero sheet's Equipment view (the Title,
  Tabard and Weapon glow boxes under the Companion box), kept in the save (`wears`), asked with
  `dress <guid> <tabard|glow> <id>` and told back in `ToClient::Hero` (`title`, `tabard`, `glow`).
* **Tabards** (14): a look is `Neutral` (any hero), `Faction(horde, alliance)` (a pair sold as one,
  each hero showing its own faction's) or `Only(faction)` (that faction's heroes only: Stormwind).
  The Officer's Tabards left out (their looks are the Knight's Colors' and the
  Stone Guard's Herald's), Lordaeron and Darkmoon added. A tabard is worn over any armour look:
  in runs equipment slot 18 shows it (an armour look entry the item query answers as a tabard,
  `game::tabard_entry`), the character list carries it, and the camp's figures and the hero
  sheet's figure wear it. A hero whose race moved it to the other faction no longer shows the
  first faction's own tabard.
* **Weapon glows** (18): a `SpellItemEnchantment.dbc` enchant whose item visual is the glow,
  shown on all the hero's weapons, the main hand, an off-hand weapon and the ranged weapon, never
  a shield, a held off-hand item or a relic (`esurv_content::glows_on`). In runs
  the hero's visible-item enchant fields carry it; at the camp benilla's scene figures take it
  (`FigureHero::enchants`). The damage glows are a ladder, each sold only once the one before is
  owned, 25, 50 and 75 points; Feedback (the purple one) 50. The hero sheet's
  figure shows the tabard only: the glue booth draws a glow's models flat white.
* Nothing of either changes a number of the game.

## 14. What a hero wears, in short

| | Where it is picked | Camp | Run |
|---|---|---|---|
| Title | hero sheet, Equipment | with the name over the fire | over the head when names are shown, unit frame |
| Tabard | hero sheet, Equipment | on the hero's figure | slot 18 over the armour |
| Weapon glow | hero sheet, Equipment | on the figure's weapons | on the hero's weapons |
| Companion | hero sheet, Equipment | beside the hero | follows it, fetches gold |

## 15. Camp decorations

**Parked for a later 3D hub**: the warband screen shows no camp
decorations, only the plain camp fire as before them; the Decorate camp button, the Shop's
Decorations tab and the trophy lines among achievement rewards are hidden too. The content
(`decorations.ron`), the save's `camps`, the asks and the code all stay, switched off by one
switch (`CAMP_DECORATIONS` in `esurv-app` `achievements/cosmetics.rs`), so the hub can take them
up. What follows is how they work when switched on (the clean-up's layout of 2026-10-27; the
warband screen's camera stays as it was). The walkable hub, on unless `ESURV_HUB=0`, has taken
them up: its spots are free, each hero's tent its own, and the Shop's Decorations tab shows with it.

* **The Horde and the Alliance camp are decorated apart**, each with its own layout (the save's
  `camps`); what the warband owns may stand in both. A piece stands on one spot of a camp at a
  time (setting it elsewhere moves it). Asked with `decorate <camp> <spot> <piece>` (empty for
  none), answered with `Achievements`.
* **Every family has its own place** (the camp clean-up): the fire is the only fire; the camp theme's pieces stand low
  to the right of the fire; the trophies gather in one trophy corner at the back right, the
  throne beside them; the zone pennants hang along the back edge; the skull spear stands alone at
  the back left; one camp sign. Left of the fire (the companions' side) and the middle of the
  back edge stay clear. Every place is yards from the fire as the camera sees it.
* **The spots** (`decorations.ron`, `spots`): the camp fire, the banner post, the camp theme and
  2 extra spots bought for 150 points each (draft), once for both camps.
* **What a spot takes:** the fire a bought fire look (empty: the fire progress grows); the
  banner post the battle banner; the camp theme one bought set (`Theme`: its id), shown once a
  piece of it is owned; an extra spot any ground piece owned (`Ground`).
* **One set at a time**: a camp shows the set chosen as its theme,
  each of its pieces the warband owns in its own place (`at`, the set's hand-placed arrangement).
  A piece set out on an extra spot stands there instead of in its set's place.
* **Bought in 4 sets** (prices are the plan's drafts): War Camp, Tavern Night, Forge Row, Wild
  Lights. Owning a whole set earns its signpost, free; the one camp sign stands once any is earned
  (`camp_sign`). The Great Anvil is sold once the rest of Forge Row is owned (the plan's "needs the
  other three", read as the anvil, tool rack, brazier and forge).
* **Trophies are never sold** and stand on no spot: every trophy earned stands in the trophy
  corner, in its place (`at`). Each comes with an achievement (`Earned::Achievement`, the
  achievement giving `Trophy(id)`) or a zone's first clear (`Earned::ZoneClear`: Nefarian's head
  for Blackrock, the troll skull pile for the Gurubashi Arena, the ogre head for Dire Maul, the
  skull lights for Stratholme, the sword in a skull for Warsong Gulch). The throne is one trophy
  whose look is the highest step earned (`Earned::Ladder`: Dark Iron at 2x, Blackwing at 3x,
  Kel'Thuzad's at 5x). Until earned a trophy shows grey in the lists with what earns it.
* **Faction looks swap by themselves:** a piece's `Faction(horde, alliance)` look is each camp's
  own (the heads on a post in the Horde camp and hanging in the Alliance camp, banners, braziers),
  each at its own size when the two models differ (`horde_scale`, `alliance_scale`).
* **Grows with progress:** the camp fire (normal, blue at 2x, Karazhan's at 3x, the great
  Midsummer bonfire at 5x: the Challenge achievements; every look the same footprint, about a yard
  and a half: what grows is the look, not the size), the pennant line (a rope along the back edge
  from the start, a pennant on it for each zone cleared, in the zones' order; a zone with a
  faction's banner shows the camp's own, no banner of the other faction in a
  camp; Sunken Temple a troll tiki head, Ahn'Qiraj a crystal spike, Duskwood the Holy banner) and
  the skull spear (standing from Slayer I, skulls piling up at its foot with each Slayer step).
* **Sizes**: nothing bought or earned stands taller than a hero but
  the battle banner, the camp's one landmark: ground pieces knee to shoulder high, trophies
  shoulder high, the pennants small. A piece made for a city (the great fires, the heads' posts,
  the thrones, Khadgar's statue, the Great Anvil) is shrunk to a camp's size (`scale`, drafts of
  the look measured from each model's bounds, no numbers of the game).
