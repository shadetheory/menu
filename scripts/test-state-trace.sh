#!/usr/bin/env bash
# Headless Sway proof that keys change query, selection, confirm, and refusal.
# No screenshot. This VM only.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="${OUT:-/opt/cursor/artifacts}"
MENU="$ROOT/target/release/omarchy-menu-rust"
[[ -x "$MENU" ]] || { echo "missing $MENU" >&2; exit 1; }
command -v sway >/dev/null && command -v wtype >/dev/null || { echo "need sway and wtype" >&2; exit 1; }
work=$(mktemp -d /tmp/omarchy-menu-rust-trace.XXXXXX)
trap 'rm -rf -- "$work"' EXIT
runtime="$work/runtime"
mkdir -p "$runtime"
chmod 700 "$runtime"
run_seq() {
  local name="$1" keys="$2"
  cat > "$work/driver.sh" << EOF
#!/bin/bash
set -u
"$MENU" --no-exec --trace-state > "$work/${name}.log" 2>&1 &
echo \$! > "$work/menu.pid"
sleep 0.6
wtype $keys
sleep 0.6
if [[ -d /proc/\$(cat "$work/menu.pid") ]]; then
  kill "\$(cat "$work/menu.pid")" 2>/dev/null || true
  echo still-open >> "$work/${name}.log"
else
  echo exited >> "$work/${name}.log"
fi
swaymsg exit || true
EOF
  chmod 755 "$work/driver.sh"
  printf 'xwayland disable\noutput * mode 1280x720\nexec %s\n' "$work/driver.sh" > "$work/sway.conf"
  timeout 15s env -u WAYLAND_DISPLAY -u DISPLAY \
    XDG_RUNTIME_DIR="$runtime" WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    sway -c "$work/sway.conf" > "$work/sway.log" 2>&1 || true
  cp "$work/${name}.log" "$OUT/omarchy-menu-rust-state-${name}.log"
}

run_seq confirm '-s 20 About -k Down -k Return -k Return'
run_seq refuse '-s 20 About -k Down -k Return -k Down -k Return'
run_seq escape '-s 20 About -k Return -k Escape -k Escape'

python3 - << PY
from pathlib import Path
out = Path("$OUT")
def lines(name):
    text = (out / f"omarchy-menu-rust-state-{name}.log").read_text()
    return [line.split("state ", 1)[1] for line in text.splitlines() if "omarchy-menu-rust: state " in line]

def later(rows, pred):
    for i, line in enumerate(rows):
        if pred(line):
            return rows[i + 1 :]
    raise SystemExit(rows)

confirm = lines("confirm")
opened = later(confirm, lambda line: "confirm=open" in line and "focus=cancel" in line and 'query="About"' in line and 'selected="About"' in line)
assert any("refusal=cancelled" in line and "confirm=none" in line and "close=0" in line for line in opened), opened

refuse = lines("refuse")
armed = later(refuse, lambda line: "confirm=open" in line and "focus=confirm" in line)
assert any("refusal=dry-run" in line and "close=0" in line for line in armed), armed

escape = lines("escape")
entered = later(escape, lambda line: 'query=""' in line and "close=0" in line and 'selected="Edit Text"' in line)
restored = later(entered, lambda line: 'query="About"' in line and "close=0" in line)
assert any("close=1" in line for line in restored), restored
print("state-trace ok", len(confirm), len(refuse), len(escape))
PY
