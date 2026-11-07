# Install

Standalone Rust binary installer. This is not an Omarchy QML plugin and it is not `omarchy plugin add`.

Do not install this build until design review approves a shipping iteration.

Documented against Omarchy 4.0.6-1 / upstream menu commit 14abc0762ce38547d1912061969df58962f6f10a, not all versions.

Default prefix: `~/.local/opt/omarchy-menu-rust`. The installer writes only under `PREFIX`. It refuses `/`, an empty prefix, and `$HOME`.

## Build

```bash
cargo build --release
```

## Install

```bash
./scripts/install-prefix.sh
```

Version comes from `share/version`. The tree is `$PREFIX/versions/<version>/{bin,actions,config,fixtures,share}`, and `$PREFIX/current` is a symlink to that version directory. A second run stages a replacement directory, renames it over that version, and repoints `current`. The installed binary's SHA256 is written to `$PREFIX/versions/<version>/share/SHA256SUMS`. `share/OWNED_BY` contains `omarchy-menu-rust`.

If `target/release/omarchy-menu-rust` is absent, the installer exits 1 and tells you to `cargo build --release`. It does not run Cargo.

Another prefix: `./scripts/install-prefix.sh --prefix=DIR`.

## Libraries

The installer checks `libc.so.6`, `libxkbcommon-x11.so.0`, `libxkbcommon.so.0`, and `libwayland-client.so.0` with `ldconfig -p` or a file test. It prints each missing name as a warning. Missing libraries stay a warning. The installer does not install or upgrade distro packages.

## Run

```bash
"$HOME/.local/opt/omarchy-menu-rust/current/bin/omarchy-menu-rust" --no-exec
```

`scripts/try.sh` prints `$PREFIX/current/bin/omarchy-menu-rust --no-exec`. It execs that command only when `--launch` is passed.

## Rollback

```bash
./scripts/rollback-prefix.sh
```

Rollback removes only the current version directory, and only when `share/OWNED_BY` contains `omarchy-menu-rust`. If another owned version directory remains, `current` is pointed at that directory. Rollback does not delete `$HOME`, edit Hyprland, unbind Super+Space, or touch `~/.config/hypr`.

## Keys

Stock Super+Space stays untouched.

`scripts/bind-alt-space.sh` refuses and exits non-zero. It does not call hyprctl. An Alt+Space toggle may already exist outside this archive (flock + exact executable + current user + pidfd). The live script is not in this repo. This installer does not rewrite that toggle.

The Wayland surface namespace is `omarchy-menu-rust` (`share/layer-namespace.txt`). A toggle that only checks a mapped X11 window will miss the layer surface. This tree does not perform that check.

`scripts/launch-menu.sh` execs the installed binary with `--no-exec` and refuses `--live`, `--exec`, and `--run-guards`. The installer copies the helper to `$PREFIX/versions/<version>/bin/toggle-demo` and atomically points the stable path `$PREFIX/bin/toggle-demo` at that file. An existing Alt+Space binding that already runs `$PREFIX/bin/toggle-demo` stays pointed there. This installer does not edit that binding, Hyprland, or stock Super+Space.

The helper signals a process only when `/proc/PID/exe` is the installed binary's inode and checksum, the uid is the current user, and the argv is exactly that binary plus `--no-exec`. A layer namespace without that process is not killed and does not start a second copy. `--live`, `--exec`, and `--run-guards` are refused. Rollback moves `current` and `$PREFIX/bin/toggle-demo` to the previous owned version together, or removes both when nothing owned remains.

## Float

Reported for Omarchy 4.0.6-1: an `o.window` rule with an exact empty class/title match for this app's class `omarchy-menu-rust`. That pattern is known on that reported install, not universally portable. This installer does not install it.

## Optional menu entry

`share/omarchy-menu-entry.jsonc.sample` is a complete entry (id, label, and action) because upstream MenuModel merge clears omitted fields. The action is the placeholder `PREFIX/current/bin/omarchy-menu-rust` with no search interpolation. Installing the sample into the user extension is manual and is not done by the installer. The installer does not write `~/.config/omarchy`.
