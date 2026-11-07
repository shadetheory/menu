#!/usr/bin/env bash
# Compositor proof for the held candidate. Headless Sway on this VM.
# One wtype process types the full queries. A second wtype is not used.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="${OUT:-/opt/cursor/artifacts}"
mkdir -p "$OUT"
BIN="${BIN:-$ROOT/target/release/omarchy-menu-rust}"
COUNTER="$ROOT/target/debug/click-counter"
CLICK="$ROOT/target/debug/seat-click"
[[ -x "$BIN" && -x "$COUNTER" && -x "$CLICK" ]] || {
  echo "missing binary (release menu, debug click-counter, debug seat-click)" >&2
  exit 1
}

work=$(mktemp -d /tmp/omarchy-menu-rust-seat.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
runtime="$work/runtime"
apps="$work/home/.local/share/applications"
mkdir -p "$runtime" "$apps"
chmod 700 "$runtime"
cat > "$apps/com.mitchellh.ghostty.desktop" << 'EOF'
[Desktop Entry]
Type=Application
Name=Ghostty
Exec=ghostty --gtk-single-instance=true
EOF

count="$work/clicks"
: > "$count"
prefix="$work/prefix"
HOME="$work/home" PREFIX="$prefix" bash "$ROOT/scripts/install-prefix.sh" --prefix="$prefix"
BIN="$prefix/current/bin/omarchy-menu-rust"
[[ -x "$BIN" ]] || { echo "install did not produce the binary" >&2; exit 1; }

cat > "$work/driver.sh" << EOF
#!/bin/bash
set -u
export XDG_DATA_HOME="$work/home/.local/share"
export XDG_DATA_DIRS=/nonexistent
export HOME="$work/home"
ps -eo pid,ppid,cmd > "$work/ps-before.txt"
"$CLICK" hold 8 600 1280 720 > "$work/hold.log" 2>&1 &
echo \$! > "$work/hold.pid"
sleep 0.3
"$COUNTER" "$count" > "$work/counter.log" 2>&1 &
echo \$! > "$work/counter.pid"
sleep 0.5
echo "control-before=\$(cat "$count" 2>/dev/null)" > "$work/clicks.txt"
"$CLICK" 8 600 1280 720 >> "$work/clicks.txt" 2>&1 || true
sleep 0.3
echo "control-after=\$(cat "$count" 2>/dev/null)" >> "$work/clicks.txt"
"$BIN" --no-exec --trace-state > "$work/menu.log" 2>&1 &
echo \$! > "$work/menu.pid"
for _ in \$(seq 1 30); do
  grep -q "visible dim" "$work/menu.log" 2>/dev/null && break
  sleep 0.1
done
sleep 0.2
ps -o pid,ppid,cmd --ppid "\$(cat "$work/menu.pid")" > "$work/children-open.txt" 2>&1 || true
grim "$OUT/omarchy-menu-rust-seat-shortcuts.png" || true
backs=()
for _ in \$(seq 1 14); do backs+=(-k BackSpace); done
wtype -s 400 -d 12 'Omarchy update' -k Return -s 900 -P Return -s 500 -p Return -s 80 \
  "\${backs[@]}" \
  -d 12 'Omarchi update' -k Return -s 900 -k Down -s 120 -k Return -s 200 \
  -k Escape -s 120 -k Escape > "$work/wtype.log" 2>&1 &
echo \$! > "$work/wtype.pid"
shot_omarchy=0
shot_omarchi=0
for _ in \$(seq 1 80); do
  last=\$(grep 'state ' "$work/menu.log" | tail -1)
  if [[ "\$shot_omarchy" -eq 0 && "\$last" == *'query="Omarchy update"'*'confirm=open'*'focus=cancel'* ]]; then
    sleep 0.15
    grim "$OUT/omarchy-menu-rust-seat-omarchy-confirm.png" || true
    shot_omarchy=1
  fi
  if [[ "\$shot_omarchi" -eq 0 && "\$last" == *'query="Omarchi update"'*'confirm=open'*'focus=cancel'* ]]; then
    sleep 0.15
    grim "$OUT/omarchy-menu-rust-seat-omarchi-confirm.png" || true
    shot_omarchi=1
  fi
  if grep -q 'close=1' "$work/menu.log"; then
    break
  fi
  sleep 0.1
done
wait "\$(cat "$work/wtype.pid")" || true
sleep 0.3
ps -o pid,ppid,cmd --ppid "\$(cat "$work/menu.pid")" > "$work/children-after.txt" 2>&1 || true
if [[ -d /proc/\$(cat "$work/menu.pid") ]]; then
  kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
  sleep 0.2
  if [[ -d /proc/\$(cat "$work/menu.pid") ]]; then
    echo menu-still-open >> "$work/menu.log"
  else
    echo menu-exited-after-close >> "$work/menu.log"
  fi
else
  echo menu-exited >> "$work/menu.log"
fi
grim "$OUT/omarchy-menu-rust-seat-after-escape.png" || true

# Outside click on a fresh menu. The counter already recorded one real click.
"$BIN" --no-exec --trace-state > "$work/menu-click.log" 2>&1 &
echo \$! > "$work/menu-click.pid"
for _ in \$(seq 1 30); do
  grep -q "visible dim" "$work/menu-click.log" 2>/dev/null && break
  sleep 0.1
done
sleep 0.2
echo "with-menu-before=\$(cat "$count" 2>/dev/null)" >> "$work/clicks.txt"
"$CLICK" 8 600 1280 720 >> "$work/clicks.txt" 2>&1 || true
sleep 0.4
echo "with-menu-after=\$(cat "$count" 2>/dev/null)" >> "$work/clicks.txt"
if [[ -d /proc/\$(cat "$work/menu-click.pid") ]]; then
  echo click-menu-still-open >> "$work/menu-click.log"
fi
grim "$OUT/omarchy-menu-rust-seat-after-click.png" || true

# Toggle the installed binary. SWAYSOCK is set because this runs under Sway.
if [[ -n "\${PREFIX:-}" && -x "\$PREFIX/current/bin/omarchy-menu-rust" ]]; then
  bash "$ROOT/scripts/alt-space-toggle.sh" --prefix="\$PREFIX" > "$work/toggle1.log" 2>&1 || true
  sleep 0.6
  grim "$OUT/omarchy-menu-rust-seat-toggle-open.png" || true
  bash "$ROOT/scripts/alt-space-toggle.sh" --prefix="\$PREFIX" > "$work/toggle2.log" 2>&1 || true
  sleep 0.5
  grim "$OUT/omarchy-menu-rust-seat-toggle-close.png" || true
fi
swaymsg -t get_tree > "$work/tree.json" 2>/dev/null || true
swaymsg exit || true
EOF
chmod 755 "$work/driver.sh"
printf 'xwayland disable\noutput * mode 1280x720\nexec %s\n' "$work/driver.sh" > "$work/sway.conf"

timeout 30s env -u WAYLAND_DISPLAY -u DISPLAY \
  XDG_RUNTIME_DIR="$runtime" \
  WLR_BACKENDS=headless \
  WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 \
  HOME="$work/home" \
  PREFIX="$prefix" \
  sway -c "$work/sway.conf" > "$work/sway.log" 2>&1 || true

cp -f "$work/menu.log" "$OUT/omarchy-menu-rust-seat-trace.log" 2>/dev/null || true
cp -f "$work/menu-click.log" "$OUT/omarchy-menu-rust-seat-click.log" 2>/dev/null || true
cp -f "$work/wtype.log" "$OUT/omarchy-menu-rust-seat-wtype.log" 2>/dev/null || true
cp -f "$work/sway.log" "$OUT/omarchy-menu-rust-seat-sway.log" 2>/dev/null || true
{
  echo "=== clicks ==="
  cat "$work/clicks.txt" 2>/dev/null || true
  echo "=== children open ==="
  cat "$work/children-open.txt" 2>/dev/null || true
  echo "=== children after ==="
  cat "$work/children-after.txt" 2>/dev/null || true
  echo "=== wtype ==="
  cat "$work/wtype.log" 2>/dev/null || true
  echo "=== menu ==="
  cat "$work/menu.log" 2>/dev/null || true
  echo "=== click menu ==="
  cat "$work/menu-click.log" 2>/dev/null || true
  echo "=== toggle1 ==="
  cat "$work/toggle1.log" 2>/dev/null || true
  echo "=== toggle2 ==="
  cat "$work/toggle2.log" 2>/dev/null || true
  echo "=== sway xdg lines ==="
  grep -n "xdg_surface\|protocol error" "$work/sway.log" || echo "no xdg_surface or protocol error lines"
  echo "=== sway tail ==="
  tail -15 "$work/sway.log"
} | tee "$OUT/omarchy-menu-rust-seat-report.txt"
