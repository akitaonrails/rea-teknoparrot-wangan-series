#!/bin/sh
# Fetch the pinned upstream sources (packaging/sources.sh) and apply our
# patches, into <out>/{openparrot,openparrot-wmmt,wal}.
#
#   scripts/fetch-sources.sh <out> [openparrot|openparrot-wmmt|wal ...]
#
# Each tree is checked out at its pinned commit (git checks the SHA-1) and
# gets patches/<name>/*.patch in name order. The build-only clang-cl
# patches (patches/clang-cl) are not applied: the releases build
# OpenParrot with MSVC, like upstream. A tree that already exists is
# replaced. Needs git.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
die() { echo "fetch-sources: $*" >&2; exit 1; }
[ $# -ge 1 ] || die "usage: $0 <out> [name ...]"
out=$1; shift
[ $# -gt 0 ] || set -- openparrot openparrot-wmmt wal
. "$root/packaging/sources.sh"
mkdir -p "$out"; out=$(cd "$out" && pwd -P)

for name in "$@"; do
	case $name in
	openparrot) repo=$OPENPARROT_REPO commit=$OPENPARROT_COMMIT ;;
	openparrot-wmmt) repo=$OPENPARROT_WMMT_REPO commit=$OPENPARROT_WMMT_COMMIT ;;
	wal) repo=$WAL_REPO commit=$WAL_COMMIT ;;
	*) die "unknown source $name" ;;
	esac
	d=$out/$name
	rm -rf "$d"
	git init -q "$d"
	git -C "$d" fetch -q --depth 1 "$repo" "$commit"
	git -C "$d" -c advice.detachedHead=false checkout -q FETCH_HEAD
	[ "$(git -C "$d" rev-parse HEAD)" = "$commit" ] || die "$name is not at $commit"
	for p in "$root/patches/$name"/*.patch; do
		[ -e "$p" ] || continue
		echo "fetch-sources: $name <- patches/$name/$(basename "$p")"
		git -C "$d" apply --whitespace=nowarn "$p"
	done
	echo "fetch-sources: $name at $commit"
done
