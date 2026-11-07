#!/usr/bin/env bash
# Alt+Space singleton for the layer-shell menu.
# Reads layer namespace and client map state. Dismisses a mapped menu, or
# starts one --no-exec process. Does not edit Hyprland, does not call a
# binder, and does not change stock Super+Space.
#
# HYPRCTL overrides the hyprctl binary (tests pass a mock).
# A live Sway socket is also read when SWAYSOCK is set. That is a test-VM
# path, not an injection from another host.
set -euo pipefail

NAMESPACE="omarchy-menu-rust"
HYPRCTL="${HYPRCTL:-hyprctl}"

if [[ -z "${PREFIX+x}" ]]; then
  PREFIX="${HOME}/.local/opt/omarchy-menu-rust"
fi

for arg in "$@"; do
  case "$arg" in
    --prefix=*) PREFIX="${arg#--prefix=}"; PREFIX_FROM_FLAG=1 ;;
    -h|--help)
      echo "Usage: scripts/alt-space-toggle.sh [--prefix=DIR]"
      echo "Dismisses a mapped ${NAMESPACE} layer or client, or starts one --no-exec."
      echo "Does not bind keys. Stock Super+Space is not changed."
      exit 0
      ;;
    --live|--exec|--run-guards|--safe-apps)
      echo "refusing ${arg}: the toggle only starts --no-exec" >&2
      exit 2
      ;;
    *)
      echo "refusing extra argument: $arg" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${HOME:-}" ]]; then
  echo "refusing to toggle without HOME set" >&2
  exit 2
fi
if [[ -z "$PREFIX" || "$PREFIX" == "/" || "$PREFIX" == "$HOME" || "$PREFIX" == "$HOME/" ]]; then
  echo "refusing prefix ${PREFIX:-<empty>}" >&2
  exit 2
fi

runtime="${XDG_RUNTIME_DIR:-/tmp}/omarchy-menu-rust-$(id -u)"
mkdir -p -- "$runtime"
chmod 700 "$runtime"
arrival="$runtime/arrival.$$"
: > "$arrival"
exec 9>"$runtime/toggle.lock"
flock 9

pidfile="$runtime/pid"
self_dir=$(dirname "$(readlink -f "$0")")
if [[ "${PREFIX_FROM_FLAG:-0}" == "1" ]]; then
  BIN="$PREFIX/current/bin/omarchy-menu-rust"
  SUMS="$PREFIX/current/share/SHA256SUMS"
elif [[ -x "$self_dir/omarchy-menu-rust" ]]; then
  BIN="$self_dir/omarchy-menu-rust"
  SUMS="$self_dir/../share/SHA256SUMS"
else
  BIN="$PREFIX/current/bin/omarchy-menu-rust"
  SUMS="$PREFIX/current/share/SHA256SUMS"
fi
if [[ ! -x "$BIN" ]]; then
  echo "missing $BIN" >&2
  exit 1
fi
BIN_CANON=$(readlink -f -- "$BIN")
if [[ -z "$BIN_CANON" || ! -f "$BIN_CANON" ]]; then
  echo "missing canonical binary for $BIN" >&2
  exit 1
fi
if [[ -f "$SUMS" ]]; then
  expected=$(awk '$2 == "bin/omarchy-menu-rust" { print $1; exit }' "$SUMS")
  got=$(sha256sum -- "$BIN_CANON" | awk 'NR==1 { print $1 }')
  if [[ -z "$expected" || "$got" != "$expected" ]]; then
    echo "refusing: installed binary checksum does not match SHA256SUMS" >&2
    exit 1
  fi
fi

layer_mapped() {
  local blob="" piece
  piece=$("$HYPRCTL" layers -j 2>/dev/null || true)
  blob+="$piece"
  piece=$("$HYPRCTL" clients -j 2>/dev/null || true)
  blob+="$piece"
  if [[ -n "${SWAYSOCK:-}" ]] && command -v swaymsg >/dev/null 2>&1; then
    piece=$(swaymsg -t get_tree 2>/dev/null || true)
    blob+="$piece"
  fi
  printf '%s' "$blob" | python3 -c '
import json, sys
ns = sys.argv[1]
raw = sys.stdin.read().strip()
if not raw:
    sys.exit(1)
found = False
decoder = json.JSONDecoder()
idx = 0
while idx < len(raw):
    while idx < len(raw) and raw[idx] not in "{[":
        idx += 1
    if idx >= len(raw):
        break
    try:
        value, end = decoder.raw_decode(raw, idx)
    except json.JSONDecodeError:
        break
    idx = end
    def walk(node):
        global found
        if found:
            return
        if isinstance(node, dict):
            if node.get("namespace") == ns:
                found = True
                return
            if node.get("app_id") == ns or node.get("class") == ns:
                if node.get("mapped", True) is not False:
                    found = True
                    return
            if node.get("type") == "layer_shell" and node.get("name") == ns:
                found = True
                return
            for child in node.values():
                walk(child)
        elif isinstance(node, list):
            for child in node:
                walk(child)
    walk(value)
sys.exit(0 if found else 1)
' "$NAMESPACE"
}

# Exact identity. A layer namespace by itself is not a process to signal.
# /proc/PID/exe must be the installed binary's inode, the argv must be only
# that path plus --no-exec, and the uid must be this user. The same checks
# run again after pidfd_open so a reused pid is not signaled.
export TOGGLE_BIN_CANON="$BIN_CANON"
export TOGGLE_UID="$(id -u)"

identity_py() {
  python3 - "$@" << 'PY'
import hashlib, os, signal, sys

canon = os.environ["TOGGLE_BIN_CANON"]
uid = int(os.environ["TOGGLE_UID"])
st_bin = os.stat(canon)
digest = hashlib.sha256(open(canon, "rb").read()).hexdigest()

def argv_of(pid):
    try:
        raw = open(f"/proc/{pid}/cmdline", "rb").read()
    except OSError:
        return None
    if raw.endswith(b"\0"):
        raw = raw[:-1]
    if not raw:
        return []
    return raw.split(b"\0")

def matches(pid):
    proc = f"/proc/{pid}"
    try:
        if os.stat(proc).st_uid != uid:
            return False
        exe_link = os.readlink(f"{proc}/exe")
        if exe_link.endswith(" (deleted)"):
            return False
        st_exe = os.stat(f"{proc}/exe")
    except OSError:
        return False
    if (st_exe.st_dev, st_exe.st_ino) != (st_bin.st_dev, st_bin.st_ino):
        return False
    if os.path.realpath(f"{proc}/exe") != canon:
        return False
    argv = argv_of(pid)
    if argv is None or len(argv) != 2 or argv[1] != b"--no-exec":
        return False
    try:
        arg0 = os.path.realpath(argv[0].decode())
    except (OSError, UnicodeError):
        return False
    if arg0 != canon:
        return False
    file_digest = hashlib.sha256(open(f"{proc}/exe", "rb").read()).hexdigest()
    return file_digest == digest

def list_pids():
    found = []
    for name in os.listdir("/proc"):
        if name.isdigit() and matches(int(name)):
            found.append(name)
    for name in sorted(found, key=int):
        print(name)

def signal_pid(pid):
    try:
        fd = os.pidfd_open(pid)
    except OSError:
        return 1
    try:
        if not matches(pid):
            return 1
        signal.pidfd_send_signal(fd, signal.SIGTERM)
        return 0
    finally:
        os.close(fd)

cmd = sys.argv[1]
if cmd == "list":
    list_pids()
elif cmd == "signal":
    sys.exit(signal_pid(int(sys.argv[2])))
else:
    sys.exit(2)
PY
}

owned_pids() {
  identity_py list || true
}

pidfd_term() {
  identity_py signal "$1" || true
}

mapped=0
if layer_mapped; then
  mapped=1
fi

mapfile -t pids < <(owned_pids)
if [[ ${#pids[@]} -gt 0 ]]; then
  peer=0
  for pid in "${pids[@]}"; do
    if [[ -d "/proc/$pid" && "/proc/$pid" -nt "$arrival" ]]; then
      peer=1
    fi
  done
  if [[ "$peer" == "1" ]]; then
    echo "omarchy-menu-rust: singleton already started" >&2
    rm -f -- "$arrival"
    exit 0
  fi
  for pid in "${pids[@]}"; do
    pidfd_term "$pid"
  done
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    mapfile -t left < <(owned_pids)
    [[ ${#left[@]} -eq 0 ]] && break
    sleep 0.05
  done
  rm -f -- "$pidfile" "$arrival"
  echo "omarchy-menu-rust: dismissed ${NAMESPACE}" >&2
  exit 0
fi
rm -f -- "$arrival"
if [[ "$mapped" == "1" ]]; then
  echo "omarchy-menu-rust: layer ${NAMESPACE} is mapped and no exact owned process was found. Not starting another." >&2
  exit 0
fi

python3 -c 'import os, sys
bin_path, pidfile = sys.argv[1], sys.argv[2]
read_fd, write_fd = os.pipe()
if os.fork() > 0:
    os.close(write_fd)
    os.read(read_fd, 1)
    os._exit(0)
os.close(read_fd)
os.setsid()
os.write(write_fd, b"1")
os.close(write_fd)
os.closerange(3, 256)
fd = os.open(pidfile, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
os.write(fd, str(os.getpid()).encode() + b"\n")
os.close(fd)
devnull = os.open(os.devnull, os.O_RDWR)
os.dup2(devnull, 0)
os.dup2(devnull, 1)
os.dup2(devnull, 2)
os.execv(bin_path, [bin_path, "--no-exec"])
' "$BIN" "$pidfile" >/dev/null 2>&1 &
wait "$!" || true
child=""
for _ in 1 2 3 4 5 6 7 8 9 10; do
  if [[ -s "$pidfile" ]]; then
    child=$(tr -d '[:space:]' < "$pidfile")
    break
  fi
  sleep 0.02
done
if [[ ! "$child" =~ ^[0-9]+$ ]]; then
  echo "omarchy-menu-rust: the menu process did not record a pid" >&2
  exit 1
fi
echo "omarchy-menu-rust: started ${child} --no-exec" >&2
exit 0
