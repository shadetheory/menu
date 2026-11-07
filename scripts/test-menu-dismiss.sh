#!/usr/bin/env bash
# Real menu: keyboard still works, and an outside click dismisses without
# incrementing the underlying window. Headless Sway on this VM.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="${OUT:-/opt/cursor/artifacts}"
MENU="$ROOT/target/release/omarchy-menu-rust"
CLICK="$ROOT/target/release/seat-click"
COUNTER="$ROOT/target/release/click-counter"
for bin in "$MENU" "$CLICK" "$COUNTER"; do
  [[ -x "$bin" ]] || { echo "missing $bin" >&2; exit 1; }
done
work=$(mktemp -d /tmp/omarchy-menu-rust-dismiss.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
runtime="$work/runtime"
mkdir -p "$runtime" "$work/home/.local/state/omarchy/current/theme"
chmod 700 "$runtime"
printf '%s\n' 'background = "#07090c"' 'foreground = "#d5e6df"' 'accent = "#7ec8ff"' > "$work/home/.local/state/omarchy/current/theme/colors.toml"
printf '%s\n' '[font]' 'base-size = 15' '[menu]' 'background = "#10161c"' 'text = "#d5e6df"' 'selected-background = "#163044"' > "$work/home/.local/state/omarchy/current/theme/shell.toml"
count="$work/clicks"
cat > "$work/driver.sh" << EOF
#!/bin/bash
set -u
"$CLICK" hold 10 600 1280 720 >/dev/null 2>&1 &
sleep 0.2
"$COUNTER" "$count" > "$work/counter.log" 2>&1 &
sleep 0.5
"$CLICK" 10 600 1280 720 > "$work/control.log" 2>&1 || true
sleep 0.3
echo "control=\$(cat "$count")" > "$work/result.txt"
HOME="$work/home" "$MENU" --no-exec > "$work/menu.log" 2>&1 &
echo \$! > "$work/menu.pid"
sleep 0.6
grim "$OUT/omarchy-menu-rust-dismiss-idle.png" || true
wtype -s 20 About
sleep 0.4
grim "$OUT/omarchy-menu-rust-dismiss-typed.png" || true
"$CLICK" 10 600 1280 720 > "$work/outside.log" 2>&1 || true
sleep 0.5
echo "after=\$(cat "$count")" >> "$work/result.txt"
if [[ -d /proc/\$(cat "$work/menu.pid") ]]; then
  echo "menu=alive" >> "$work/result.txt"
else
  echo "menu=exited" >> "$work/result.txt"
fi
grim "$OUT/omarchy-menu-rust-dismiss-after.png" || true
swaymsg exit || true
EOF
chmod 755 "$work/driver.sh"
printf 'xwayland disable\noutput * mode 1280x720\nexec %s\n' "$work/driver.sh" > "$work/sway.conf"
timeout 20s env -u WAYLAND_DISPLAY -u DISPLAY \
  XDG_RUNTIME_DIR="$runtime" WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$work/sway.conf" > "$work/sway.log" 2>&1 || true
{
  echo "=== result ==="
  cat "$work/result.txt" 2>/dev/null || true
  echo "=== menu ==="
  cat "$work/menu.log" 2>/dev/null || true
  echo "=== counter ==="
  cat "$work/counter.log" 2>/dev/null || true
  echo "=== sway ==="
  rg "xdg_surface|error" "$work/sway.log" || true
} | tee "$OUT/omarchy-menu-rust-menu-dismiss.txt"
