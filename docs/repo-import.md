# Repo import

This tree is imported onto the existing public repository `shadetheory/menu`.

The starter commit `14cfe3c031426ef076efa7c344f6ddeecfad6162` stays in history. It added `.gitignore`, `LICENSE`, and `README.md`. This import does not rewrite that commit. The license file remains the GNU Affero General Public License v3.0 only. `Cargo.toml` records `AGPL-3.0-only` so the package metadata matches that file. The imported snapshot had no `LICENSE` file and had set `license = "MIT"` in `Cargo.toml`; that metadata was aligned to the repository license rather than replacing `LICENSE`.

Source snapshot: `6991b734608ed266d51d12219e78ba304ce5b038`.

Archive SHA256: `3b88b2ff74e79ab38fa620946c53f1c371c4a7b14471677a8cf6789c44bb2924`.

No GitHub Actions workflow was added. Nothing was published to a crate registry. The default branch remains `master`.

## What the program is

`omarchy-menu-rust` is a standalone Rust app that runs alongside Omarchy. It is not an official QML shell plugin.

Official current shell plugins are a `manifest.json` plus QML, installed with `omarchy plugin add`. Sources:

- [Shell plugins manual](https://omarchy.org/manual/shell-plugins/)
- [shell/plugins/README.md](https://github.com/basecamp/omarchy/blob/quattro/shell/plugins/README.md)

Menu contract: [docs/menu.md](https://github.com/basecamp/omarchy/blob/fa955bfa/docs/menu.md). This tree is pinned to upstream commit `14abc0762ce38547d1912061969df58962f6f10a`.

The README states the same boundary: standalone Rust app alongside Omarchy, not an official plugin.

## Tested-on note

One reported install was omarchy `4.0.6-1`, version file `4.0.0.alpha`, Quickshell `0.3.2.r7`. That is a tested-on note. It is not universal support for every Omarchy or Quickshell release.

## Left out of the public tree

Private roster identifiers, home-directory launcher paths, and draft notes that name a person or a machine were not imported. A separate theme package was not imported. JetBrains Mono stays under the SIL Open Font License.
