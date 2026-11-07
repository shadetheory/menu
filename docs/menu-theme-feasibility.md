# Stock Omarchy menu theming

Recommendation: use the existing menu. It already inherits the Omarchy theme palette that drives the terminal. A personal override is one user file, `~/.config/omarchy/shell.toml`. Do not patch Quickshell, do not bind Alt+Space, and do not build a replacement menu for this goal.

Verified remotely against `omacom/omarchy` (GitHub redirects `basecamp/omarchy` here):

| Pin | Value |
|---|---|
| Commit | `14abc0762ce38547d1912061969df58962f6f10a` |
| Subject | Run the runtime smoke test alone, and keep the CI VM's kernel log (#17935) |
| Date | 2026-11-06 21:03:28 +0100 |
| `version` file | `4.0.0.alpha` |
| Older docs pointer | `fa955bfa9d2c94339f452e4c56cb5bbfc5e1718e` (2026-08-18, "Better without gaps"). Not the tree read below. |

No installed-host files were read for this note. A short config check is still needed before treating a live machine as matching this tree.

## What already happens

`colors.toml` is the palette. `omarchy-theme-set` renders `default/themed/*.tpl` into `~/.local/state/omarchy/current/theme/`, then pushes that palette into the running shell. Terminal files (`alacritty.toml.tpl`, foot, ghostty, kitty) and the menu both consume that palette. The menu does not read `~/.config/alacritty/alacritty.toml` (or foot/kitty/ghostty) at draw time.

Flow, from `docs/theming.md`, `bin/omarchy-theme-set`, and `shell/shell.qml` `applyTheme`:

1. Stage the theme, generate `colors.toml` from `alacritty.toml` only when the theme has no `colors.toml` (`bin/omarchy-theme-colors-from-alacritty`).
2. Render `default/themed/shell.toml.tpl` unless the theme already ships `shell.toml`. A theme may replace one section with `shell.<section>.toml` (example in-tree: `themes/tokyo-night/shell.lock.toml`).
3. Replace `~/.local/state/omarchy/current/theme` and write `theme.name`.
4. IPC `omarchy-shell shell applyTheme <colors-base64> <shell-base64>` calls `Commons.Color.loadColors` and `loadShell` immediately. Terminal retint runs beside that (`omarchy-theme-set-foot`, `omarchy-restart-terminal`, and the other `visible_theme_commands`).

`shell/plugins/menu/Menu.qml` does not hardcode a palette. It binds:

- `Commons.Color.menu.background`, `.text`, `.border`, `.scrim`, `.selectedBackground`, `.selectedText`, `.selectedBorder`
- `Border.surfaceSpec("menu", "border", …)` and `("menu", "selected-border", …)` for width and gradient
- `Style.font.menuFamily`, `Style.font.*` sizes, `Style.spacing.*`, `Style.cornerRadius` (Hyprland `decoration:rounding`), `Style.gapsOut` (half of Hyprland `general:gaps_out`)

`shell/Commons/Color.qml` fills `[menu]` from the active theme `shell.toml`, then overlays `~/.config/omarchy/shell.toml`. User keys win per key. That user file is watched (`watchChanges: true`), so an edit re-parses without a theme switch and without a shell restart. `docs/omarchy-shell.md` states the same rule. `omarchy display text size` writes only `[font] base-size` in that file and leaves other keys in place (`bin/omarchy-display-text-size`).

Generated `[menu]` defaults (`default/themed/shell.toml.tpl`):

```toml
[menu]
background                = "{{ background }}"
background-alpha          = 1.0
text                      = "{{ foreground }}"
border                    = "hyprland.active-border-foreground"
border-alpha              = 1.0
scrim                     = "{{ background }}"
scrim-alpha               = 0.5
selected-background       = "{{ foreground }}"
selected-background-alpha = 0.08
selected-text             = "{{ accent }}"
selected-border           = "hyprland.active-border-foreground"
selected-border-alpha     = 0.25
```

`background` / `foreground` / `accent` are the same semantic keys the terminal templates use. Tokyo Night's `themes/tokyo-night/colors.toml` sets `background = "#1a1b26"`, `foreground = "#a9b1d6"`, `accent = "#7aa2f7"`. The menu card, label, and selected label use those roles. The menu does not paint rows in ANSI green/yellow/red; `red` becomes `Color.urgent` for other surfaces, not menu row chrome.

Font family is the fontconfig alias `monospace`, which `omarchy font set` writes to `~/.config/fontconfig/fonts.conf` and to the terminal configs (`bin/omarchy-font-set`). The menu follows that alias (`Style.qml` `fc-match monospace`), unless `OMARCHY_MENU_FONT` is set. That env var is read once at shell start. Font size for the shell is `[font] base-size` (default 12). `omarchy display text size` moves shell px, GTK scale, and terminal pt together (12px shell ↔ 9pt terminal). They stay linked; they are not the same number.

Spacing and type scale are official `[spacing]` and `[font]` keys. `scale` and `base-size` resize the menu card, rows, and type. `cardWidth` is `Style.space(300)` (520 for font, kernel, and screenrecord menus), so width scales. Row structure does not change.

Corners follow Hyprland rounding, refreshed on `configreloaded`. A theme's `hyprland.lua` can change them. The terminal config cannot.

## Styling versus layout

| Concern | Official hook | Changes layout? |
|---|---|---|
| Card, text, scrim, selection colors, border color, alphas | `[menu]` in theme `shell.toml` or user `~/.config/omarchy/shell.toml` | No |
| Border width, per-side width, gradient border | `[menu] border-width`, `border-width-left`, `selected-border`, `selected-border-width` via `Border.surfaceSpec` | No |
| Type size and per-token sizes | `[font] base-size` and tokens (`heading`, `body`, `icon-large`, …) | No |
| Padding, row height, card width scale | `[spacing] scale`, `scale-with-font`, and tokens (`panel-padding`, `md`, `xs`, `row-padding-x`, …) | No. Proportions stay. The single column, header, icon, label, and chevron stay. |
| Font family | `omarchy font set` (shared with terminals) or startup-only `OMARCHY_MENU_FONT` | No |
| Corner radius, outer gap | Hyprland `decoration:rounding`, `general:gaps_out` | No |
| Which rows exist, search ranking, providers, actions | `omarchy-menu.jsonc` and `~/.config/omarchy/extensions/omarchy-menu.jsonc` | Content, not chrome |
| Spotlight/Raycast layout, a second card, a different row widget | None. Hardcoded in `Menu.qml` | Yes. Requires a QML patch or another UI. |

Clipboard, emoji picker, and the app-grid uninstall confirm share `Commons.Color.menu`. A `[menu]` override restyles those surfaces too. `[launcher]` is a separate six-token block and does not follow `[menu]` unless set to the same values.

Glass terminal opacity (`omarchy-theme-set-glass`, alpha about 0.65) is written into the terminal theme files only. Menu `background-alpha` stays 1.0 unless `[menu]` sets it. The menu will not automatically go glassy with the terminal.

## Official hook versus patch

Official, reversible, no upstream edit:

```toml
# ~/.config/omarchy/shell.toml
# Only keys listed here override the active theme. Delete this file to revert.

[menu]
background                = "#1a1b26"
background-alpha          = 0.96
text                      = "#a9b1d6"
selected-text             = "#7aa2f7"
selected-background       = "#a9b1d6"
selected-background-alpha = 0.12
border                    = "foreground"
border-alpha              = 0.35

[font]
base-size = 14

[spacing]
scale = 1.05
```

Role names (`foreground`, `background`, `accent`, `urgent`, `muted`) and `#rrggbb` both parse (`Color.qml` `parseShell` / `flatColor`). Hex in the user file is fixed across theme switches, which is what you want only when the menu must not follow the next theme. To keep following every theme and only change density or selection strength, set alphas and `[font]` / `[spacing]`, and leave color keys as roles or omit them.

Do not edit `~/.local/state/omarchy/current/theme/shell.toml` or `colors.toml`. `omarchy-theme-set` deletes that directory and writes a new one. Edits there vanish on the next theme switch.

A theme the user authors under `~/.config/omarchy/themes/<name>/` may ship `shell.toml` or `shell.menu.toml`. That is baked at theme-set for that theme only. A later `omarchy theme update` can replace a cloned theme's copy. The machine file `~/.config/omarchy/shell.toml` is the layer that survives switches and updates.

`omarchy update` can change `shell.toml.tpl` or `Menu.qml`. The user file still overlays by key name. A renamed key would stop matching and fall back to the new template. That is the update risk: key names, not a merge conflict in upstream files, because upstream files are untouched.

There is no hook that polls a hand-edited terminal config and repaints the menu. Direction is `colors.toml` → generated terminal config and `shell.toml`. The reverse exists only for legacy themes that have `alacritty.toml` and no `colors.toml`, and only at theme-set time.

## Smallest proposal

1. Leave Super+Space, startup, and PATH alone. The themed surface is the stock menu.
2. On a machine that already has Omarchy, run the read-only check below. If the terminal config is the generated theme file and `colors.toml` matches what the terminal shows, stop. The menu already uses that palette.
3. If the menu should differ in density, selection, or alpha only, add those keys to `~/.config/omarchy/shell.toml`. Revert by removing the keys or the file. No shell restart for that file. `OMARCHY_MENU_FONT` still needs a shell restart.
4. If the terminal was hand-edited away from `colors.toml`, put the desired colors back into a user theme's `colors.toml` and run `omarchy theme set <name>` so terminals and the menu share one palette again. Do not teach the menu to parse Alacritty.
5. Revisit a custom UI only for a layout `shell.toml` cannot express. That work is not started; see [rust-menu-fallback-status.md](rust-menu-fallback-status.md).

## Gaps

- Menu chrome uses semantic roles, not the 16 ANSI colors and not terminal cursor/search/footer colors.
- Terminal glass opacity does not drive menu alpha.
- `OMARCHY_MENU_FONT` is not live.
- Nerd glyphs and the private `omarchy` icon font are per-entry `iconFont`, not the terminal family.
- Card structure, search field, and submenu behavior are QML. Spacing only scales them.
- This session did not open the menu on a display and did not read an installed host. "Already themed" is from the source at the commit above, not from a screenshot on that host.

## Read-only facts still needed from an installed host

These decide whether step 2 is enough. They do not require a binding change.

- `cat ~/.local/state/omarchy/current/theme.name` and the installed Omarchy version (package or `~/.local/share/omarchy/version` if present).
- `readlink -f` of the active terminal config (`~/.config/alacritty/alacritty.toml`, `~/.config/foot/foot.ini`, `~/.config/kitty/kitty.conf`, or `~/.config/ghostty/config`). Note whether it points at `~/.local/state/omarchy/current/theme/`.
- `[menu]`, `[font]`, and `[spacing]` from both `~/.local/state/omarchy/current/theme/shell.toml` and `~/.config/omarchy/shell.toml` if that file exists.
- `printf '%s\n' "$OMARCHY_MENU_FONT"` and `fc-match -f '%{family}\n' monospace`.
- One foreground/background/accent line from `~/.local/state/omarchy/current/theme/colors.toml`.

If the terminal config is not the generated theme file, or those three color keys disagree with what the terminal shows, say so before writing `shell.toml`. The override is still safe; it would just be pinning a second palette.
