#!/bin/sh
# The GPL corresponding source of a release, as one archive:
#
#   scripts/source-archive.sh   dist/pkg/WanganLauncher-<version>-source.tar.gz
#
# Holds this repository at HEAD (git archive), the pinned upstream trees
# with our patches applied (scripts/fetch-sources.sh: OpenParrot,
# OpenParrot_WMMT, WanganArcadeLoader, pcsx2x6), and the FFmpeg source
# tarball the Linux package's FFmpeg libraries are built from. Wine's source is
# referenced in licenses/wine/SOURCE.txt (unmodified WineHQ release).
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
name=WanganLauncher-$version-source
w=$root/dist/src-archive
rm -rf "$w"; mkdir -p "$w/$name" "$root/dist/pkg"

git -C "$root" archive --format=tar --prefix=launcher/ HEAD | tar -x -C "$w/$name"
"$root/scripts/fetch-sources.sh" "$w/$name/upstream"
"$root/scripts/fetch-sources.sh" "$w/$name/upstream" pcsx2x6
for d in "$w/$name"/upstream/*; do rm -rf "$d/.git"; done

. "$root/sdk/packaging/pins.sh"
PIN_FILE=$root/packaging/components.pins
mkdir -p "$w/$name/upstream/ffmpeg"
cp "$(pin_fetch ffmpeg "$root/dist/dl")" "$w/$name/upstream/ffmpeg/"

cat > "$w/$name/README-SOURCE.txt" <<TXT
Wangan Midnight launcher $version: corresponding source

launcher/                  this repository at the release commit
                           (build: launcher/README.md, launcher/.github/workflows/build.yml)
upstream/openparrot/       teknogods/OpenParrot $(sed -n 's/^OPENPARROT_COMMIT=//p' "$root/packaging/sources.sh")
                           + launcher/patches/openparrot/*.patch (applied)
upstream/openparrot-wmmt/  valkyrieasyraf/OpenParrot_WMMT $(sed -n 's/^OPENPARROT_WMMT_COMMIT=//p' "$root/packaging/sources.sh")
                           + launcher/patches/openparrot-wmmt/*.patch (applied)
upstream/wal/              vixen256/WanganArcadeLoader0.5 $(sed -n 's/^WAL_COMMIT=//p' "$root/packaging/sources.sh")
                           + launcher/patches/wal/*.patch (applied)
upstream/pcsx2x6/          PS2Homebrew-arcade/pcsx2x6 $(sed -n 's/^PCSX2X6_COMMIT=//p' "$root/packaging/sources.sh")
                           (tag v0.2.22; the package ships its release AppImage unchanged)
upstream/ffmpeg/           FFmpeg 4.4.5 source as released (configure line:
                           launcher/scripts/build-ffmpeg.sh)

OpenParrot is built with MSVC (premake5 vs2022, msbuild Release x64),
WanganArcadeLoader with launcher/scripts/build-wal.sh. Wine 11.22 is an
unmodified WineHQ release: https://dl.winehq.org/wine/source/11.x/wine-11.22.tar.xz
TXT

tar -C "$w" -czf "$root/dist/pkg/$name.tar.gz" "$name"
rm -rf "$w"
echo "source-archive: $root/dist/pkg/$name.tar.gz"
