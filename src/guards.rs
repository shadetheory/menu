//! Guard checks for menu `when`, `checked`, and `disabled`.
//!
//! Nothing is spawned unless the expression is an exact opted-in grant.
//! The literals `true` and `false` are decided in this process. `--no-exec`,
//! `--live`, and `--exec` do not grant guard execution. Search text is not a guard.

use crate::model::Item;
use std::collections::BTreeMap;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardFacts {
    pub when_results: BTreeMap<String, bool>,
    pub disabled: BTreeMap<String, bool>,
    pub checked: BTreeMap<String, bool>,
    /// Fail closed. The row stays listed with this reason and must not run.
    pub blocks: BTreeMap<String, String>,
    pub skipped_shell: usize,
    pub errors: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuardGrant {
    /// Exact argument vectors. A joined command line is not a grant.
    pub argv: std::collections::BTreeSet<Vec<String>>,
    /// Exact shell expressions. Separate from argv grants.
    pub shell: std::collections::BTreeSet<String>,
}

impl GuardGrant {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.argv.is_empty() && self.shell.is_empty()
    }
}

pub fn evaluate_items(items: &[Item]) -> GuardFacts {
    evaluate_items_allowed(items, &GuardGrant::default())
}

/// `allow` is an explicit grant. An empty grant spawns nothing.
pub fn evaluate_items_allowed(items: &[Item], allow: &GuardGrant) -> GuardFacts {
    let mut facts = GuardFacts {
        when_results: BTreeMap::new(),
        disabled: BTreeMap::new(),
        checked: BTreeMap::new(),
        blocks: BTreeMap::new(),
        skipped_shell: 0,
        errors: 0,
    };
    let mut runs = 0usize;
    for item in items {
        apply_one(
            &mut facts,
            &mut runs,
            &item.id,
            &item.when_guard,
            Slot::When,
            allow,
        );
        apply_one(
            &mut facts,
            &mut runs,
            &item.id,
            &item.disabled,
            Slot::Disabled,
            allow,
        );
        apply_one(
            &mut facts,
            &mut runs,
            &item.id,
            &item.checked,
            Slot::Checked,
            allow,
        );
    }
    facts
}

enum Slot {
    When,
    Disabled,
    Checked,
}

const MAX_ALLOW_LINES: usize = 32;
const MAX_ALLOW_CHARS: usize = 256;
const MAX_GUARD_RUNS: usize = 48;

pub fn parse_allow(text: &str) -> GuardGrant {
    let mut grant = GuardGrant::default();
    let mut count = 0usize;
    for line in text.lines() {
        if count >= MAX_ALLOW_LINES {
            break;
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.len() > MAX_ALLOW_CHARS {
            continue;
        }
        count += 1;
        if let Some(rest) = line.strip_prefix("argv ") {
            if let Some(argv) = json_string_array(rest.trim()) {
                grant.argv.insert(argv);
            }
        } else if let Some(rest) = line.strip_prefix("shell ") {
            if !rest.is_empty() {
                grant.shell.insert(rest.to_string());
            }
        }
    }
    grant
}

fn json_string_array(raw: &str) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let serde_json::Value::Array(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items {
        let serde_json::Value::String(text) = item else {
            return None;
        };
        if text.is_empty() || text.len() > MAX_ALLOW_CHARS {
            return None;
        }
        out.push(text);
    }
    if out.is_empty() || out.len() > 8 {
        return None;
    }
    Some(out)
}

fn apply_one(
    facts: &mut GuardFacts,
    runs: &mut usize,
    id: &str,
    expr: &str,
    slot: Slot,
    allow: &GuardGrant,
) {
    let expr = expr.trim();
    if expr.is_empty() {
        return;
    }
    if let Some(value) = literal_bool(expr) {
        record(facts, id, slot, value);
        return;
    }
    if *runs >= MAX_GUARD_RUNS {
        facts.blocks.insert(
            id.to_string(),
            "Guard evaluation limit reached. This was not run and is not treated as success."
                .into(),
        );
        if matches!(slot, Slot::Disabled | Slot::When) {
            facts.disabled.insert(id.to_string(), true);
        }
        return;
    }
    match classify_guard(expr) {
        GuardClass::Shell if allow.shell.contains(expr) => {
            *runs += 1;
            match run_argv(&["bash".into(), "-c".into(), expr.to_string()]) {
                Ok(value) => {
                    let map = match slot {
                        Slot::When => &mut facts.when_results,
                        Slot::Disabled => &mut facts.disabled,
                        Slot::Checked => &mut facts.checked,
                    };
                    map.insert(id.to_string(), value);
                }
                Err(reason) => {
                    facts.errors += 1;
                    facts.blocks.insert(
                    id.to_string(),
                    format!("Allow-listed guard did not finish ({reason}). This is not treated as success."),
                );
                }
            }
        }
        GuardClass::Shell => {
            facts.skipped_shell += 1;
            facts.blocks.insert(
                id.to_string(),
                "Shell guard was not run. No exact shell grant matches it. Nothing will start."
                    .into(),
            );
            if matches!(slot, Slot::Disabled) {
                facts.disabled.insert(id.to_string(), true);
            }
        }
        GuardClass::Argv(argv) if allow.argv.contains(&argv) => {
            *runs += 1;
            match run_argv(&argv) {
                Ok(value) => {
                    let map = match slot {
                        Slot::When => &mut facts.when_results,
                        Slot::Disabled => &mut facts.disabled,
                        Slot::Checked => &mut facts.checked,
                    };
                    map.insert(id.to_string(), value);
                }
                Err(reason) => {
                    facts.errors += 1;
                    facts.blocks.insert(
                        id.to_string(),
                        format!("Guard did not finish ({reason}). This is not treated as success."),
                    );
                    if matches!(slot, Slot::Disabled | Slot::When) {
                        facts.disabled.insert(id.to_string(), true);
                    }
                }
            }
        }
        GuardClass::Argv(_) => {
            facts.blocks.insert(
                id.to_string(),
                "Argv guard was not run. No exact argument-vector grant matches it. Nothing will start.".into(),
            );
            if matches!(slot, Slot::Disabled | Slot::When) {
                facts.disabled.insert(id.to_string(), true);
            }
        }
    }
}

fn literal_bool(expr: &str) -> Option<bool> {
    match expr {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn record(facts: &mut GuardFacts, id: &str, slot: Slot, value: bool) {
    let map = match slot {
        Slot::When => &mut facts.when_results,
        Slot::Disabled => &mut facts.disabled,
        Slot::Checked => &mut facts.checked,
    };
    map.insert(id.to_string(), value);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardClass {
    Argv(Vec<String>),
    Shell,
}

pub fn classify_guard(expr: &str) -> GuardClass {
    let expr = expr.trim();
    if expr.is_empty()
        || expr.contains(|ch: char| {
            matches!(
                ch,
                '|' | ';'
                    | '&'
                    | '$'
                    | '`'
                    | '<'
                    | '>'
                    | '\n'
                    | '('
                    | ')'
                    | '*'
                    | '?'
                    | '\\'
                    | '"'
                    | '\''
                    | '!'
                    | '{'
                    | '}'
                    | '['
                    | ']'
            )
        })
    {
        return GuardClass::Shell;
    }
    let parts: Vec<String> = expr.split_whitespace().map(str::to_string).collect();
    if parts.is_empty()
        || parts.len() > 8
        || parts.iter().any(|part| part.len() > MAX_ALLOW_CHARS)
        || matches!(parts[0].as_str(), "bash" | "sh" | "dash" | "zsh" | "env")
    {
        return GuardClass::Shell;
    }
    if parts[0].contains('/') && !executable_file(Path::new(&parts[0])) {
        // Still an argv shape. The runner reports the permission or missing file.
    }
    GuardClass::Argv(parts)
}

fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn run_argv(argv: &[String]) -> Result<bool, String> {
    if argv.is_empty() {
        return Err("empty".into());
    }
    if argv[0].contains('/') && !executable_file(Path::new(&argv[0])) {
        return Err("not executable".into());
    }
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|err| err.to_string())?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) if start.elapsed() > Duration::from_millis(400) => {
                reap_group(&mut child);
                return Err("timed out".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(err) => return Err(err.to_string()),
        }
    }
}

fn reap_group(child: &mut std::process::Child) {
    let pid = child.id() as i32;
    unsafe {
        kill(-pid, 9);
    }
    let _ = child.wait();
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{is_disabled, is_visible, merge_menu_sources, parse_menu_jsonc};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn true_and_false_guards_and_shell_is_not_run() {
        let marker = std::env::temp_dir().join(format!("omr-guard-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let shell = format!("true; touch {}", marker.display());
        let items = parse_menu_jsonc(&format!(
            r#"{{
              "shown": {{"label": "Shown", "when": "true", "action": "true"}},
              "hidden": {{"label": "Hidden", "when": "false", "action": "true"}},
              "shell": {{"label": "Shell", "when": {shell:?}, "action": "true"}},
              "off": {{"label": "Off", "disabled": "true", "action": "true"}},
              "on": {{"label": "On", "checked": "true", "action": "true"}}
            }}"#
        ));
        let menu = merge_menu_sources(&items, &[]);
        let facts = evaluate_items(&items);
        assert!(is_visible(
            &menu,
            &facts.when_results,
            &menu.items["shown"],
            0
        ));
        assert!(!is_visible(
            &menu,
            &facts.when_results,
            &menu.items["hidden"],
            0
        ));
        assert!(is_visible(
            &menu,
            &facts.when_results,
            &menu.items["shell"],
            0
        ));
        assert!(facts.skipped_shell >= 1);
        assert!(facts.blocks["shell"].contains("not run"));
        assert!(!marker.exists());
        assert!(is_disabled(&facts.disabled, &menu.items["off"]));
        assert_eq!(facts.checked.get("on"), Some(&true));
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn unexecutable_guard_does_not_hide_the_row() {
        let dir = std::env::temp_dir().join(format!("omr-guard-perm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("not-exec");
        std::fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = std::fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&bin, perms).unwrap();
        let items = parse_menu_jsonc(&format!(
            r#"{{"row": {{"label": "Row", "when": "{}", "action": "true"}}}}"#,
            bin.display()
        ));
        let menu = merge_menu_sources(&items, &[]);
        let facts = evaluate_items(&items);
        assert!(facts.blocks["row"].contains("not run"));
        assert!(!facts.when_results.contains_key("row"));
        assert!(is_visible(
            &menu,
            &facts.when_results,
            &menu.items["row"],
            0
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timeout_does_not_count_as_success_or_leave_a_marker() {
        let dir = std::env::temp_dir().join(format!("omr-guard-timeout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("late");
        let bin = dir.join("slow");
        std::fs::write(
            &bin,
            format!("#!/bin/sh\nsleep 2\ntouch '{}'\n", marker.display()),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin, perms).unwrap();
        let items = parse_menu_jsonc(&format!(
            r#"{{"row": {{"label": "Slow", "when": "{}", "action": "true"}}}}"#,
            bin.display()
        ));
        let facts = evaluate_items(&items);
        assert!(!facts.when_results.contains_key("row"));
        assert!(facts.blocks["row"].contains("not run"));
        assert!(!facts.blocks["row"].to_lowercase().contains("success"));
        assert!(!marker.exists());
        let mut grant = GuardGrant::default();
        grant.argv.insert(vec![bin.to_string_lossy().into_owned()]);
        let granted = evaluate_items_allowed(&items, &grant);
        assert!(granted.blocks["row"].contains("timed out"));
        assert!(!marker.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn allow_file_is_exact_and_does_not_treat_a_star_as_a_pattern() {
        let marker = std::env::temp_dir().join(format!("omr-allow-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let blocked = format!("true; touch {}", marker.display());
        let items = parse_menu_jsonc(&format!(
            r#"{{
              "ok": {{"label": "Ok", "when": "true && true", "action": "true"}},
              "no": {{"label": "No", "when": {blocked:?}, "action": "true"}}
            }}"#
        ));
        let allow = parse_allow("shell true && true\ntrue*\nargv /tmp/evil touch\n# comment\n");
        assert!(allow.shell.contains("true && true"));
        assert!(allow.argv.is_empty());
        assert!(!allow.shell.contains(&blocked));
        assert!(!allow.shell.contains("true*"));
        let facts = evaluate_items_allowed(&items, &allow);
        assert_eq!(facts.when_results.get("ok"), Some(&true));
        assert!(facts.blocks["no"].contains("not run"));
        assert!(!marker.exists());
        let prefix = "true && true; touch /no/such".to_string();
        assert!(!allow.shell.contains(&prefix));
        let huge = "x".repeat(300);
        let bounded = parse_allow(&format!("shell {huge}\nshell true && true\n"));
        assert!(!bounded.shell.iter().any(|line| line.len() > 256));
        assert!(bounded.shell.contains("true && true"));
        let vector = parse_allow(r#"argv ["/opt/My App/bin","--flag"]"#);
        assert!(vector
            .argv
            .contains(&vec!["/opt/My App/bin".to_string(), "--flag".to_string()]));
        assert!(!vector
            .argv
            .contains(&vec!["/opt/My App/bin --flag".to_string()]));
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn startup_does_not_spawn_an_argv_guard_without_an_exact_vector_grant() {
        let dir = std::env::temp_dir().join(format!("omr-guard-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("pwned");
        let bin = dir.join("touch-marker");
        std::fs::write(&bin, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        let mut perms = std::fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin, perms).unwrap();
        let path = bin.to_string_lossy().into_owned();
        let items = parse_menu_jsonc(&format!(
            r#"{{"row": {{"label": "Row", "when": "{path}", "action": "true"}}}}"#
        ));
        let denied = evaluate_items(&items);
        assert!(denied.blocks["row"].contains("not run"));
        assert!(!marker.exists());
        let joined = parse_allow(&format!("argv {path}\n"));
        assert!(joined.argv.is_empty());
        let still = evaluate_items_allowed(&items, &joined);
        assert!(still.blocks["row"].contains("not run"));
        assert!(!marker.exists());
        let mut grant = GuardGrant::default();
        grant.argv.insert(vec![path]);
        let ran = evaluate_items_allowed(&items, &grant);
        assert!(ran.when_results.get("row") == Some(&true));
        assert!(marker.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
