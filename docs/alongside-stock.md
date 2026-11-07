# Alongside stock Omarchy (local draft 8)

`omarchy-menu-rust` is a standalone Rust window that sits next to the stock menu. It is not an official shell plugin, and it does not replace `omarchy.menu`.

Official plugins are `manifest.json` plus QML, installed with `omarchy plugin add`. See the [shell plugins manual](https://omarchy.org/manual/shell-plugins/) and [shell/plugins/README.md](https://github.com/basecamp/omarchy/blob/quattro/shell/plugins/README.md). Menu entries and this binary are a different path. Menu contract: [docs/menu.md](https://github.com/basecamp/omarchy/blob/fa955bfa/docs/menu.md). Upstream pin: `14abc0762ce38547d1912061969df58962f6f10a`.

Mouse and keyboard both navigate the scaffold. Search in the open window is the scaffold filter only. Session-persistent search adapters are not live.

The HTML tree under [preview/](../preview/README.md) is a nonfunctional mock. It is not the product.

## Three layers

Keep the layers separate. Mixing them is how Super+Space gets clobbered.

1. **Stock Super+Space** stays the QML `omarchy.menu`. Do not bind Super+Space to this binary. Do not replace that plugin.
2. **Optional menu extension.** A later install may add one Shade-owned entry in `~/.config/omarchy/extensions/omarchy-menu.jsonc`, then refresh. The entry must be complete: id, label, and action together. A partial overlay clears `action`. Upstream `MenuModel` fills omitted fields with empty strings, and the merge copies those empty strings. `docs/menu.md` reads as a sparser overlay. This tree follows the script. Ship every field you intend to keep, including `action`.
3. **Personal Alt+Space** stays pointed at `$PREFIX/bin/toggle-demo`. Install replaces that symlink atomically with the helper that belongs to the same version as the binary. Rollback moves the helper and the binary together. The helper dismisses only a current-user process whose `/proc/PID/exe` inode and checksum are the installed binary and whose argv is exactly that binary plus `--no-exec`. It does not bind Super+Space. `scripts/bind-alt-space.sh` still exits without writing a key binding. This installer does not edit an existing binding.

## Complete jsonc sample

Shade-owned example only. Put it in `~/.config/omarchy/extensions/omarchy-menu.jsonc` when an install is actually requested. Then run `omarchy menu refresh`.

`action` is the binary path. It does not include search text. Do not concatenate the question, the query, or a typed phrase into that string.

```jsonc
{
  // Shade-owned. id + label + action together. Partial objects clear action.
  "shade.menu": {
    "label": "Shade menu",
    "title": "Shade menu",
    "description": "Shade-owned standalone Rust menu. Not omarchy.menu.",
    "icon": "",
    "iconFont": "",
    "parent": "root",
    "target": "",
    "action": "PREFIX/current/bin/omarchy-menu-rust",
    "provider": "",
    "aliases": [],
    "searchTerms": [],
    "when": "",
    "checked": "",
    "disabled": ""
  }
}
```

If the prefix differs, replace that path with the installed binary and leave the rest of the object filled in. A fixed `--no-exec` argument is still a constant flag, not a place to insert search text. The sample above is the path alone.

This file does not write that jsonc onto a machine.

## Tested on

| Report | Value |
| --- | --- |
| Reported package | omarchy `4.0.6-1` |
| Version file | `4.0.0.alpha` |
| Shell | Quickshell `0.3.2.r7` |
| Menu contract pin | `14abc0762ce38547d1912061969df58962f6f10a` |

Those numbers are a tested-on note for one reported install. They are not universal support.

## Fixtures, not live services

Example grocery, inbox, bots, and jobs rows are fixtures. They do not call a store, an inbox, or an agent.

The five verbs (`message`, `call`, `lookup`, `run-output`, `run-monitor`) and the persistent jobs card are design-only. They are not implemented in this tree. A later card can show running work and an explicit nothing-needed state. Unavailable is not zero. No secret sync. No balances are loaded.

## Non-goals

- Not a QML plugin, and not installed with `omarchy plugin add`.
- Not a replacement for `omarchy.menu`.
- Not a published plugin.
- Not a Super+Space binding.
- Not a reconstructed Alt+Space toggle from another host.
- Not a live theme sync, a live jobs card, or a live five-verb API.
- Not a separate theme package. None is imported here.

Further official install-convention detail may still be pending. Follow that research when it lands. Do not invent a Rust plugin API to fill the gap.
