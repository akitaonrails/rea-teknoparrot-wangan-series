# Changelog

All notable changes to this launcher. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

### Changed

- Updated rea-launcher-sdk to v0.10.0: the window, the command line, the
  library scan, the runtime folder, the game's processes and the input
  thread are now the SDK's TeknoParrot hub modules, shared with the
  Initial D, Nintendo and Sega launchers. The window keeps its look, its
  colours and its pages.
- The Barlow typefaces are inside the program; the packages no longer
  carry a `components/fonts` folder.
- `launcher.cfg` can name several game folders: `--library` (and the
  window's Game folders page) adds a folder instead of replacing the one
  before. The `library = …` line 0.2.0 wrote still reads.
- The command line is the hub family's: `--set <game> <options>` saves
  options (`--game` still works), `--check [<game>]` says whether a game
  could start without starting it, `--look`, `--key=value`, and the test
  options `--input-script` and `--seconds`. Exit codes: 0 done, 1 refused,
  2 bad command line.
- Each game's log (`<game>.log` in the data folder) starts afresh at every
  start.

### Added

- Looks: Original, Recommended and Steam Deck set the render size, the
  output size and the upscaler at once (window and `--look`). The first
  time a game's settings are read, the Recommended look for your screen
  is saved (on a Steam Deck, the Steam Deck look).
- Every control of the window has an accessible name for screen readers.
- A row that is off says its whole reason (it was cut off on Windows).
- The running game has a Stop button, and closing the window while a game
  runs stops the game first.
- A game that closes by itself within seconds of its start says so, with
  the lines of its log that tell why.
- On Linux the game folders are bound read-only around the game when
  bubblewrap is installed.

### Fixed

- Closing the window while a game ran left the game, the I/O board and
  Wine running; a killed launcher did too. Both now end the whole session.
- On Windows the launcher no longer opens a black console window beside
  its own; commands still print to the terminal that started them.
- A second launcher no longer starts while one is open (`--list` still
  works beside it).
- A setting the launcher can't read (`fullscreen = off # mine`) stays as
  you wrote it; saving another setting no longer replaces it with the
  default. A settings file or `launcher.cfg` that can't be read is never
  overwritten, and a byte-order mark no longer hides the first key.
- The command line refuses options a game doesn't take (`--render` for
  the Maximum Tune 5-6RR games, gamescope's options on Windows), as the
  window shows them off.
- A folder with a stray file named like another game's main file no longer
  hides the game in it.
- The Add files page no longer reads the board files at every frame.
- A tab click followed by a double click on a game no longer counts as a
  triple click: the game starts.
- Every copy in a game's runtime folder is writable (a read-only file in a
  dump broke the next start on Windows).
- Errors before the window opens (a refused `--play`, another launcher
  open) are written to `launcher.log` and, when someone may be watching,
  shown in a message window.

## 0.2.0 - 2026-11-17

### Added

- Wangan Midnight R (Namco System 246) plays on Linux through pcsx2x6,
  upscaled to fit the window (6× at 2160p), with the same pad and wheel
  controls as the other games. It needs the System 246 board files.
- Add files on a game's tile (or `wangan-launcher --add-files <folder>`)
  looks through a folder or zip and keeps only the board files whose size
  and SHA-1 match MAME's records, under any name. The launcher never
  downloads them.
- The I/O board gains a Chihiro mode (`wangan-io --chihiro`) for Maximum
  Tune 1, which stays off (its emulator freezes in most runs under Wine).

### Changed

- MT3/3DX+ draw at your output size when no render size is set.

## 0.1.0 - 2026-11-13

First release: Maximum Tune 3, 3DX+ (natively on Linux), 5, 5DX+, 6, 6R
and 6RR (OpenParrot under Wine, or directly on Windows) from one hub.
