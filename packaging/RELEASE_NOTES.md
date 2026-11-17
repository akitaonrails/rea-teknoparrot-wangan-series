Wangan Midnight R joins the hub, and the launcher can now take the board
files that Wangan R and Maximum Tune need from a folder or zip you
point it at. No game files or board files are included.

## Downloads

- **Linux** (`WanganLauncher-*-linux-x86_64.tar.gz`): Wangan Midnight R,
  MT3, 3DX+, 5, 5DX+, 6, 6R, 6RR. Wine 11.21 and pcsx2x6 are bundled;
  install gamescope, a Vulkan driver, 32-bit OpenGL (MT3/3DX+) and
  GStreamer's libav plugin (movies) from your distribution. Run
  `./wangan-launcher`.
- **Windows** (`WanganLauncher-*-windows-x86_64.zip`): MT5, 5DX+, 6, 6R,
  6RR. Run `wangan-launcher.exe`. MT3/3DX+ are Linux programs, and
  Wangan R has only been set up on Linux so far.
- **Source** (`WanganLauncher-*-source.tar.gz`): this repository plus the
  pinned OpenParrot, OpenParrot_WMMT, WanganArcadeLoader and pcsx2x6
  sources (with our patches where we have any), and the FFmpeg source.
- `SHA256SUMS` for all of the above.

## New in 0.2.0

- **Wangan Midnight R** (Namco System 246) plays on Linux through
  pcsx2x6, upscaled to fit the window (6× at 2160p), with the same pad
  and wheel controls as the other games. It needs the System 246 board
  files, which you add with Add files (below).
- **Add files** on a game's tile (or `wangan-launcher --add-files
  <folder>`) looks through a folder or zip and keeps only the board files
  whose size and SHA-1 match MAME's records, under any name. The launcher
  never downloads them.
- MT3/3DX+ draw at your output size when no render size is set.
- The I/O board gains a Chihiro mode (`wangan-io --chihiro`) for Maximum
  Tune 1.

## Still not supported

- **Maximum Tune 1** (Sega Chihiro). It boots, takes coins and the card,
  and races under TeknoParrot's Cxbx-Reloaded in Wine, but freezes in
  most runs, so Play stays off. What was tried is in
  `docs/RUNTIME_NOTES.md`.

## Not verified yet

- The Windows build was run under Wine, not on a Windows PC.
- FSR/NIS upscaling on a real screen, a physical wheel and force
  feedback, frame pacing and audio.
- Maximum Tune 6 saves are written, but the game does not yet show
  restored progress. 6RR plays as a guest on credits.
