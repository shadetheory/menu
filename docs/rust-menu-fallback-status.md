# Rust menu notes

The scaffold now lives at the repository root (`omarchy-menu-rust`). The notes below are the contract read that preceded that code. They are kept. "Not built" described the tree before the scaffold landed. See [INSTALL.md](../INSTALL.md) for what the binary does and which contract pieces are delegated.

Work stopped when the request pivoted to theming the stock Quickshell menu. That path is feasible for color, font, and spacing, and it is the one to use. Resume a Rust UI only if a later request needs layout or behavior `shell.toml` cannot express (a different card structure, a bot roster inside the menu, a new hotkey). None of that was implemented.

## What was verified before the stop

Upstream at `14abc0762ce38547d1912061969df58962f6f10a` (`version` `4.0.0.alpha`, 2026-11-06). The menu is the `omarchy.menu` Quickshell plugin, not the older Walker script.

| Piece | Path |
|---|---|
| Contract writeup | `docs/menu.md` |
| Entry data | `default/omarchy/omarchy-menu.jsonc` (about 467 lines; full stock tree, not a sample) |
| Pure logic | `shell/plugins/menu/MenuModel.js` |
| Rendering and providers | `shell/plugins/menu/Menu.qml` |
| CLI | `bin/omarchy-menu` → `omarchy-shell shell toggle\|summon\|hide\|call` |
| User entries | `~/.config/omarchy/extensions/omarchy-menu.jsonc` (shipped sample is comments only) |
| Node tests | `test/shell.d/menu-test.sh`, `menu-guards-test.sh` |

Preserved semantics, if implementation resumes: JSONC is whole-line `//` comments plus trailing-comma strip, not a full JSONC grammar; kind is inferred from `action` then `target`; dotted ids are the tree; user keys overlay per field and keep order; guards `when` / `checked` / `disabled` run as one bash batch; providers are `apps` (QML app library), `fonts`, `power-profiles`, `kernels`, `languages` (tab-separated `label\tvalue\tcurrent`, optional fourth search term); summon of a leaf alias runs the action; search text must never be concatenated into `sh -c`; dangerous actions stay the stock command strings so their own confirms still run; fallback to `omarchy-menu` must refuse to exec the replacement binary.

Hyprland on this tree binds keys from Lua (`default/hypr/bindings/*.lua`, `o.bind`), which `hyprctl -j binds` reports as dispatcher `__lua`. A reported binding read found Super+Space at modmask 64 (stock menu) and no bare Alt+Space (modmask 8). `default/hypr/bindings/dictation.lua` binds Alt+Space to dictation only when dictation is enabled. That binding was not changed. No runtime `hyprctl keyword bind` was issued from here.

## Explicitly not done

- No Cargo project, release binary, source tarball, or `/opt/cursor/artifacts` payload.
- No GUI, so no cloud screenshot and no installed-host screenshot.
- No `~/.local/opt/omarchy-menu-rust` tree and no Alt+Space helper script.
- No parity test run. The stock `menu-test.sh` was read, not executed.
- No claim of behavioral parity with `MenuModel.js`.
