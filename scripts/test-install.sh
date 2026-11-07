#!/usr/bin/env bash
# Exercise install and rollback against a prefix under /tmp.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
INSTALL="$ROOT/scripts/install-prefix.sh"
ROLLBACK="$ROOT/scripts/rollback-prefix.sh"
BIND="$ROOT/scripts/bind-alt-space.sh"
TRY="$ROOT/scripts/try.sh"
LAUNCH="$ROOT/scripts/launch-menu.sh"
TOGGLE="$ROOT/scripts/alt-space-toggle.sh"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

tmp=$(mktemp -d /tmp/omarchy-menu-rust-install-test.XXXXXX)
trap 'rm -rf -- "$tmp"' EXIT

forbidden="$tmp/forbidden.log"
touch "$forbidden"
bindir="$tmp/bin"
mkdir -p "$bindir"
for cmd in apt apt-get hyprctl xdg-open; do
  cat > "$bindir/$cmd" <<EOF
#!/bin/sh
echo "\$0 \$*" >> "$forbidden"
exit 99
EOF
  chmod 755 "$bindir/$cmd"
done
export PATH="$bindir:$PATH"

bash -n "$INSTALL"
bash -n "$ROLLBACK"
bash -n "$BIND"
bash -n "$TRY"
bash -n "$LAUNCH"
bash -n "$TOGGLE"

if grep -nE '(^|[^[:alnum:]_./-])(apt-get|apt|hyprctl|xdg-open)([^[:alnum:]_]|$)' \
  "$INSTALL" "$ROLLBACK"
then
  fail "install or rollback names a forbidden command"
fi

fake_home="$tmp/home"
mkdir -p "$fake_home"
rc=0
HOME="$fake_home" PREFIX="$fake_home" bash "$INSTALL" || rc=$?
[[ "$rc" -eq 2 ]] || fail "expected exit 2 for PREFIX=\$HOME, got $rc"
[[ -z "$(ls -A "$fake_home")" ]] || fail "PREFIX=\$HOME wrote into HOME"

rc=0
HOME="$fake_home" PREFIX=/ bash "$INSTALL" || rc=$?
[[ "$rc" -eq 2 ]] || fail "expected exit 2 for PREFIX=/, got $rc"

rc=0
HOME="$fake_home" PREFIX= bash "$INSTALL" || rc=$?
[[ "$rc" -eq 2 ]] || fail "expected exit 2 for empty PREFIX, got $rc"
[[ -z "$(ls -A "$fake_home")" ]] || fail "empty PREFIX wrote into HOME"

empty_root="$tmp/empty-root"
mkdir -p "$empty_root/scripts" "$empty_root/share"
cp -- "$INSTALL" "$empty_root/scripts/install-prefix.sh"
cp -- "$ROOT/share/version" "$empty_root/share/version"
rc=0
absent_log="$tmp/absent.log"
HOME="$fake_home" bash "$empty_root/scripts/install-prefix.sh" --prefix="$tmp/should-not-exist" >"$absent_log" 2>&1 || rc=$?
[[ "$rc" -eq 1 ]] || fail "expected exit 1 when the binary is absent, got $rc"
[[ ! -e "$tmp/should-not-exist" ]] || fail "absent binary still created a prefix"
grep -q -F "cargo build --release" "$absent_log" || fail "absent binary did not tell the user to cargo build --release"
[[ -s "$forbidden" ]] && fail "forbidden command during refusal checks: $(cat "$forbidden")"

version=$(tr -d '[:space:]' < "$ROOT/share/version")
[[ "$version" =~ ^[0-9A-Za-z._+-]+$ ]] || fail "bad share/version"

prefix="$tmp/prefix"
bash "$INSTALL" --prefix="$prefix"

current="$prefix/current"
[[ -L "$current" ]] || fail "current is not a symlink"
target=$(realpath -- "$current")
verdir="$prefix/versions/$version"
[[ "$target" == "$(realpath -- "$verdir")" ]] || fail "current does not point at $verdir"
[[ -x "$verdir/bin/omarchy-menu-rust" ]] || fail "binary missing"
[[ -f "$verdir/share/SHA256SUMS" ]] || fail "SHA256SUMS missing"
[[ -f "$verdir/share/OWNED_BY" ]] || fail "OWNED_BY missing"
grep -q -F "omarchy-menu-rust" "$verdir/share/OWNED_BY" || fail "OWNED_BY marker mismatch"
( cd "$verdir" && sha256sum -c share/SHA256SUMS >/dev/null ) || fail "SHA256SUMS does not match the installed binary"
[[ -f "$verdir/share/omarchy-menu-entry.jsonc.sample" ]] || fail "sample jsonc was not installed"
grep -q -F "PREFIX/current/bin/omarchy-menu-rust" "$verdir/share/omarchy-menu-entry.jsonc.sample" || fail "sample action placeholder missing"
grep -q -F "omarchy-menu-rust" "$verdir/share/layer-namespace.txt" || fail "layer namespace file missing"
[[ -x "$verdir/bin/alt-space-toggle" ]] || fail "layer toggle was not installed"
[[ -x "$verdir/bin/toggle-demo" ]] || fail "toggle-demo was not installed with the binary"
[[ -L "$prefix/bin/toggle-demo" ]] || fail "stable toggle-demo symlink missing"
[[ "$(readlink -f -- "$prefix/bin/toggle-demo")" == "$(readlink -f -- "$verdir/bin/toggle-demo")" ]] || fail "toggle-demo does not point at this version"
grep -q "Super+Space is not changed" "$verdir/bin/alt-space-toggle" || fail "installed toggle lost the Super+Space note"
help_out=$(PREFIX="$prefix" bash "$LAUNCH" --help)
grep -q -F "omarchy-menu-rust" <<<"$help_out" || fail "launcher help omitted the namespace"
rc=0
PREFIX="$prefix" bash "$LAUNCH" --live || rc=$?
[[ "$rc" -eq 2 ]] || fail "launcher accepted --live, exit $rc"

count_versions() {
  find "$prefix/versions" -mindepth 1 -maxdepth 1 -type d | wc -l
}
count_current() {
  find "$prefix" -name current -type l | wc -l
}
[[ "$(count_versions)" -eq 1 ]] || fail "expected one version directory"
[[ "$(count_current)" -eq 1 ]] || fail "expected one current symlink"

echo planted > "$verdir/share/PLANTED"
bash "$INSTALL" --prefix="$prefix"
[[ ! -e "$verdir/share/PLANTED" ]] || fail "second install did not replace the version directory"
[[ -L "$current" ]] || fail "current missing after second install"
[[ "$(realpath -- "$current")" == "$(realpath -- "$verdir")" ]] || fail "current moved after second install"
[[ "$(count_versions)" -eq 1 ]] || fail "second install left more than one version directory"
[[ "$(count_current)" -eq 1 ]] || fail "second install left more than one current symlink"
[[ -d "$(realpath -- "$current")" ]] || fail "current points at a missing directory after second install"
( cd "$verdir" && sha256sum -c share/SHA256SUMS >/dev/null ) || fail "checksum failed after second install"

cp -a -- "$verdir" "$prefix/versions/0.0.9"
printf '%s\n' "omarchy-menu-rust" > "$prefix/versions/0.0.9/share/OWNED_BY"
touch -d '2020-01-01 00:00:00' "$prefix/versions/0.0.9"
bash "$ROLLBACK" --prefix="$prefix"
[[ ! -d "$verdir" ]] || fail "rollback left the current version directory"
[[ -d "$prefix/versions/0.0.9" ]] || fail "rollback removed the previous version"
[[ -L "$current" ]] || fail "rollback dropped current while a previous version remains"
[[ "$(realpath -- "$current")" == "$(realpath -- "$prefix/versions/0.0.9")" ]] || fail "current does not point at the previous version"
[[ "$(readlink -f -- "$prefix/bin/toggle-demo")" == "$(readlink -f -- "$prefix/versions/0.0.9/bin/toggle-demo")" ]] || fail "toggle-demo did not roll back with the binary"
[[ -d "$(realpath -- "$current")" ]] || fail "current points at a removed directory"

bash "$ROLLBACK" --prefix="$prefix"
[[ ! -d "$prefix/versions/0.0.9" ]] || fail "second rollback left the previous version"
if [[ -L "$current" || -e "$current" ]]; then
  fail "current still exists after the last owned version was removed"
fi
if [[ -L "$prefix/bin/toggle-demo" || -e "$prefix/bin/toggle-demo" ]]; then
  fail "toggle-demo still exists after the last owned version was removed"
fi
[[ -d "$prefix" ]] || fail "rollback removed the prefix itself"

try_out=$(PREFIX="$prefix" bash "$TRY")
[[ "$try_out" == "$prefix/current/bin/omarchy-menu-rust --no-exec" ]] || fail "try.sh printed: $try_out"

rc=0
bash "$BIND" --apply || rc=$?
[[ "$rc" -ne 0 ]] || fail "bind-alt-space.sh exited 0"
rc=0
OMARCHY_MENU_RUST_BIND_AUTHORIZED=yes bash "$BIND" --apply || rc=$?
[[ "$rc" -ne 0 ]] || fail "bind-alt-space.sh exited 0 with authorization env"

bash "$ROOT/scripts/test-toggle.sh"

if [[ -s "$forbidden" ]]; then
  fail "script called a forbidden command: $(cat "$forbidden")"
fi

echo "ok prefix=$prefix version=$version"
