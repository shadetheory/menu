#!/usr/bin/env bash
# Hand one route to the stock Omarchy menu. Never calls this Rust binary.
# Route is an argument, not a shell string. Search text is not accepted.
set -euo pipefail

if [[ "${OMARCHY_MENU_RUST_FALLBACK:-}" == "1" ]]; then
  echo "omarchy-menu-rust: stock fallback already active; refusing recursion" >&2
  exit 3
fi

verb="${1:-summon}"
route="${2:-root}"

case "$verb" in
  summon|toggle|close|refresh|ping) ;;
  *)
    echo "omarchy-menu-rust: fallback verb must be summon|toggle|close|refresh|ping" >&2
    exit 2
    ;;
esac

if [[ "$route" == *$'\n'* ]]; then
  echo "omarchy-menu-rust: refusing route that contains a newline" >&2
  exit 2
fi

export OMARCHY_MENU_RUST_FALLBACK=1
unset OMARCHY_MENU_QUERY || true

self=""
if [[ -n "${OMARCHY_MENU_RUST_EXE:-}" ]]; then
  self=$(readlink -f "$OMARCHY_MENU_RUST_EXE" 2>/dev/null || printf '%s' "$OMARCHY_MENU_RUST_EXE")
fi

resolve() {
  command -v "$1" 2>/dev/null || true
}

target=$(resolve omarchy-menu || true)
if [[ -z "$target" && -n "${OMARCHY_PATH:-}" && -x "${OMARCHY_PATH}/bin/omarchy-menu" ]]; then
  target="${OMARCHY_PATH}/bin/omarchy-menu"
fi

if [[ -z "$target" ]]; then
  echo "omarchy-menu-rust: stock omarchy-menu is not on PATH. Nothing was started." >&2
  exit 1
fi

resolved=$(readlink -f "$target" 2>/dev/null || printf '%s' "$target")
base=$(basename "$resolved")
if [[ "$base" == "omarchy-menu-rust" ]]; then
  echo "omarchy-menu-rust: stock target is this binary; refusing recursion" >&2
  exit 3
fi
if [[ -n "$self" && "$resolved" == "$self" ]]; then
  echo "omarchy-menu-rust: stock target is this binary; refusing recursion" >&2
  exit 3
fi

# argv only. The stock wrapper talks to Quickshell; it does not eval the route.
exec "$target" "$verb" "$route"
