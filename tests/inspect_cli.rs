use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn dry_run_and_list_do_not_start_the_desktop_app() {
    let dir = std::env::temp_dir().join(format!("omr-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("ran");
    let script = dir.join("harmless");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    std::fs::write(
        dir.join("harmless.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Cli Harmless\nExec={}\nTerminal=true\n",
            script.display()
        ),
    )
    .unwrap();
    let exe = env!("CARGO_BIN_EXE_omarchy-menu-rust");
    let dry = Command::new(exe)
        .args([
            "--dry-run",
            "Cli Harmless",
            "--apps-dir",
            dir.to_str().unwrap(),
            "--terminal-argv",
            "ghostty -e",
            "--live",
        ])
        .env_remove("OMARCHY_MENU_TERMINAL")
        .output()
        .unwrap();
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(text.contains("mode dry-run"));
    assert!(text.contains("would_execute false"));
    assert!(text.contains("name Cli Harmless"));
    assert!(text.contains("ghostty -e"));
    assert!(!text.contains("bash -c"));
    assert!(!marker.exists());
    let list = Command::new(exe)
        .args(["--list", "--apps-dir", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(list.status.success());
    let listed = String::from_utf8_lossy(&list.stdout);
    assert!(listed.contains("mode list"));
    assert!(listed.contains("would_execute false"));
    assert!(listed.contains("verb Unavailable"));
    assert!(!marker.exists());
    let refused = Command::new(exe)
        .args(["--dry-run", "x", "--terminal-argv", "bash -c editor"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    let allow = dir.join("allow.txt");
    std::fs::write(&allow, format!("true; touch {}\n", marker.display())).unwrap();
    let inspected = Command::new(exe)
        .args([
            "--apps-dir",
            dir.to_str().unwrap(),
            "--dry-run",
            "Cli Harmless",
            "--exec",
            "--live",
        ])
        .env("OMARCHY_MENU_GUARD_ALLOW", &allow)
        .env_remove("OMARCHY_MENU_TERMINAL")
        .output()
        .unwrap();
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let body = String::from_utf8_lossy(&inspected.stdout);
    assert!(body.contains("would_execute false"));
    assert!(!body.contains("bash -c"));
    assert!(!marker.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trace_state_is_opt_in() {
    let exe = env!("CARGO_BIN_EXE_omarchy-menu-rust");
    let help = Command::new(exe).arg("--help").output().unwrap();
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("--trace-state"));
    let quiet = Command::new(exe)
        .args(["--dry-run", "About", "--no-exec"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&quiet.stderr);
    assert!(!err.contains("omarchy-menu-rust: state "));
}
