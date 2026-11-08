# Architecture seams

The module split keeps rendering, retrieval, providers, and action dispatch separate. Current launch modes and their constraints are defined in [the runbook](runbook.md); recall remains read-only under every mode.

Guards require the separate opt-in and exact grants documented in the runbook. A missing `when` result does not hide a row. Only an explicit `false` hides it.

## Modules

| Module | Owner issue draft | Notes |
|---|---|---|
| `src/main.rs` | 1 | CLI. Declares the modules below. |
| `src/model.rs` | 1 | JSONC menu model, routes, and parsed guard fields. Re-exports ranking. |
| `src/ranking.rs` | 1 | `matches_query`, `search_score`, `fuzzy_hit`. |
| `src/dispatch.rs` | 1 | Trusted plans. Refuses query interpolation. Spawn stays behind confirm. |
| `src/theme.rs` | 2 | `ThemeTokens`, bundled snapshot, `load_theme`. |
| `src/desktop.rs` | 3 | Desktop entry parser and Exec argv splitting. |
| `src/guards.rs` | 3 | Argv guard checks. Shell expressions are skipped. |
| `src/providers.rs` | 3 | App and `omarchy-*` discovery. Other providers are an unavailable row. |
| `src/gui.rs` | 5 | Window, paint, select, input, confirm, and pointer. |
| `src/bots.rs` | 6 | Seed roster fixtures. |
| `src/recall.rs` | R02 | Explicit source records, change events, cache, lexical and optional loopback semantic retrieval. |
| `src/recall_ui.rs` | R02 | Background worker, query generations, result/preview state, fixed topic picks. |
| `src/present_wayland.rs` | R01/R02 | Layer-shell surface, dim layer, input, worker polling, presentation. |
| `src/place.rs` | R01 | Output-relative placement, center at 55% height, clamped bounds. |
| installer | 4 | Not under `src`. Scripts and install docs stay outside these modules. |

The original issue numbers above describe the module split history. Current work is ordered in [the roadmap](roadmap.md). Guard evaluation is already implemented; keep provider discovery separate from GUI rendering and ranking.

## Exec default

`--no-exec` is the default. `--live` may start a confirmed non-dangerous app argv. `--exec` may start a confirmed stock action. `confirm_run` asks `gate_spawn` before `spawn_report`. The question text is never placed in `bash -c`.

## Input

Keyboard Up, Down, Enter, Esc, and typing still navigate. A left click on a visible row selects that row and activates it the same way the first Enter does. One click opens a menu, an example panel, or the confirm step. A click does not skip confirm and does not spawn.

The selection lives in the process only. It is not written to disk and does not persist across OS sessions.

## Theme

The window calls `theme::load_theme` on the active theme `colors.toml`, that theme's `shell.toml` and `shell.menu.toml`, `~/.config/omarchy/shell.toml`, the theme `ghostty.conf`, and `~/.config/ghostty/config`. Missing files keep the bundled Halloween Hacker snapshot (`#050805` / `#6dff57` / `#ff6a00`) and body size 15. `[font] base-size` applies only when that key is present. Size 20 is not forced. `[menu] background`, `text`, and `selected-background` set the inner surface, body ink, and selection. `shell.menu.toml` overrides the theme `shell.toml` section, and the user `shell.toml` `[menu]` overrides both. Search, answer, and muted colors are derived from those tokens. An open window watches `~/.local/state/omarchy`, `~/.config/omarchy`, and `~/.config/ghostty` with `notify` and redraws after a 200 ms quiet period. A truncated color file keeps the last good colors. These are file reads, not shell probes. Wayland presents premultiplied alpha with translucent panel plates and opaque glyph interiors. The work view uses a more opaque plate for source readability. X11 is an opaque fallback. This does not claim compositor blur.

## Providers

`classify` maps `apps`, `commands`, `fonts`, `power-profiles`, `kernels`, and `languages`. Apps are desktop files. Commands are executable `omarchy-*` files in the directories the caller supplies. The other kinds produce one unavailable row and do not change the system.

## Example panels

Grocery, inbox, recent work, decisions, and bot rows open a not-connected panel. They are not live sessions, they are not read from an account, and they do not contact anyone.

## Recall boundary

Configured source files → reconcile records/cache → lexical candidates → optional semantic retrieval → generation-scoped UI results → in-process source preview. Disk and embedding work run on the recall worker, not the paint/input thread. Records never become `model::Item` actions. Source-change events are in process; an external listener or bus is future work. See [the recall contract](work-recall.md) and [decisions](decisions.md).
