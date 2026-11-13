#!/bin/sh
# Build the FFmpeg 4.4 libraries the bundled Wine's movie demuxer needs.
#
#   scripts/build-ffmpeg.sh <out_dir>
#
# Kron4ek's Wine 11.21 build links winedmo.so (Media Foundation's FFmpeg
# source, used with DisableGstByteStreamHandler for the MT5/MT6 movies,
# docs/RUNTIME_NOTES.md) against libavformat.so.58, libavcodec.so.58 and
# libavutil.so.56, which current distributions no longer ship. This builds
# exactly those three from the pinned FFmpeg 4.4.5 source (LGPL-2.1+, no
# GPL parts, no external libraries): every demuxer, parser and bitstream
# filter (winedmo only demuxes; Wine's decoders use GStreamer), and the
# Windows Media decoders for stream probing. Output:
# <out_dir>/lib/{libavutil.so.56,libavcodec.so.58,libavformat.so.58} and
# <out_dir>/CONFIGURE.txt (the configure line, for the source offer) and
# <out_dir>/COPYING.LGPLv2.1.
#
# Needs a C compiler, make and curl. Builds in <out_dir>/build.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
die() { echo "build-ffmpeg: $*" >&2; exit 1; }
[ $# -eq 1 ] || die "usage: $0 <out_dir>"
mkdir -p "$1"; out=$(cd "$1" && pwd -P)

. "$root/sdk/packaging/pins.sh"
PIN_FILE=$root/packaging/components.pins
tarball=$(pin_fetch ffmpeg "$out/dl") || die "cannot fetch the FFmpeg source"

b=$out/build
rm -rf "$b" "$out/lib"; mkdir -p "$b" "$out/lib"
tar -xJf "$tarball" -C "$b" --strip-components=1

flags="--prefix=/usr --enable-shared --disable-static --disable-programs --disable-doc
 --disable-avdevice --disable-swresample --disable-swscale --disable-postproc --disable-avfilter
 --disable-network --disable-autodetect --disable-x86asm --disable-debug
 --disable-everything --enable-demuxers --enable-parsers --enable-bsfs --enable-protocol=file
 --enable-decoder=wmv1,wmv2,wmv3,vc1,wmav1,wmav2,wmapro,wmavoice,msmpeg4v3"
# shellcheck disable=SC2086
(cd "$b" && ./configure $flags >/dev/null && make -j"${JOBS:-$(nproc)}" >/dev/null)

for l in libavutil/libavutil.so.56 libavcodec/libavcodec.so.58 libavformat/libavformat.so.58; do
	cp -L "$b/$l" "$out/lib/"
	strip --strip-unneeded "$out/lib/$(basename "$l")"
done
cp "$b/COPYING.LGPLv2.1" "$out/"
echo "FFmpeg 4.4.5 ($(pin_url ffmpeg))
configure $(echo $flags)" > "$out/CONFIGURE.txt"
ls -l "$out/lib"
