# Third-party components and credits

Nothing from the games is in this repository or its packages. The player
points the launcher at their own dumps; the launcher reads them in place.

## Shipped with the launcher

| Component | What it does here | Licence | Source / pin |
|---|---|---|---|
| OpenParrot (teknogods) | Loader and game patches for Maximum Tune 5, 5DX+, 6, 6R under Wine (dongle/card emulation: owner decision 2026-11-13, see `docs/RESEARCH.md`) | GPL-3.0 | built from fc8ba63 with `patches/clang-cl/openparrot` + `patches/openparrot/0001-0002`: OpenParrot64.dll sha256 `6d987e925c2029421a192811c2c87d2d8063f875e76b3191910f3174bcbadad1` |
| OpenParrot_WMMT (valkyrieasyraf) | The Maximum Tune 6RR module | GPL-3.0 | built from a89d064 with `patches/clang-cl/openparrot-wmmt` + `patches/openparrot-wmmt/0001`: OpenParrot64.dll sha256 `2b599aea784af881f23e91b63d42b3a569c56f2772e6550b773a68fdb7c6f8d6` |
| OpenBanapass (ProjectAsakura), as `bngrw.dll` in the OpenParrot package | Card reader stand-in | MIT | same archive |
| DXVK 3.1.1 | Direct3D 11 on Vulkan | zlib | `dxvk-3.1.1.tar.gz` sha256 `40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608` |
| WanganArcadeLoader (vixen256) | Runs Maximum Tune 3 / 3DX+ natively on Linux (dongle/card emulation: same owner decision) | GPL-3.0 | built from d3fc426 with `patches/wal/0001` (input block): libwal_3dxp.so sha256 `b5bcd16f4b14502e8d189bcb89b2ca225a9f97faef90240c48633bfa0987f8a8`; `libopenal.so`, `gamecontrollerdb.txt` from release `0.5.1` `dist.zip` sha256 `3b04a845015caecbac610ec530cc35bd26ab4a1d2da718c9392fbb92bcbe66c1` |
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
