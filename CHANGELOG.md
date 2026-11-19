# Changelog

All notable changes to this launcher. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## Unreleased

## 0.4.1 - 2026-11-19

### Fixed

- Linux: on an NVIDIA card under Wayland, gamescope 3.17 could crash a
  few seconds into a game (the window closed, often right as a race
  loaded). The game no longer goes through gamescope's Vulkan WSI layer;
  it shows through gamescope's own Xwayland instead.

### Changed

- Updated rea-launcher-sdk to v0.11.2: gamescope's WSI layer stays out
  of the game on Linux.

## 0.4.0 - 2026-11-19

### Added

- Controls: change a button. Choose a row of the pad layout (Gas,
  Brake, a cabinet button), then press the pad button or trigger it
  should use; Reset goes back to the launcher's layout. The layout is
  saved in the data folder's `controls.cfg` and used by the window and
  `--play`.

### Fixed

- Controls: pressing the pad's buttons to try them on the Controls page
  switched pages (LB/RB) or left it (B). The pad's presses now only light
  up the live check there; press Back and Start together to move through
  the menu with the pad, and "Test the pad" to try the buttons again (#1).
- A PXN wheel (the VD6 and other models with PXN's own USB ID) was
  listed as "Other" and nothing drove the car ("No controller"); it is a
  wheel now.

### Changed

- Updated rea-launcher-sdk to v0.11.1: PXN wheels (the VD6 and the older
  Lite Star models) are wheels, and the Controls page can test the pad.

## 0.3.1 - 2026-11-18

The first published release with everything in 0.3.0 below: 0.3.0 was
tagged, but its release build stopped on the dependency audit (the zip
advisories fixed here), so it was never published.

### Security

- The board-file search and Wangan R's dongle card read zip archives with
  zip 9.7 (was 2.6.1, with RUSTSEC-2026-0367, -0368, -0369, -0429, -0545
  and -0548). Entries are still read only up to the size they must have,
  and never unpacked to disk.

## 0.3.0 - 2026-11-18

### Fixed

- Closing the window while a game ran left the game, the I/O board and
  Wine running; a killed launcher did too. Both now end the whole session.
- On Windows the launcher no longer opens a black console window beside
  its own; commands still print to the terminal that started them.
- A second launcher no longer starts while one is open (`--list` still
  works beside it; `--list --library <folder>`, which writes, doesn't).
- Stopping a game tells the I/O board to quit only once the game has
  gone (or failed to stop politely), then ends Wine.
- A setting the launcher can't read (`fullscreen = off # mine`) stays as
  you wrote it; saving another setting no longer replaces it with the
  default. A settings file or `launcher.cfg` that can't be read is never
  overwritten, and a byte-order mark no longer hides the first key.
- The command line refuses options a game doesn't take (`--render` for
  the Maximum Tune 5-6RR games, gamescope's options on Windows), as the
  window shows them off.
- A folder with a stray file named like another game's main file no longer
  hides the game in it.
- A game folder on a drive that isn't plugged in no longer stops a game
  from starting (Linux), and with your home as a game folder
  (`--library ~`) the launcher's own folder stays writable.
- The Add files page no longer reads the board files at every frame.
- A tab click followed by a double click on a game no longer counts as a
  triple click: the game starts.
- Every copy in a game's runtime folder is writable (a read-only file in a
  dump broke the next start on Windows). On Windows a file in the runtime
  folder that differs from the dump's and wasn't copied there by the
  launcher (one the game wrote) is now kept as it is instead of being
  overwritten with the dump's.
- Errors before the window opens (a refused `--play`, another launcher
  open) are written to `launcher.log` and, when someone may be watching,
  shown in a message window.

### Added

- Looks: Original, Recommended and Steam Deck set the render size, the
  output size and the upscaler at once (window and `--look`). Until you
  choose a picture, a game uses the Recommended look for your screen (on
  a Steam Deck, the Steam Deck look), saved the first time the window or
  the Windows command line sees the screen. The Linux command line, which
  can't see it, uses the look for a 1920×1080 screen for that run only.
  `--look` followed by a picture flag (`--render`, `--output`,
  `--upscaler`) makes the picture custom.
- Every control of the window has an accessible name for screen readers.
- A settings row that is off says its whole reason (it was cut off on
  Windows).
- The running game has a Stop button, and closing the window while a game
  runs stops the game first.
- A game that closes by itself within seconds of its start says so, with
  the lines of its log that tell why.
- On Linux the game folders are bound read-only around the game when
  bubblewrap is installed.

### Changed

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
- Updated rea-launcher-sdk to v0.10.0: the window, the command line, the
  library scan, the runtime folder, the game's processes and the input
  thread are now the SDK's TeknoParrot hub modules, shared with the
  Initial D, Nintendo and Sega launchers. The window keeps its look, its
  colours and its pages.

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
