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
| Resolution `width/height` 3840×2160 | not run | full 16:9 frame at 3840×2160, sharp UI |
| 1280×720 | yes | yes |
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
