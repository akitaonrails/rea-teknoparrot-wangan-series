# rea-teknoparrot-wangan-series — instructions for coding agents

One launcher (a hub, like Azimuth for Forza) for the Wangan Midnight arcade
series the owner dumped: Wangan Midnight R, Maximum Tune 1, 3, 3DX+, 5, 5DX+,
6, 6R, 6RR. Built on `~/Projects/rea-launcher-sdk`; read its
`docs/STANDARDS.md` (binding) and `docs/RESEARCH.md` here first.

## Owner decisions (2026-11-13, see docs/RESEARCH.md §6)

- **DRM exception**: community loaders that emulate the cabinets' HASP
  dongle / card reader are approved. Never write our own crack, never ship
  or point to game files.
- **Runtime**: native-first hybrid (WanganArcadeLoader native for MT3/3DX+,
  OpenParrot_WMMT under Wine + our Rust I/O board for MT5–6RR, local
  Bayshore for saves, pcsx2x6 Linux AppImage for Wangan R, Cxbx-R under
  Wine for MT1).
- **Licence**: GPL-3.0.
- **Art**: original illustrations only.

## Rules that bite here

- The game library is on NFS and read-only. Runtime = per-game private
  folder (symlinks to the dump + real files for what the game/loader
  writes). Never write into the dump (`teknoparrot.ini`, shader recompiles,
  `libso` symlink fixes all go to the runtime copy).
- No option ships until its effect was seen in the real game; record it in
  `docs/RUNTIME_NOTES.md`. DLSS is not possible for these games; upscaling
  is native resolution patches + gamescope FSR/NIS.
- Game runs are headless (`gamescope --backend headless`) unless the owner
  is told first.
