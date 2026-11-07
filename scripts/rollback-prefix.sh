#!/usr/bin/env bash
# Remove only the current owned version under PREFIX and repoint current.
# Does not delete $HOME, edit Hyprland, unbind Super+Space, or touch ~/.config/hypr.
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
      echo "Usage: scripts/rollback-prefix.sh [--prefix=DIR]"
      echo "Removes the current version only when share/OWNED_BY contains omarchy-menu-rust."
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${HOME:-}" ]]; then
  echo "refusing to run without HOME set" >&2
  exit 2
fi
if [[ -z "$PREFIX" || "$PREFIX" == "/" || "$PREFIX" == "$HOME" || "$PREFIX" == "$HOME/" ]]; then
  echo "refusing to modify ${PREFIX:-<empty>}" >&2
  exit 2
fi

prefix_canon=$(realpath -m -- "$PREFIX")
home_canon=$(realpath -m -- "$HOME")
if [[ -z "$prefix_canon" || "$prefix_canon" == "/" || "$prefix_canon" == "$home_canon" ]]; then
  echo "refusing to modify $PREFIX" >&2
  exit 2
fi
PREFIX="$prefix_canon"

if [[ ! -d "$PREFIX" ]]; then
  echo "no prefix at $PREFIX" >&2
  exit 1
fi
if [[ ! -L "$PREFIX/current" ]]; then
  echo "no current symlink at $PREFIX/current" >&2
  exit 1
fi

link=$(readlink -- "$PREFIX/current")
if [[ "$link" != /* ]]; then
  link="$PREFIX/$link"
fi
target=$(realpath -m -- "$link")
versions_canon=$(realpath -m -- "$PREFIX/versions")
if [[ "$versions_canon" != "$PREFIX/versions" ]]; then
  echo "refusing: versions directory escapes the prefix" >&2
  exit 2
fi
case "$target" in
  "$PREFIX/versions"/*) ;;
  *)
    echo "refusing to remove $target" >&2
    exit 2
    ;;
esac
rest="${target#"$PREFIX/versions"/}"
if [[ -z "$rest" || "$rest" == *"/"* || "$rest" == "." || "$rest" == ".." ]]; then
  echo "refusing to remove $target" >&2
  exit 2
fi
if [[ "$target" == "$PREFIX" || "$target" == "$home_canon" || "$target" == "/" ]]; then
  echo "refusing to remove $target" >&2
  exit 2
fi
if [[ -L "$PREFIX/versions/$rest" ]]; then
  echo "refusing to remove symlinked version $target" >&2
  exit 2
fi
if [[ ! -d "$target" ]]; then
  echo "current target is not a directory: $target" >&2
  exit 2
fi

marker="$target/share/OWNED_BY"
if [[ ! -f "$marker" ]] || ! grep -q -F "omarchy-menu-rust" "$marker"; then
  echo "refusing to remove $target (share/OWNED_BY is not omarchy-menu-rust)" >&2
  exit 2
fi

prev=""
prev_mtime=-1
shopt -s nullglob
for dir in "$PREFIX/versions"/*; do
  [[ -d "$dir" && ! -L "$dir" ]] || continue
  dir_canon=$(realpath -m -- "$dir")
  [[ "$dir_canon" == "$target" ]] && continue
  case "$dir_canon" in
    "$PREFIX/versions"/*) ;;
    *) continue ;;
  esac
  other_rest="${dir_canon#"$PREFIX/versions"/}"
  [[ -n "$other_rest" && "$other_rest" != *"/"* ]] || continue
  other_marker="$dir/share/OWNED_BY"
  [[ -f "$other_marker" ]] || continue
  grep -q -F "omarchy-menu-rust" "$other_marker" || continue
  mtime=$(stat -c %Y -- "$dir")
  if (( mtime > prev_mtime )); then
    prev="$dir_canon"
    prev_mtime=$mtime
  fi
done
shopt -u nullglob

repoint_toggle() {
  local dest="$PREFIX/bin/toggle-demo"
  if [[ -n "$1" && -x "$1/bin/toggle-demo" ]]; then
    mkdir -p -- "$PREFIX/bin"
    local toggle_tmp="$PREFIX/bin/.toggle-demo.$$"
    ln -sfn -- "$1/bin/toggle-demo" "$toggle_tmp"
    mv -Tf -- "$toggle_tmp" "$dest"
  elif [[ -L "$dest" || -e "$dest" ]]; then
    rm -f -- "$dest"
  fi
}

if [[ -n "$prev" ]]; then
  link_tmp="$PREFIX/.current.$$"
  ln -sfn -- "$prev" "$link_tmp"
  mv -Tf -- "$link_tmp" "$PREFIX/current"
  repoint_toggle "$prev"
else
  rm -f -- "$PREFIX/current"
  repoint_toggle ""
fi

rm -rf -- "$target"

if [[ -L "$PREFIX/current" ]]; then
  now=$(readlink -- "$PREFIX/current")
  if [[ "$now" != /* ]]; then
    now="$PREFIX/$now"
  fi
  now=$(realpath -m -- "$now")
  if [[ ! -d "$now" ]]; then
    rm -f -- "$PREFIX/current"
    echo "removed dangling current symlink" >&2
    exit 1
  fi
  echo "removed $target"
  echo "current -> $now"
else
  echo "removed $target"
  echo "no current version remains"
fi
