# Third-party components and credits

Nothing from the games is in this repository or its packages. The player
points the launcher at their own dumps; the launcher reads them in place.

## Shipped with the launcher

| Component | What it does here | Licence | Source / pin |
|---|---|---|---|
| OpenParrot (teknogods) | Loader and game patches for Maximum Tune 5, 5DX+, 6R under Wine (dongle/card emulation: owner decision 2026-11-13, see `docs/RESEARCH.md`) | GPL-3.0 | release tag `OpenParrotx64`, asset `OpenParrotx64.zip` sha256 `7da6b385b670734447fc1a56a13fb28361d517b0a4d7a56b24b2ec5c156ae5c0` (built 2026-09-11) |
| OpenBanapass (ProjectAsakura), as `bngrw.dll` in the OpenParrot package | Card reader stand-in | MIT | same archive |
| DXVK 3.1.1 | Direct3D 11 on Vulkan | zlib | `dxvk-3.1.1.tar.gz` sha256 `40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608` |
| WanganArcadeLoader 0.5.1 (vixen256) | Runs Maximum Tune 3 / 3DX+ natively on Linux (dongle/card emulation: same owner decision) | GPL-3.0 | release `0.5.1` `dist.zip` sha256 `3b04a845015caecbac610ec530cc35bd26ab4a1d2da718c9392fbb92bcbe66c1` |
| Barlow, Barlow Condensed (The Barlow Project Authors) | Interface typefaces | SIL OFL 1.1 (`assets/fonts/OFL.txt`) | google/fonts `ofl/barlow`, `ofl/barlowcondensed` |

## Code we ported or follow

| Project | What we took | Licence |
|---|---|---|
| TeknoParrotUI (teknogods; ProjectAsakura's fork of its source) | The Namco JVS I/O board replies, the Wangan control mapping and gear encoding, ported to Rust in `crates/wangan-jvs` | GPL-3.0 |
| TeknoParrotUI GameProfiles | Field lists of `teknoparrot.ini` for MT5–6RR (`crates/wangan-core/data/ini`) | GPL-3.0 |
| Wine (`wmvdecod`, `winegstreamer`) | Read to diagnose the movie crash; no code copied | LGPL-2.1 |

## Art

`assets/art/*.jpg` are original illustrations drawn by `scripts/art/scenes.py`
for this launcher (shapes and a fixed seed; no photos, no game files, no
official art). Licensed with the repository (GPL-3.0-or-later).

## Thanks

The Wangan Midnight emulation community, ProjectAsakura (OpenParrot fork,
Bayshore, OpenBanapass), vixen256 (WanganArcadeLoader), valkyrieasyraf
(OpenParrot_WMMT), PS2Homebrew-arcade (pcsx2x6) and teknogods.
