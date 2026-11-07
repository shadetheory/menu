//! Opt-in launches for a few desktop apps whose system file and executable
//! path were recorded. An Exec string is not itself a safe launch.
//!
//! Limits, on purpose:
//! - A desktop file is admitted only when its canonical path stays inside a
//!   trusted system applications directory. `~/.local/share/applications`,
//!   `--apps-dir`, and a symlink that resolves outside those directories do not.
//! - The executable is the documented absolute path after `canonicalize`.
//!   A bare name is accepted only as that file's name. `$PATH` is not searched,
//!   so a same-named file earlier on `PATH` is not started.
//! - This does not hash the binary or prove the system directory is intact.
//!   Replacing the documented file itself is outside this check.
//! - Basenames from the 2026-11-06 notes for Ghostty, Grok Bot, and Mattermost
//!   are not launchable. Those notes did not record a canonical executable.
//! - `Terminal=true`, field codes, shell wrappers, stock actions, updates, and
//!   messaging commands are not on this list.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub struct SafeApp {
    pub desktop_id: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    pub provenance: &'static str,
}

/// Launchable rows. Ghostty, Grok Bot, and Mattermost are omitted: the notes
/// recorded only a basename, which is not a canonical executable.
pub const SAFE_APPS: &[SafeApp] = &[
    SafeApp {
        desktop_id: "com.thisisgm.flea.desktop",
        program: "/usr/bin/flea",
        args: &["--gui"],
        provenance: "Desktop file /usr/share/applications/com.thisisgm.flea.desktop. Executable /usr/bin/flea from the 2026-11-06 quick-action note. A bare name flea means that path, not PATH.",
    },
    SafeApp {
        desktop_id: "cursor.desktop",
        program: "/usr/share/cursor/cursor",
        args: &[],
        provenance: "Desktop file /usr/share/applications/cursor.desktop. Executable /usr/share/cursor/cursor from the 2026-11-06 desktop Exec. A different local wrapper is not this path. %F does not match.",
    },
];

pub const SYSTEM_DESKTOP_ROOTS: &[&str] =
    &["/usr/share/applications", "/usr/local/share/applications"];

pub struct AppFact<'a> {
    pub desktop_id: &'a str,
    pub desktop_file: &'a Path,
    pub argv: &'a [String],
    pub terminal: bool,
    pub needs_input: bool,
    pub dangerous: bool,
    pub unavailable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub provenance: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

#[derive(Clone)]
pub struct Spec {
    pub desktop_id: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub provenance: String,
}

pub struct Trust<'a> {
    pub desktop_roots: &'a [PathBuf],
    pub specs: &'a [Spec],
}

pub fn system_trust() -> (Vec<PathBuf>, Vec<Spec>) {
    let roots = SYSTEM_DESKTOP_ROOTS.iter().map(PathBuf::from).collect();
    let specs = SAFE_APPS
        .iter()
        .map(|app| Spec {
            desktop_id: app.desktop_id.to_string(),
            program: PathBuf::from(app.program),
            args: app.args.iter().map(|arg| (*arg).to_string()).collect(),
            provenance: app.provenance.to_string(),
        })
        .collect();
    (roots, specs)
}

pub fn admit(app: AppFact<'_>, trust: &Trust<'_>) -> Result<Admission, &'static str> {
    if app.terminal {
        return Err("Terminal=true entries are not safe-app launches");
    }
    if app.needs_input {
        return Err("an Exec that still needs typed input is not a safe-app launch");
    }
    if app.dangerous {
        return Err("a destructive desktop Exec is not a safe-app launch");
    }
    if app.unavailable {
        return Err("an unavailable desktop entry is not a safe-app launch");
    }
    if app.desktop_id.is_empty() || app.desktop_file.as_os_str().is_empty() {
        return Err("a row without a system desktop file is not a safe-app launch");
    }
    let desktop = canonical_under(app.desktop_file, trust.desktop_roots)
        .ok_or("desktop file is not under a trusted system applications directory")?;
    if desktop.file_name().and_then(|name| name.to_str()) != Some(app.desktop_id) {
        return Err("desktop filename does not match the scanned id");
    }
    let Some(spec) = trust
        .specs
        .iter()
        .find(|spec| spec.desktop_id == app.desktop_id)
    else {
        return Err("that desktop id is not on the safe-app list");
    };
    let program = resolve_program(
        app.argv.first().map(String::as_str).unwrap_or(""),
        &spec.program,
    )?;
    let got_args = app.argv.get(1..).unwrap_or(&[]);
    if got_args.len() != spec.args.len()
        || got_args
            .iter()
            .zip(spec.args.iter())
            .any(|(got, want)| got != want)
    {
        return Err("the scanned Exec args do not match the documented executable");
    }
    Ok(Admission {
        provenance: spec.provenance.clone(),
        program,
        args: spec.args.clone(),
    })
}

fn canonical_under(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let canon = std::fs::canonicalize(path).ok()?;
    let inside = roots.iter().any(|root| {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        canon.starts_with(&root)
    });
    inside.then_some(canon)
}

/// `PATH` is intentionally unused. A bare name launches only `spec_program`.
fn resolve_program(argv0: &str, spec_program: &Path) -> Result<PathBuf, &'static str> {
    let want = std::fs::canonicalize(spec_program)
        .map_err(|_| "documented executable is not on this system")?;
    if argv0.is_empty() {
        return Err("Exec program is empty");
    }
    let got_path = Path::new(argv0);
    if got_path.is_absolute() {
        let got =
            std::fs::canonicalize(got_path).map_err(|_| "Exec program could not be resolved")?;
        if got != want {
            return Err("Exec program is not the documented canonical executable");
        }
        return Ok(want);
    }
    if got_path.components().count() != 1 || got_path.file_name() != spec_program.file_name() {
        return Err("Exec program is not the documented executable name");
    }
    Ok(want)
}

pub fn admit_configured(app: AppFact<'_>) -> Result<Admission, &'static str> {
    #[cfg(test)]
    if let Some((roots, specs)) = TEST_TRUST.with(|slot| slot.borrow().clone()) {
        let trust = Trust {
            desktop_roots: &roots,
            specs: &specs,
        };
        return admit(app, &trust);
    }
    let (roots, specs) = system_trust();
    let trust = Trust {
        desktop_roots: &roots,
        specs: &specs,
    };
    admit(app, &trust)
}

#[cfg(test)]
thread_local! {
    static TEST_TRUST: std::cell::RefCell<Option<(Vec<PathBuf>, Vec<Spec>)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub struct TestTrustGuard;

#[cfg(test)]
impl Drop for TestTrustGuard {
    fn drop(&mut self) {
        TEST_TRUST.with(|slot| *slot.borrow_mut() = None);
    }
}

/// Test-only trust roots. Release builds have no hook.
#[cfg(test)]
pub fn install_test_trust(roots: Vec<PathBuf>, specs: Vec<Spec>) -> TestTrustGuard {
    TEST_TRUST.with(|slot| *slot.borrow_mut() = Some((roots, specs)));
    TestTrustGuard
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn script(path: &Path, marker: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(
            path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$0\" > '{}'\n",
                marker.display()
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn shadow_desktop_and_path_do_not_replace_the_documented_file() {
        let dir = std::env::temp_dir().join(format!("omr-prov-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("usr/share/applications");
        let user = dir.join("home/applications");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&user).unwrap();
        let real = dir.join("usr/bin/flea");
        let real_mark = dir.join("real-ran");
        let shadow = dir.join("shadow/flea");
        let shadow_mark = dir.join("shadow-ran");
        script(&real, &real_mark);
        script(&shadow, &shadow_mark);
        let desktop = root.join("com.thisisgm.flea.desktop");
        fs::write(
            &desktop,
            "[Desktop Entry]\nType=Application\nName=Flea\nExec=flea --gui\n",
        )
        .unwrap();
        let user_desktop = user.join("com.thisisgm.flea.desktop");
        fs::write(
            &user_desktop,
            "[Desktop Entry]\nType=Application\nName=Flea\nExec=flea --gui\n",
        )
        .unwrap();
        let outside = dir.join("outside.desktop");
        fs::write(&outside, "not used\n").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked.desktop")).unwrap();

        let specs = vec![
            Spec {
                desktop_id: "com.thisisgm.flea.desktop".into(),
                program: real.clone(),
                args: vec!["--gui".into()],
                provenance: "fixture".into(),
            },
            Spec {
                desktop_id: "linked.desktop".into(),
                program: real.clone(),
                args: vec!["--gui".into()],
                provenance: "symlink fixture".into(),
            },
        ];
        let roots = vec![root.clone()];
        let trust = Trust {
            desktop_roots: &roots,
            specs: &specs,
        };
        let argv = vec!["flea".into(), "--gui".into()];
        let admitted = admit(
            AppFact {
                desktop_id: "com.thisisgm.flea.desktop",
                desktop_file: &desktop,
                argv: &argv,
                terminal: false,
                needs_input: false,
                dangerous: false,
                unavailable: false,
            },
            &trust,
        )
        .unwrap();
        assert_eq!(admitted.program, fs::canonicalize(&real).unwrap());

        let mut cmd = std::process::Command::new(&admitted.program);
        cmd.args(&admitted.args);
        cmd.env("PATH", dir.join("shadow"));
        assert!(cmd.status().unwrap().success());
        assert_eq!(
            fs::read_to_string(&real_mark).unwrap().trim(),
            fs::canonicalize(&real).unwrap().to_string_lossy()
        );
        assert!(!shadow_mark.exists());

        assert!(admit(
            AppFact {
                desktop_id: "com.thisisgm.flea.desktop",
                desktop_file: &user_desktop,
                argv: &argv,
                terminal: false,
                needs_input: false,
                dangerous: false,
                unavailable: false,
            },
            &trust,
        )
        .is_err());
        assert!(admit(
            AppFact {
                desktop_id: "linked.desktop",
                desktop_file: &root.join("linked.desktop"),
                argv: &argv,
                terminal: false,
                needs_input: false,
                dangerous: false,
                unavailable: false,
            },
            &trust,
        )
        .is_err());
        let other = dir.join("usr/bin/not-flea");
        script(&other, &dir.join("other-ran"));
        let other_argv = vec![other.to_string_lossy().into_owned(), "--gui".into()];
        assert!(admit(
            AppFact {
                desktop_id: "com.thisisgm.flea.desktop",
                desktop_file: &desktop,
                argv: &other_argv,
                terminal: false,
                needs_input: false,
                dangerous: false,
                unavailable: false,
            },
            &trust,
        )
        .is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
