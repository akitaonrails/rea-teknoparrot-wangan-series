#!/bin/sh
# Build libwal_3dxp.so (WanganArcadeLoader, i686 Linux) with our patches.
#
# usage: scripts/build-wal.sh <src_dir> <out_dir>
#
#   src_dir  a vixen256/WanganArcadeLoader0.5 checkout at d3fc426 (pristine,
#            or already patched by scripts/fetch-sources.sh); never modified
#   out_dir  gets a scratch copy (out_dir/src), the cargo target dir
#            (out_dir/target), libwal_3dxp.so and SHA256SUMS
#
# Needs cargo with the i686-unknown-linux-gnu target, cmake, gcc -m32 and the
# 32-bit X11/GL/dbus/wayland development packages: Arch's lib32-* (the
# `gaming` distrobox) or Debian's :i386 multiarch ones (the release build
# runs on Debian bookworm, so the library needs only glibc 2.36).
#
# Build fixes that stay out of the patch (they are about today's toolchain,
# not about WAL): SDL2 header paths and C standard, a rustc lint, and:
# - CMake 4 refuses the bundled SDL2's cmake_minimum_required; the policy
#   floor is set from the environment;
# - Rust >= 1.90 rejects `extern "win64"` on 32-bit targets, which WAL's
#   retour fork (BroGamer4256/retour-rs 1155d02) instantiates for every
#   target; a copy with that one line removed is patched in.
set -eu

src=$(realpath "$1") out=$2
patches=$(dirname "$(realpath "$0")")/../patches/wal
target=i686-unknown-linux-gnu

mkdir -p "$out"; out=$(realpath "$out")
s=$out/src
rm -rf "$s" "$out/retour"

# 1. scratch copy + patches
cp -a "$src" "$s"; rm -rf "$s/.git"
already=
grep -qs WANGAN_INPUT_BLOCK "$s/src/block.rs" && already=1
for p in "$(realpath "$patches")"/*.patch; do
  [ -e "$p" ] && [ -z "$already" ] || continue
  echo "patch: $(basename "$p")"
  # not `git apply`: inside another git work tree it silently skips the files
  patch -p1 --forward --batch -d "$s" < "$p"
done
grep -q WANGAN_INPUT_BLOCK "$s/src/block.rs" || { echo "patch not applied" >&2; exit 1; }
# a standalone package, even when the copy sits inside another workspace
printf '\n[workspace]\n' >> "$s/Cargo.toml"

export CARGO_TARGET_DIR=$out/target
export CMAKE_POLICY_VERSION_MINIMUM=3.5
# 32-bit pkg-config files: Arch lib32 or Debian multiarch
if [ -d /usr/lib/i386-linux-gnu/pkgconfig ]; then
  lib32=/usr/lib/i386-linux-gnu
  export PKG_CONFIG_PATH=$lib32/pkgconfig
else
  lib32=/usr/lib32
fi
# as WAL's makefile and CI: lets pkg-config answer for the 32-bit cross build
export PKG_CONFIG_SYSROOT_DIR=$lib32/pkgconfig/
# ...but that also prefixes the include paths the bundled SDL2 gets from
# pkg-config; WAL's CI copies these headers to /usr/include, we pass them
CFLAGS="${CFLAGS:-} -I$lib32/dbus-1.0/include -I$lib32/glib-2.0/include"
for d in dbus-1.0 libdecor-0 libdrm pipewire-0.3 spa-0.2 ibus-1.0 glib-2.0; do
  if [ -d "/usr/include/$d" ]; then CFLAGS="$CFLAGS -I/usr/include/$d"; fi
done
# SDL 2.0.x predates C23 (GCC 15 default) and GCC 14 making this an error
export CFLAGS="$CFLAGS -std=gnu17 -Wno-error=incompatible-pointer-types"
# WAL interposes libc `open` with a 2-argument signature; newer rustc denies
# that by default (same calling convention on i686 cdecl, kept as upstream)
export RUSTFLAGS="${RUSTFLAGS:-} -A invalid_runtime_symbol_definitions"

# 2. retour without the win64 ABI
cd "$s"
cargo fetch --locked --target "$target"
retour=$(cargo metadata --locked --format-version 1 --filter-platform "$target" |
  tr ',' '\n' | sed -n 's|^"manifest_path":"\(.*/retour-rs-[^"]*\)/Cargo.toml"$|\1|p' | head -n1)
[ -n "$retour" ] || { echo "retour checkout not found" >&2; exit 1; }
cp -a "$retour" "$out/retour"
sed -i '/extern "win64"/d' "$out/retour/src/macros.rs"

# 3. build
cargo build --release --target "$target" \
  --config "patch.'https://github.com/BroGamer4256/retour-rs.git'.retour.path='$out/retour'"

cp "$CARGO_TARGET_DIR/$target/release/libwal_3dxp.so" "$out/"
(cd "$out" && sha256sum libwal_3dxp.so > SHA256SUMS && cat SHA256SUMS)
