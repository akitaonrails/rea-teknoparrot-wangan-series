# Research — Wangan Midnight series launcher

Phase 1 of `rea-launcher-sdk/docs/playbooks/new-launcher.md`. Observations,
inferences and decisions are kept apart. Everything below was read from the
files, binaries, decompiled code or upstream repositories named in each row,
on 2026-11-13. Nothing in `.work/` is committed.

## 1. The owner's games (read-only, never copied)

Library: `$WANGAN_GAMES_DIR` (owner's copy lives under
`roms_rare/TeknoParrot/`). SHA-256 of each main executable:

| Game | Board | Main exe | SHA-256 | Size |
|---|---|---|---|---|
| Wangan Midnight R (WMR1 Ver. A, 2002) | Namco System 246 | `wanganmr.zip` (`wmr1vera.ic002`, `wmr1vera_spr.ic002`) + `wanganmr/wmr1-a.chd` | `4d78fe15…3ca8` (zip) | 1 325 746 |
| Maximum Tune (3.07 Rev B, EXP, 2004) | Sega Chihiro | `V307.xbe` | `2d74371e5a0bb83a6b5185350792f7e3a4242076b62306fab103936ddd2dd760` | 2 256 896 |
| Maximum Tune 3 (Export, 2007) | Namco N2 (Linux) | `main` (ELF32 i386) | `0574cf963d8dd94788f247e65cc4da7e47f4b366588d1309fe869f4264a26475` | 28 470 624 |
| Maximum Tune 3DX+ (EXP, 2010) | Namco N2 (Linux) | `main` (ELF32 i386) | `3dc7cc6174806fe4ea6687625c233ed5468e0225e3bcce15e225b25b2934be5b` | 24 925 524 |
| Maximum Tune 5 (5.00, Japan, 2014) | Namco ES3B (Win x64) | `wmn5r.exe` | `4701c88d3598d56c31055fdaba98ec1c17e3455e96ad701abebc65eba6105c3e` | 27 366 912 |
| Maximum Tune 5DX+ (Export, 2017) | Namco ES3B | `wmn5r.exe` | `738e88b4d03ef6ab84208464d470e69557a8591cf5a293570badb1dfdcbc3b2a` | 34 272 768 |
| Maximum Tune 6 (1.03.04, Export, 2018) | Namco ES3B | `wmn6r.exe` | `92f02199a44fa65a35af3ed162b5ce5477cfc8b2e3a13ccc95936356680f1479` | 32 727 552 |
| Maximum Tune 6R (Japan, 2020) | Namco ES3B | `wmn6r.exe` | `bf9fded65f5d029deaed5c3b5d06e8035dae70eee2e03c51e3adc2b1444ae2be` | 34 958 848 |
| Maximum Tune 6RR (3.05.03, Japan, 2022) | Namco ES3B | `wmn6r.exe` | `9203e373b73a522c3ca213df9add0e3d4cce3f81aaba8c48ce5e46c132932d4b` | 33 797 120 |

Observations on the dumps:
- They are not pristine. WMMT6 has a replaced `bngrw.dll`
  (`350757…e1aa`) with the original kept as `bngrw.dll.bak` (`124190…0a60`);
  6R/6RR carry `bngrw.dll` `3d0091…c7eb`. 6RR ships a `teknoparrot.ini`
  and a `TeknoParrot/` save folder; WMMT6 has `D3D8.dll`/`DDraw.dll`
  leftovers and PowerShell scripts (`init.ps1`, `project.ps1` — data, never
  run). WMMT1 ships `YACardEmu` (card reader emulator).
- No Namco System 246 BIOS was found in the library or bios folders, and
  neither TeknoParrot's `pcsx2x6` package nor pcsx2x6 upstream bundles one.
  **Wangan R cannot boot until the owner supplies that BIOS.**
- The ROM share is NFS (`/mnt/terachad`); the project is on btrfs, so
  hard-linking a runtime copy is impossible. The runtime must be a symlink
  farm or a copy (9–16 GB per ES3B title).

## 2. How TeknoParrot runs these games (decompiled, not guessed)

Sources: `TeknoParrotUi.exe` 1.0.0.2296 (`sha256 5016…5d2f` inside
`TeknoParrotUi.zip` `1433…145d`) and its embedded
`TeknoParrotUi.Common.dll`, decompiled with ILSpy into `.work/src/`;
GameProfiles/Metadata from the same zip.

| Profile | EmulatorType | EmulationProfile | Loader the UI starts | Profile options that matter |
|---|---|---|---|---|
| `wanganmr` | pcsx2x6 | pcsx2x6 | `pcsx2x6\pcsx2-qtx64[-avx2].exe` | Resolution Native…4320p (`upscale_multiplier`), backend D3D11/12/GL/Vulkan |
| `WMMT1` | cxbxr | cxbxr | `cxbxr\cxbxr-ldr.exe` | Windowed, Region |
| `WMMT3`, `WMMT3DXP` | ElfLdr2 | NamcoWmmt3 | `ElfLdr2\BudgieLoader.exe` | CustomResolution + W/H, Framelimiter, Skip LAN, AA (3DX+) |
| `WMMT5`, `WMMT5DXPlus` | TeknoParrot | NamcoWmmt5 | `TeknoParrot\BudgieLoader.exe` | FreePlay, TerminalEmulator, SkipMovies, Force Full Tune — **no resolution option** |
| `WMMT6` | TeknoParrot | NamcoWmmt5 | same, plus `amauthd.exe` first | no resolution option |
| `WMMT6R`, `WMMT6RR` | TeknoParrot | NamcoWmmt6RR | same | Custom Resolution W/H, MSAA 0/2/4/8x, FreePlay, Terminal Emu |

None is Patreon-gated (`<Patreon>false</Patreon>`); the 6RR metadata says
"Saving requires a subscription". Wheel rotation in metadata: 270°.

**The UI is the I/O board.** While a game runs, `TeknoParrotUi.exe` reads
DirectInput/XInput/RawInput itself, keeps the state in `InputCode`, serves
the JVS protocol on the named pipe `\\.\pipe\TeknoParrot_JVS`
(`SerialPortHandler`, `JvsPackageEmulator`) and the 64-byte shared memory
`TeknoParrot_JvsState` (`JvsHelper`), and writes the profile's options to
`teknoparrot.ini` next to the game exe (`ConfigurationWriter`). The core
loader alone gives a game with no controls. So "run BudgieLoader without the
UI" is not an option; either the UI runs under Wine, or something else plays
the I/O board.

TeknoParrot's own pieces: the UI was GPL-3.0 (the ProjectAsakura fork
carries the licence); `TeknoParrot.dll`/`TeknoParrot64.dll`/`BudgieLoader`
(the core) are closed, unlicensed binaries re-uploaded under fixed GitHub
tags (`teknoparrot.com/api/updates/components` publishes the current
SHA-256 per asset). Not bundleable.

## 3. Prior attempt (distrobox-gaming, issue #7)

TeknoParrotUI under Wine in the `gaming` box worked for the UI and some
games but was parked on 2026-08-20: per-game tuning, Wine 11 new-WoW64
crashed OpenParrot at init (`0xc00002b5`), wine-staging-9.19 + 
`UseXVidMode=N` needed; a 1920×1080 virtual desktop to stop games grabbing
the 4K multi-monitor span; .NET Framework 4.6.2 via winetricks; the native
`TeknoParrotUI-net8` build ships no loaders (dead end).

## 4. Community projects found

| Project | Licence | Maintained | Covers | Notes | Decision |
|---|---|---|---|---|---|
| [vixen256/WanganArcadeLoader0.5](https://github.com/vixen256/WanganArcadeLoader0.5) | GPL-3.0 | last push 2025-02 (0.5.1) | **MT3, 3DX, 3DX+ natively on Linux** | Rust `LD_PRELOAD` into the original i386 ELF; SDL2 input with analog deadzone, keyboard; card + **dongle emulation**; GLFW window, width/height; needs `nvidia-cg-toolkit` (`cgc`), lib32 libs, MangoHud for the 60 fps cap | candidate (owner decision: dongle) |
| [valkyrieasyraf/OpenParrot_WMMT](https://github.com/valkyrieasyraf/OpenParrot_WMMT) | GPL-3.0 | 2026-01 | WMMT5, 5DX, 5DX+, 6, 6R, 6RR (Windows, ES3B) | fork of ProjectAsakura/OpenParrot; resolution patches (5, 5DX+, 6RR), **HASP dongle emulation**, Banapass, terminal; README: "How to use: Don't, yet" | candidate, needs verification per title |
| [ProjectAsakura/OpenParrot](https://github.com/ProjectAsakura/OpenParrot) | GPL-3.0 | 2022-12 | WMMT5/6 networking | base of the above | superseded |
| [ProjectAsakura/TeknoParrotUI](https://github.com/ProjectAsakura/TeknoParrotUI) | GPL-3.0 | 2022-07 | TPUI fork for Maxitune | source of the JVS/I/O board logic under GPL | reference for an I/O board |
| [ProjectAsakura/Bayshore](https://github.com/ProjectAsakura/Bayshore) (+ 5DX+, 6R, 6RR forks) | AGPL-3.0 | 2025-12 | local save/network server for 5DX+, 6, 6R, 6RR | TypeScript/Node; would replace TeknoParrot's subscription saves on 6RR | candidate |
| [ProjectAsakura/OpenBanapass](https://github.com/ProjectAsakura/OpenBanapass) | MIT | 2023-09 | card reader DLL for WMMT6 | | candidate |
| [PS2Homebrew-arcade/pcsx2x6](https://github.com/PS2Homebrew-arcade/pcsx2x6) | GPL-3.0 | 2026-11 | Namco System 246/256 incl. **Wangan Midnight R (NM00005)** | official **Linux AppImage** and Windows builds; needs System 246 BIOS | candidate (blocked on BIOS) |
| TeknoParrot (core + UI) | closed core | active | all nine | Windows/.NET under Wine; prior attempt parked | fallback only |
| Cxbx-Reloaded (TeknoParrot build) | GPL-2.0 upstream | | WMMT1 (Chihiro) | Windows only; Chihiro support is TP-patched | needs a Wine probe |

## 5. Where "4K / 60 fps+ / DLSS/FSR" can come from (facts so far)

- Native resolution exists in: pcsx2x6 (upscale multiplier up to 4320p),
  WanganArcadeLoader (width/height), OpenParrot_WMMT (resolution patches for
  5, 5DX+, 6RR), TeknoParrot profiles (MT3/3DX+ custom res; 6R/6RR custom res
  and MSAA). WMMT6 and 6R have no resolution patch in OpenParrot_WMMT.
- Upscaling of what the game renders: gamescope FSR 1 / NIS. **DLSS is not
  possible**: it needs the game's motion vectors and an engine integration
  none of these games has. Saying otherwise would be false.
- Frame rate: these are 60 Hz arcade titles; MT3's loader caps at 60 with
  MangoHud. Anything above 60 must be shown to keep game speed correct
  before it ships (not verified for any title yet).

## 6. Owner decisions needed (STANDARDS: DRM, licence, public)

1. **Dongle/HASP and card emulation.** Every loader that runs these dumps
   (TeknoParrot core, OpenParrot_WMMT, WanganArcadeLoader) answers the
   cabinet's HASP dongle checks. STANDARDS ("DRM exceptions") makes using
   them an owner decision, recorded with date and reason.
2. **Runtime route** (see the recommendation given to the owner).
3. **Licence of this repository**: porting the GPL I/O-board logic or
   bundling GPL loaders makes GPL-3.0 the natural licence, unlike the MIT/
   Apache SDK launchers.
4. **Wangan R BIOS**: supply a System 246 BIOS, or the title stays listed but
   disabled with the reason.
5. **Art**: freely licensed photos vs. original illustrations; official key
   art only by owner decision.
