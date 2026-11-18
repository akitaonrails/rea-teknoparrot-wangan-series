# Third-party components and credits

Nothing from the games is in this repository or its packages. The player
points the launcher at their own dumps; the launcher reads them in place.

## Shipped with the launcher

| Component | What it does here | Licence | Source / pin |
|---|---|---|---|
| OpenParrot (teknogods) | Loader and game patches for Maximum Tune 5, 5DX+, 6, 6R (dongle/card emulation: owner decision 2026-11-13, see `docs/RESEARCH.md`) | GPL-3.0 | `OpenParrot64.dll`, `OpenParrotLoader64.exe` built in the release workflow with MSVC from fc8ba63 (`packaging/sources.sh`) + `patches/openparrot/0001-0002`; `iDmacDrv64.dll` from the release asset `OpenParrotx64.zip` sha256 `7da6b385b670734447fc1a56a13fb28361d517b0a4d7a56b24b2ec5c156ae5c0` (`packaging/components.pins`) |
| OpenParrot_WMMT (valkyrieasyraf) | The Maximum Tune 6RR module | GPL-3.0 | built the same way from a89d064 + `patches/openparrot-wmmt/0001` |
| OpenBanapass (ProjectAsakura), as `bngrw.dll` in the OpenParrot package | Card reader stand-in | MIT | same archive |
| DXVK 3.1.1 | Direct3D 11 on Vulkan under Wine (Linux package) | zlib | `dxvk-3.1.1.tar.gz` sha256 `40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608` |
| WanganArcadeLoader (vixen256) | Runs Maximum Tune 3 / 3DX+ natively on Linux (dongle/card emulation: same owner decision) | GPL-3.0 | `libwal_3dxp.so` built from d3fc426 with `patches/wal/0001` (input block) by `scripts/build-wal.sh` on Debian bookworm; `libopenal.so`, `gamecontrollerdb.txt`, `tmp/` from release `0.5.1` `dist.zip` sha256 `3b04a845015caecbac610ec530cc35bd26ab4a1d2da718c9392fbb92bcbe66c1` |
| pcsx2x6 v0.2.22 (PS2Homebrew-arcade) | Runs Wangan Midnight R (Namco System 246) on Linux; the AppImage extracted into `components/pcsx2x6/app` | GPL-3.0 | `pcsx2x6-v0.2.22-linux-appimage-x64-Qt.AppImage` sha256 `a28e9354046804e9f3d658b63856f7b2d3d53aefd9f141f053ac9926c08c255d`; source: github.com/PS2Homebrew-arcade/pcsx2x6 tag `v0.2.22` |
| Proverb (PS2Homebrew-arcade) | pcsx2x6's homebrew boot loader for Wangan Midnight R (`components/pcsx2x6/NM00005/boot.elf`) | AFL-3.0 | `bin/NM00005/boot.elf` from the `nightly` release `proverb.zip` sha256 `ca1a6eaae507a09c0d8737a05ecfd9365f2622059d870f9e663dd8a8f44a7ed2` |
| Wine 11.21 (WineHQ; Kron4ek's portable WoW64 build) | Runs the Windows games on Linux (`components/wine` in the Linux package) | LGPL-2.1+ | `wine-11.21-amd64-wow64.tar.xz` sha256 `33f3c5b56b1deb8f6725539ea006f27b5d781b3b4ae6a8a1f492d8af4fb9dc83` (the SDK's Wine pin); source: unmodified WineHQ 11.21 |
| FFmpeg 4.4.5 (libavformat, libavcodec, libavutil) | Wine's movie demuxer (`winedmo`) links against FFmpeg 4; built without GPL parts or external libraries by `scripts/build-ffmpeg.sh` (`components/wine/lib/ffmpeg`) | LGPL-2.1+ | `ffmpeg-4.4.5.tar.xz` sha256 `f9514e0d3515aee5a271283df71636e1d1ff7274b15853bcd84e144be416ab07`; the tarball is in the release's source archive |
| SDL2 | Pads and wheels | zlib | Windows: `SDL2.dll` from `SDL2-devel-2.32.10-mingw.tar.gz` (rea-launcher-sdk's `sdk/packaging/sdl2.pins`); Linux: Debian bookworm's `libSDL2-2.0.so.0`, in `lib/` |
| Barlow, Barlow Condensed (The Barlow Project Authors) | Interface typefaces, embedded in the program by rea-launcher-ui | SIL OFL 1.1 (`assets/fonts/OFL.txt`, shipped as `licenses/Barlow-OFL.txt`) | google/fonts `ofl/barlow`, `ofl/barlowcondensed` |

Corresponding source: every release has `WanganLauncher-<version>-source.tar.gz`
(`scripts/source-archive.sh`): this repository, the three GPL upstream
trees above at their pinned commits with our patches applied, and the
FFmpeg source.

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
