#!/bin/sh
# Run the local release build of the launcher on the real screen, with
# sound, against the components built in .work/components and a dev data
# folder (.work/dev-data: settings, logs, Wine prefix, game runtimes).
#
#   scripts/dev-run.sh                 # the hub
#   scripts/dev-run.sh --play mt6      # any launcher option
#
# Build first: cargo build --release -p wangan-launcher
# gamescope and Wine come from the host if it has them, else from the
# `gaming` distrobox (override with WANGAN_BOX).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd -P)
exe=$root/target/release/wangan-launcher
[ -x "$exe" ] || { echo "dev-run: build first: cargo build --release -p wangan-launcher" >&2; exit 1; }

export WANGAN_DATA=${WANGAN_DATA:-$root/.work/dev-data}
export WANGAN_COMPONENTS=${WANGAN_COMPONENTS:-$root/.work/components}
mkdir -p "$WANGAN_DATA"

if command -v gamescope >/dev/null 2>&1 && command -v wine >/dev/null 2>&1; then
	exec "$exe" "$@"
fi
box=${WANGAN_BOX:-gaming}
cd /
exec distrobox enter "$box" -- env WANGAN_DATA="$WANGAN_DATA" WANGAN_COMPONENTS="$WANGAN_COMPONENTS" "$exe" "$@"
