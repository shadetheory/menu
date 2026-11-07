//! XDG desktop entries and Exec field codes.
//!
//! The Exec value is split into argv. It is never passed to a shell.
//! File and URL field codes are dropped because this menu has no file selection.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopApp {
    pub name: String,
    pub comment: String,
    /// Filename only, for example `cursor.desktop`. Shortcut rows match this exactly.
    pub desktop_id: String,
    pub desktop_file: PathBuf,
    pub argv: Vec<String>,
    pub input_at: Option<usize>,
    pub terminal: bool,
    pub unavailable: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopScan {
    pub apps: Vec<DesktopApp>,
    pub malformed: usize,
    pub unreadable: usize,
    pub skipped: usize,
}

pub fn scan_app_dirs(dirs: &[PathBuf]) -> DesktopScan {
    scan_app_dirs_with(dirs, &[])
}

/// `terminal` is a configured argv prefix, already split. It is not a shell command.
pub fn scan_app_dirs_with(dirs: &[PathBuf], terminal: &[String]) -> DesktopScan {
    let mut scan = DesktopScan::default();
    let mut seen = std::collections::BTreeSet::new();
    for dir in dirs {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                scan.unreadable += 1;
                continue;
            }
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().and_then(|ext| ext.to_str()) == Some("desktop") && path.is_file()
            })
            .collect();
        files.sort();
        for path in files {
            let id = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() || !seen.insert(id.clone()) {
                continue;
            }
            match load_desktop(&path, terminal) {
                Ok(Load::App(mut app)) => {
                    app.desktop_id = id;
                    scan.apps.push(app);
                }
                Ok(Load::Skip) => scan.skipped += 1,
                Err(LoadError::Unreadable) => scan.unreadable += 1,
                Err(LoadError::Malformed) => scan.malformed += 1,
            }
        }
    }
    scan.apps.sort_by_key(|app| app.name.to_lowercase());
    scan
}

enum Load {
    App(DesktopApp),
    Skip,
}

enum LoadError {
    Unreadable,
    Malformed,
}

fn load_desktop(path: &Path, terminal: &[String]) -> Result<Load, LoadError> {
    let text = fs::read_to_string(path).map_err(|_| LoadError::Unreadable)?;
    let fields = match parse_desktop_entry(&text) {
        Some(fields) => fields,
        None => return Err(LoadError::Malformed),
    };
    if flag(&fields, "Hidden") || flag(&fields, "NoDisplay") {
        return Ok(Load::Skip);
    }
    let Some(kind) = fields.get("Type").map(String::as_str) else {
        return Err(LoadError::Malformed);
    };
    if !kind.eq_ignore_ascii_case("Application") {
        return Ok(Load::Skip);
    }
    match desktop_visibility(&fields, &current_desktops()) {
        Visibility::Skip => return Ok(Load::Skip),
        Visibility::Unavailable(reason) => {
            return Ok(Load::App(DesktopApp {
                name: fields
                    .get("Name")
                    .cloned()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "desktop entry".into()),
                comment: String::new(),
                desktop_id: String::new(),
                desktop_file: path.to_path_buf(),
                argv: Vec::new(),
                input_at: None,
                terminal: false,
                unavailable: Some(reason),
            }));
        }
        Visibility::Show => {}
    }
    let name = fields
        .get("Name")
        .map(String::as_str)
        .filter(|name| !name.is_empty())
        .ok_or(LoadError::Malformed)?;
    let exec = fields
        .get("Exec")
        .map(String::as_str)
        .filter(|exec| !exec.trim().is_empty())
        .ok_or(LoadError::Malformed)?;
    let icon = fields.get("Icon").map(String::as_str).unwrap_or("");
    let comment = fields.get("Comment").cloned().unwrap_or_default();
    let parsed =
        parse_exec(exec, name, &path.to_string_lossy(), icon).map_err(|_| LoadError::Malformed)?;
    let mut unavailable = None;
    let mut argv = parsed.argv;
    let mut input_at = parsed.input_at;
    let is_terminal = flag(&fields, "Terminal");
    if parsed.unsupported_list {
        unavailable = Some(
            "A list of files or URLs (%F or %U) is not supported. This entry was not opened."
                .into(),
        );
        argv.clear();
        input_at = None;
    }
    if is_terminal && terminal.is_empty() {
        unavailable = Some(
            "Terminal application. No terminal argv is configured, so this row does not launch it."
                .into(),
        );
    } else if is_terminal {
        let mut prefixed = terminal.to_vec();
        input_at = input_at.map(|index| index + prefixed.len());
        prefixed.append(&mut argv);
        argv = prefixed;
    } else if let Some(try_exec) = fields.get("TryExec") {
        if !try_exec_ok(try_exec) {
            unavailable = Some(format!(
                "TryExec {try_exec} was not found or is not executable."
            ));
        }
    }
    Ok(Load::App(DesktopApp {
        name: name.to_string(),
        comment,
        desktop_id: String::new(),
        desktop_file: path.to_path_buf(),
        argv: if unavailable.is_some() {
            Vec::new()
        } else {
            argv
        },
        input_at: if unavailable.is_some() {
            None
        } else {
            input_at
        },
        terminal: is_terminal,
        unavailable,
    }))
}

fn flag(fields: &std::collections::BTreeMap<String, String>, key: &str) -> bool {
    matches!(
        fields.get(key).map(String::as_str),
        Some("true" | "1" | "True" | "TRUE")
    )
}

fn try_exec_ok(value: &str) -> bool {
    let token = value.split_whitespace().next().unwrap_or("");
    if token.is_empty() || token.contains('/') {
        return Path::new(token).is_file() && is_executable(Path::new(token));
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(token);
        candidate.is_file() && is_executable(&candidate)
    })
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Keys from the first `[Desktop Entry]` group. Later groups are ignored.
pub fn parse_desktop_entry(text: &str) -> Option<std::collections::BTreeMap<String, String>> {
    let mut in_entry = false;
    let mut saw_entry = false;
    let mut fields = std::collections::BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if in_entry {
                break;
            }
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            saw_entry = saw_entry || in_entry;
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.contains('[') {
            continue;
        }
        fields
            .entry(key.to_string())
            .or_insert_with(|| value.trim().to_string());
    }
    if !saw_entry {
        return None;
    }
    Some(fields)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedExec {
    pub argv: Vec<String>,
    /// Index where one typed file or URL argument is inserted. `%f` and `%u` only.
    pub input_at: Option<usize>,
    /// `%F` and `%U` are a list. This menu does not expand a list.
    pub unsupported_list: bool,
}

#[derive(Clone, Copy)]
enum Field {
    Drop,
    Input,
    List,
    Name,
    Desktop,
    Percent,
    Icon,
}

/// Split a desktop Exec value into argv. Unquoted shell metacharacters are refused.
pub fn parse_exec(
    exec: &str,
    name: &str,
    desktop_path: &str,
    icon: &str,
) -> Result<ParsedExec, String> {
    let chars: Vec<char> = exec.chars().collect();
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escape = false;
    let mut started = false;
    let mut input_at = None;
    let mut unsupported_list = false;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if escape {
            if quoted && matches!(ch, '`' | '$' | '"' | '\\') || !quoted {
                current.push(ch);
            } else {
                return Err("bad escape".into());
            }
            escape = false;
            started = true;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escape = true;
            i += 1;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
            started = true;
            i += 1;
            continue;
        }
        if ch == '%' && !quoted {
            let code = chars.get(i + 1).copied().ok_or("truncated field code")?;
            let field = field_code(code)?;
            let alone = current.is_empty()
                && !quoted
                && chars.get(i + 2).is_none_or(|next| next.is_whitespace());
            match field {
                Field::Percent => {
                    current.push('%');
                    started = true;
                }
                Field::Name => {
                    current.push_str(name);
                    started = true;
                }
                Field::Desktop => {
                    current.push_str(desktop_path);
                    started = true;
                }
                Field::Drop => {
                    if alone {
                        i += 2;
                        finish_arg(&mut args, &mut current, false)?;
                        while chars.get(i).is_some_and(|next| next.is_whitespace()) {
                            i += 1;
                        }
                        started = false;
                        continue;
                    }
                }
                Field::List => {
                    unsupported_list = true;
                    if alone {
                        i += 2;
                        finish_arg(&mut args, &mut current, false)?;
                        while chars.get(i).is_some_and(|next| next.is_whitespace()) {
                            i += 1;
                        }
                        started = false;
                        continue;
                    }
                }
                Field::Input => {
                    if input_at.is_none() {
                        input_at = Some(if alone { args.len() } else { usize::MAX });
                    }
                    if alone {
                        i += 2;
                        finish_arg(&mut args, &mut current, false)?;
                        while chars.get(i).is_some_and(|next| next.is_whitespace()) {
                            i += 1;
                        }
                        started = false;
                        continue;
                    }
                }
                Field::Icon => {
                    if !alone {
                        return Err("icon field code must be its own argument".into());
                    }
                    i += 2;
                    finish_arg(&mut args, &mut current, false)?;
                    if !icon.is_empty() {
                        args.push("--icon".into());
                        args.push(icon.to_string());
                    }
                    while chars.get(i).is_some_and(|next| next.is_whitespace()) {
                        i += 1;
                    }
                    started = false;
                    continue;
                }
            }
            i += 2;
            continue;
        }
        if !quoted && is_shell_meta(ch) {
            return Err(format!("unquoted shell character {ch}"));
        }
        if !quoted && ch.is_whitespace() {
            if started {
                finish_arg(&mut args, &mut current, true)?;
                started = false;
            }
            i += 1;
            continue;
        }
        current.push(ch);
        started = true;
        i += 1;
    }
    if escape || quoted {
        return Err("unclosed exec quote".into());
    }
    if started {
        finish_arg(&mut args, &mut current, true)?;
    }
    if args.is_empty() || args.iter().any(|arg| arg.is_empty() || arg.contains('\0')) {
        return Err("empty exec".into());
    }
    if input_at == Some(usize::MAX) {
        input_at = Some(args.len());
    }
    if input_at == Some(0) {
        input_at = Some(1);
    }
    Ok(ParsedExec {
        argv: args,
        input_at: if unsupported_list { None } else { input_at },
        unsupported_list,
    })
}

fn finish_arg(args: &mut Vec<String>, current: &mut String, keep: bool) -> Result<(), String> {
    if keep && !current.is_empty() {
        if current.contains('\n') {
            return Err("newline in argument".into());
        }
        args.push(std::mem::take(current));
    } else {
        current.clear();
    }
    Ok(())
}

fn field_code(code: char) -> Result<Field, String> {
    match code {
        '%' => Ok(Field::Percent),
        'c' => Ok(Field::Name),
        'k' => Ok(Field::Desktop),
        'i' => Ok(Field::Icon),
        'f' | 'u' => Ok(Field::Input),
        'F' | 'U' => Ok(Field::List),
        'd' | 'D' | 'n' | 'N' | 'v' | 'm' => Ok(Field::Drop),
        other => Err(format!("unknown field code %{other}")),
    }
}

/// A JSON string array, or a quoted argv. It is never evaluated as a shell.
pub fn parse_terminal_argv(raw: &str) -> Result<Vec<String>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let parts = if raw.starts_with('[') {
        parse_json_argv(raw)?
    } else {
        parse_quoted_argv(raw)?
    };
    if parts.is_empty() || parts.len() > 8 {
        return Err("terminal argv length is outside the bound".into());
    }
    if parts
        .iter()
        .any(|part| part.is_empty() || part.len() > 256 || part.contains('\0'))
    {
        return Err("terminal argv token is empty or too long".into());
    }
    if matches!(parts[0].as_str(), "bash" | "sh" | "dash" | "zsh") {
        return Err("terminal argv must not be a shell".into());
    }
    Ok(parts)
}

fn parse_json_argv(raw: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "terminal argv JSON".to_string())?;
    let serde_json::Value::Array(items) = value else {
        return Err("terminal argv JSON must be an array".into());
    };
    let mut out = Vec::new();
    for item in items {
        let serde_json::Value::String(text) = item else {
            return Err("terminal argv JSON values must be strings".into());
        };
        out.push(text);
    }
    Ok(out)
}

fn parse_quoted_argv(raw: &str) -> Result<Vec<String>, String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escape = false;
    let mut started = false;
    for ch in chars {
        if escape {
            current.push(ch);
            escape = false;
            started = true;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
            started = true;
            continue;
        }
        if !quoted && is_shell_meta(ch) {
            return Err(format!("unquoted shell character {ch}"));
        }
        if !quoted && ch.is_whitespace() {
            if started {
                args.push(std::mem::take(&mut current));
                started = false;
            }
            continue;
        }
        current.push(ch);
        started = true;
    }
    if escape || quoted {
        return Err("unclosed terminal argv quote".into());
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

enum Visibility {
    Show,
    Skip,
    Unavailable(String),
}

fn desktop_visibility(
    fields: &std::collections::BTreeMap<String, String>,
    current: &[String],
) -> Visibility {
    if let Some(not_show) = fields.get("NotShowIn") {
        if list_hits(not_show, current) {
            return Visibility::Skip;
        }
    }
    if let Some(only) = fields.get("OnlyShowIn") {
        if only.trim().is_empty() {
            return Visibility::Show;
        }
        if current.is_empty() {
            return Visibility::Unavailable(
                "OnlyShowIn is set and XDG_CURRENT_DESKTOP is unset, so this entry is not opened."
                    .into(),
            );
        }
        if !list_hits(only, current) {
            return Visibility::Skip;
        }
    }
    Visibility::Show
}

fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.to_ascii_lowercase())
        .collect()
}

fn list_hits(list: &str, current: &[String]) -> bool {
    list.split(';').map(str::trim).any(|name| {
        !name.is_empty()
            && current
                .iter()
                .any(|desktop| desktop == &name.to_ascii_lowercase())
    })
}

fn is_shell_meta(ch: char) -> bool {
    matches!(
        ch,
        ';' | '|' | '&' | '<' | '>' | '$' | '`' | '(' | ')' | '\n'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn exec_quotes_field_codes_and_refuses_shell() {
        let args = parse_exec(
            r#""/opt/My App/demo" "a;b$(reboot)" %u %% %c %k"#,
            "Demo",
            "/tmp/demo.desktop",
            "",
        )
        .unwrap();
        assert_eq!(args.input_at, Some(2));
        assert_eq!(
            args.argv,
            vec![
                "/opt/My App/demo".to_string(),
                "a;b$(reboot)".to_string(),
                "%".to_string(),
                "Demo".to_string(),
                "/tmp/demo.desktop".to_string(),
            ]
        );
        assert!(parse_exec("demo a;rm", "Demo", "x", "").is_err());
        assert!(parse_exec("demo $(reboot)", "Demo", "x", "").is_err());
        let list = parse_exec("app %F", "App", "x", "").unwrap();
        assert!(list.unsupported_list);
        assert!(list.input_at.is_none());
        let quoted_code = parse_exec("demo \"%f\"", "Demo", "x", "").unwrap();
        assert_eq!(quoted_code.argv, vec!["demo".to_string(), "%f".to_string()]);
        assert!(quoted_code.input_at.is_none());
        let dropped = parse_exec("firefox %u", "Firefox", "x", "").unwrap();
        assert_eq!(dropped.argv, vec!["firefox".to_string()]);
        assert_eq!(dropped.input_at, Some(1));
        let icon = parse_exec("demo %i", "Demo", "x", "demo-icon").unwrap();
        assert!(icon.input_at.is_none());
        assert_eq!(
            icon.argv,
            vec![
                "demo".to_string(),
                "--icon".to_string(),
                "demo-icon".to_string()
            ]
        );
    }

    #[test]
    fn scan_skips_malformed_hidden_and_unreadable() {
        let dir = std::env::temp_dir().join(format!("omr-desktop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("good.desktop"),
            "[Desktop Entry]\nType=Application\nName=Good\nExec=good %f\nComment=local\n",
        )
        .unwrap();
        fs::write(
            dir.join("bad.desktop"),
            "[Desktop Entry]\nType=Application\nName=Bad\nExec=bad a;rm\n",
        )
        .unwrap();
        fs::write(
            dir.join("hidden.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("term.desktop"),
            "[Desktop Entry]\nType=Application\nName=Term\nExec=term\nTerminal=true\n",
        )
        .unwrap();
        let missing = dir.join("missing-try.desktop");
        fs::write(
            &missing,
            "[Desktop Entry]\nType=Application\nName=Missing\nExec=missing\nTryExec=/no/such/omarchy-bin\n",
        )
        .unwrap();
        let locked = dir.join("locked.desktop");
        fs::write(&locked, "[Desktop Entry]\nName=Locked\nExec=locked\n").unwrap();
        let mut perms = fs::metadata(&locked).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&locked, perms).unwrap();
        let scan = scan_app_dirs(std::slice::from_ref(&dir));
        let _ = fs::set_permissions(&locked, {
            let mut restored = fs::metadata(&locked).unwrap().permissions();
            restored.set_mode(0o644);
            restored
        });
        assert!(scan
            .apps
            .iter()
            .any(|app| app.name == "Good" && app.argv == vec!["good"] && app.input_at == Some(1)));
        assert!(scan
            .apps
            .iter()
            .any(|app| app.name == "Term" && app.unavailable.is_some() && app.argv.is_empty()));
        assert!(scan
            .apps
            .iter()
            .any(|app| app.name == "Missing" && app.unavailable.is_some()));
        assert!(!scan
            .apps
            .iter()
            .any(|app| app.name == "Bad" || app.name == "Hidden"));
        assert!(scan.malformed >= 1);
        assert!(scan.unreadable >= 1);
        assert!(scan.skipped >= 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn terminal_argv_is_prefixed_and_shell_text_is_refused() {
        let dir = std::env::temp_dir().join(format!("omr-term-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("term.desktop"),
            "[Desktop Entry]\nType=Application\nName=Term App\nExec=editor %u\nTerminal=true\n",
        )
        .unwrap();
        let plain = scan_app_dirs(std::slice::from_ref(&dir));
        let blocked = plain
            .apps
            .iter()
            .find(|app| app.name == "Term App")
            .unwrap();
        assert!(blocked.unavailable.is_some());
        assert!(blocked.argv.is_empty());
        let scan = scan_app_dirs_with(std::slice::from_ref(&dir), &["ghostty".into(), "-e".into()]);
        let app = scan.apps.iter().find(|app| app.name == "Term App").unwrap();
        assert!(app.unavailable.is_none());
        assert_eq!(
            app.argv,
            vec![
                "ghostty".to_string(),
                "-e".to_string(),
                "editor".to_string()
            ]
        );
        assert_eq!(app.input_at, Some(3));
        assert!(parse_terminal_argv("bash -c editor").is_err());
        assert!(parse_terminal_argv("ghostty -e $(id)").is_err());
        assert_eq!(
            parse_terminal_argv("ghostty -e").unwrap(),
            vec!["ghostty".to_string(), "-e".to_string()]
        );
        assert_eq!(
            parse_terminal_argv(r#"["/opt/My Term/ghostty","-e"]"#).unwrap(),
            vec!["/opt/My Term/ghostty".to_string(), "-e".to_string()]
        );
        assert_eq!(
            parse_terminal_argv(r#"ghostty -e "/opt/My Term/bin""#).unwrap(),
            vec![
                "ghostty".to_string(),
                "-e".to_string(),
                "/opt/My Term/bin".to_string()
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discovery_does_not_execute_and_honors_type_and_desktop() {
        let dir = std::env::temp_dir().join(format!("omr-disc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("ran");
        let script = dir.join("touch-me");
        fs::write(
            &script,
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        )
        .unwrap();
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
        fs::write(
            dir.join("run.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Would Run\nExec={}\n",
                script.display()
            ),
        )
        .unwrap();
        fs::write(
            dir.join("hidden.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Hidden Run\nHidden=true\nExec={}\n",
                script.display()
            ),
        )
        .unwrap();
        fs::write(
            dir.join("link.desktop"),
            format!(
                "[Desktop Entry]\nType=Link\nName=Link\nURL=file://{}\n",
                script.display()
            ),
        )
        .unwrap();
        fs::write(
            dir.join("notype.desktop"),
            format!("[Desktop Entry]\nName=No Type\nExec={}\n", script.display()),
        )
        .unwrap();
        fs::write(
            dir.join("only.desktop"),
            "[Desktop Entry]\nType=Application\nName=Only Gnome\nExec=only\nOnlyShowIn=NotThisDesktopXYZ;\n",
        )
        .unwrap();
        fs::write(
            dir.join("quoted.desktop"),
            "[Desktop Entry]\nType=Application\nName=Quoted\nExec=demo \"%f\"\n",
        )
        .unwrap();
        let scan = scan_app_dirs(std::slice::from_ref(&dir));
        assert!(!marker.exists());
        assert!(scan.apps.iter().any(|app| app.name == "Would Run"));
        assert!(!scan
            .apps
            .iter()
            .any(|app| app.name == "Hidden Run" || app.name == "Link" || app.name == "No Type"));
        let only = scan.apps.iter().find(|app| app.name == "Only Gnome");
        let current = current_desktops();
        if current.iter().any(|name| name == "notthisdesktopxyz") {
            assert!(only.is_some());
        } else if current.is_empty() {
            assert!(only.unwrap().unavailable.is_some());
            assert!(only.unwrap().argv.is_empty());
        } else {
            assert!(only.is_none());
        }
        let quoted = scan.apps.iter().find(|app| app.name == "Quoted").unwrap();
        assert_eq!(quoted.argv, vec!["demo".to_string(), "%f".to_string()]);
        assert!(quoted.input_at.is_none());
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("OnlyShowIn".into(), "Hyprland".into());
        assert!(matches!(
            desktop_visibility(&fields, &[]),
            Visibility::Unavailable(_)
        ));
        assert!(matches!(
            desktop_visibility(&fields, &["gnome".into()]),
            Visibility::Skip
        ));
        assert!(matches!(
            desktop_visibility(&fields, &["hyprland".into()]),
            Visibility::Show
        ));
        fields.insert("NotShowIn".into(), "Hyprland".into());
        assert!(matches!(
            desktop_visibility(&fields, &["hyprland".into()]),
            Visibility::Skip
        ));
        let _ = fs::remove_dir_all(&dir);
    }
}
