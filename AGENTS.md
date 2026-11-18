# rea-teknoparrot-wangan-series — instructions for coding agents

One launcher (a hub, like Azimuth for Forza) for the Wangan Midnight arcade
series the owner dumped: Wangan Midnight R, Maximum Tune 1, 3, 3DX+, 5, 5DX+,
6, 6R, 6RR. Built on rea-launcher-sdk (its TeknoParrot hub modules).
Read sdk/docs/STANDARDS.md before any work; the playbooks are in
sdk/docs/playbooks/. Read `docs/RESEARCH.md` here first too.

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

## Layout

- `crates/wangan-core`: the game table (`catalog`, `rea_launcher::catalog::Title`),
  the board files (`firmware`), the runtime folder rules (`runtime`, a
  `linkfarm::Policy` per route), the option table (`settings::OPTIONS`,
  looks), launch plans, Wangan R's files (`sys246`). Host-tested.
- `crates/wangan-launcher`: the hub's backend (`ui.rs`, a `HubBackend`;
  the Add files page is its extra page), the settings rows (`ui/form.rs`),
  the command line (`main.rs`: `rea_launcher::hub::cli` plus
  `--add-files`), sessions (`session.rs`: `group` + `supervise`, the
  `Ending` watcher), the input block (`input.rs`, a feeder `Wire`).
- `crates/wangan-jvs` (GPL port of TeknoParrotUI's JVS code) and
  `crates/wangan-io` (the board program) stay this launcher's own.
- Golden snapshots: `crates/wangan-launcher/tests/snapshots/` (the window's
  in `window/<linux|windows>/`); `REA_UPDATE_SNAPSHOTS=1` rewrites them,
  only after reading every diff.

## Checks

`cargo fmt --all --check`, `cargo test --workspace`, clippy for the host and
`x86_64-pc-windows-gnu` (SDL2 from `scripts/build-launcher.sh sdl2`),
`sdk/scripts/wine-test --target x86_64-pc-windows-gnu --locked --workspace`,
`sdk/scripts/launcher-check .`. Command-line tests run the launcher with
`--linux` and no `DISPLAY`/`WAYLAND_DISPLAY`, so a refused command never
opens a window.
