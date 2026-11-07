//! Trusted command dispatch.
//!
//! The search string is never appended, substituted, or placed in `bash -c`.
//! Only an action string that came from the menu file, or a configured
//! launcher argv, can be spawned. Sample panels have no action.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanKind {
    /// Trusted menu action. `bash -c` of that action only. Requires `--exec`.
    ShellAction,
    /// Desktop Exec or an `omarchy-*` path, already split. `--live` may run it.
    AppArgv,
    /// Hands a route to the stock menu. Requires `--exec`.
    StockFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnPlan {
    pub program: String,
    pub args: Vec<String>,
    pub kind: PlanKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    Empty,
    QueryLeak,
    Refused,
}

/// `bash -c` of a trusted action. `query` is checked, never interpolated.
pub fn plan_action(action: &str, query: &str) -> Result<SpawnPlan, PlanError> {
    let action = action.trim();
    if action.is_empty() {
        return Err(PlanError::Empty);
    }
    if !query.is_empty() && action.contains(query) {
        // The query happened to be a substring of a trusted command. Still
        // refuse to spawn while that query is active so a typed string cannot
        // be what makes the command line.
        return Err(PlanError::QueryLeak);
    }
    Ok(SpawnPlan {
        program: "bash".into(),
        args: vec!["-c".into(), action.to_string()],
        kind: PlanKind::ShellAction,
    })
}

pub fn plan_argv(argv: &[String]) -> Result<SpawnPlan, PlanError> {
    if argv.is_empty() || argv.iter().any(|a| a.is_empty()) {
        return Err(PlanError::Empty);
    }
    if argv.iter().any(|a| a.contains('\n')) {
        return Err(PlanError::Refused);
    }
    let program = argv[0].as_str();
    if matches!(program, "bash" | "sh" | "dash" | "zsh")
        && argv
            .get(1)
            .is_some_and(|flag| flag == "-c" || flag == "-lc")
    {
        return Err(PlanError::Refused);
    }
    Ok(SpawnPlan {
        program: argv[0].clone(),
        args: argv[1..].to_vec(),
        kind: PlanKind::AppArgv,
    })
}

pub fn is_dangerous(action: &str) -> bool {
    let n = action.to_lowercase();
    const NEEDLES: &[&str] = &[
        "systemctl reboot",
        "systemctl poweroff",
        "systemctl hibernate",
        "systemctl suspend",
        "omarchy-system reboot",
        "omarchy-system shutdown",
        "omarchy-system logout",
        "factory-reset",
        "omarchy-drive-password",
        " passwd",
        "sudo ",
        "rm -",
        "pacman -r",
        "mkfs",
        "shutdown",
        "reboot",
    ];
    NEEDLES.iter().any(|n0| n.contains(n0)) || n == "passwd"
}

pub fn fallback_script() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(root) = std::env::var("OMARCHY_MENU_RUST_ROOT") {
        candidates.push(PathBuf::from(root).join("actions/fallback-stock.sh"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../actions/fallback-stock.sh"));
            candidates.push(dir.join("../../actions/fallback-stock.sh"));
            candidates.push(dir.join("../../../actions/fallback-stock.sh"));
        }
    }
    candidates.push(PathBuf::from("actions/fallback-stock.sh"));
    candidates.into_iter().find(|p| p.is_file())
}

pub fn plan_stock_fallback(route: &str) -> Result<SpawnPlan, PlanError> {
    if route.contains('\n') || route.contains('\0') {
        return Err(PlanError::Refused);
    }
    let Some(script) = fallback_script() else {
        return Err(PlanError::Empty);
    };
    // Argv, not a shell string. The script refuses to re-enter this binary.
    Ok(SpawnPlan {
        program: script.to_string_lossy().into_owned(),
        args: vec!["summon".into(), route.to_string()],
        kind: PlanKind::StockFallback,
    })
}

/// Decides whether a confirmed plan may start.
///
/// `--no-exec` blocks every kind unless `--live` or an admitted `--safe-apps`
/// row says otherwise. `--live` allows a non-dangerous app argv and still
/// blocks shell actions, stock handoff, and dangerous commands. `--safe-apps`
/// allows only a row the caller already admitted. `--exec` (`no_exec` false)
/// allows the confirmed plan, including a dangerous stock action.
pub fn gate_spawn(
    no_exec: bool,
    live: bool,
    safe_apps: bool,
    admitted: bool,
    plan: &SpawnPlan,
    dangerous: bool,
) -> Result<(), &'static str> {
    match plan.kind {
        PlanKind::ShellAction | PlanKind::StockFallback => {
            if no_exec {
                Err("shell action was not started")
            } else {
                Ok(())
            }
        }
        PlanKind::AppArgv => {
            if dangerous {
                if no_exec {
                    Err("dangerous command was not started")
                } else {
                    Ok(())
                }
            } else if !no_exec || live || (safe_apps && admitted) {
                Ok(())
            } else {
                Err("command was not started")
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static FAKE_LAUNCHER: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub struct FakeLauncherGuard;

#[cfg(test)]
impl Drop for FakeLauncherGuard {
    fn drop(&mut self) {
        FAKE_LAUNCHER.with(|slot| *slot.borrow_mut() = None);
    }
}

/// Point later `spawn_report` calls on this thread at a test binary.
/// Release builds have no hook. Other threads are unchanged.
#[cfg(test)]
pub fn install_fake_launcher(path: PathBuf) -> FakeLauncherGuard {
    FAKE_LAUNCHER.with(|slot| *slot.borrow_mut() = Some(path));
    FakeLauncherGuard
}

/// Starts the plan and returns before it exits. Spawn errors are immediate.
/// The callback later receives the exit status. The search string is not exported.
pub fn spawn_report(
    plan: &SpawnPlan,
    report: impl Fn(String) + Send + 'static,
) -> Result<String, String> {
    let mut child = spawn_command(plan)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("OMARCHY_MENU_QUERY")
        .spawn()
        .map_err(|e| format!("did not start {}: {e}", plan.program))?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    thread::spawn(move || {
        let mut out = String::new();
        let mut err = String::new();
        if let Some(pipe) = stdout.as_mut() {
            let _ = pipe.read_to_string(&mut out);
        }
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_string(&mut err);
        }
        match child.wait() {
            Ok(status) if status.success() => {
                let extra = if out.trim().is_empty() {
                    String::new()
                } else {
                    format!(" {}", out.trim())
                };
                report(format!("exited 0.{extra}"));
            }
            Ok(status) => {
                let detail = if err.trim().is_empty() {
                    out.trim()
                } else {
                    err.trim()
                };
                report(format!("failed {status}. {detail}"));
            }
            Err(e) => report(format!("failed while waiting: {e}")),
        }
    });
    Ok(format!("started {} (detached)", plan.program))
}

fn spawn_command(plan: &SpawnPlan) -> Command {
    #[cfg(test)]
    if let Some(fake) = FAKE_LAUNCHER.with(|slot| slot.borrow().clone()) {
        let mut cmd = Command::new(fake);
        cmd.arg(&plan.program).args(&plan.args);
        return cmd;
    }
    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_text_never_enters_bash() {
        let queries = [
            "$(reboot)",
            "; rm -rf /",
            "'; shutdown",
            "check on my groceries",
            "call example",
        ];
        let action = "omarchy-launch-about";
        for q in queries {
            let plan = plan_action(action, q).unwrap();
            assert_eq!(plan.program, "bash");
            assert_eq!(plan.args, vec!["-c".to_string(), action.to_string()]);
            assert!(!plan.args.iter().any(|a| a.contains(q)));
        }
    }

    #[test]
    fn query_that_is_inside_the_action_is_refused() {
        let err = plan_action("echo check on my groceries", "check on my groceries").unwrap_err();
        assert_eq!(err, PlanError::QueryLeak);
    }

    #[test]
    fn empty_and_shell_wrappers_refused() {
        assert_eq!(plan_action("   ", "").unwrap_err(), PlanError::Empty);
        assert_eq!(
            plan_argv(&["bash".into(), "-c".into(), "id".into()]).unwrap_err(),
            PlanError::Refused
        );
    }

    #[test]
    fn dangerous_stock_commands_are_marked() {
        assert!(is_dangerous("omarchy-system reboot"));
        assert!(is_dangerous("systemctl suspend"));
        assert!(is_dangerous(
            "omarchy-launch-floating-terminal-with-presentation omarchy-system-factory-reset"
        ));
        assert!(!is_dangerous("omarchy-launch-about"));
    }

    #[test]
    fn fallback_script_refuses_recursion_and_keeps_argv() {
        let script = fallback_script().expect("script");
        let denied = std::process::Command::new("bash")
            .arg(&script)
            .arg("summon")
            .arg("system")
            .env("OMARCHY_MENU_RUST_FALLBACK", "1")
            .output()
            .expect("run");
        assert!(!denied.status.success());
        let err = String::from_utf8_lossy(&denied.stderr);
        assert!(err.contains("recursion"), "{err}");

        let dir =
            std::env::temp_dir().join(format!("omarchy-menu-rust-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("omarchy-menu");
        let out = dir.join("out");
        std::fs::write(
            &fake,
            "#!/bin/sh\nprintf '%s\\n' \"$1\" \"$2\" > \"$OMR_OUT\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&fake).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake, perms).unwrap();
        let ran = std::process::Command::new("bash")
            .arg(&script)
            .arg("summon")
            .arg("system")
            .env("PATH", format!("{}:/usr/bin:/bin", dir.display()))
            .env("OMR_OUT", &out)
            .env_remove("OMARCHY_MENU_RUST_FALLBACK")
            .env_remove("OMARCHY_PATH")
            .output()
            .expect("fallback");
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let recorded = std::fs::read_to_string(&out).unwrap();
        assert_eq!(recorded, "summon\nsystem\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_launcher_does_not_use_xdg_open_or_default_profile() {
        let text = include_str!("../preview/scripts/launch-preview.sh");
        assert!(text.contains("--user-data-dir"));
        assert!(text.contains("--app="));
        assert!(
            text.lines()
                .any(|line| line.contains("xdg-open") && line.trim_start().starts_with('#'))
                || text.contains("will not call xdg-open")
        );
        assert!(!text.lines().any(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with('#') && trimmed.contains("xdg-open")
        }));
        assert!(text.contains("google-chrome"));
    }

    #[test]
    fn live_allows_app_argv_and_blocks_shell_and_danger() {
        let app = plan_argv(&["/usr/bin/true".into()]).unwrap();
        assert!(gate_spawn(true, true, false, false, &app, false).is_ok());
        assert!(gate_spawn(true, false, false, false, &app, false)
            .unwrap_err()
            .contains("not started"));
        let shell = plan_action("omarchy-launch-about", "").unwrap();
        assert!(gate_spawn(true, true, true, true, &shell, false).is_err());
        assert!(gate_spawn(false, false, false, false, &shell, false).is_ok());
        let danger = plan_argv(&["/usr/bin/omarchy-reboot-test".into()]).unwrap();
        assert!(gate_spawn(true, true, true, true, &danger, true)
            .unwrap_err()
            .contains("not started"));
        assert!(gate_spawn(false, false, false, false, &danger, true).is_ok());
        let ghostty = plan_argv(&["ghostty".into(), "--gtk-single-instance=true".into()]).unwrap();
        assert!(gate_spawn(true, false, true, true, &ghostty, false).is_ok());
        assert!(gate_spawn(true, false, true, false, &ghostty, false).is_err());
        assert!(gate_spawn(true, false, false, true, &ghostty, false).is_err());
        let quoted = crate::desktop::parse_exec(
            r#"/tmp/demo "a;b$(reboot)""#,
            "Demo",
            "/tmp/demo.desktop",
            "",
        )
        .unwrap();
        let plan = plan_argv(&quoted.argv).unwrap();
        assert_eq!(plan.kind, PlanKind::AppArgv);
        assert_eq!(plan.args, vec!["a;b$(reboot)".to_string()]);
        assert_ne!(plan.program, "bash");
    }

    #[test]
    fn fallback_plan_keeps_route_as_argv() {
        let plan = plan_stock_fallback("system").expect("script");
        assert!(plan.program.ends_with("fallback-stock.sh"));
        assert_eq!(plan.args, vec!["summon".to_string(), "system".to_string()]);
        assert!(!plan.args.iter().any(|a| a.contains("bash")));
    }
}
