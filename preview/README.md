# Visual mock (not the menu)

Static Halloween Hacker snapshot: background `#050805`, foreground `#6dff57`, accent `#ff6a00`. Soft text colors live in `config/palette.js` and `config/palette.json`. The page does not watch Ghostty or `shell.toml`.

Sample grocery, inbox, and bot threads are example copy. The page has no shell, no network fetch, and no account call.

```bash
preview/scripts/launch-preview.sh
```

That command uses Chromium `--app` and a private `--user-data-dir` under `$XDG_RUNTIME_DIR` or `/tmp`. It does not call `xdg-open` and it does not use the regular browser profile. It does not bind Alt+Space.

The Rust scaffold in the repository root is the menu program. This folder is only the earlier visual mock.
