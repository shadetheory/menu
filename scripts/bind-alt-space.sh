#!/usr/bin/env bash
# Refuses to bind keys. Stock Super+Space stays untouched.
# Does not edit Hyprland and does not touch ~/.config/hypr.
# An Alt+Space toggle may already exist outside this archive
# (flock + exact executable + current user + pidfd).
# The live script is not in this repo. The parent can supply it later.
# This file does not reconstruct that toggle.
set -euo pipefail

echo "omarchy-menu-rust: Alt+Space binding is withheld." >&2
echo "Stock Super+Space stays untouched." >&2
echo "This script does not call hyprctl." >&2
echo "scripts/alt-space-toggle.sh is the layer-aware singleton. This script does not install it onto a key." >&2
echo "A parent can point an existing Alt+Space binding at that toggle. This script does not." >&2
echo "Wayland layer namespace is omarchy-menu-rust." >&2
echo "A map check that only looks for an X11 window will miss that layer surface." >&2
echo "This script does not perform that check." >&2
exit 2
