#!/usr/bin/env bash
# Print the installed binary with --no-exec. Does not bind keys.
# Exec only when --launch is passed.
set -euo pipefail

if [[ -z "${PREFIX+x}" ]]; then
  PREFIX="${HOME}/.local/opt/omarchy-menu-rust"
fi

launch=0
for arg in "$@"; do
  case "$arg" in
    --prefix=*) PREFIX="${arg#--prefix=}" ;;
    --launch) launch=1 ;;
    -h|--help)
      echo "Usage: scripts/try.sh [--prefix=DIR] [--launch]"
      echo "Prints \$PREFIX/current/bin/omarchy-menu-rust --no-exec"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

BIN="$PREFIX/current/bin/omarchy-menu-rust"
echo "$BIN --no-exec"
if [[ "$launch" == "1" ]]; then
  exec "$BIN" --no-exec
fi
