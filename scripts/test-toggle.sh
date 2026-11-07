#!/usr/bin/env bash
# Mock hyprctl layers/clients. Does not bind Super+Space and does not touch an installed host.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TOGGLE="$ROOT/scripts/alt-space-toggle.sh"
fail() { echo "FAIL: $*" >&2; exit 1; }

bash -n "$TOGGLE"
grep -q "Super+Space is not changed" "$TOGGLE" || fail "toggle help no longer preserves Super+Space"
if grep -nE 'hyprctl[[:space:]]+keyword|bind[lr]?=' "$TOGGLE"; then
  fail "toggle script looks like a key binder"
fi
command -v gcc >/dev/null || fail "gcc is required to build the exact-exe stand-in"

tmp=$(mktemp -d /tmp/omarchy-menu-rust-toggle.XXXXXX)
trap 'rm -rf -- "$tmp"' EXIT
export XDG_RUNTIME_DIR="$tmp/runtime"
export HOME="$tmp/home"
mkdir -p "$HOME" "$XDG_RUNTIME_DIR"
prefix="$tmp/prefix"
mkdir -p "$prefix/current/bin"
starts="$tmp/starts"
killed="$tmp/killed"
export TOGGLE_STARTS="$starts"
export TOGGLE_KILLED="$killed"
cat > "$tmp/standin.c" << 'EOF'
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
static void on_term(int sig) {
  const char *path = getenv("TOGGLE_KILLED");
  FILE *f;
  (void)sig;
  if (path && (f = fopen(path, "a"))) {
    fprintf(f, "%d\n", getpid());
    fclose(f);
  }
  _exit(0);
}
int main(void) {
  const char *path = getenv("TOGGLE_STARTS");
  FILE *f;
  if (path && (f = fopen(path, "a"))) {
    fprintf(f, "%d\n", getpid());
    fclose(f);
  }
  signal(SIGTERM, on_term);
  for (;;) pause();
}
EOF
gcc -O2 -o "$prefix/current/bin/omarchy-menu-rust" "$tmp/standin.c"
other="$tmp/other-prefix"
mkdir -p "$other/current/bin"
gcc -O2 -o "$other/current/bin/omarchy-menu-rust" "$tmp/standin.c"

mock="$tmp/hyprctl"
cat > "$mock" << 'EOF'
#!/bin/sh
case "$1" in
  layers) cat "$MOCK_LAYERS" ;;
  clients) cat "$MOCK_CLIENTS" ;;
  *) exit 0 ;;
esac
EOF
chmod 755 "$mock"
empty="$tmp/empty.json"
printf '%s\n' '{"DP-1":{"levels":{"2":[]}}}' > "$empty"
clients_empty="$tmp/clients-empty.json"
printf '%s\n' '[]' > "$clients_empty"
mapped="$tmp/layers-mapped.json"
printf '%s\n' '{"DP-1":{"levels":{"2":[{"namespace":"omarchy-menu-rust","w":680,"h":74}]}}}' > "$mapped"
client_mapped="$tmp/clients-mapped.json"
printf '%s\n' '[{"class":"omarchy-menu-rust","title":"shadetheory","mapped":true}]' > "$client_mapped"
client_unmapped="$tmp/clients-unmapped.json"
printf '%s\n' '[{"class":"omarchy-menu-rust","mapped":false}]' > "$client_unmapped"

export HYPRCTL="$mock"
export PREFIX="$prefix"
unset SWAYSOCK || true
alive() { [[ -d "/proc/$1" ]]; }

MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.1
[[ -s "$starts" ]] || fail "first toggle did not start the menu"
lines=$(wc -l < "$starts")
[[ "$lines" -eq 1 ]] || fail "first toggle started $lines processes"
pid=$(tail -n 1 "$starts")
alive "$pid" || fail "started pid $pid is not alive"

MOCK_LAYERS="$mapped" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.3
alive "$pid" && fail "mapped layer did not dismiss pid $pid"
[[ -s "$killed" && "$(wc -l < "$killed")" -eq 1 ]] || fail "dismiss did not signal TERM"
[[ "$(wc -l < "$starts")" -eq 1 ]] || fail "dismiss started another process"

MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.1
[[ "$(wc -l < "$starts")" -eq 2 ]] || fail "reopen did not start exactly one more process"
pid=$(tail -n 1 "$starts")
alive "$pid" || fail "reopened pid is not alive"

MOCK_LAYERS="$empty" MOCK_CLIENTS="$client_mapped" bash "$TOGGLE" --prefix="$prefix"
sleep 0.3
alive "$pid" && fail "mapped client did not dismiss pid $pid"
[[ "$(wc -l < "$killed")" -eq 2 ]] || fail "client dismiss did not signal TERM"

MOCK_LAYERS="$empty" MOCK_CLIENTS="$client_unmapped" bash "$TOGGLE" --prefix="$prefix"
sleep 0.1
[[ "$(wc -l < "$starts")" -eq 3 ]] || fail "unmapped client should reopen"
pid=$(tail -n 1 "$starts")
kill -TERM "$pid" 2>/dev/null || true
sleep 0.2
alive "$pid" && fail "could not stop the menu before the dim-namespace check"

# The dim layer is not the menu. The real namespace still blocks a start when no process exists.
dim_only="$tmp/layers-dim.json"
printf '%s\n' '{"DP-1":{"levels":{"2":[{"namespace":"omarchy-menu-rust-dim","w":1280,"h":720}]}}}' > "$dim_only"
before=$(wc -l < "$starts")
MOCK_LAYERS="$dim_only" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.1
[[ "$(wc -l < "$starts")" -eq $((before + 1)) ]] || fail "dim namespace did not allow a start"
pid=$(tail -n 1 "$starts")
alive "$pid" || fail "dim-only start is not alive"
kill -TERM "$pid" 2>/dev/null || true
sleep 0.2
alive "$pid" && fail "could not stop the dim-only start"
MOCK_LAYERS="$mapped" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix" >"$tmp/dim-mapped.out" 2>&1 || true
sleep 0.2
[[ "$(wc -l < "$starts")" -eq $((before + 1)) ]] || fail "real namespace started while only the dim check should have"
grep -q "is mapped and no exact owned process" "$tmp/dim-mapped.out" || fail "real namespace was not treated as mapped"

# Decoy: the binary path is only an argument. It must not be signaled.
decoy_starts="$tmp/decoy-starts"
: > "$decoy_starts"
TOGGLE_STARTS="$decoy_starts" python3 -c 'import time; time.sleep(120)' "$prefix/current/bin/omarchy-menu-rust" --no-exec &
decoy=$!
sleep 0.1
alive "$decoy" || fail "decoy did not start"
before=$(wc -l < "$starts")
MOCK_LAYERS="$mapped" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.2
alive "$decoy" || fail "decoy with the binary path in argv was signaled"
[[ "$(wc -l < "$starts")" -eq "$before" ]] || fail "mapped namespace plus a decoy started another menu"
kill -TERM "$decoy" 2>/dev/null || true

# Other installation: different inode, even with --no-exec.
other_starts="$tmp/other-starts"
: > "$other_starts"
TOGGLE_STARTS="$other_starts" "$other/current/bin/omarchy-menu-rust" --no-exec &
other_pid=$!
sleep 0.1
alive "$other_pid" || fail "other installation did not start"
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.2
alive "$other_pid" || fail "other installation was signaled"
kill -TERM "$other_pid" 2>/dev/null || true
kill -TERM "$(tail -n 1 "$starts")" 2>/dev/null || true
sleep 0.1

# Replaced inode: the running process is not the file now installed.
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.1
old=$(tail -n 1 "$starts")
alive "$old" || fail "inode fixture did not start"
cp -a -- "$prefix/current/bin/omarchy-menu-rust" "$tmp/previous-inode"
gcc -O2 -o "$prefix/current/bin/omarchy-menu-rust" "$tmp/standin.c"
before_killed=$(wc -l < "$killed")
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.2
alive "$old" || fail "previous inode was signaled"
new=$(tail -n 1 "$starts")
[[ "$new" != "$old" ]] || fail "replaced binary did not start the new inode"
[[ "$(wc -l < "$killed")" -eq "$before_killed" ]] || fail "replaced inode recorded a signal"
kill -TERM "$old" "$new" 2>/dev/null || true
sleep 0.1

# Stale pidfile must not signal a different process, including after the pid is gone.
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.1
stale=$(tail -n 1 "$starts")
kill -KILL "$stale" 2>/dev/null || true
sleep 0.1
printf '%s\n' "$stale" > "$XDG_RUNTIME_DIR/omarchy-menu-rust-$(id -u)/pid"
decoy2_starts="$tmp/decoy2-starts"
: > "$decoy2_starts"
TOGGLE_STARTS="$decoy2_starts" python3 -c 'import time; time.sleep(120)' "$prefix/current/bin/omarchy-menu-rust" --no-exec &
decoy2=$!
sleep 0.1
MOCK_LAYERS="$mapped" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.2
alive "$decoy2" || fail "stale pidfile signaled a decoy"
[[ ! -s "$killed" || "$(tail -n 1 "$killed")" != "$decoy2" ]] || fail "decoy pid was recorded as signaled"
kill -TERM "$decoy2" 2>/dev/null || true

# Wrong argv on the real inode is not our child.
wrong_starts="$tmp/wrong-starts"
: > "$wrong_starts"
TOGGLE_STARTS="$wrong_starts" "$prefix/current/bin/omarchy-menu-rust" --live &
wrong=$!
sleep 0.1
before=$(wc -l < "$starts")
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix"
sleep 0.2
alive "$wrong" || fail "--live argv on the real binary was signaled"
[[ "$(wc -l < "$starts")" -eq $((before + 1)) ]] || fail "wrong argv blocked a real --no-exec start"
kill -TERM "$wrong" "$(tail -n 1 "$starts")" 2>/dev/null || true
sleep 0.1

# Concurrent callers serialize to one process, not two and not zero.
: > "$starts"
: > "$killed"
uid_runtime="$XDG_RUNTIME_DIR/omarchy-menu-rust-$(id -u)"
mkdir -p -- "$uid_runtime"
exec 8>"$uid_runtime/toggle.lock"
flock 8
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix" &
first=$!
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix" &
second=$!
sleep 0.3
flock -u 8
wait "$first" "$second"
exec 8>&-
sleep 0.2
[[ "$(wc -l < "$starts")" -eq 1 ]] || fail "concurrent toggles started $(wc -l < "$starts") processes"
live_n=0
while read -r one; do
  alive "$one" && live_n=$((live_n + 1))
done < "$starts"
[[ "$live_n" -eq 1 ]] || fail "concurrent toggles left $live_n processes"
kill -TERM "$(tail -n 1 "$starts")" 2>/dev/null || true

# Checksum mismatch refuses to start.
mkdir -p "$prefix/current/share"
printf '%s\n' "0000000000000000000000000000000000000000000000000000000000000000  bin/omarchy-menu-rust" > "$prefix/current/share/SHA256SUMS"
rc=0
MOCK_LAYERS="$empty" MOCK_CLIENTS="$clients_empty" bash "$TOGGLE" --prefix="$prefix" || rc=$?
[[ "$rc" -eq 1 ]] || fail "bad SHA256SUMS exited $rc"
rm -f -- "$prefix/current/share/SHA256SUMS"

rc=0
bash "$TOGGLE" --prefix="$prefix" --live || rc=$?
[[ "$rc" -eq 2 ]] || fail "toggle accepted --live"
rc=0
bash "$TOGGLE" --prefix="$prefix" --exec || rc=$?
[[ "$rc" -eq 2 ]] || fail "toggle accepted --exec"
rc=0
bash "$TOGGLE" --prefix="$prefix" --run-guards || rc=$?
[[ "$rc" -eq 2 ]] || fail "toggle accepted --run-guards"
rc=0
bash "$TOGGLE" --prefix="$prefix" --safe-apps || rc=$?
[[ "$rc" -eq 2 ]] || fail "toggle accepted --safe-apps"

echo "ok toggle"
