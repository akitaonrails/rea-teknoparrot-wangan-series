#!/bin/sh
# Build OpenParrot64.dll and OpenParrotLoader64.exe (x86_64-pc-windows-msvc)
# on Linux with clang-cl + lld-link against an xwin-splatted MSVC CRT/SDK.
#
# usage: XWIN_DIR=<xwin splat dir> scripts/build-openparrot.sh <src_dir> <patch_dirs|-> <out_dir>
#
#   src_dir   an OpenParrot checkout (teknogods/OpenParrot or a fork); never modified
#   patch_dirs ':'-separated directories of *.patch files, each applied in
#             name order ("-" = none), e.g. patches/openparrot:patches/clang-cl/openparrot
#   out_dir   gets a scratch copy (out_dir/src), objects (out_dir/obj) and the
#             binaries + SHA256SUMS
#   JOBS      parallel compiles (default: nproc)
#   XWIN_DIR  output of `xwin splat --use-winsysroot-style --preserve-ms-arch-notation`
#             (with --include-atl: OpenParrot uses <atlstr.h>)
#
# Build driver: the upstream build is premake5 -> VS2017 (appveyor.yml,
# x64 Release). premake's gmake output assumes gcc/mingw, so this script
# replays premake5.lua by hand instead:
#   workspace: staticruntime On (/MT), optimize speed (/O2), NDEBUG,
#              characterset Unicode, /std:c++17, includedirs deps/inc deps/udis86,
#              libdirs deps/lib
#   MinHook (static): deps/src/{buffer,hook,trampoline}.c + hde/hde64.c
#   udis86 (static):  deps/udis86/libudis86/*.c
#   OpenParrot (DLL, suffix 64): OpenParrot/src/**.cpp, OpenParrot/deps/cpp/**.cpp,
#              OpenParrot.rc, win64init.asm; includedirs src, OpenParrot/deps/inc
#   OpenParrotLoader (console, suffix 64): OpenParrotLoader/src/**.cpp
# Libraries come from the sources' #pragma comment(lib, ...) lines, as with MSVC.
# /Brepro (no timestamps) makes the output byte-identical between runs.
#
# example (repo root):
#   XWIN_DIR=.work/tools/xwin/sdk scripts/build-openparrot.sh .work/upstream/tg-openparrot \
#     patches/openparrot:patches/clang-cl/openparrot .work/build/openparrot/upstream
set -eu

src=$(realpath "$1") patches=$2 out=$3
: "${XWIN_DIR:?set XWIN_DIR to the xwin splat directory}"
XWIN_DIR=$(realpath "$XWIN_DIR")
jobs=${JOBS:-$(nproc)}

mkdir -p "$out"; out=$(realpath "$out")
s=$out/src o=$out/obj
rm -rf "$s" "$o"; mkdir -p "$o"

# 1. scratch copy + patches. The copy gets its own (empty) git repo: inside
# another repo, `git apply` would silently skip paths outside that repo's root.
cp -a "$src" "$s"; rm -rf "$s/.git"; git -C "$s" init -q
if [ "$patches" != - ]; then
  echo "$patches" | tr : '\n' | while read -r pd; do
    for p in "$(realpath "$pd")"/*.patch; do
      [ -e "$p" ] || continue
      echo "patch: $pd/$(basename "$p")"
      git -C "$s" apply --whitespace=nowarn "$p"
    done
  done
fi
# MSVC resolves `#pragma comment(lib, "../inc/DirectXSDK/...")` against the
# libdir deps/lib; on Linux that directory must exist for "deps/lib/.." to work.
mkdir -p "$s/deps/lib"

# Windows file names are case-insensitive: `#include <WinNT.h>` must find
# winnt.h, "PE.H" must find PE.h. For every included name that does not
# exist with that exact case, link it from a case-insensitive match into
# $o/ci, which goes last on the include path.
ci=$o/ci; mkdir -p "$ci"
dirs=$(printf '%s\n' "$XWIN_DIR"/VC/Tools/MSVC/*/include "$XWIN_DIR/Windows Kits/10/Include/"*/*; find "$s" -type d)
grep -rhoE '#[[:space:]]*include[[:space:]]*[<"][^>"]+[>"]' "$s" --include='*.[ch]' --include='*.cpp' --include='*.hpp' |
  sed -E 's/.*[<"]([^>"]+)[>"]/\1/; s|\\+|/|g' | sort -u | while read -r n; do
  echo "$dirs" | while read -r d; do [ -e "$d/$n" ] && echo y && break; done | grep -q y && continue
  m=$(echo "$dirs" | while read -r d; do find "$d" -maxdepth 3 -ipath "$d/$n" 2>/dev/null; done | head -1)
  [ -n "$m" ] || continue
  mkdir -p "$ci/$(dirname "$n")"; ln -sf "$m" "$ci/$n"; echo "case: $n -> ${m#"$XWIN_DIR"/}"
done

# Same for `#pragma comment(lib, "Ws2_32.lib")`: link the SDK's ws2_32.lib
# as Ws2_32.lib into $o/cilib, which goes on the linker's library path.
cil=$o/cilib; mkdir -p "$cil"
libdirs=$(printf '%s\n' "$XWIN_DIR"/VC/Tools/MSVC/*/lib/x64 "$XWIN_DIR/Windows Kits/10/Lib/"*/*/x64)
grep -rhoE 'pragma[[:space:]]+comment[[:space:]]*\([[:space:]]*lib[[:space:]]*,[[:space:]]*"[^"]+"' "$s" |
  sed -E 's/.*"([^"]+)"/\1/' | sort -u | while read -r n; do
  case "$n" in */*|*\\*) continue ;; esac   # explicit paths (DirectXSDK) resolve as is
  case "$n" in *.lib|*.LIB) ;; *) n=$n.lib ;; esac
  echo "$libdirs" | while read -r d; do [ -e "$d/$n" ] && echo y && break; done | grep -q y && continue
  m=$(echo "$libdirs" | while read -r d; do find "$d" -maxdepth 1 -iname "$n"; done | head -1)
  [ -n "$m" ] || continue
  ln -sf "$m" "$cil/$n"; echo "case: $n -> ${m#"$XWIN_DIR"/}"
done

# 2. compilers
cflags="/nologo /c /Brepro /MT /O2 /Oi /GS /EHsc /w /DNDEBUG /DUNICODE /D_UNICODE"
cflags="$cflags /winsysroot $XWIN_DIR -m64 /I$s/deps/inc /I$s/deps/udis86"
# MSVC (permissive, C++17 mode) accepts what clang turns into errors by
# default: C89 implicit declarations (udis86), `register`, narrowing in
# brace initialisers. The remaining source-level differences are in the
# patches/clang-cl/* series. `#include <Utility\GameDetect.h>` works as is:
# clang-cl maps '\' in include paths to '/'.
cflags="$cflags -Wno-error=implicit-function-declaration -Wno-error=int-conversion"
cxxflags="/std:c++17 -Wno-register -Wno-c++11-narrowing /FIcstddef /I$ci"
# /FIcstddef: VS2017's STL pulled std::nullptr_t in through <cstdint>/<cstdio>,
# which injector.hpp relies on; current STL headers no longer do.
cc() { # cc <obj_subdir> <extra flags...> -- files...
  d=$o/$1; shift; mkdir -p "$d"; extra=""
  while [ "$1" != -- ]; do extra="$extra $1"; shift; done; shift
  for f in "$@"; do echo "$f"; done | xargs -P "$jobs" -I{} sh -c '
    f={}; obj="$0/$(echo "$f" | sed "s|^'"$s"'/||; s|/|_|g; s|\.[^.]*$|.obj|")"
    clang-cl '"$cflags $extra"' "/Fo$obj" -- "$f" || { echo "FAILED: $f" >&2; exit 255; }
  ' "$d"
}

# 3. static libs (MinHook calls __movsb without <intrin.h>: MSVC knows the
# intrinsic anyway, clang needs the header, hence /FIintrin.h)
cc minhook /FIintrin.h -- "$s"/deps/src/buffer.c "$s"/deps/src/hook.c "$s"/deps/src/trampoline.c "$s"/deps/src/hde/hde64.c
llvm-lib /nologo /out:"$o/MinHook.lib" "$o"/minhook/*.obj
cc udis86 -- "$s"/deps/udis86/libudis86/*.c
llvm-lib /nologo /out:"$o/udis86.lib" "$o"/udis86/*.obj

# 4. OpenParrot64.dll
op=$s/OpenParrot
cc op $cxxflags /D_WINDLL "/I$op/src" "/I$op/deps/inc" -- \
  $(find "$op/src" "$op/deps" -name '*.cpp' | sort)
llvm-ml -m64 -nologo -c "-Fo$o/win64init.obj" "$op/src/win64init.asm"
# the .rc is UTF-16LE (Visual Studio); llvm-rc wants UTF-8
iconv -f UTF-16LE -t UTF-8 "$op/src/OpenParrot.rc" | sed '1s/^\xEF\xBB\xBF//' > "$op/src/OpenParrot.utf8.rc"
# (llvm-rc, like clang-cl, needs "--" before a path that starts with '/')
sdkinc=$(echo "$XWIN_DIR/Windows Kits/10/Include/"*)
llvm-rc /nologo /C 65001 /I "$op/src" /I "$sdkinc/um" /I "$sdkinc/shared" \
  /FO "$o/OpenParrot.res" -- "$op/src/OpenParrot.utf8.rc"
(cd "$op" && lld-link /nologo /Brepro /DLL /MACHINE:X64 /OPT:REF /OPT:ICF /winsysroot:"$XWIN_DIR" \
  /LIBPATH:"$s/deps/lib" /LIBPATH:"$cil" /OUT:"$out/OpenParrot64.dll" \
  "$o"/op/*.obj "$o/win64init.obj" "$o/OpenParrot.res" "$o/MinHook.lib" "$o/udis86.lib")

# 5. OpenParrotLoader64.exe
ld=$s/OpenParrotLoader
cc loader $cxxflags "/I$ld/src" -- $(find "$ld/src" -name '*.cpp' | sort)
lld-link /nologo /Brepro /SUBSYSTEM:CONSOLE /MACHINE:X64 /OPT:REF /OPT:ICF /winsysroot:"$XWIN_DIR" \
  /LIBPATH:"$cil" /OUT:"$out/OpenParrotLoader64.exe" "$o"/loader/*.obj

rm -f "$out"/*.lib "$out"/*.pdb
(cd "$out" && sha256sum OpenParrot64.dll OpenParrotLoader64.exe > SHA256SUMS && cat SHA256SUMS)
