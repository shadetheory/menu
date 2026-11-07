#!/usr/bin/env bash
# Headless Sway seat proof on this test VM. Does not touch an installed host and does not inject input there.
# Requires sway, grim, and wtype. Screenshots land in /opt/cursor/artifacts.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="$ROOT/target/release/omarchy-menu-rust"
OUT="${OUT:-/opt/cursor/artifacts}"
if [[ ! -x "$BIN" ]]; then
  echo "build first: cargo build --release" >&2
  exit 1
fi
for cmd in sway grim wtype python3; do
  command -v "$cmd" >/dev/null || { echo "missing $cmd" >&2; exit 1; }
done

work=$(mktemp -d /tmp/omarchy-menu-rust-seat.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
runtime="$work/runtime"
mkdir -p "$runtime"
chmod 700 "$runtime"

make_theme() {
  local home=$1 bg=$2 fg=$3 accent=$4 card=$5 text=$6 sel=$7
  mkdir -p "$home/.local/state/omarchy/current/theme" "$home/.local/share/applications" "$home/.config/omarchy"
  printf 'background = "%s"\nforeground = "%s"\naccent = "%s"\n' "$bg" "$fg" "$accent" > "$home/.local/state/omarchy/current/theme/colors.toml"
  printf '[font]\nbase-size = 15\n[menu]\nbackground = "%s"\ntext = "%s"\nselected-background = "%s"\n' "$card" "$text" "$sel" > "$home/.local/state/omarchy/current/theme/shell.toml"
  cat > "$home/.local/share/applications/needs-input.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Needs Input
Exec=$BIN %f
EOF
}

make_theme "$work/dark" "#07090c" "#d5e6df" "#7ec8ff" "#10161c" "#d5e6df" "#163044"
make_theme "$work/light" "#f7f8fa" "#1a2330" "#0b6bcb" "#ffffff" "#1a2330" "#d7e4f5"

python3 - << PY
import struct, zlib
def checker(path, a, b):
    w, h = 1280, 720
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            row += bytes(a if ((x // 32) + (y // 32)) % 2 == 0 else b)
        rows.append(bytes(row))
    raw = b"".join(rows)
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(png)
checker("$work/checker-dark.png", (24, 26, 28), (232, 234, 236))
checker("$work/checker-light.png", (250, 250, 252), (180, 186, 194))
PY

prefix="$work/prefix"
mkdir -p "$prefix/current/bin"
ln -s "$BIN" "$prefix/current/bin/omarchy-menu-rust"
cat > "$work/hyprctl" << EOF
#!/bin/sh
case "\$1" in
  layers) cat "$work/layers.json" ;;
  clients) printf '%s\n' '[]' ;;
esac
EOF
chmod 755 "$work/hyprctl"
printf '%s\n' '{"DP-1":{"levels":{"2":[{"namespace":"omarchy-menu-rust","w":680,"h":74}]}}}' > "$work/layers.json"

cat > "$work/driver.sh" << EOF
#!/bin/bash
set -euo pipefail
export XDG_RUNTIME_DIR="$runtime"
export PREFIX="$prefix"
export HYPRCTL="$work/hyprctl"
shot() { grim "\$1"; }
launch() {
  HOME="\$1" XDG_DATA_HOME="\$1/.local/share" "$prefix/current/bin/omarchy-menu-rust" --no-exec &
  echo \$! > "$work/menu.pid"
  sleep 0.7
}
launch "$work/dark"
swaymsg -t get_tree > "$work/tree-dark.json" || true
shot "$OUT/omarchy-menu-rust-sway-dark-search.png"
wtype -s 25 About
sleep 0.4
shot "$OUT/omarchy-menu-rust-sway-dark-typed.png"
kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
sleep 0.3
launch "$work/dark"
wtype -s 25 About -k Down -k Return
sleep 0.6
shot "$OUT/omarchy-menu-rust-sway-dark-confirm.png"
kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
sleep 0.3
launch "$work/dark"
wtype -s 20 "Needs Input" -k Return
sleep 0.6
shot "$OUT/omarchy-menu-rust-sway-dark-input.png"
kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
sleep 0.3
swaymsg output "*" bg "$work/checker-light.png" fill || true
launch "$work/light"
shot "$OUT/omarchy-menu-rust-sway-light-search.png"
kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
sleep 0.3
launch "$work/light"
wtype -s 25 About -k Down -k Return
sleep 0.6
shot "$OUT/omarchy-menu-rust-sway-light-confirm.png"
MOCK_LAYERS="$work/layers.json" MOCK_CLIENTS=/dev/null bash "$ROOT/scripts/alt-space-toggle.sh" --prefix="$prefix" > "$work/toggle-dismiss.txt" 2>&1 || true
sleep 0.4
shot "$OUT/omarchy-menu-rust-sway-after-dismiss.png"
if [[ -d /proc/\$(cat "$work/menu.pid") ]]; then
  echo "menu still alive after toggle" > "$work/toggle-result.txt"
else
  echo "dismissed" > "$work/toggle-result.txt"
fi
printf '%s\n' '{"DP-1":{"levels":{"2":[]}}}' > "$work/layers.json"
bash "$ROOT/scripts/alt-space-toggle.sh" --prefix="$prefix" > "$work/toggle-reopen.txt" 2>&1 || true
sleep 0.6
shot "$OUT/omarchy-menu-rust-sway-after-reopen.png"
swaymsg exit
EOF
chmod 755 "$work/driver.sh"

cat > "$work/sway.conf" << EOF
xwayland disable
output * mode 1280x720 bg $work/checker-dark.png fill
exec $work/driver.sh
EOF

timeout 50s env -u WAYLAND_DISPLAY -u DISPLAY \
  XDG_RUNTIME_DIR="$runtime" \
  WLR_BACKENDS=headless \
  WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$work/sway.conf" > "$work/sway.log" 2>&1 || true
cp "$work/sway.log" "$OUT/omarchy-menu-rust-sway-seat.log"
cp "$work/toggle-result.txt" "$OUT/omarchy-menu-rust-toggle-result.txt" 2>/dev/null || true
cp "$work/toggle-dismiss.txt" "$OUT/omarchy-menu-rust-toggle-dismiss.txt" 2>/dev/null || true
echo "---- sway log tail ----"
tail -30 "$work/sway.log"
echo "---- toggle ----"
cat "$work/toggle-result.txt" 2>/dev/null || true
cat "$work/toggle-dismiss.txt" 2>/dev/null || true
ls -l "$OUT"/omarchy-menu-rust-sway-*.png
