#!/usr/bin/env bash
# Start the installed binary with --no-exec only.
# Does not pass --live, --exec, or --run-guards.
# Does not call hyprctl and does not change key bindings.
# Wayland layer namespace: omarchy-menu-rust
set -euo pipefail

if [[ -z "${PREFIX+x}" ]]; then
  PREFIX="${HOME}/.local/opt/omarchy-menu-rust"
fi

for arg in "$@"; do
  case "$arg" in
    --prefix=*) PREFIX="${arg#--prefix=}" ;;
    -h|--help)
      echo "Usage: scripts/launch-menu.sh [--prefix=DIR]"
      echo "Execs \$PREFIX/current/bin/omarchy-menu-rust --no-exec"
      echo "Extra arguments are refused. --live, --exec, and --run-guards stay off."
      echo "Layer namespace: omarchy-menu-rust"
      exit 0
      ;;
    *)
      echo "refusing extra argument: $arg" >&2
      echo "this launcher only starts --no-exec" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${HOME:-}" ]]; then
  echo "refusing to launch without HOME set" >&2
  exit 2
fi
if [[ -z "$PREFIX" || "$PREFIX" == "/" || "$PREFIX" == "$HOME" || "$PREFIX" == "$HOME/" ]]; then
  echo "refusing prefix ${PREFIX:-<empty>}" >&2
  exit 2
fi

BIN="$PREFIX/current/bin/omarchy-menu-rust"
if [[ ! -x "$BIN" ]]; then
  echo "missing $BIN" >&2
  echo "install first: scripts/install-prefix.sh" >&2
  exit 1
fi

exec "$BIN" --no-exec
