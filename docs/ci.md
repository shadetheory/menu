# Menu CI

`.github/workflows/ci.yml` runs on pushes to every branch, pull requests, and manual dispatch. It uses GitHub-hosted Ubuntu 24.04 and the locally verified Rust 1.99.0 toolchain. The job installs native Wayland/XKB/X11 build libraries, checks formatting, runs all target tests, runs Clippy with warnings denied, and builds release binaries. Isolated installer/rollback and mock-compositor toggle scripts follow the build.

Cargo commands use `--locked`; dependency/build caching is an optimization, not a correctness requirement. The workflow has read-only repository permissions, does not retain checkout credentials, cancels superseded runs for the same branch/PR, and stops after 25 minutes. It does not publish releases or install onto Thor.

Recall config/cache paths are isolated under runner temporary storage. Tests use temporary files and loopback HTTP fixtures; no Ollama service, local generation model, personal corpus, or API secret is required. The workflow does not prove native Hyprland focus, physical Alt+Space delivery, visual quality, or actual-model latency. Those remain Thor acceptance checks.

## Verification and operation

Local authoring checks: parsed the workflow as YAML and reviewed trigger/job/step structure and shell syntax. The isolated install/rollback script passed locally, including its embedded toggle checks. actionlint was not installed; hosted Actions validation remains required. GitHub execution must be observed after pushing before calling CI operational. A workflow_dispatch event generally requires the workflow to exist on the default branch; the initial feature-branch push is the first-run trigger.

After a successful hosted run, the repository owner can make **Rust checks and release build** a required status check in branch protection. No branch protection or repository settings were changed by adding this workflow.

Inspect runs in the repository Actions tab or with:

```bash
gh run list --workflow ci.yml --branch feat/work-recall
gh run view RUN_ID --log-failed
```

The action references and inputs were checked against their maintainers' documentation: [checkout](https://github.com/actions/checkout), [Rust toolchain](https://github.com/dtolnay/rust-toolchain), and [Rust cache](https://github.com/Swatinem/rust-cache). Toolchain upgrades should be deliberate and validated with the locked dependency graph.

## Hosted verification

First run passed on implementation commit `83ebb3c`: [Menu CI](https://github.com/shadetheory/menu/actions/runs/42438635168). Formatting, all-target tests, Clippy, release build, installer/rollback, and mock toggle checks all succeeded on GitHub Ubuntu 24.04. This does not establish native Thor acceptance.
