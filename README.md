# Wangan Midnight launcher

One launcher for the Wangan Midnight arcade series on Linux, built on
[rea-launcher-sdk](https://github.com/akitaonrails/rea-launcher-sdk). Point
it at the folder with your own game dumps and it finds each game, sets it
up and runs it full screen, with your pad or wheel.

![The hub](docs/images/hub.jpg)

No game files are included or downloaded. The launcher never writes to your
games folder: each game runs from a private folder that links to your dump.

## What runs

| Game | Board | How it runs | State |
|---|---|---|---|
| Maximum Tune 3 (Export) | Namco N2 | natively on Linux (WanganArcadeLoader, patched to take the launcher's controls) | plays |
| Maximum Tune 3DX+ (Export) | Namco N2 | same | plays |
| Maximum Tune 5 (5.00, Japan) | Namco ES3B | Wine + DXVK + OpenParrot + this launcher's I/O board | plays |
| Maximum Tune 5DX+ (Export) | Namco ES3B | same | plays |
| Maximum Tune 6R (Japan) | Namco ES3B | same | plays |
| Maximum Tune 6 (1.03.04, Export) | Namco ES3B | same | blocked: crashes after the card prompt under Wine |
| Maximum Tune 6RR (3.05.03, Japan) | Namco ES3B | same, with OpenParrot_WMMT's 6RR module | blocked: steering-board check (E0551) |
| Maximum Tune (3.07 Rev B) | Sega Chihiro | needs four Chihiro media-board files you supply | waiting for files |
| Wangan Midnight R | Namco System 246 | needs a System 246 BIOS you supply | waiting for files |

"Plays" means a race was driven in the real game
with the launcher's input, recorded in [docs/RUNTIME_NOTES.md](docs/RUNTIME_NOTES.md).
Only the exact builds listed in the hub are started; another build is shown
with its SHA-256 so you can report it.

## What it solves

- **The I/O board.** The Maximum Tune 5–6RR cabinets read their wheel,
  pedals and buttons from a Namco JVS board. The launcher brings its own
  (`wangan-io`), so no TeknoParrot UI is needed.
- **One input path for every game.** Pads and wheels are read natively with
  SDL: analog steering and pedals, dead zone, sequential shifting on
  shoulders or paddles, wheel detection from the SDK's known-wheel table.
- **Intro movies under Wine.** The 5/6 series crashed on their first movie;
  the launcher's Wine prefix gets the two settings that make them play.
- **4K on the native games.** MT3/3DX+ draw at up to 3840×2160.
- **Clean exit.** Closing the game stops the game, then the I/O board, then
  Wine, in that order, so the cabinet never shows a board error.

## Requirements

Linux with Wine (11.x), gamescope, a Vulkan driver, 32-bit OpenGL for the
N2 games, and SDL2. The author runs it inside the `gaming` distrobox from
[distrobox-gaming](https://github.com/akitaonrails/distrobox-gaming).

## Use

```sh
wangan-launcher                                # the window
wangan-launcher --library /path/to/your/games  # remember the games folder
wangan-launcher --list                         # what was found
wangan-launcher --play mt5dxp                  # start one game
wangan-launcher --game mt3dxp --render 3840x2160 --output 3840x2160
```

Every option in the window has the same flag (`--help`).

| Pad | Cabinet |
|---|---|
| Left stick | Steering |
| RT / LT | Gas / brake |
| RB / LB | Shift up / down |
| Y | View change |
| X | Intrude |
| Back | Insert card |
| A | Test menu enter |

## Where things are

| What | Where |
|---|---|
| Settings, logs | `~/.local/share/rea-wangan/` (`launcher.cfg`, `settings/`, `launcher.log`) |
| Each game's private folder, saves included | `~/.local/share/rea-wangan/runtimes/<game>/` |
| Wine prefix | `~/.local/share/rea-wangan/prefix/` |

To remove everything, delete that folder (back up `runtimes/` first: your
card data and records live there).

## Not verified yet

- FSR 1 / NIS upscaling and the output-size option on a real screen
  (gamescope can't use FSR in the headless test display).
- Real wheels and force feedback (pads and wheels were exercised through
  the input path; no physical wheel on the test machine yet).
- Frame pacing at 60 Hz on a real display, and audio.

## Credits and licence

GPL-3.0-or-later. Built on OpenParrot, OpenBanapass, WanganArcadeLoader,
DXVK and the TeknoParrotUI source; see [THIRD_PARTY.md](THIRD_PARTY.md).
Research and decisions: [docs/RESEARCH.md](docs/RESEARCH.md).
