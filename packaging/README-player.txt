WANGAN MIDNIGHT LAUNCHER
========================

One launcher for the Wangan Midnight arcade games you dumped yourself:
Wangan Midnight R, Maximum Tune 3, 3DX+ (Linux), 5, 5DX+, 6, 6R and 6RR
(Linux and Windows). No game files are supplied. Bring your own; Wangan R
also needs the System 246 BIOS from the board (Add files on its tile).

Project page: https://github.com/akitaonrails/rea-teknoparrot-wangan-series


LINUX
-----
1. Unpack this folder anywhere.
2. Install from your distribution:
   - gamescope
   - a Vulkan driver (Mesa or NVIDIA)
   - 32-bit OpenGL, for MT3 and 3DX+
       Arch: lib32-mesa or lib32-nvidia-utils
       Debian/Ubuntu: libgl1:i386
   - GStreamer with its libav plugin, for the MT5/MT6 movies
       Arch: gst-libav        Debian/Ubuntu: gstreamer1.0-libav
   Wine is included (components/wine); you don't need your own.
3. Run ./wangan-launcher and choose the folder that holds your game
   folders. The first start of an MT5/MT6 game creates the Wine prefix
   and takes a minute.


WINDOWS
-------
1. Unpack this folder anywhere.
2. Run wangan-launcher.exe and choose the folder that holds your game
   folders. Windows SmartScreen may warn about an unknown publisher:
   "More info", then "Run anyway".
MT3 and 3DX+ are Linux programs: they are listed but can't start on
Windows. The screen options (output size, upscaler, full screen) are
gamescope's and exist on Linux only (shown greyed out, with the reason);
on Windows the game runs full screen at its own 1360 x 768.
If your games are on a network share, Windows needs developer mode for
the launcher to link their folders (Settings > System > For developers).


CONTROLS
--------
Pads and wheels are read with SDL. Pad layout:

  Left stick   steering          RT / LT   gas / brake
  RB / LB      shift up / down   Y         view change
  X            intrude           Back      insert card
  Start        insert coin (6RR plays on credits)
  A            test menu enter

The Controls page shows live steering, pedal and button readings. The
pad also works the launcher's window: d-pad or left stick to move, A to
choose, B to go back, Start to play, Y for Settings, the shoulder buttons
change tabs. While a game runs the pad is the game's, and the window has
a Stop button; closing the window stops the game first.


COMMAND LINE
------------
  wangan-launcher --library <folder>   add a folder with your games
  wangan-launcher --list               what was found
  wangan-launcher --check              what could start, without starting
  wangan-launcher --play mt5dxp        start one game
  wangan-launcher --add-files <path>   add the board files from a folder or zip
  wangan-launcher --help               every option


WHERE THINGS ARE
----------------
Settings, logs and each game's private folder (your saves):
  Linux    ~/.local/share/rea-wangan/
  Windows  %LOCALAPPDATA%\rea-wangan\
Your game folders are only read, never changed. Back up
rea-wangan/runtimes/ before deleting anything: card data and records
live there.


LICENCES
--------
GPL-3.0-or-later. Third-party components and their licences:
licenses/THIRD_PARTY.md; where to get the source: licenses/SOURCE.txt.
