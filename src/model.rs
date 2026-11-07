//! Menu model aligned with Omarchy `shell/plugins/menu/MenuModel.js`
//! at commit 14abc0762ce38547d1912061969df58962f6f10a.
//!
//! Field overlay matches that file: `normalizeItem` fills every key, and
//! `mergeMenuSources` copies every key, so a partial user entry clears
//! omitted fields (including `action`). `docs/menu.md` describes a sparser
//! overlay. This crate follows the script, and records the mismatch.
//!
//! `when`, `checked`, and `disabled` are stored as text. They are not
//! evaluated here. Search scoring lives in `ranking` and is re-exported.

use serde_json::Value;
use std::collections::BTreeMap;

pub use crate::ranking::{fuzzy_hit, matches_query, search_score};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Action,
    Link,
    Menu,
    /// In-process sample panel. Not an upstream kind. Never dispatched.
    Panel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub parent: String,
    pub kind: Kind,
    pub icon: String,
    pub icon_font: String,
    pub label: String,
    pub title: String,
    pub target: String,
    pub description: String,
    pub action: String,
    pub provider: String,
    pub aliases: Vec<String>,
    pub search_terms: Vec<String>,
    pub when_guard: String,
    pub checked: String,
    pub disabled: String,
    pub order: usize,
    pub panel: String,
}

#[derive(Clone, Debug)]
pub struct Menu {
    pub items: BTreeMap<String, Item>,
    pub order: Vec<String>,
}

pub fn strip_jsonc(raw: &str) -> String {
    let mut kept = String::new();
    for line in raw.split_inclusive('\n') {
        let (body, nl) = line.split_once('\n').map_or((line, ""), |(b, _)| (b, "\n"));
        let body = body.trim_end_matches('\r');
        if body.trim_start().starts_with("//") {
            kept.push_str(nl);
        } else {
            kept.push_str(body);
            kept.push_str(nl);
        }
    }
    strip_trailing_commas(&kept)
}

fn strip_trailing_commas(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                i += 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn aliases_of(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        Value::String(s) if !s.is_empty() => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn str_field(obj: &serde_json::Map<String, Value>, key: &str) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn normalize_item(id: &str, raw: &Value) -> Item {
    let obj = raw.as_object();
    let blank = serde_json::Map::new();
    let obj = obj.unwrap_or(&blank);
    let aliases = obj.get("aliases").map(aliases_of).unwrap_or_default();
    let search_terms = obj.get("searchTerms").map(aliases_of).unwrap_or_default();
    let mut parent =
        if obj.get("parent").map(|v| v.is_null()).unwrap_or(true) && !obj.contains_key("parent") {
            if id.contains('.') {
                id.rsplit_once('.')
                    .map(|(h, _)| h.to_string())
                    .unwrap_or_default()
            } else if id == "root" {
                String::new()
            } else {
                "root".to_string()
            }
        } else {
            str_field(obj, "parent")
        };
    if id == "root" {
        parent.clear();
    }
    let action = str_field(obj, "action");
    let target = str_field(obj, "target");
    let panel = str_field(obj, "panel");
    let kind = if !panel.is_empty() {
        Kind::Panel
    } else if !action.is_empty() {
        Kind::Action
    } else if !target.is_empty() {
        Kind::Link
    } else {
        Kind::Menu
    };
    Item {
        id: id.to_string(),
        parent,
        kind,
        icon: str_field(obj, "icon"),
        icon_font: str_field(obj, "iconFont"),
        label: {
            let label = str_field(obj, "label");
            if label.is_empty() {
                id.to_string()
            } else {
                label
            }
        },
        title: str_field(obj, "title"),
        target,
        description: str_field(obj, "description"),
        action: if panel.is_empty() {
            action
        } else {
            String::new()
        },
        provider: str_field(obj, "provider"),
        aliases,
        search_terms,
        when_guard: str_field(obj, "when"),
        checked: str_field(obj, "checked"),
        disabled: str_field(obj, "disabled"),
        order: 0,
        panel,
    }
}

/// `None` when the text is not a menu object. Comments-only is an empty menu.
pub fn read_menu_jsonc(raw: &str) -> Option<Vec<Item>> {
    let stripped = strip_jsonc(raw);
    if stripped.trim().is_empty() {
        return Some(Vec::new());
    }
    let parsed: Value = serde_json::from_str(&stripped).ok()?;
    let Value::Object(map) = parsed else {
        return None;
    };
    let source = match map.get("items") {
        Some(Value::Object(items)) => items,
        _ => &map,
    };
    let mut out = Vec::new();
    for (id, entry) in source {
        if entry.is_object() {
            out.push(normalize_item(id, entry));
        }
    }
    Some(out)
}

pub fn parse_menu_jsonc(raw: &str) -> Vec<Item> {
    read_menu_jsonc(raw).unwrap_or_default()
}

pub fn keep_menu_rows(previous: &[Item], raw: &str, required: bool) -> Vec<Item> {
    match read_menu_jsonc(raw) {
        Some(items) if !(required && items.is_empty()) => items,
        _ => previous.to_vec(),
    }
}

pub fn merge_menu_sources(default_items: &[Item], user_items: &[Item]) -> Menu {
    let mut items: BTreeMap<String, Item> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for src in [default_items, user_items] {
        for entry in src {
            if entry.id.is_empty() {
                continue;
            }
            if !items.contains_key(&entry.id) {
                order.push(entry.id.clone());
            }
            // MenuModel.js copies every normalized key, so omitted fields arrive empty.
            let mut merged = entry.clone();
            merged.id = entry.id.clone();
            items.insert(entry.id.clone(), merged);
        }
    }
    if !items.contains_key("root") {
        items.insert(
            "root".into(),
            Item {
                id: "root".into(),
                parent: String::new(),
                kind: Kind::Menu,
                icon: String::new(),
                icon_font: String::new(),
                label: "Go".into(),
                title: String::new(),
                target: String::new(),
                description: String::new(),
                action: String::new(),
                provider: String::new(),
                aliases: Vec::new(),
                search_terms: Vec::new(),
                when_guard: String::new(),
                checked: String::new(),
                disabled: String::new(),
                order: 0,
                panel: String::new(),
            },
        );
        order.insert(0, "root".into());
    }
    for (i, id) in order.iter().enumerate() {
        if let Some(item) = items.get_mut(id) {
            item.order = i;
        }
    }
    Menu { items, order }
}

pub fn item<'a>(menu: &'a Menu, id: &str) -> Option<&'a Item> {
    menu.items.get(id)
}

fn norm_route(input: &str) -> String {
    input.to_lowercase().replace('_', "-")
}

pub fn resolve_route(menu: &Menu, input: &str) -> String {
    let raw = norm_route(input);
    if raw.is_empty() || raw == "go" || raw == "menu" {
        return "root".into();
    }
    if menu.items.contains_key(&raw) {
        return raw;
    }
    for id in &menu.order {
        let Some(entry) = menu.items.get(id) else {
            continue;
        };
        if entry.kind == Kind::Panel {
            continue;
        }
        // App rows are not in this scaffold. Aliases on real entries are routable.
        for alias in &entry.aliases {
            if norm_route(alias) == raw {
                return entry.id.clone();
            }
        }
    }
    raw
}

pub fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "item".into()
    } else {
        out
    }
}

pub fn path_for(menu: &Menu, id: &str) -> String {
    let mut labels = Vec::new();
    let mut current = item(menu, id);
    let mut guard = 0;
    while let Some(entry) = current {
        if entry.id == "root" || guard >= 32 {
            break;
        }
        labels.push(entry.label.clone());
        current = item(menu, &entry.parent);
        guard += 1;
    }
    labels.reverse();
    labels.join(" › ")
}

/// Visibility from already-parsed guard results.
///
/// `when` is not evaluated here and this function does not start a shell.
/// A missing `when` result does not hide the row. Only an explicit `false`
/// in `when_results` hides it.
pub fn is_visible(
    menu: &Menu,
    when_results: &BTreeMap<String, bool>,
    entry: &Item,
    depth: usize,
) -> bool {
    // Missing results do not hide. Only an explicit false does.
    if !entry.when_guard.is_empty() && when_results.get(&entry.id) == Some(&false) {
        return false;
    }
    if entry.kind != Kind::Menu && entry.kind != Kind::Link {
        return true;
    }
    if !entry.provider.is_empty() {
        return true;
    }
    if depth >= 32 {
        return false;
    }
    let target = if entry.kind == Kind::Link {
        entry.target.as_str()
    } else {
        entry.id.as_str()
    };
    for id in &menu.order {
        if let Some(child) = item(menu, id) {
            if child.parent == target && is_visible(menu, when_results, child, depth + 1) {
                return true;
            }
        }
    }
    false
}

pub fn is_disabled(disabled_true: &BTreeMap<String, bool>, entry: &Item) -> bool {
    !entry.disabled.is_empty() && disabled_true.get(&entry.id).copied().unwrap_or(false)
}

pub fn label_for(entry: &Item, checked: bool, disabled: bool) -> String {
    if checked || disabled {
        format!("{} ✓", entry.label)
    } else {
        entry.label.clone()
    }
}

pub fn stock_menu() -> Menu {
    let raw = include_str!("../fixtures/upstream/omarchy-menu.jsonc");
    let samples = include_str!("../config/samples.jsonc");
    merge_menu_sources(&parse_menu_jsonc(raw), &parse_menu_jsonc(samples))
}

/// Stock entries, the example panels, and the seed bot roster.
pub fn full_menu() -> Menu {
    let base = stock_menu();
    let stock: Vec<Item> = base
        .order
        .iter()
        .filter_map(|id| base.items.get(id))
        .filter(|item| item.id != "root")
        .cloned()
        .collect();
    let mut extra = crate::bots::bot_menu_items(&crate::bots::embedded_bots());
    extra.push(local_commands_item());
    merge_menu_sources(&stock, &extra)
}

fn local_commands_item() -> Item {
    Item {
        id: "local.commands".into(),
        parent: "root".into(),
        kind: Kind::Menu,
        icon: String::new(),
        icon_font: String::new(),
        label: "Commands".into(),
        title: String::new(),
        target: String::new(),
        description: "omarchy-* programs on PATH. Not a shell.".into(),
        action: String::new(),
        provider: "commands".into(),
        aliases: vec!["commands".into()],
        search_terms: vec!["omarchy".into()],
        when_guard: String::new(),
        checked: String::new(),
        disabled: String::new(),
        order: 0,
        panel: String::new(),
    }
}

pub const UPSTREAM_COMMIT: &str = include_str!("../fixtures/upstream/COMMIT");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc_comments_and_trailing_commas() {
        let items = parse_menu_jsonc(
            "{\n  // comment\n  \"a\": {\"label\": \"A\", \"action\": \"x\",},\n}\n",
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "a");
        assert_eq!(items[0].kind, Kind::Action);
        assert_eq!(items[0].label, "A");
    }

    #[test]
    fn partial_override_clears_omitted_fields_like_menu_model() {
        let defaults = parse_menu_jsonc(
            r#"{"style.theme":{"label":"Themes","action":"keep-me","aliases":["theme"]}}"#,
        );
        let user = parse_menu_jsonc(r#"{"style.theme":{"label":"Picker"}}"#);
        let merged = merge_menu_sources(&defaults, &user);
        let item = &merged.items["style.theme"];
        assert_eq!(item.label, "Picker");
        assert_eq!(item.action, "");
        assert_eq!(item.kind, Kind::Menu);
        assert!(item.aliases.is_empty());
    }

    #[test]
    fn routes_match_upstream_oracle() {
        let menu = stock_menu();
        assert_eq!(
            parse_menu_jsonc(include_str!("../fixtures/upstream/omarchy-menu.jsonc")).len(),
            427
        );
        assert_eq!(menu.order[0], "root");
        assert_eq!(
            &menu.order[1..10].join(","),
            "apps,learn,appearance,trigger,setup,install,remove,update,about"
        );
        assert_eq!(resolve_route(&menu, "system"), "system");
        assert_eq!(resolve_route(&menu, "power-menu"), "system");
        assert_eq!(resolve_route(&menu, "power_menu"), "system");
        assert_eq!(resolve_route(&menu, "style"), "appearance");
        assert_eq!(resolve_route(&menu, "toggle"), "trigger.toggle");
        assert_eq!(resolve_route(&menu, "appearance.theme"), "appearance.theme");
        assert_eq!(resolve_route(&menu, "screenrecord"), "trigger.capture");
        assert_eq!(resolve_route(&menu, ""), "root");
        assert_eq!(resolve_route(&menu, "go"), "root");
        assert_eq!(resolve_route(&menu, "no-such"), "no-such");
        assert_eq!(menu.items["about"].action, "omarchy-launch-about");
        assert_eq!(slugify("Power Saver!"), "power-saver");
        assert_eq!(
            path_for(&menu, "setup.default.browser.zen"),
            "Setup › Defaults › Browser › Zen"
        );
        let theme = &menu.items["appearance.theme"];
        assert_eq!(search_score(&menu, theme, "theme"), 52);
        assert!(!matches_query(&menu, theme, "colors", true));
        assert!(matches_query(&menu, theme, "theme", true));
    }

    #[test]
    fn disabled_rows_keep_a_check_mark() {
        let entry = normalize_item(
            "install.browser.zen",
            &serde_json::json!({"label":"Zen","disabled":"cmd","action":"install-zen"}),
        );
        let mut disabled = BTreeMap::new();
        disabled.insert(entry.id.clone(), true);
        assert!(is_disabled(&disabled, &entry));
        assert_eq!(label_for(&entry, false, true), "Zen ✓");
    }

    #[test]
    fn hidden_when_false_and_provider_stays() {
        let items = parse_menu_jsonc(
            r#"{
          "hardware": {"label": "Hardware"},
          "hardware.laptop": {"label": "Laptop", "when": "is-laptop", "action": "toggle-laptop"},
          "dynamic": {"label": "Dynamic", "provider": "fonts"}
        }"#,
        );
        let menu = merge_menu_sources(&items, &[]);
        let mut hidden = BTreeMap::new();
        hidden.insert("hardware.laptop".into(), false);
        assert!(!is_visible(&menu, &hidden, &menu.items["hardware"], 0));
        assert!(is_visible(
            &menu,
            &BTreeMap::new(),
            &menu.items["dynamic"],
            0
        ));
    }

    #[test]
    fn broken_extension_contributes_nothing() {
        let good = parse_menu_jsonc(r#"{"tools":{"label":"Tools","action":"true"}}"#);
        assert!(read_menu_jsonc("{").is_none());
        assert_eq!(keep_menu_rows(&good, "{", false).len(), 1);
        assert!(keep_menu_rows(&good, "// gone\n", false).is_empty());
    }

    #[test]
    fn query_is_not_a_route_command() {
        let menu = stock_menu();
        let nasty = "$(reboot); rm -rf /";
        let route = resolve_route(&menu, nasty);
        assert_eq!(route, nasty.to_lowercase().replace('_', "-"));
        assert!(!menu.items.contains_key(&route));
    }
}
