# Omarchy menu Rust scaffold

`omarchy-menu-rust` is a standalone Rust app that runs alongside Omarchy. It is not an official QML shell plugin. Official plugins are `manifest.json` plus QML, installed with `omarchy plugin add` ([shell plugins manual](https://omarchy.org/manual/shell-plugins/), [shell/plugins/README.md](https://github.com/basecamp/omarchy/blob/quattro/shell/plugins/README.md)). This program does not replace the stock Quickshell `omarchy.menu` and it does not bind Super+Space.

```bash
cargo build --release
./target/release/omarchy-menu-rust
```

The release binary is `target/release/omarchy-menu-rust`. The default does not start commands, guards, or a clipboard tool. `--no-exec` means the same thing. `--live` is the opt-in for a confirmed desktop app or `omarchy-*` program, passed as argv. `--exec` is a separate opt-in for a trusted stock action. The last of those three flags wins for app launch only. Guards run only with `--run-guards` plus an exact grant. Search text is never placed in `bash -c`. Keyboard and a left click both navigate. The click is not saved across sessions. The launch flow is in [docs/runbook.md](docs/runbook.md).

On Wayland the window is a panel-sized layer surface named `omarchy-menu-rust`: 680 wide, a 54px search, growing down to 440. The panel background is translucent. Glyph interiors are opaque. X11 shows the same layout as an opaque window and prints that the desktop does not show through. A click outside the panel still reaches the window underneath. That is unsolved. This build is not a daily driver. An already installed copy stays the safe one until this binary is tested there.

Prefix install, dependency notes, and rollback live in [INSTALL.md](INSTALL.md). That page is the installer procedure. This file does not repeat it.

If `~/.local/state/omarchy/current/theme/colors.toml` exists, background, foreground, and accent are read from it. Theme `shell.toml` `[menu]`, `shell.menu.toml`, and `~/.config/omarchy/shell.toml` `[menu]` then set the inner surface, body ink, and selection, in that order. `[font] base-size` is used only when that key is present, and the user file wins over the theme file and over Ghostty `font-size`. Ghostty `font-family` is read from the theme `ghostty.conf` and then `~/.config/ghostty/config`. A matching local font file is used. Otherwise drawing falls back to the bundled JetBrains Mono. Missing theme files keep the Halloween Hacker snapshot (`#050805`, `#6dff57`, `#ff6a00`) and body size 15. Size 20 is not forced. An open window reloads these files after they change, including a theme symlink switch. A truncated color file keeps the last good colors. Wayland composites that theme through the panel alpha. X11 does not.

One reported install was omarchy `4.0.6-1`, version file `4.0.0.alpha`, Quickshell `0.3.2.r7`. That is a tested-on note, not universal support. Menu contract pin: `14abc0762ce38547d1912061969df58962f6f10a` ([docs/menu.md](https://github.com/basecamp/omarchy/blob/fa955bfa/docs/menu.md)).

Music, bots, groceries, inbox, and store rows are hidden. They are not shown as live panels. Real desktop apps and the enabled Omarchy actions stay. A future CLI or REST plugin contract is written in [docs/plugin-future.md](docs/plugin-future.md) and is not built. The pill's measured sizes are in [docs/visual-tokens.md](docs/visual-tokens.md). That note is not a separate theme package.

The HTML tree under [preview/](preview/README.md) is a nonfunctional mock, not this program.

- [docs/seams.md](docs/seams.md) — module boundaries
- [docs/alongside-stock.md](docs/alongside-stock.md) — stock Super+Space, optional jsonc entry, personal Alt+Space
- [docs/repo-import.md](docs/repo-import.md) — how this tree was imported

No package is published from this tree.

## License

GNU Affero General Public License v3.0 only (`AGPL-3.0-only`). See [LICENSE](LICENSE). The bundled JetBrains Mono files under `preview/assets/fonts/` remain under the SIL Open Font License, Version 1.1.
