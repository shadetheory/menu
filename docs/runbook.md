# Runbook

This binary stays beside the stock Omarchy menu. It does not bind Super+Space and it does not call `hyprctl`. `scripts/alt-space-toggle.sh` is the layer-aware singleton an existing Alt+Space binding can call. Install and rollback still write only under the owned prefix. See [INSTALL.md](../INSTALL.md).

## Flags

`--no-exec` refuses every launch, in any order, including when `--safe-apps`, `--live`, or `--exec` is also present. Those three are valid only alone. Passing more than one of them is an error.

| Flag, alone | What a confirmed Enter may start |
|---|---|
| default, or any command that includes `--no-exec` | Nothing |
| `--safe-apps` | A desktop file whose canonical path stays in `/usr/share/applications` or `/usr/local/share/applications`, and whose executable canonicalizes to an absolute path in `src/safe_apps.rs`. `PATH` is not searched. |
| `--live` | A desktop Exec argv, or one executable `omarchy-*` file, when it is not marked dangerous |
| `--exec` | A trusted menu action (`bash -c` of that action only), a stock-menu handoff, and a dangerous command |

An Exec string that matches a name on the list is not a safe launch. A copy in `~/.local/share/applications`, a symlink that resolves outside the system applications directory, or a same-named binary earlier on `PATH` does not start. Ghostty, Grok Bot, and Mattermost are not launchable here: the 2026-11-06 notes recorded only a basename. This check does not hash the binary. `--live` does not enable shell actions or guards. `--exec` is the switch for shell actions. It does not run guards. `--run-guards` is a separate opt-in. Even then, an argv guard runs only from an exact JSON string array, and a shell guard only from an exact `shell` line. `true` and `false` are decided in process. `--no-exec` and `--safe-apps` start no clipboard tool. Search text and the filter input are never inserted into a command. The Alt+Space toggle still starts only `--no-exec` and still refuses `--safe-apps`.

### Safe-app provenance evidence and limits

For an ordinary safe-app row, Enter repeats admission immediately before dispatch; it does not trust the earlier display-time admission. It canonicalizes the desktop path under a trusted root, matches its filename and allowlisted id, resolves the scanned Exec program against the current canonical allowlisted executable, and compares arguments exactly. Dispatch uses that canonical program and the allowlisted arguments, not a PATH lookup. A symlink alias to that executable is accepted; an alias retargeted to a different executable before Enter is refused.

These are path checks, not an integrity or sandbox guarantee:

- Ownership, permissions, and mutability of desktop files, executables, and parent directories are **not checked**. No hash or binary identity is pinned. Replacing the allowlisted executable itself, including changing its symlink target, is outside this check: both sides resolve against its current target.
- The desktop file's contents are **not reread** at launch; its path is rechecked against the row's already-scanned id and argv. Replacement with another file at the same trusted path is not detected as an integrity change.
- Admission and spawn are separate filesystem operations. Replacement between them is **not prevented**; no file descriptor pins the executable through exec.
- Working directory is **not checked or isolated**; the child inherits the menu process's cwd. Environment is **not allowlisted or cleared**; it is inherited except that dispatch removes `OMARCHY_MENU_QUERY`. Inherited environment can still affect the launched program or its interpreter.

Repeatable orb checks for issue #2:

```bash
cargo test --locked gui::tests::safe_apps_starts_only_the_documented_desktop_exec -- --exact
cargo test --locked safe_apps::tests::shadow_desktop_and_path_do_not_replace_the_documented_file -- --exact
cargo test --locked tests::no_exec_refuses_every_order_and_mixed_modes_are_rejected -- --exact
```

The first test drives the menu model's Enter path through the real dispatcher and waits for a temporary fixture script to exit. It records the canonical executable, exact `--gui` argument, inherited cwd and HOME, and absence of `OMARCHY_MENU_QUERY`. It then changes an executable symlink and a desktop symlink after the row was displayed, and checks that neither the admitted nor substitute script runs; a direct substitute executable is also refused. The second test covers a shadow desktop file and a same-named PATH substitute. All app executables in these fixtures live under temporary directories; no installed Flea or Cursor is started. The launch-mode test exhausts all flag sequences of length one through four containing `--no-exec`, including all permutations of the four mode flags and repeated flags. The menu test also checks that default no-exec does not launch the allowlisted fixture.

This is a real fixture-process launch in an orb, **not native installed-app provenance proof**, a compositor interaction test, or proof that the unchecked properties above are safe. Production allowlist and launch behavior are unchanged.

The window is one 680px translucent shell. Idle search is 54px of that shell, not a second rounded slab. It grows down to 440px. Focus is a one-pixel border. Wayland uses layer-shell namespace `omarchy-menu-rust` and a premultiplied ARGB buffer. X11 is an opaque fallback and prints that. A press on the visible dim layer dismisses the menu. That layer is `omarchy-menu-rust-dim`, on the focused output only. See [outside-click.md](outside-click.md). `scripts/launch-menu.sh` starts only `--no-exec`. `scripts/alt-space-toggle.sh` reads layer namespace and client map state, dismisses a mapped menu, or starts one `--no-exec`. It does not bind a key. `scripts/bind-alt-space.sh` still refuses and does not call hyprctl. Stock Super+Space stays untouched. The menu keeps the configured Omarchy font, including a monospace face. It does not swap in a proportional face.

## One step at a time

Enter and a single click do one step. Arrows and hover only move the highlight. A repeated Enter does not activate.

| Row verb | One activation does |
|---|---|
| Browse | Opens the next menu |
| Open | Starts an ordinary app |
| Run… | Opens an input. Empty text cannot run |
| Confirm… | Opens confirmation. Cancel is selected. An unknown shell command says its effects have not been verified, then shows the raw command |
| Answer | Expands the text. Copy is a separate control |
| Unavailable | Shows why. Nothing starts |

Confirm runs only after the highlight moves to Confirm and that control is activated. `--no-exec` shows `Dry run: would start ...` and does not say the command succeeded. Esc goes back one level and keeps the query and highlight. From the root list, Esc closes the window. This program does not call `hyprctl`, so the compositor restores the previous focus.

`--live` is what starts an ordinary app. `--exec` is what starts a stock shell action or a dangerous command. The spec's launch note says `--exec`; this tree keeps those two switches apart so opening an app does not enable shell actions.

## State trace

`--trace-state` prints one stderr line after the first frame and after each key or click:

```text
omarchy-menu-rust: state query="About" selected="About" verb="Confirm…" confirm=open focus=cancel close=0 refusal=none
```

`focus` is `cancel` or `confirm` while confirmation is open. `refusal` is `dry-run`, `cancelled`, `stale`, `empty`, `refused`, or `none`. The flag does not start a program, open a socket, or draw a panel. A launch without the flag does not print these lines. Alt+Space still starts only `--no-exec`, so a traced run is a separate command:

```bash
omarchy-menu-rust --no-exec --trace-state
```

`scripts/test-state-trace.sh` drives that command under headless Sway with `wtype` and checks the lines. It does not use a screenshot.

## Read-only inspection

```bash
omarchy-menu-rust --list --apps-dir /usr/share/applications
omarchy-menu-rust --dry-run "Harmless" --apps-dir /usr/share/applications --terminal-argv "ghostty -e"
```

Both print `would_execute false`. `--live` on the same command line does not start the app. `--terminal-argv` is whitespace-separated. `bash -c` is refused.

Guards do not run at startup, during search, or during a theme reload. `--no-exec`, `--live`, and `--exec` do not change that. An allow line is either `argv` plus a JSON string array, or `shell` plus the exact expression. `true` and `false` alone are decided without a process. The default file is absent, so no guard program starts.

## Daily launch

```bash
omarchy-menu-rust --live
```

1. Type to filter the stock tree, or open Apps / Commands.
2. Enter on Apps reads `.desktop` files from `XDG_DATA_HOME` and `XDG_DATA_DIRS`.
3. Enter on a row opens confirm. Esc cancels and starts nothing.
4. Enter again starts a normal app as argv. A shell-shaped Exec line is refused.
5. Fonts, power profiles, kernels, and languages show an unavailable row. They are not changed.
6. Grocery, inbox, and bot rows say they are not connected.

Use `--exec` only when a confirmed stock action should run. Dangerous names still show a confirm first.

## Continuous integration

`.github/workflows/ci.yml` runs on pushes to `master`, pull requests, and manual
dispatch. The `Rust quality` job uses a GitHub-hosted Ubuntu 24.04 runner with
read-only repository permissions. It installs stable Rust, caches Cargo
dependencies and build outputs by compiler and lockfile, and runs `.agents/setup`
to share the orb's dependency and test/release build configuration. Shell syntax,
locked Rust/CLI tests, and warning-free Clippy must pass.

This is not an Omarchy desktop acceptance check. The headless seat/state-trace
and installer/toggle scripts are not CI gates yet: the state-trace check failed
and the toggle check was intermittent during orb validation. Native acceptance
still needs those checks stabilized and a real Omarchy environment. No private
or self-hosted runner is configured by this workflow.

## Acceptance checklist

- `cargo test` covers desktop parsing, malformed and unreadable entries, `true` / `false` guards, a shell guard that is not executed, a non-executable guard that does not hide its row, select / input / cancel, quoted Exec arguments, `--no-exec` refusal, and `--live` launch of a local test program.
- `cargo clippy --all-targets -- -D warnings` is clean.
- `bash scripts/test-install.sh` still refuses `/`, an empty prefix, and `$HOME`, and still rolls back an owned version.
- `omarchy-menu-rust --no-exec --screenshot out.png --exit-after-first-frame` paints one frame and does not launch an app.
- `scripts/bind-alt-space.sh` still exits without binding a key.

## Not in this build

Guard expressions with shell syntax stay unevaluated, so those rows stay visible. Terminal apps are listed and not launched. Font, kernel, language, and power-profile changes are not applied. No inbox, grocery, or contact account is connected. The window does not log keystrokes to disk.
