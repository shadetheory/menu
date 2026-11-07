#!/usr/bin/env bash
# Seat proof for xdg_popup grab at Alt+Space launch, when the only serial is
# keyboard enter. An underlying layer counts clicks. This does not touch an installed host.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="${OUT:-/opt/cursor/artifacts}"
mkdir -p "$OUT"
for bin in click-counter popup-grab-probe seat-click; do
  [[ -x "$ROOT/target/debug/$bin" || -x "$ROOT/target/release/$bin" ]] || {
    echo "missing $bin" >&2
    exit 1
  }
done
pick() {
  if [[ -x "$ROOT/target/release/$1" ]]; then
    printf '%s\n' "$ROOT/target/release/$1"
  else
    printf '%s\n' "$ROOT/target/debug/$1"
  fi
}
COUNTER=$(pick click-counter)
PROBE=$(pick popup-grab-probe)
CLICK=$(pick seat-click)

work=$(mktemp -d /tmp/omarchy-menu-rust-outside.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
runtime="$work/runtime"
mkdir -p "$runtime"
chmod 700 "$runtime"
count="$work/clicks"
result="$work/probe.txt"
: > "$count"
: > "$result"

cat > "$work/driver.sh" << EOF
#!/bin/bash
set -u
"$CLICK" hold 10 600 1280 720 > "$work/hold.log" 2>&1 &
echo \$! > "$work/hold.pid"
sleep 0.2
"$COUNTER" "$count" > "$work/counter.log" 2>&1 &
echo \$! > "$work/counter.pid"
sleep 0.5
swaymsg -t get_tree > "$work/tree-control.json" || true
grim "$OUT/omarchy-menu-rust-outside-control.png" || true
echo "control-before=\$(cat "$count" 2>/dev/null)" > "$work/control.txt"
"$CLICK" 10 600 1280 720 >> "$work/control.txt" 2>&1 || true
sleep 0.3
echo "control-after=\$(cat "$count" 2>/dev/null)" >> "$work/control.txt"
"$PROBE" "$result" > "$work/probe.log" 2>&1 &
echo \$! > "$work/probe.pid"
sleep 0.6
echo "=== before keyboard ===" > "$work/phases.txt"
cat "$result" >> "$work/phases.txt" 2>/dev/null || true
# A virtual keyboard is what gives this seat a keyboard. Alt+Space on a real
# compositor already has one; the key itself is not delivered to the new client.
for _ in 1 2 3 4 5 6 7 8 9 10; do
  grep -q grab-sent "$result" 2>/dev/null && break
  sleep 0.1
done
wtype -s 15 hi -k Escape > "$work/wtype.log" 2>&1 || true
sleep 0.3
echo "=== after key ===" >> "$work/phases.txt"
cat "$result" >> "$work/phases.txt" 2>/dev/null || true
sleep 0.4
{
  echo "after-key clicks=\$(cat "$count" 2>/dev/null)"
  "$CLICK" 10 600 1280 720 || true
  sleep 0.3
  echo "after-outside clicks=\$(cat "$count" 2>/dev/null)"
  echo "=== after outside click ===" >> "$work/phases.txt"
  cat "$result" >> "$work/phases.txt" 2>/dev/null || true
  "$CLICK" 640 70 1280 720 || true
  sleep 0.3
  echo "after-pill clicks=\$(cat "$count" 2>/dev/null)"
} > "$work/click-out.log" 2>&1
sleep 0.2
swaymsg exit || true
EOF
chmod 755 "$work/driver.sh"
printf 'xwayland disable\noutput * mode 1280x720\nexec %s\n' "$work/driver.sh" > "$work/sway.conf"

timeout 20s env -u WAYLAND_DISPLAY -u DISPLAY \
  XDG_RUNTIME_DIR="$runtime" \
  WLR_BACKENDS=headless \
  WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$work/sway.conf" > "$work/sway.log" 2>&1 || true

{
  echo "=== probe ==="
  cat "$result" 2>/dev/null || true
  echo "=== probe log ==="
  cat "$work/probe.log" 2>/dev/null || true
  echo "=== counter clicks ==="
  cat "$count" 2>/dev/null || echo missing
  echo "=== counter log ==="
  cat "$work/counter.log" 2>/dev/null || true
  echo "=== control ==="
  cat "$work/control.txt" 2>/dev/null || true
  echo "=== phases ==="
  cat "$work/phases.txt" 2>/dev/null || true
  echo "=== outputs ==="
  cat "$work/outputs.json" 2>/dev/null || true
  echo "=== wtype ==="
  cat "$work/wtype.log" 2>/dev/null || true
  echo "=== clicks ==="
  cat "$work/click-out.log" 2>/dev/null || true
  echo "=== sway ==="
  tail -20 "$work/sway.log"
} | tee "$OUT/omarchy-menu-rust-outside-click.txt"
