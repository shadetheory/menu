//! Read-only inspection. These functions do not start a program, a guard, or a clipboard tool, and they do not write a file.

use crate::providers::{self, ListedRow};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectRow {
    pub name: String,
    pub verb: String,
    pub kind: String,
    pub reason: String,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectReport {
    pub mode: String,
    pub would_execute: bool,
    pub query: String,
    pub rows: Vec<InspectRow>,
}

pub fn list_apps(dirs: &[PathBuf], terminal: &[String]) -> InspectReport {
    report("list", "", dirs, terminal)
}

pub fn dry_run(query: &str, dirs: &[PathBuf], terminal: &[String]) -> InspectReport {
    let mut report = report("dry-run", query, dirs, terminal);
    let needle = query.to_lowercase();
    report.rows.retain(|row| {
        row.name.to_lowercase().contains(&needle) || row.reason.to_lowercase().contains(&needle)
    });
    report
}

fn report(mode: &str, query: &str, dirs: &[PathBuf], terminal: &[String]) -> InspectReport {
    let listed = providers::discover_apps(dirs, terminal);
    let mut rows: Vec<InspectRow> = listed.rows.into_iter().map(inspect_row).collect();
    if listed.unreadable > 0 {
        rows.push(InspectRow {
            name: "applications".into(),
            verb: "Unavailable".into(),
            kind: "provider".into(),
            reason: format!(
                "{} application director{} could not be read.",
                listed.unreadable,
                if listed.unreadable == 1 { "y" } else { "ies" }
            ),
            argv: Vec::new(),
        });
    }
    InspectReport {
        mode: mode.into(),
        would_execute: false,
        query: query.into(),
        rows,
    }
}

fn inspect_row(item: ListedRow) -> InspectRow {
    let verb = if item.unavailable {
        "Unavailable"
    } else if item.input_at.is_some() {
        "Run…"
    } else if item.dangerous {
        "Confirm…"
    } else {
        "Open"
    };
    let kind = if item.hint.contains("Terminal application") {
        "terminal"
    } else if item.unavailable {
        "unavailable"
    } else {
        "app"
    };
    InspectRow {
        name: item.title,
        verb: verb.into(),
        kind: kind.into(),
        reason: item.hint,
        argv: item.argv,
    }
}

pub fn format_report(report: &InspectReport) -> String {
    let mut out = format!(
        "mode {}\nwould_execute {}\nquery {}\ncount {}\n",
        report.mode,
        if report.would_execute {
            "true"
        } else {
            "false"
        },
        report.query,
        report.rows.len()
    );
    for row in &report.rows {
        out.push_str("---\n");
        out.push_str(&format!("name {}\n", row.name));
        out.push_str(&format!("verb {}\n", row.verb));
        out.push_str(&format!("kind {}\n", row.kind));
        out.push_str(&format!("reason {}\n", row.reason));
        out.push_str("argv");
        for arg in &row.argv {
            out.push(' ');
            out.push_str(arg);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_lists_a_terminal_plan_and_does_not_touch_the_script() {
        let dir = std::env::temp_dir().join(format!("omr-inspect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("ran");
        let script = dir.join("harmless");
        std::fs::write(
            &script,
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        std::fs::write(
            dir.join("harmless.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Harmless\nExec={}\n",
                script.display()
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("term.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Term Harmless\nExec={}\nTerminal=true\n",
                script.display()
            ),
        )
        .unwrap();
        let terminal = crate::desktop::parse_terminal_argv("ghostty -e").unwrap();
        let report = dry_run("Harmless", std::slice::from_ref(&dir), &terminal);
        let text = format_report(&report);
        assert!(text.contains("mode dry-run"));
        assert!(text.contains("would_execute false"));
        assert!(text.contains("name Harmless"));
        assert!(text.contains("name Term Harmless"));
        assert!(text.contains("ghostty -e"));
        assert!(!text.contains("bash -c"));
        assert!(!marker.exists());
        let listed = list_apps(std::slice::from_ref(&dir), &[]);
        let blocked = listed
            .rows
            .iter()
            .find(|row| row.name == "Term Harmless")
            .unwrap();
        assert_eq!(blocked.verb, "Unavailable");
        assert!(blocked.argv.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
