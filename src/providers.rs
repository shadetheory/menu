//! Local providers.
//!
//! Apps come from desktop files. Commands are executable `omarchy-*` names on
//! the directories the caller passes. Other stock providers stay an explicit
//! unavailable row. This module does not start those programs.

use crate::desktop::{self, DesktopScan};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Apps,
    Commands,
    Fonts,
    PowerProfiles,
    Kernels,
    Languages,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedRow {
    pub title: String,
    pub hint: String,
    /// Desktop filename, such as `cursor.desktop`. Empty for commands.
    pub desktop_id: String,
    /// Path of the scanned desktop file. Empty for commands.
    pub desktop_file: PathBuf,
    pub argv: Vec<String>,
    pub input_at: Option<usize>,
    pub terminal: bool,
    pub unavailable: bool,
    pub dangerous: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub rows: Vec<ListedRow>,
    pub malformed: usize,
    pub unreadable: usize,
}

pub fn classify(name: &str) -> ProviderKind {
    match name.to_ascii_lowercase().as_str() {
        "apps" => ProviderKind::Apps,
        "commands" => ProviderKind::Commands,
        "fonts" => ProviderKind::Fonts,
        "power-profiles" => ProviderKind::PowerProfiles,
        "kernels" => ProviderKind::Kernels,
        "languages" => ProviderKind::Languages,
        _ => ProviderKind::Unknown(name.to_string()),
    }
}

pub fn kind_name(kind: &ProviderKind) -> String {
    match kind {
        ProviderKind::Apps => "apps".to_string(),
        ProviderKind::Commands => "commands".to_string(),
        ProviderKind::Fonts => "fonts".to_string(),
        ProviderKind::PowerProfiles => "power-profiles".to_string(),
        ProviderKind::Kernels => "kernels".to_string(),
        ProviderKind::Languages => "languages".to_string(),
        ProviderKind::Unknown(name) => name.clone(),
    }
}

pub fn discover_apps(dirs: &[PathBuf], terminal: &[String]) -> Listed {
    let scan: DesktopScan = if terminal.is_empty() {
        desktop::scan_app_dirs(dirs)
    } else {
        desktop::scan_app_dirs_with(dirs, terminal)
    };
    let rows = scan
        .apps
        .into_iter()
        .map(|app| {
            let unavailable = app.unavailable.is_some();
            let hint = app.unavailable.clone().unwrap_or_else(|| {
                if app.terminal {
                    "Terminal application. The configured terminal argv is used. No shell.".into()
                } else if app.input_at.is_some() {
                    "Needs one argument. Nothing runs until that text is confirmed.".into()
                } else if app.comment.is_empty() {
                    "Desktop entry. One Enter opens it.".into()
                } else {
                    app.comment
                }
            });
            let own = app
                .argv
                .get(if app.terminal { terminal.len() } else { 0 })
                .or_else(|| app.argv.first());
            ListedRow {
                dangerous: own.is_some_and(|program| crate::dispatch::is_dangerous(program)),
                title: app.name,
                hint,
                desktop_id: app.desktop_id,
                desktop_file: app.desktop_file,
                input_at: app.input_at,
                terminal: app.terminal,
                argv: app.argv,
                unavailable,
            }
        })
        .collect();
    Listed {
        rows,
        malformed: scan.malformed,
        unreadable: scan.unreadable,
    }
}

pub fn discover_commands(dirs: &[PathBuf]) -> Listed {
    let mut rows = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for dir in dirs {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("omarchy-")
                || name.contains("..")
                || !seen.insert(name.to_string())
            {
                continue;
            }
            if !path.is_file() || !is_executable(&path) {
                continue;
            }
            let dangerous = crate::dispatch::is_dangerous(name);
            rows.push(ListedRow {
                title: name.to_string(),
                hint: if dangerous {
                    "Dangerous command. Cancel is the default.".into()
                } else {
                    "Omarchy command. One Enter opens it. It is not a shell.".into()
                },
                desktop_id: String::new(),
                desktop_file: PathBuf::new(),
                argv: vec![path.to_string_lossy().into_owned()],
                input_at: None,
                terminal: false,
                unavailable: false,
                dangerous,
            });
        }
    }
    rows.sort_by(|a, b| a.title.cmp(&b.title));
    Listed {
        rows,
        malformed: 0,
        unreadable: 0,
    }
}

pub fn unavailable_list(kind: &ProviderKind) -> Listed {
    let name = kind_name(kind);
    Listed {
        rows: vec![ListedRow {
            title: format!("{name} unavailable"),
            hint: format!(
                "{name} is not connected in this menu. Nothing was changed. The stock menu remains the fallback."
            ),
            desktop_id: String::new(),
            desktop_file: PathBuf::new(),
            argv: Vec::new(),
            input_at: None,
            terminal: false,
            unavailable: true,
            dangerous: false,
        }],
        malformed: 0,
        unreadable: 0,
    }
}

pub fn default_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home).join("applications"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_DIRS") {
        for dir in std::env::split_paths(&data) {
            dirs.push(dir.join("applications"));
        }
    } else {
        dirs.push(PathBuf::from("/usr/local/share/applications"));
        dirs.push(PathBuf::from("/usr/share/applications"));
    }
    dirs
}

pub fn default_command_dirs() -> Vec<PathBuf> {
    match std::env::var_os("PATH") {
        Some(path) => std::env::split_paths(&path).collect(),
        None => Vec::new(),
    }
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// The list is not a live account. Fonts, profiles, kernels, and languages are not changed here.
pub fn fallback_row_label(kind: &ProviderKind) -> String {
    match kind {
        ProviderKind::Apps => {
            "Desktop entries from the local application directories.".into()
        }
        ProviderKind::Commands => "Executable omarchy-* names. No shell is used.".into(),
        _ => format!(
            "{} is unavailable here. The stock menu is the explicit fallback. No search text is sent.",
            kind_name(kind)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn classify_fonts_and_unknown() {
        assert_eq!(classify("fonts"), ProviderKind::Fonts);
        assert_eq!(classify("nope"), ProviderKind::Unknown("nope".into()));
        assert_eq!(classify("apps"), ProviderKind::Apps);
        assert_eq!(classify("commands"), ProviderKind::Commands);
        assert_eq!(classify("power-profiles"), ProviderKind::PowerProfiles);
        assert_eq!(classify("kernels"), ProviderKind::Kernels);
        assert_eq!(classify("languages"), ProviderKind::Languages);
        let label = fallback_row_label(&ProviderKind::Fonts);
        assert!(label.contains("unavailable"));
        assert!(label.contains("stock menu"));
        assert!(label.contains("explicit fallback"));
        let fonts = unavailable_list(&ProviderKind::Fonts);
        assert!(fonts.rows[0].unavailable);
        assert!(fonts.rows[0].argv.is_empty());
    }

    #[test]
    fn commands_are_omarchy_executables_only() {
        let dir = std::env::temp_dir().join(format!("omr-cmd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "omarchy-menu-test",
            "omarchy-reboot-test",
            "not-omarchy",
            "omarchy-plain",
        ] {
            let path = dir.join(name);
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(if name == "omarchy-plain" {
                0o644
            } else {
                0o755
            });
            fs::set_permissions(&path, perms).unwrap();
        }
        let listed = discover_commands(std::slice::from_ref(&dir));
        let titles: Vec<_> = listed.rows.iter().map(|row| row.title.as_str()).collect();
        assert!(titles.contains(&"omarchy-menu-test"));
        assert!(titles.contains(&"omarchy-reboot-test"));
        assert!(!titles.contains(&"not-omarchy"));
        assert!(!titles.contains(&"omarchy-plain"));
        assert!(
            listed
                .rows
                .iter()
                .find(|row| row.title == "omarchy-reboot-test")
                .unwrap()
                .dangerous
        );
        assert!(!listed
            .rows
            .iter()
            .any(|row| row.argv.iter().any(|arg| arg.contains("sh -c"))));
        let _ = fs::remove_dir_all(&dir);
    }
}
