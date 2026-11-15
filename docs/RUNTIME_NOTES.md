# Runtime notes

What was run against the real games, how, and what was seen. Observations
only; anything not listed here has **not** been verified.

Machine: Arch (Omarchy) host, `gaming` distrobox (docker network namespace),
NVIDIA RTX 5090, driver 615.78.08. Game runs are headless: `gamescope 3.17
--backend headless` inside the box, screenshots with `gamescopectl
screenshot`, key input with xdotool on gamescope's private Xwayland display
(never the desktop's). Probe scripts live in `.work/probe/` (not committed).

## Namco N2 — WanganArcadeLoader 0.5.1 (native Linux, no Wine)

Loader: `dist.zip` from release 0.5.1, SHA-256
`3b04a845015caecbac610ec530cc35bd26ab4a1d2da718c9392fbb92bcbe66c1`
(`libwal_3dxp.so`, `libopenal.so`, `gamecontrollerdb.txt`, templates).

Runtime layout that worked (the dump is never written):
- every top-level dump entry symlinked; `main` **copied** and made
  executable (MT3's `main` has no execute bit on the share);
- `libso/`: only real ELF libraries linked; the dump's lost symlinks
  (0-byte files in 3DX+, text stubs in MT3) ignored and each library's
  SONAME link rebuilt; the dump's `libstdc++`/`libz` left out so the system
  lib32 ones load (as the loader's own `start.sh` does);
- `tmp/` from the loader's template plus `sys_04.wav`; `config.toml`,
  `keyconfig.toml` in the runtime;
- `glfix/libGLX_nvidia.so.0 -> /usr/lib32/libGLX_nvidia.so.<ver>`: the box
  has the 32-bit NVIDIA GLX library only under its versioned name, and
  without the link glvnd would not pick NVIDIA.
- run: `env LC_ALL=C LD_LIBRARY_PATH=$RT/glfix:$RT:$RT/libso
  LD_PRELOAD=libwal_3dxp.so ./main` from the runtime folder.

| Check (2026-11-13) | MT3 (Export) | MT3DX+ (EXP) |
|---|---|---|
| Boots, GPU | yes, `libGLX_nvidia` mapped in `main` | yes, same |
| Resolution `width/height` 3840×2160 | yes (2026-11-14, through the launcher): `render destination width = 3840` in the game log, attract-mode 3D car with one-pixel edges in a 1:1 crop (`.work/out/mt3-4k-b/`) | full 16:9 frame at 3840×2160, sharp UI |
| 1280×720 | yes | yes |

Since 2026-11-14 an N2 game whose render size is unset draws at the output
size (4K by default), not 1280×720.
| Boot link screen skipped with View Change | yes | yes |
| Card creation (gas = Yes), brand/car/colour/transmission, name entry (wheel left/right + gas) | yes (Skyline GT-R) | yes (GT-R R35) |
| Story mode → level select → intro → **race on the Daikoku line** | yes | yes |
| Result screen | yes | yes |
| Keyboard input | only while the game window has X focus (loader checks `XGetInputFocus`); works after focusing | same |
| Gamepad (SDL) | **not yet tested** (needs a virtual pad; `/dev/uinput` is root-only here) | same |
| Wheel | not tested | not tested |
| Audio | not checked (headless, no sink) | not checked |
| Frame rate | not measured | not measured |
| Process cleanup after TERM to gamescope | no `main`/gamescope left (checked by exe path) | same |

Not usable: software GL under Xvfb segfaults early in boot (Mesa lib32) —
the loader needs a real GPU driver.

## Namco ES3B — OpenParrot + our I/O board (Wine 11.21, DXVK 3.1.1)

Loader: teknogods `OpenParrotx64.zip` (release tag `OpenParrotx64`, asset
built 2026-09-11, SHA-256
`7da6b385b670734447fc1a56a13fb28361d517b0a4d7a56b24b2ec5c156ae5c0`):
`OpenParrotLoader64.exe OpenParrot64 "<Z: path to wmn5r.exe>"` (the full
path is required: the loader takes the working directory from it).
DXVK 3.1.1 `d3d11.dll`/`dxgi.dll` beside the exe, `WINEDLLOVERRIDES=d3d11,dxgi=n,b`.
`teknoparrot.ini` written as TeknoParrotUI's `ConfigurationWriter` does.
Runtime: small top-level files copied (the game rewrites its `F*/G*` backup
files there), big files and data folders symlinked.

### Findings (MT5DX+ Export, `wmn5r.exe` 738e88b4…2b3a2a)

1. **Without an I/O board** the game stops at boot with *E0301 I/O error1 —
   Cannot find I/O board*. OpenParrot redirects the JVS port to
   `\\.\pipe\teknoparrot_jvs`; TeknoParrotUI normally answers it.
2. **`wangan-io.exe`** (this repository) answers it: reset, address,
   identity `NBGI.;NA-JV;Ver6.01;JPN,MK3100-1-NA-APR0-A01`, features,
   main-board id (`NBGI.;WinArc;Ver2.04;JPN`), then the steady poll
   `20 01 03 / 21 01 / 22 06 / 32 03 … / 70 15 00 FF`. Boot check passes.
3. **Movies crash the game** under stock Wine: the movie player asks
   `IMFSourceReader::SetCurrentMediaType` for RGB32; Wine's source hands
   out decoded YUV and finds no YUV→RGB32 decoder (`0xC00D5212`), the game
   asserts (`EXCEPTION_BREAKPOINT`). Two prefix settings fix it, both
   verified together:
   - `HKCU\Software\Wine\MediaFoundation\DisableGstByteStreamHandler=1`
     (REG_DWORD): Wine's FFmpeg-based `winedmo` source, which emits the
     compressed WMV2/WMV3 stream as Windows does;
   - add `{MFMediaType_Video, MFVideoFormat_RGB32}` to the registered
     `OutputTypes` of `HKLM\Software\Classes\MediaFoundation\Transforms\82d353df-90bd-4382-8bc2-3f6192b76e34`
     (WMVideo Decoder MFT). Wine's decoder accepts RGB32
     (`winegstreamer/video_decoder.c`, `wmv_decoder_output_types`), but
     `wmvdecod` registers RGB outputs only as the D3DFMT-style
     `DMOVideoFormat_RGB32`, so `MFTEnumEx(WMV→RGB32)` found nothing.
   With both, the intro movies, title and attract loop play.
4. **A full guest race works** with input from the block (gas, gears,
   steering): Banapass prompt (No) → car select (RX-8) → transmission →
   story episode 1 → intro → race on the Yokohama Minatomirai route, speed
   and gear on the HUD follow the inputs (61–85 km/h, 2nd gear).
5. **Stop order matters:** when the board exits while the game runs, the
   game shows *E0303 I/O error3 — Check I/O board connection*. The
   launcher must stop the game first, then the board.

GE-Proton 8-26 was tried for comparison: white screen and a page fault —
not used.

### Per game (2026-11-13, same setup; ini generated from TeknoParrot's GameProfile the way `ConfigurationWriter` writes it)

| Game | Result | Notes |
|---|---|---|
| MT5 (Japan 5.00) | **race played** | Japanese menus, episode 1, Yokohama route |
| MT5DX+ (Export) | **race played** | see above |
| MT6R (Japan) | **race played** | wheel range 20–236; no AMAuthd needed |
| MT6 (Export 1.03.04) | **blocked** | AMAuthd started first through the loader, as TeknoParrot does. A writable, empty `TP/` folder is required (OpenParrot redirects the game's save paths there); with it, boot and the Banapass prompt work. With `Banapass Connection=0` OpenParrot picks its *legacy* module, which crashes right after "Not Use Card": `SaveGameData()` in `WMMT6_LEG.cpp` calls `fwrite(carData, 1, 0xE0, file)` on the `FILE*` it closed a line earlier (should be `carSave`). With `Banapass Connection=1` the normal module stops at *E2212 STR PCB Link error* (steering board). Needs an OpenParrot build with the fix (MSVC toolchain). |
| MT6RR | not run | upstream OpenParrot has no 6RR module; valkyrieasyraf/OpenParrot_WMMT does (needs an MSVC build) |
| TeknoParrot core | not usable | `BudgieLoader.exe <game>` exits within seconds when started without TeknoParrotUI |

Runtime rules learned: dgVoodoo leftovers (`D3D8/D3D9/DDraw/D3DImm.dll`), `bngrw.dll(.bak)` from the dump and `[SYSTEM]`/`data_jp_ori` are left out; OpenParrot's own `bngrw.dll` (OpenBanapass) is used; `data_jp/network/` is copied (OpenParrot's WMMT6 module writes certificates there).

## Through the launcher (`wangan-launcher --play <game> --headless`, 2026-11-13)

Isolated data folder, the launcher's own Wine prefix (created on first
start with the two Media Foundation fixes), runtime built from the library
by edition hash, input block driven by `examples/drive` (`--no-input`).

| Game | Result |
|---|---|
| MT5DX+ | race played (104 km/h), steering input turned the car; clean exit |
| MT5 | race played (Galant, 104 km/h) |
| MT6R | race played (91 km/h) |
| MT3 | race played from the input block with the patched loader (`patches/wal`, `libwal_3dxp.so` b5bcd16f…f8a8): card creation, name entry by steering, story race, 103 km/h, steering |
| MT3DX+ | race played from the input block with the patched loader: manual gearbox 1→2→3 on the HUD, 108 km/h, steering both ways; card insert reloads the saved card |

gamescope's FSR output fails in the **headless** backend
(`SyncobjFdToHandle … failed to create fsr output`), so headless runs use
`upscaler = linear`. FSR/NIS on a real screen: **not yet verified**.
The first launcher build forgot WanganArcadeLoader's `keyconfig.toml`; the
loader panics without it (now always written, with a test).

## OpenParrot built from source (`scripts/build-openparrot.sh`, clang-cl + xwin SDK)

Reproducible builds (byte-identical twice). Patches: `patches/clang-cl/*`
(build-only), `patches/openparrot/0001-wmmt6-legacy-save-closed-file.patch`
(the `fwrite` on a closed `FILE*`).

| Build | OpenParrot64.dll SHA-256 |
|---|---|
| upstream fc8ba63 + fix | `1812cbbb92abef6acf5c97e6604ad158da11aecce80426e20a03236d3db16949` |
| valkyrieasyraf fork a89d064 | `9a922262a5a40b6c0c1c98b4714f33cc4e01102b2473abc1e5d1f2c56283537b` |

- MT5DX+ with the self-built upstream DLL: race played (regression OK).
- **MT6**: still crashes right after "Not Use Card", with the patched and
  the prebuilt DLL alike: access violation in Wine's ntdll relative-path
  code with a garbage current-directory pointer, the main thread on an
  unusual stack — memory corruption before any save (no `openprogress.sav`
  written, so the `fwrite` fix is real but not this crash). Suspect: the
  legacy module's load hooks (`imageBase+0x6C8818`, `+0x647FB0`); not proven.
- **MT6RR** with the fork DLL (`[Resolution] RES=1360x768`, AMAuthd first):
  boots (Rev 3.05.03, I/O OK), steering check stays "checking…", then
  *E0551*. The fork hooks COM3 for the steering board; nothing answers it.
  Like MT6's *E2212 STR PCB Link error*, this needs a steering-board
  emulator whose protocol we have not found documented.

## MT6 and 6RR fixed (2026-11-13)

- **MT6 root cause** (REA + diagnostic build): the game's boost.context
  `make_fcontext` (0x140CEE280) never initialises frame+0xB8;
  `jump_fcontext` (0x140CEE2F0) loads it into the TEB's `SubSystemTib`
  (gs:[0x18]). Windows ignores that field; Wine's ntdll uses it, when
  non-zero, to find the current directory. OpenParrot's legacy module runs
  `LoadGameData` inside a fresh coroutine, so its relative `fopen` faulted
  (logged: `slotB8=0x100`, `SubSystemTib=0x100`, then the AV at
  ntdll+0x5C0E6). Fix: `patches/openparrot/0002` hooks `make_fcontext`
  (found by a unique byte pattern) and zeroes the slot.
  Open: saves are written/read back without crashing, but the game does not
  yet show restored progress (it creates a new car each time).
- **6RR root cause**: with no `setting.lua.gz` the game writes defaults with
  `mForceFeedback = 1` (steering check waits for an FFB board on COM1,
  0x140742d18) and `mIcCardVender = 1` (IC-card vendor check NG → E0551 at
  0x140732970). `patches/openparrot-wmmt/0001` seeds the game's own default
  `setting.lua` with both at 0 when missing (never overwrites), and ports
  6R's terminal-link jump fix to 6RR (+0x744552 → +0x744627, original bytes
  checked). 6RR's module ignores TeknoParrot's FreePlay, so play is on
  credits: the launcher's Start button inserts a coin. AMAuthd is started
  first.
- **Through the launcher, silent** (`PULSE_SERVER`/`PIPEWIRE_REMOTE` pointed
  nowhere), components from the self-built DLLs: 6RR race 133 km/h, MT6 race
  104 km/h with steering, MT5 and 6R unchanged (races at 103 and 86 km/h).
  Screenshots `.work/out/batch-*/`.

## Release packages (CI-built, 2026-11-13)

Built by `.github/workflows/release.yml` from pinned sources (OpenParrot and
OpenParrot_WMMT with MSVC on a Windows runner; WanganArcadeLoader and the
launcher on Debian bookworm; portable Wine 11.21 with a minimal FFmpeg 4.4.5
for its `winedmo`). Played headless and silent:

| Package | Game | Result |
|---|---|---|
| Linux `.tar.gz`, unpacked, fresh data folder, bundled Wine | MT6 | race (96 km/h) with the CI-built DLL; bundled `winedmo.so` and `libavformat.so.58` loaded |
| same | MT3DX+ | race on the Daikoku line (92 km/h) with the CI-built loader |
| Windows `.zip`, run under Wine (Windows code path: board and loader started directly) | MT6 | race (105 km/h); leftovers in the runtime stopped on exit |
| Windows launcher (local build) under Wine | MT5DX+ | race (103 km/h); board quit after the game |

Not run: 6RR from the packages; the Windows build on a real Windows PC.

## Namco System 246: pcsx2x6 v0.2.22 (Wangan Midnight R, 2026-11-15)

pcsx2x6 `pcsx2x6-v0.2.22-linux-appimage-x64-Qt.AppImage` (sha256
a28e9354…255d), extracted, run natively in gamescope; Proverb `boot.elf`
from the `nightly` `proverb.zip` (sha256 ca1a6eae…7ed2). The owner's dump:
`wanganmr.zip` (MAME dongle files, SHA-1 matching MAME) and
`wanganmr/wmr1-a.chd`. BIOS: the owner's `r27v1602f.7d`, SHA-1 matching
MAME, added with `--add-files`.

| Check | Result |
|---|---|
| MAME 2 MiB BIOS named in `[Filenames] BIOS` | boots: `BIOS Found: COH-H System 246 Rack C 20021119-163841` |
| Dongle: `wmr1vera.ic002` pages interleaved with `_spr` spare bytes as `memcards/NM00005.ps2` (8,650,752 bytes) | `McdSlot 0 [File]: NM00005.ps2 [8 MB, Formatted]`; the game accepts it |
| CHD linked as `NM00005/NM00005.chd` | opened; attract mode, intro movies |
| Coin (`[JVS] Coin1`), Start, gas to decide | CREDIT(S) 2/1, car select (Skyline GT-R), shift select, name entry |
| **Race** | driven on the Shuto against a rival: gas held, 66–81 km/h, gear 2, steering; result screen (YOU LOSE) |
| `upscale_multiplier = 6` at 3840×2160 | 4K frame, geometry sharp in a 1:1 crop; the game is 4:3, pillarboxed |
| Escape (`[Hotkeys] ShutdownVM`) | game ends, launcher session ends with status 0, no pcsx2x6 left |
| Audio | not checked. With no sound server pcsx2x6 shows an error box over the game, so headless runs write `[SPU2/Output] Backend = Null` |
| Pad (SDL bindings in `[JVS]`) | **not yet tested** (keyboard bindings were used) |

Probe runs `.work/out/wmr-1…4`; through the launcher
(`--play wangan-r --headless --no-input`, silent) `.work/out/wmr-l1/`.
