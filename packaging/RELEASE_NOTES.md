First release of the Wangan Midnight launcher: one hub for the Maximum
Tune arcade games you dumped yourself. No game files are included.

## Downloads

- **Linux** (`WanganLauncher-*-linux-x86_64.tar.gz`): MT3, 3DX+, 5, 5DX+,
  6, 6R, 6RR. Wine 11.21 is bundled; install gamescope, a Vulkan driver,
  32-bit OpenGL (MT3/3DX+) and GStreamer's libav plugin (movies) from
  your distribution. Run `./wangan-launcher`.
- **Windows** (`WanganLauncher-*-windows-x86_64.zip`): MT5, 5DX+, 6, 6R,
  6RR. Run `wangan-launcher.exe`. MT3/3DX+ are Linux programs and are
  listed as unavailable.
- **Source** (`WanganLauncher-*-source.tar.gz`): this repository plus the
  pinned OpenParrot, OpenParrot_WMMT and WanganArcadeLoader sources with
  our patches, and the FFmpeg source (GPL/LGPL corresponding source).
- `SHA256SUMS` for all of the above.

## What's in it

- MT5–6RR through OpenParrot (built from source with our fixes for MT6's
  crash under Wine and 6RR's first boot) and our own JVS I/O board.
- MT3/3DX+ natively on Linux through WanganArcadeLoader, with up to 4K
  rendering.
- Pads and wheels through one input path, with a live Controls page.
- Every window option has a command-line flag.

## Not verified yet

- The Windows build was run under Wine, not on a Windows PC.
- FSR/NIS upscaling on a real screen, a physical wheel and force
  feedback, frame pacing and audio.
- Maximum Tune 6 saves are written, but the game does not yet show
  restored progress. 6RR plays as a guest on credits.
