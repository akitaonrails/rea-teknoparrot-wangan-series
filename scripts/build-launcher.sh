#!/bin/sh
# Build the launcher and the I/O board: the SDK's build-launcher.sh
# (sdk/packaging/build-launcher.sh, SDL2 from its pins) plus the board.
#
#   scripts/build-launcher.sh linux     target/release/wangan-launcher
#                                       + target/x86_64-pc-windows-gnu/release/wangan-io.exe
#   scripts/build-launcher.sh windows   target/x86_64-pc-windows-gnu/release/{wangan-launcher,wangan-io}.exe
#                                       (+ dist/sdl2/SDL2.dll)
#   scripts/build-launcher.sh io        only the I/O board (wangan-io.exe)
#   scripts/build-launcher.sh sdl2      only fetch the pinned SDL2 MinGW package
#                                       into dist/sdl2 (for Windows-target tests)
#
# The Windows programs need MinGW-w64 and LLD (.cargo/config.toml); locally
# run this in the `gaming` distrobox, which has both. CARGO_TARGET_DIR is
# respected.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
what=${1:-linux}
sdk() { sh "$root/sdk/packaging/build-launcher.sh" --root "$root" "$@"; }
target_dir=${CARGO_TARGET_DIR:-$root/target}

build_io() {
	cargo build --release --locked -p wangan-io --target x86_64-pc-windows-gnu \
		--manifest-path "$root/Cargo.toml"
	echo "build-launcher: $target_dir/x86_64-pc-windows-gnu/release/wangan-io.exe"
}

case $what in
sdl2) sdk sdl2 ;;
io) build_io ;;
linux)
	sdk linux wangan-launcher
	build_io
	;;
windows) sdk windows wangan-launcher wangan-io ;;
*)
	echo "build-launcher: unknown target '$what' (linux, windows, io or sdl2)" >&2
	exit 1
	;;
esac
