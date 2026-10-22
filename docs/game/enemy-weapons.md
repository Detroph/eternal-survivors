# Enemy weapons

What the creature enemies and bosses hold, and where it comes from. Reference: the local vmangos
database (`mangos`, 1.12 rows: the highest `patch` ≤ 10). Each of our roster entries and bosses is
matched to the `creature_template` row of its name; its `equipment_id` names a
`creature_equip_template` row (item entries `item1` main hand, `item2` off hand, `item3` ranged,
every row here `patch_min` 0 or 1 to `patch_max` 10, one row each), and each item's
`item_template` row gives its look (`display_id`), class and subclass, `inventory_type`, `sheath`
and `material`. The zone files carry them as `weapons:` (zones.md, section 2, "What enemies
hold"); the server sends them as the creature's virtual items (server.md, "What the client is
told"). Player-looking enemies (Warsong Gulch) wear their class's looks instead and are not listed.

The display ids in our zone files are not always vmangos's model for that name (several are
another creature's model of the same family); the name decides the match. Every look below is in
the 1.12 client's `ItemDisplayInfo.dbc` (test `the_creature_weapons_are_the_client_s`).

Columns: kind (our `WeaponKind`, from class/subclass), look, inventory type, sheath, material, and
the vmangos item.

## Scarlet Monastery (`scarlet.ron`)

| Enemy | vmangos entry | Main hand | Off hand |
|---|---|---|---|
| Scarlet Soldier | 4286 | OneHandedSword 7483, 13, 3, 1 (1899 Sword, Long Basic) | Shield 1705, 14, 4, 1 (1984 Shield, Kite Metal) |
| Scarlet Tracking Hound | 4304 | none (beast) | |
| Scarlet Monk | 4540 | none (`equipment_id` 0: fists) | |
| Scarlet Adept | 4296 | Staff 1600, 17, 2, 2 (1908 Staff, Crooked) | |
| Scarlet Myrmidon | 4295 | OneHandedSword 7526, 13, 3, 1 (5305 Broadsword Silver Hilt) | OneHandedSword 7488, 13, 3, 1 (1895 Sword, Short Rusty) |
| Scarlet Defender | 4298 | OneHandedSword 7487, 13, 3, 1 (1896 Sword, Short Basic) | Shield 1705, 14, 4, 1 (1984) |
| Scarlet Centurion | 4301 | TwoHandedSword 7490, 17, 1, 1 (1983 Sword2H, Basic) | |
| Scarlet Wizard | 4300 | Staff 10654, 17, 2, 2 (1907 Staff, Basic) | |
| Scarlet Champion | 4302 | TwoHandedMace 2839, 17, 1, 2 (5491 Mace2H, Large Metal) | |
| Scarlet Abbot (elite) | 4303 | OneHandedMace 5224, **21**, 3, 2 (5291 Mace, Jeweled Club) | |
| Scarlet Protector (elite) | 4292 | OneHandedMace 7478, 13, 3, 2 (2813 Mace, Standard Basic) | Shield 1705, 14, 4, 1 (1984) |
| Scarlet Guardsman (elite) | 4290 | Polearm 7437, 17, **2**, 1 (3432 Glaive - 1 Blade Basic) | |
| Houndmaster Loksey | 3974 | OneHandedAxe 7508, 13, 3, 1 (5286 One-Handed Double Axe) | the same axe |
| Arcanist Doan | 6487 | Staff 1927, 17, 2, 2 (2177 Staff, Ornate Mage Staff) | |
| Herod | 3975 | TwoHandedAxe 15576, 17, 1, 1 (7612 Axe, 2H Special NPC (Herod)) | |
| High Inquisitor Fairbanks | 4542 | OneHandedMace 7438, 13, 3, 2 (2182 Mace, Ornate Metal Hammer) | |
| Scarlet Commander Mograine | 3976 | TwoHandedMace 15786, **13**, 1, 2 (7706 Mace2H, Special NPC (Mograine)) | |
| High Inquisitor Whitemane | 3977 | Staff 15990, 17, 2, 2 (7826 Staff, Special NPC (Whitemane)) | |

## Dire Maul (`feralas.ron`)

| Enemy | vmangos entry | Main hand |
|---|---|---|
| Whip Lasher, Longtooth Runner, Sprite Darter, Ironfur Bear, Feral Scar Yeti, Warpwood Treant, Ironbark Protector | 13022, 5286, 5278, 5268, 5292, 11462, 11459 | none (`equipment_id` 0) |
| Eldreth Sorcerer | 11470 | Staff 21376, 17, 2, 2 (11424 Staff, Wooden Handle Spiral Head) |
| Gordunni Ogre | 5229 | OneHandedMace 7476, 13, 3, 2 (2809 Mace, Spiked Basic) |
| Wildspawn Satyr | 11451 | OneHandedSword 7420, 13, 3, 1 (2179 Sword, Scimitar Badass) |
| Gordunni Ogre-Mage | 5237 "Gordunni Ogre Mage" (no hyphen in vmangos) | Staff 2840, 17, 2, 2 (5303) |
| Gordunni Brute (elite) | 5232 | TwoHandedMace 3879, 17, 1, 2 (5301 Mace2H, Huge Wooden Maul) |
| Zevrim Thornhoof, Alzzin the Wildshaper, Tendris Warpwood, Immol'thar | 11490, 11492, 11489, 11496 | none (`equipment_id` 0) |
| King Gordok | 11501 | TwoHandedSword 23969, 17, 1, 1 (13316 Sword2H, Claymore Silver Yellow Glow) |

## Stratholme (`strat.ron`)

| Enemy | vmangos entry | Main hand | Off hand |
|---|---|---|---|
| Mangled Cadaver, Plague Ghoul, Ghoul Ravener, Patchwork Horror, Rockwing Gargoyle, Shrieking Banshee, Venom Belcher, Crypt Beast, Bile Spewer, Mindless Undead | 10382, 10405, 10406, 10414, 10408, 10463, 10417, 10413, 10416, 11030 | none (`equipment_id` 0) | |
| Skeletal Berserker | 10391 | OneHandedAxe 7428, 13, 3, 1 (1905 Axe, Metal Basic) | the same axe |
| Thuzadin Shadowcaster | 10398 | Dagger 19555, 13, 3, 1 (10616 Dagger, Curvey Silver) | HeldInOffHand 23175, 23, 0, 1 (12861 Item, Book - Black Skull Glowing Offhand) |
| Black Guard Swordsmith (elite) | 11121 | TwoHandedSword 23355, 17, 1, 1 (12882 Sword2H, Horde Curved Silver) | |
| Black Guard Sentry | 10394 | Polearm 22492, 17, **2**, 1 (12403 Polearm, Black) | |
| Magistrate Barthilas, Baroness Anastari, Nerub'enkan, Ramstein the Gorger | 10435, 10436, 10437, 10439 | none (`equipment_id` 0) | |
| Maleki the Pallid | 10438 | Staff 23539, 17, 2, 2 (12959 Staff, Demon Skull Staff) | |
| Baron Rivendare | 10440 | TwoHandedSword 7489, 17, 1, 1 (2181 Sword2H, Baron Rivendare) | |

The Shadowcaster casts its Shadow Bolt with its dagger and tome in hand; the Banshee wails
unarmed.

## Blackrock Mountain (`brm.ron`)

| Enemy | vmangos entry | Main hand | Off hand |
|---|---|---|---|
| Dark Iron Slaver | 5844 | OneHandedSword 7419, 13, 3, 1 (2147 Sword, Falchion) | |
| Black Broodling, Blazing Elemental, Black Drake, Fireguard Destroyer, Core Hound, Molten Giant | 7047, 5850, 7044, 8911, 11671, 11658 | none (`equipment_id` 0) | |
| Blackrock Grunt | 440 | OneHandedAxe 7428, 13, 3, 1 (1905 Axe, Metal Basic) | |
| Dark Iron Geomancer | **no row of this name**: the Dark Iron Geologist's (5839, a Dark Iron caster) | Staff 10654, 17, 2, 2 (1907 Staff, Basic) | |
| Anvilrage Guardsman | 8891 | OneHandedMace 7476, 13, 3, 2 (2809 Mace, Spiked Basic) | Shield 1705, 14, 4, 1 (1984) |
| Firegut Ogre | 7033 | OneHandedMace 7477, 13, 3, 2 (3361 Mace, Spiked Heavy) | |
| Black Wyrmkin | 7041 | Staff 10654, 17, 2, 2 (1907) | |
| General Angerforge | 9033 | TwoHandedAxe 21238, 17, 1, 1 (11342 Axe, 2H Pendulum of Doom) | |
| Warchief Rend Blackhand | 10429 | Polearm 22411, 17, **2**, 1 (12338 Polearm, Rend Blackhand) | |
| Emperor Dagran Thaurissan | 9019 | OneHandedMace 21751, 13, 3, 2 (11369 Mace, Thaurissan Hammer) | |
| Majordomo Executus | 12018 | Polearm 22031, 17, **2**, 1 (12063 Trident, Wicked) | |
| Nefarian | 11583 | none (`equipment_id` 0) | |

The Geomancer's model (8799) is vmangos's Shadowforge Citizen (8902), who carries only a gun in
its ranged slot; our Geomancer is a fire caster, so it takes the Geologist's staff instead.

No enemy in these zones holds a ranged weapon, so none shoots one: their casters (Adept, Wizard,
Mystic, Ogre-Mage, Geomancer, Wyrmkin) cast their spell with a staff in hand, the Shadowcaster with
its dagger and tome.

**Bold** marks where `item_template` differs from our player rule for the kind
(`WeaponKind::inventory_type` / `sheath`): monster polearms sheathe as staves (2), the Abbot's mace
is a main-hand item (21), Mograine's two-handed mace is marked one-hand (13). The swing does not
change with them: the client picks it by class and subclass.

## How they show and swing

The server sends each weapon as the creature's virtual item: `UNIT_VIRTUAL_ITEM_SLOT_DISPLAY + slot`
the look, `UNIT_VIRTUAL_ITEM_INFO + 2·slot` class | subclass << 8 | material << 16 | inventory
type << 24, and `+ 1` the sheath; with weapons drawn (`UNIT_BYTES_2` sheath state 1, as vmangos's
creatures). A creature holding nothing is sent none of these, as before.

The client (`creature_anim::select`) picks the swing by the main hand's class and subclass:
one-handed axe, mace and sword Attack1H (17), two-handed axe, mace and sword Attack2H (18),
polearm and staff Attack2HL (19), dagger Attack1HPierce (85), nothing AttackUnarmed (16). A swing
logged with `HITINFO_LEFTSWING` (0x04) plays the off hand's: AttackOff (87) for a weapon. A
creature with a weapon in each hand (Scarlet Myrmidon, Houndmaster Loksey) has its logged swings
alternate main hand, off hand (`Game::enemy_swings_left`); a shield never swings. Only the look:
the swings' timing and damage are the simulation's, unchanged. Rares hold their elite's.
