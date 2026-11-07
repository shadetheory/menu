#!/usr/bin/env bash
# Headless Sway captures for the unpromoted candidate. This VM only.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="${OUT:-/opt/cursor/artifacts}"
mkdir -p "$OUT"
BIN="$ROOT/target/debug/omarchy-menu-rust"
COUNTER="$ROOT/target/debug/click-counter"
[[ -x "$BIN" && -x "$COUNTER" ]] || {
  echo "build debug binaries first" >&2
  exit 1
}

work=$(mktemp -d /tmp/omarchy-menu-rust-review.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
runtime="$work/runtime"
apps="$work/home/.local/share/applications"
mkdir -p "$runtime" "$apps"
chmod 700 "$runtime"

write_desktop() {
  local name="$1" title="$2" exec="$3"
  cat > "$apps/$name" << EOF
[Desktop Entry]
Type=Application
Name=$title
Exec=$exec
EOF
}
write_desktop com.mitchellh.ghostty.desktop Ghostty "ghostty --gtk-single-instance=true"
write_desktop com.thisisgm.flea.desktop Flea "flea --gui"
write_desktop cursor.desktop Cursor "/usr/share/cursor/cursor"
write_desktop grok-bot.desktop "Grok Bot" "grok-bot"
write_desktop mattermost-desktop.desktop Mattermost "mattermost-desktop"

# Evidence argv. Created only for this capture, never executed (--no-exec).
for path in \
  /usr/share/omarchy/bin/omarchy-launch-browser
do
  if mkdir -p "$(dirname "$path")" 2>/dev/null && printf '%s\n' '#!/bin/sh' > "$path" 2>/dev/null; then
    echo "stub $path"
  else
    echo "absent $path"
  fi
done

count="$work/clicks"
: > "$count"

cat > "$work/pixels.py" << 'PY'
import struct, sys, zlib

def load(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"not a png: {path}")
    pos = 8
    width = height = color = None
    idat = b""
    while pos + 8 <= len(data):
        ln, typ = struct.unpack(">I4s", data[pos : pos + 8])
        chunk = data[pos + 8 : pos + 8 + ln]
        if typ == b"IHDR":
            width, height, _bit, color, _comp, _filt, _inter = struct.unpack(">IIBBBBB", chunk[:13])
        elif typ == b"IDAT":
            idat += chunk
        elif typ == b"IEND":
            break
        pos += 12 + ln
    bpp = 4 if color == 6 else 3 if color == 2 else 0
    if bpp == 0:
        raise SystemExit(f"unsupported color {color}")
    raw = zlib.decompress(idat)
    stride = width * bpp
    rows = []
    i = 0
    prev = bytearray(stride)
    for _y in range(height):
        filt = raw[i]
        i += 1
        row = bytearray(raw[i : i + stride])
        i += stride
        if filt == 1:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + left) & 255
        elif filt == 2:
            for x in range(stride):
                row[x] = (row[x] + prev[x]) & 255
        elif filt == 3:
            for x in range(stride):
                left = row[x - bpp] if x >= bpp else 0
                row[x] = (row[x] + ((left + prev[x]) // 2)) & 255
        elif filt == 4:
            for x in range(stride):
                a = row[x - bpp] if x >= bpp else 0
                b = prev[x]
                c = prev[x - bpp] if x >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if pa <= pb and pa <= pc else b if pb <= pc else c
                row[x] = (row[x] + pr) & 255
        elif filt != 0:
            raise SystemExit(f"bad filter {filt}")
        prev = row
        rows.append(bytes(row))
    return width, height, bpp, rows

def px(rows, bpp, x, y):
    row = rows[y]
    i = x * bpp
    return tuple(row[i : i + 3])

path, xs = sys.argv[1], sys.argv[2:]
w, h, bpp, rows = load(path)
print(f"size {w}x{h}")
for spec in xs:
    x, y = (int(part) for part in spec.split(","))
    print(f"{x},{y} {px(rows, bpp, x, y)}")
PY

cat > "$work/driver.sh" << EOF
#!/bin/bash
set -u
export XDG_DATA_HOME="$work/home/.local/share"
export XDG_DATA_DIRS="/nonexistent"
"$COUNTER" "$count" > "$work/counter.log" 2>&1 &
echo \$! > "$work/counter.pid"
sleep 0.6
grim "$OUT/omarchy-menu-rust-review-before.png" || true
"$BIN" --no-exec --trace-state > "$work/menu.log" 2>&1 &
echo \$! > "$work/menu.pid"
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  grep -q "visible dim" "$work/menu.log" 2>/dev/null && grep -q "center output" "$work/menu.log" && break
  sleep 0.15
done
sleep 0.3
grim "$OUT/omarchy-menu-rust-review-open.png" || true
swaymsg -t get_tree > "$work/tree-open.json" || true
swaymsg -t get_outputs > "$work/outputs.json" || true
kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
sleep 0.4
grim "$OUT/omarchy-menu-rust-review-close.png" || true
# Separate menu so the query is the first wtype process on that seat.
"$BIN" --no-exec --trace-state > "$work/update.log" 2>&1 &
echo \$! > "$work/update.pid"
sleep 0.7
wtype -s 25 "Omarchy update" > "$work/wtype.log" 2>&1 || true
sleep 0.5
grim "$OUT/omarchy-menu-rust-review-update.png" || true
kill "\$(cat "$work/update.pid")" 2>/dev/null || true
swaymsg exit || true
EOF
chmod 755 "$work/driver.sh"
printf 'xwayland disable\noutput * mode 1280x720\nexec %s\n' "$work/driver.sh" > "$work/sway.conf"

timeout 25s env -u WAYLAND_DISPLAY -u DISPLAY \
  XDG_RUNTIME_DIR="$runtime" \
  WLR_BACKENDS=headless \
  WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 \
  sway -c "$work/sway.conf" > "$work/sway.log" 2>&1 || true

python3 "$work/pixels.py" "$OUT/omarchy-menu-rust-review-before.png" 8,8 8,600 640,360 > "$work/pixels-before.txt" || true
python3 "$work/pixels.py" "$OUT/omarchy-menu-rust-review-open.png" 8,8 8,600 640,360 > "$work/pixels-open.txt" || true
python3 "$work/pixels.py" "$OUT/omarchy-menu-rust-review-close.png" 8,8 8,600 640,360 > "$work/pixels-close.txt" || true
python3 "$work/pixels.py" "$OUT/omarchy-menu-rust-review-update.png" 8,8 340,200 > "$work/pixels-update.txt" || true

{
  echo "=== menu log ==="
  cat "$work/menu.log" 2>/dev/null || true
  echo "=== update log ==="
  cat "$work/update.log" 2>/dev/null || true
  echo "=== pixels before ==="
  cat "$work/pixels-before.txt" 2>/dev/null || true
  echo "=== pixels open ==="
  cat "$work/pixels-open.txt" 2>/dev/null || true
  echo "=== pixels close ==="
  cat "$work/pixels-close.txt" 2>/dev/null || true
  echo "=== pixels update ==="
  cat "$work/pixels-update.txt" 2>/dev/null || true
  echo "=== wtype ==="
  cat "$work/wtype.log" 2>/dev/null || true
  echo "=== sway tail ==="
  tail -30 "$work/sway.log"
} | tee "$OUT/omarchy-menu-rust-review.txt"

cp -f "$work/menu.log" "$OUT/omarchy-menu-rust-review-menu.log" 2>/dev/null || true
