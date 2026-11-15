#!/bin/sh
# Assemble a release package from the built pieces and the pinned
# downloads (packaging/components.pins).
#
#   scripts/package.sh linux     dist/pkg/WanganLauncher-<version>-linux-x86_64.tar.gz
#   scripts/package.sh windows   dist/pkg/WanganLauncher-<version>-windows-x86_64.zip
#
# Built pieces (the release workflow puts them there; override with the
# variables in brackets):
#   both     dist/openparrot/{upstream,wmmt}/{OpenParrot64.dll,OpenParrotLoader64.exe}
#                                          (MSVC builds, scripts/fetch-sources.sh trees) [OPENPARROT_DIR]
#            dist/windows/wangan-io.exe    [IO_EXE]
#   linux    dist/linux/wangan-launcher, dist/linux/lib/*.so* (SDL2) [LINUX_DIR]
#            dist/linux/libwal_3dxp.so     (scripts/build-wal.sh)
#            dist/linux/ffmpeg/            (scripts/build-ffmpeg.sh: lib/, CONFIGURE.txt)
#   windows  dist/windows/wangan-launcher.exe, dist/windows/SDL2.dll,
#            dist/windows/SDL2-LICENSE.txt [WINDOWS_DIR]
#
# Layout: the launcher beside components/ (crates/wangan-launcher/src/paths.rs):
# openparrot/, openparrot-6rr/, io/, art/, fonts/, and on Linux dxvk/, wal/,
# pcsx2x6/ and wine/ (portable Wine with the FFmpeg libraries in wine/lib/ffmpeg).
# No game file is ever packaged.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
os=${1:-linux}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
die() { echo "package: $*" >&2; exit 1; }

op=${OPENPARROT_DIR:-$root/dist/openparrot}
win=${WINDOWS_DIR:-$root/dist/windows}
lin=${LINUX_DIR:-$root/dist/linux}
io=${IO_EXE:-$win/wangan-io.exe}
dl=$root/dist/dl

. "$root/sdk/packaging/pins.sh"
PIN_FILE=$root/packaging/components.pins
fetch() { pin_fetch "$1" "$dl" || die "cannot fetch $1"; }

case $os in
linux) name=WanganLauncher-$version-linux-x86_64 ;;
windows) name=WanganLauncher-$version-windows-x86_64 ;;
*) die "unknown target '$os' (linux or windows)" ;;
esac

need="$op/upstream/OpenParrot64.dll $op/upstream/OpenParrotLoader64.exe
 $op/wmmt/OpenParrot64.dll $op/wmmt/OpenParrotLoader64.exe $io"
if [ "$os" = linux ]; then
	need="$need $lin/wangan-launcher $lin/libwal_3dxp.so $lin/ffmpeg/lib/libavformat.so.58"
else
	need="$need $win/wangan-launcher.exe $win/SDL2.dll"
fi
for f in $need; do [ -f "$f" ] || die "missing $f - build it first"; done

out=$root/dist/pkg/$name
rm -rf "$out"
c=$out/components
mkdir -p "$c/openparrot" "$c/openparrot-6rr" "$c/io" "$c/art" "$c/fonts" "$out/licenses"

# OpenParrot: our MSVC builds + iDmacDrv64.dll and bngrw.dll (OpenBanapass) from the pinned release
opzip=$(fetch openparrot-x64)
t=$root/dist/tmp; rm -rf "$t"; mkdir -p "$t/op"
unzip -q -o "$opzip" iDmacDrv64.dll bngrw.dll -d "$t/op"
for d in openparrot:upstream openparrot-6rr:wmmt; do
	cp "$op/${d#*:}/OpenParrot64.dll" "$op/${d#*:}/OpenParrotLoader64.exe" "$c/${d%%:*}/"
	cp "$t/op/iDmacDrv64.dll" "$t/op/bngrw.dll" "$c/${d%%:*}/"
done
cp "$io" "$c/io/wangan-io.exe"
cp "$root"/assets/art/*.jpg "$c/art/"
cp "$root"/assets/fonts/* "$c/fonts/"

cp "$root/packaging/README-player.txt" "$out/README.txt"
cp "$root/LICENSE" "$out/licenses/LICENSE-GPL-3.0.txt"
cp "$root/THIRD_PARTY.md" "$out/licenses/"
cp "$root/packaging/SOURCE.txt" "$out/licenses/SOURCE.txt"
cp "$root/assets/fonts/OFL.txt" "$out/licenses/Barlow-OFL.txt"

if [ "$os" = linux ]; then
	cp "$lin/wangan-launcher" "$out/wangan-launcher"
	chmod 755 "$out/wangan-launcher"
	if [ -d "$lin/lib" ]; then
		mkdir -p "$out/lib"
		cp -L "$lin"/lib/*.so* "$out/lib/"
	fi
	[ ! -f "$lin/SDL2-copyright.txt" ] || cp "$lin/SDL2-copyright.txt" "$out/licenses/"

	# DXVK
	mkdir -p "$c/dxvk"
	tar -xzf "$(fetch dxvk)" -C "$t" dxvk-3.1.1/x64/d3d11.dll dxvk-3.1.1/x64/dxgi.dll
	cp "$t"/dxvk-3.1.1/x64/d3d11.dll "$t"/dxvk-3.1.1/x64/dxgi.dll "$c/dxvk/"

	# WanganArcadeLoader: our libwal_3dxp.so, the release's OpenAL, pad database, tmp/ template
	mkdir -p "$c/wal" "$t/wal"
	unzip -q -o "$(fetch wal-dist)" -d "$t/wal"
	cp "$lin/libwal_3dxp.so" "$c/wal/"
	cp "$t/wal/libopenal.so" "$t/wal/gamecontrollerdb.txt" "$c/wal/"
	cp -r "$t/wal/tmp" "$c/wal/tmp"

	# pcsx2x6 (Wangan R): the AppImage extracted (no FUSE needed), Proverb's boot.elf for NM00005
	mkdir -p "$c/pcsx2x6/NM00005" "$t/pcsx2x6"
	cp "$(fetch pcsx2x6)" "$t/pcsx2x6/pcsx2x6.AppImage"
	chmod +x "$t/pcsx2x6/pcsx2x6.AppImage"
	(cd "$t/pcsx2x6" && ./pcsx2x6.AppImage --appimage-extract >/dev/null)
	[ -e "$t/pcsx2x6/squashfs-root/AppRun" ] || die "the pcsx2x6 AppImage has no AppRun"
	mv "$t/pcsx2x6/squashfs-root" "$c/pcsx2x6/app"
	unzip -q -o -j "$(fetch proverb)" bin/NM00005/boot.elf -d "$c/pcsx2x6/NM00005"

	# Wine (as sdk/packaging/appimage-wine.sh trims it) + FFmpeg 4 for its movie demuxer
	mkdir -p "$c/wine"
	tar -xJf "$(fetch wine)" -C "$c/wine" --strip-components=1
	[ -x "$c/wine/bin/wine" ] || die "the Wine archive has no bin/wine"
	rm -rf "$c/wine/include" "$c/wine/share/man" "$c/wine/share/applications"
	for tool in function_grep.pl widl winebuild winecpp winedump wineg++ winegcc winemaker wmc wrc msidb; do
		rm -f "$c/wine/bin/$tool"
	done
	find "$c/wine/lib" -name '*.a' -delete
	mkdir -p "$c/wine/lib/ffmpeg"
	cp "$lin"/ffmpeg/lib/*.so.* "$c/wine/lib/ffmpeg/"
	mkdir -p "$out/licenses/wine" "$out/licenses/ffmpeg"
	cp "$root/sdk/packaging/appimage/wine/COPYING.LIB" "$root/sdk/packaging/appimage/wine/LICENSE" \
		"$root/sdk/packaging/appimage/wine/SOURCE.txt" "$out/licenses/wine/"
	cp "$lin/ffmpeg/CONFIGURE.txt" "$out/licenses/ffmpeg/"
	cp "$lin/ffmpeg/COPYING.LGPLv2.1" "$out/licenses/ffmpeg/"

	tar -C "$root/dist/pkg" -czf "$root/dist/pkg/$name.tar.gz" "$name"
	echo "package: $root/dist/pkg/$name.tar.gz"
else
	cp "$win/wangan-launcher.exe" "$out/wangan-launcher.exe"
	cp "$win/SDL2.dll" "$out/"
	[ ! -f "$win/SDL2-LICENSE.txt" ] || cp "$win/SDL2-LICENSE.txt" "$out/licenses/"
	(cd "$root/dist/pkg" && rm -f "$name.zip" && zip -q -r -X "$name.zip" "$name")
	echo "package: $root/dist/pkg/$name.zip"
fi
rm -rf "$t"
