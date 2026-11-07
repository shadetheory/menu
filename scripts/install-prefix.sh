#!/usr/bin/env bash
# Install the standalone omarchy-menu-rust binary under a versioned prefix.
# Writes only under PREFIX. Does not install packages, edit Hyprland,
# bind keys, write ~/.config/hypr, or write ~/.config/omarchy.
set -euo pipefail

if [[ -z "${PREFIX+x}" ]]; then
  PREFIX="${HOME}/.local/opt/omarchy-menu-rust"
fi

for arg in "$@"; do
  case "$arg" in
    --prefix=*) PREFIX="${arg#--prefix=}" ;;
    --prefix)
      echo "pass --prefix=DIR" >&2
      exit 2
      ;;
    -h|--help)
      echo "Usage: scripts/install-prefix.sh [--prefix=DIR]"
      echo "Default prefix: ~/.local/opt/omarchy-menu-rust"
      echo "Build first: cargo build --release"
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${HOME:-}" ]]; then
  echo "refusing to write without HOME set" >&2
  exit 2
fi
if [[ -z "$PREFIX" || "$PREFIX" == "/" || "$PREFIX" == "$HOME" || "$PREFIX" == "$HOME/" ]]; then
  echo "refusing to write to ${PREFIX:-<empty>}" >&2
  exit 2
fi

prefix_canon=$(realpath -m -- "$PREFIX")
home_canon=$(realpath -m -- "$HOME")
if [[ -z "$prefix_canon" || "$prefix_canon" == "/" || "$prefix_canon" == "$home_canon" ]]; then
  echo "refusing to write to $PREFIX" >&2
  exit 2
fi
PREFIX="$prefix_canon"

ROOT=$(cd "$(dirname "$0")/.." && pwd)
VERSION_FILE="$ROOT/share/version"
if [[ ! -f "$VERSION_FILE" ]]; then
  echo "missing $VERSION_FILE" >&2
  exit 2
fi
VERSION=$(tr -d '[:space:]' < "$VERSION_FILE")
if [[ ! "$VERSION" =~ ^[0-9A-Za-z._+-]+$ ]]; then
  echo "refusing version string from $VERSION_FILE" >&2
  exit 2
fi

lib_present() {
  local name="$1"
  local escaped dir
  escaped=${name//./\\.}
  if command -v ldconfig >/dev/null 2>&1; then
    if ldconfig -p 2>/dev/null | grep -E "(^|[[:space:]])${escaped}([[:space:]]|\\()" >/dev/null; then
      return 0
    fi
  fi
  for dir in \
    /lib /lib64 /usr/lib /usr/lib64 \
    /lib/x86_64-linux-gnu /usr/lib/x86_64-linux-gnu \
    /lib/aarch64-linux-gnu /usr/lib/aarch64-linux-gnu
  do
    if [[ -e "$dir/$name" ]]; then
      return 0
    fi
  done
  return 1
}

missing_libs=()
for lib_name in libc.so.6 libxkbcommon-x11.so.0 libxkbcommon.so.0 libwayland-client.so.0; do
  if ! lib_present "$lib_name"; then
    missing_libs+=("$lib_name")
  fi
done
if ((${#missing_libs[@]} > 0)); then
  echo "warning: missing libraries (names only; nothing will be installed):" >&2
  for lib_name in "${missing_libs[@]}"; do
    echo "  $lib_name" >&2
  done
fi

BIN="$ROOT/target/release/omarchy-menu-rust"
if [[ ! -f "$BIN" ]]; then
  echo "build first: cargo build --release" >&2
  echo "expected $BIN" >&2
  exit 1
fi

final="$PREFIX/versions/$VERSION"
final_canon=$(realpath -m -- "$final")
case "$final_canon" in
  "$PREFIX"/*) ;;
  *)
    echo "refusing version path outside prefix" >&2
    exit 2
    ;;
esac

mkdir -p -- "$PREFIX"
staging=$(mktemp -d "$PREFIX/.staging.XXXXXX")
backup=""
cleanup() {
  if [[ -n "${staging}" && -d "$staging" && "$staging" != "$final" ]]; then
    rm -rf -- "$staging"
  fi
  if [[ -n "${backup}" && -d "$backup" && "$backup" != "$final" ]]; then
    rm -rf -- "$backup"
  fi
}
trap cleanup EXIT

mkdir -p -- "$staging/bin" "$staging/actions" "$staging/config" "$staging/fixtures" "$staging/share"
cp -- "$BIN" "$staging/bin/omarchy-menu-rust"
chmod 755 "$staging/bin/omarchy-menu-rust"
if [[ -f "$ROOT/scripts/alt-space-toggle.sh" ]]; then
  cp -- "$ROOT/scripts/alt-space-toggle.sh" "$staging/bin/alt-space-toggle"
  cp -- "$ROOT/scripts/alt-space-toggle.sh" "$staging/bin/toggle-demo"
  chmod 755 "$staging/bin/alt-space-toggle" "$staging/bin/toggle-demo"
fi
if [[ -d "$ROOT/actions" ]]; then
  cp -a -- "$ROOT/actions/." "$staging/actions/"
fi
if [[ -d "$ROOT/config" ]]; then
  cp -a -- "$ROOT/config/." "$staging/config/"
fi
if [[ -d "$ROOT/fixtures" ]]; then
  cp -a -- "$ROOT/fixtures/." "$staging/fixtures/"
fi
if [[ -d "$ROOT/share" ]]; then
  cp -a -- "$ROOT/share/." "$staging/share/"
fi
printf '%s\n' "omarchy-menu-rust" > "$staging/share/OWNED_BY"

hash=$(sha256sum -- "$staging/bin/omarchy-menu-rust" | awk 'NR==1 { print $1 }')
printf '%s  %s\n' "$hash" "bin/omarchy-menu-rust" > "$staging/share/SHA256SUMS"
got=$(sha256sum -- "$staging/bin/omarchy-menu-rust" | awk 'NR==1 { print $1 }')
if [[ "$got" != "$hash" ]]; then
  echo "checksum mismatch for installed binary" >&2
  exit 1
fi

mkdir -p -- "$PREFIX/versions"
if [[ -e "$final" ]]; then
  backup="$PREFIX/.backup-${VERSION}-$$"
  if [[ -e "$backup" ]]; then
    echo "backup path already exists: $backup" >&2
    exit 1
  fi
  mv -- "$final" "$backup"
fi
if ! mv -- "$staging" "$final"; then
  staging=""
  if [[ -n "$backup" && -d "$backup" ]]; then
    mv -- "$backup" "$final"
    backup=""
  fi
  echo "failed to publish $final" >&2
  exit 1
fi
staging=""
if [[ -n "$backup" && -d "$backup" ]]; then
  rm -rf -- "$backup"
  backup=""
fi

link_tmp="$PREFIX/.current.$$"
ln -sfn -- "$final" "$link_tmp"
mv -Tf -- "$link_tmp" "$PREFIX/current"

if [[ -x "$final/bin/toggle-demo" ]]; then
  mkdir -p -- "$PREFIX/bin"
  toggle_tmp="$PREFIX/bin/.toggle-demo.$$"
  ln -sfn -- "$final/bin/toggle-demo" "$toggle_tmp"
  mv -Tf -- "$toggle_tmp" "$PREFIX/bin/toggle-demo"
fi

echo "installed $VERSION under $PREFIX"
echo "run: $PREFIX/current/bin/omarchy-menu-rust --no-exec"
echo "existing Alt+Space target stays: $PREFIX/bin/toggle-demo"
