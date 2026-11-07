#!/usr/bin/env bash
# Open the visual mock in its own Chromium app window.
# Never uses xdg-open. Never uses the default browser profile.
# Does not change the default browser. Does not bind keys.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
HTML="$ROOT/index.html"
if [[ ! -f "$HTML" ]]; then
  echo "missing $HTML" >&2
  exit 1
fi

PROFILE="${XDG_RUNTIME_DIR:-/tmp}/omarchy-menu-preview/chromium-profile"
case "$PROFILE" in
  "$HOME/.config/google-chrome"|"$HOME/.config/google-chrome"/*|\
  "$HOME/.config/chromium"|"$HOME/.config/chromium"/*|\
  "$HOME/.config/BraveSoftware"|"$HOME/.config/BraveSoftware"/*|\
  "$HOME/.mozilla"|"$HOME/.mozilla"/*)
    echo "refusing to reuse a regular browser profile: $PROFILE" >&2
    exit 2
    ;;
esac
mkdir -p "$PROFILE"

pick=""
for candidate in google-chrome google-chrome-stable chromium chromium-browser; do
  if command -v "$candidate" >/dev/null 2>&1; then
    pick=$(command -v "$candidate")
    break
  fi
done
if [[ -z "$pick" ]]; then
  echo "No Chromium-family browser on PATH. Refusing to open a URL in another browser." >&2
  exit 1
fi

URL="file://${HTML}?presentation=window"
CLASS="omarchy-menu-preview"

# A different --user-data-dir is what stops Chromium from handing the URL to
# an already running browser as a new tab. --app opens a window, not a tab.
exec "$pick" \
  --user-data-dir="$PROFILE" \
  --class="$CLASS" \
  --name="$CLASS" \
  --app="$URL" \
  --no-first-run \
  --no-default-browser-check \
  --disable-sync \
  --disable-extensions \
  --disable-component-update \
  --disable-default-apps \
  --no-pings \
  --disable-background-networking \
  --window-size=960,720
