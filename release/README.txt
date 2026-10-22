Eternal Survivors
==================

Eternal Survivors is a free survivors-style action game set in Azeroth. Pick a hero of one of
the nine classes, enter an arena and hold out against ever larger waves of enemies: every level
offers new spells and upgrades, bosses drop gear, and talents and a collection carry over from
run to run.

Eternal Survivors is a free fan project, not affiliated with or endorsed by Blizzard
Entertainment.

It plays on your own World of Warcraft client: it carries no game files of its own and reads
the world, models, sounds and names from your installed client while it runs.


What you need
-------------

  * Windows 10 or 11, 64-bit.
  * World of Warcraft 1.12.1, build 5875, English (enUS), installed: the folder holding WoW.exe
    and the Data folder.


Install
-------

  1. Unzip this archive anywhere.
  2. Copy "Eternal Survivors Setup.exe" into your World of Warcraft 1.12.1 folder, beside
     WoW.exe.
  3. Run it there. It checks that the folder holds the right client and says clearly what is
     wrong if it does not. Then it creates the folder "Eternal Survivors" inside your WoW
     folder.
  4. Start the game with "Eternal Survivors\Eternal Survivors.exe". A shortcut to it on your
     desktop works too.

The setup and the game only read your WoW client; neither changes any of its files.


Update
------

The setup is also the patcher. Copy the newer "Eternal Survivors Setup.exe" into your WoW folder
(replace the old one) and run it there. It reads the version already installed and says what it
does before it changes anything:

  * "Installing Eternal Survivors v0.2.0"                no game there yet.
  * "Updating Eternal Survivors from v0.1.0 to v0.2.0"   an older version is replaced.
  * "Reinstalling Eternal Survivors v0.2.0"              the same version, written again (it
                                                          repairs a damaged install).
  * "... v0.3.0 is installed, newer than this setup's v0.2.0": it asks before going back; type
    yes to go on, anything else changes nothing.

It replaces the game, its content and its licenses, and removes the files an older version had
that this one no longer has. It never touches your saves, your logs, your settings or any file
of your own in the game folder, and your heroes carry over: a save from an older version loads
in the newer one.

Close the game first: while it runs, the setup says so and changes nothing.


Where your progress is kept
---------------------------

  * Eternal Survivors\saves\     your heroes and their progress (heroes.ron). Copy this folder
                                  to back them up.
  * Eternal Survivors\logs\      what the game wrote while it ran (esurv.log, and the launch
                                  before as esurv.prev.log). Send these if something goes wrong.
  * Eternal Survivors\benilla-config\   settings: key bindings, camera, chat, interface layout.


Uninstall
---------

Delete the folder "Eternal Survivors" inside your WoW folder (keep its saves folder first if
you may come back) and the setup. Nothing else on your computer was changed.


If something goes wrong
-----------------------

  * "found no World of Warcraft 1.12.1 client" or "is not that client": the game folder must sit
    inside your WoW 1.12.1 folder, as the setup makes it. Run the setup from the WoW folder.
  * The game reads its data from ..\Data, the WoW folder's Data folder. To use a client
    somewhere else, set the environment variable WOW_DATA to that client's Data folder.


Credits
-------

Eternal Survivors is built on benilla, a World of Warcraft 1.12.1 client written from scratch
in Rust, Copyright (c) 2026 Sam, under the MIT or Apache-2.0 license (LICENSE-MIT.txt,
LICENSE-APACHE.txt). See NOTICE.txt. The open-source libraries it is built with, and their
licenses, are listed in THIRD-PARTY-LICENSES.txt.

World of Warcraft and Warcraft are trademarks or registered trademarks of Blizzard
Entertainment, Inc.
