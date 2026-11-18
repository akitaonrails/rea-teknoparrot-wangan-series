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

Built by the release workflow (now `.github/workflows/build.yml`) from pinned sources (OpenParrot and
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

## Sega Chihiro: Cxbx-Reloaded CI-585c49a under Wine 11.21 (Maximum Tune 1, 2026-11-15)

Not playable yet. Probe only (`.work/c1/`, nothing wired into the
launcher). `CxbxReloaded-Release.zip` sha256 010d1e85…3bf43, portable
`settings.ini` beside the exe, the owner's four media-board files (MAME
SHA-1s) in `EmuMediaBoard/`, a probe prefix.

| Step | Result |
|---|---|
| vcrun2019 (MSVCP140 14.29) | crash `0xC0000005` inside `MSVCP140.dll` at start; this build needs the 2022 runtime |
| vcrun2022 + d3dcompiler_47 | starts; Chihiro detected from `boot.id` |
| Boot path | `cxbxr-ldr` writes `Partition2/3.bin` from `fpr21042`, then starts a **second** loader process on SEGABOOT and exits: the session must wait for the Wine server, not the first process |
| No sound device (silent test) | fatal "select a valid audio adapter"; an ALSA `null` PCM as Wine's audio device works and stays silent |
| SEGABOOT | draws ("Initializing media board"); once ended at *Error 22: communication error between main board and media board*, later runs passed it (timing) |
| Game `A\V307.xbe` | started by SEGABOOT, writes `backup_ram.bin` and `ic11`, but the screen stays black for 5 minutes |
| Without `boot.id` | not possible: Cxbx-R's media board aborts "Could not open Chihiro boot.id" |

Next to look at: what V307 waits for at the black screen (the dump
ships YACardEmu, a card-reader emulator on a named pipe; Cxbx-R's
media-board stub answers only some commands).

## Sega Chihiro: TeknoParrot's Cxbx-Reloaded fork under Wine 11.21 (Maximum Tune 1, 2026-11-16)

Boots and plays some of the time, freezes most of the time: **not
ready**. Probe only (`.work/c1/`). Emulator: TeknoParrot's
`cxbxr_1.0.0.20.zip` (`cxbxr-emu.dll` sha256 4396e61e…), built from
the public GPL-2.0 fork `ReaverTeknoGods/Cxbx-Reloaded` master
2ed90fe6 (2026-08-08, no newer commits on 2026-11-16). Upstream
CI-585c49a lacks the fork's MT1 symbol fixes, hence its black screen.

| Step | Result |
|---|---|
| Data layout | "TeknoParrot mode": `<exe>/TeknoParrot/settings.ini`, `EmuMediaBoard/fpr21042_m29w160et.bin`, `EmuMediaBoard/Chihiro/{ic10_g24lc64,pc20_g24lc64,ic11_24lc024}.bin`; per-game state under `TeknoParrot/Games/<key>/` |
| `beta.ini` beside the exe | `wmmt_device_poll_yield_ms=1` and `wmmt_gamepad_init_bypass=1` needed under Wine (both 0: black screen; yield only: white screen) |
| Card reader | the fork is a client of `\\.\pipe\YACardEmu`, connecting once: the dump's `YACardEmu.exe` (`targetdevice = C1231LR`, API bound to 127.0.0.1) must start first, else "Card writer error E55" |
| Controls | read only from the `TeknoParrot_JvsState` section; `wangan-io --chihiro` fills it from the launcher block: coins (CREDIT(S) 2/2), gas, wheel and the card screens work |
| Seen in the game | handle check, attract (Namco logo, Sofdec intro, ranking), Purchase card → brand → body color (Chaser) → transmission → time zone → C1 inbound race running (once) |
| Freeze | most runs stall on a black screen at the attract or right after the gas press: render thread idle, six emulator threads at ~50 % CPU each (also normal in good runs) |

Freeze rate, 75 s runs (coin at 35 s, gas at 45 s), "good" = card
screen shown by 65 s:

| beta.ini | good |
|---|---|
| shipped | 2/8 (and 1/4 in an earlier batch) |
| `apc_try_lock=1`, `timer_try_lock=0` | 0/8 |
| `apc_try_lock=1` | 1/4 |

Also no help: `periodic_irq10=1`; `cooperative_self_suspend=1` with
`notification_event_wait_recheck=1`; pinning to 4 cores (worse); the
card-reader thread's 1 s read timeout patched to INFINITE (0/4; the
card protocol going quiet is a symptom, it only stopped mid-command in
one of four hangs). winedbg attaching kills or freezes the game; gdb
inside the box attaches cleanly but cannot unwind past Wine's syscall
layer into the 32-bit code.

Building the fork with clang-cl + xwin gets through SDL, the loader
and XbSymbolDatabase after local fixes (case symlinks, no -Werror,
gnu C mode, CRT shims), then stops on 43 MSVC `__declspec(naked)`
functions mixing C and inline asm (`Direct3D9.cpp`, `EmuFS.cpp`):
a source build needs MSVC (Windows CI or msvc-wine). The fork's
scheduler/CRI trace (`scheduler_io_trace`) is compiled out of release
builds and would be the next tool.

## On the SDK's hub modules (2026-11-17)

The launcher moved onto rea-launcher-sdk's hub pieces (v0.10.0): the game's
processes are a `group` (Linux: gamescope's process group, every process
tagged `WANGAN_SESSION`, a sentinel for a killed launcher) run by
`supervise`; the session ends game first, then the board (quit byte), then
`wineserver -k`. The input block is written by the SDK's input thread
(`feeder`, the block as a hex line put in place by `input::BlockSink`).
Release build, headless (`--linux --play <game> --headless`), silent
(`PULSE_SERVER`/`PIPEWIRE_REMOTE` pointed nowhere), fresh data folder,
input from `--input-script` (taps of View and Enter, then gas and
steering). Box: gamescope 3.17, the box's Wine, NVIDIA RTX 5090.
"Left" counts the processes of the game's kind (gamescope, Wine, the
board, the game, the emulator, bwrap, sh) that appeared during the run
and were still there 3 s after it.

| Game (route) | Ended by | Result | Left |
|---|---|---|---|
| MT3DX+ (N2, WanganArcadeLoader) | Stop (`--seconds 105`, the window's Stop flag) | link screen skipped with View from the block, attract mode at 1920×1080 (the Recommended look with no screen known) | 0 |
| MT3DX+ | the game's window closing (TERM to gamescope at 40 s) | session ended at once, no "couldn't start" message (ran > 20 s) | 0 |
| MT3DX+ | launcher killed (KILL at 40 s) | the sentinel ended the session | 0 |
| Wangan R (System 246, pcsx2x6) | Stop at 60 s | Proverb boot, the game's intro movie | 0 |
| Wangan R | window closing at 60 s | pcsx2x6 shut down gracefully | 0 |
| Wangan R | launcher killed at 60 s | — | 0 |
| MT6 (ES3B, OpenParrot under Wine) | Stop at 150 s | Wine prefix created on first start, game connected to `wangan-io`, which answered its JVS polls; DXVK presenting at 1360×768 | 0 |
| MT6 | window closing at 90 s | as above; two short-lived processes named `main` seen during the window were not this session's (no MT6 process has that name) and were gone a moment later | 0 |
| MT6 | launcher killed at 90 s | the sentinel ended gamescope, the game, the board and Wine | 0 |

gamescope 3.17 crashes on its own way out after a TERM (`SIGSEGV` in the
leader's exit status, also seen by the Initial D launcher before this
change); nothing of the game is left. In these runs the quit byte came
300 ms after the polite stop and `wineserver -k` reached the board first
(no "quit requested" in its log); since then a Stop waits for the game to
go (up to 5 s) before the quit byte, and `wineserver -k` stays last.
Rerun with that order (MT6, Stop at 90 s, twice): 0 of the session's
processes left; still no "quit requested" line, because gamescope's
reaper ends everything it started (the board too) as it goes, before the
quit byte. A shell and a Wine set seen in one of those windows had no
link to this session (another program on the shared machine).

`--check` against the owner's library: every game but Maximum Tune 1 ok
(its emulator stays off).

Quit paths: the cabinets have no quit from the game menu (only the
operator's test menu), so a game ends by its window closing (the TERM to
gamescope above), by the launcher window's Stop or close button (closing
the window while a game runs is a Stop, then the window closes), or by
SIGINT/SIGTERM (Ctrl+C) to `wangan-launcher --play`, which sets the same
stop flag as `--seconds` above. No leftover processes in any of the runs.
