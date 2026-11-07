//! Contract search ranking. Moved out of `model` so theme and provider work
//! do not edit the same file. Scores match MenuModel.searchScore.

use crate::model::{item, Item, Kind, Menu};

fn searchable_token(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch == '.' || ch == '_' || ch == '-' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn leaf_id(id: &str) -> &str {
    id.rsplit('.').next().unwrap_or(id)
}

fn ancestor_labels(menu: &Menu, entry: &Item) -> String {
    let mut labels = Vec::new();
    let mut current = item(menu, &entry.parent);
    let mut guard = 0;
    while let Some(parent) = current {
        if parent.id == "root" || guard >= 16 {
            break;
        }
        labels.push(parent.label.clone());
        current = item(menu, &parent.parent);
        guard += 1;
    }
    labels.reverse();
    labels.join(" ")
}

fn name_search_text(menu: &Menu, entry: &Item) -> String {
    let mut aliases = String::new();
    for value in entry.aliases.iter().chain(entry.search_terms.iter()) {
        if !aliases.is_empty() {
            aliases.push(' ');
        }
        aliases.push_str(&searchable_token(value));
    }
    // Typed "Omarchi" is not a stock alias. It only helps find the existing leaf.
    // "omarchi" is a typed alias for the existing leaf only. The reversed
    // phrase is included so "Omarchy update" is one searchable string.
    let typed = if entry.id == "update.omarchy" {
        "omarchi omarchy update"
    } else {
        ""
    };
    format!(
        "{} {} {} {} {}",
        entry.label,
        searchable_token(leaf_id(&entry.id)),
        aliases,
        ancestor_labels(menu, entry),
        typed
    )
    .to_lowercase()
}

fn term_in_words(term: &str, text: &str) -> bool {
    text.to_lowercase().split_whitespace().any(|w| w == term)
}

fn depth_for(menu: &Menu, id: &str) -> usize {
    let mut depth = 0;
    let mut current = item(menu, id);
    let mut guard = 0;
    while let Some(entry) = current {
        if entry.parent.is_empty() || entry.parent == "root" || guard >= 32 {
            break;
        }
        depth += 1;
        current = item(menu, &entry.parent);
        guard += 1;
    }
    depth
}

pub fn matches_query(menu: &Menu, entry: &Item, query: &str, visible: bool) -> bool {
    if entry.id == "root" || !visible {
        return false;
    }
    let name = name_search_text(menu, entry);
    let description = entry.description.to_lowercase();
    for term in query.to_lowercase().split_whitespace() {
        if term.is_empty() {
            continue;
        }
        if name.contains(term) {
            continue;
        }
        if term_in_words(term, &description) {
            continue;
        }
        return false;
    }
    true
}

/// Lower is a better match. Same arithmetic as `MenuModel.searchScore`.
pub fn search_score(menu: &Menu, entry: &Item, query: &str) -> i64 {
    let needle = query.to_lowercase();
    let needle = needle.trim();
    let label = entry.label.to_lowercase();
    let name = name_search_text(menu, entry);
    let description = entry.description.to_lowercase();
    let mut score: i64 = 80;
    if label == needle {
        score = if entry.parent == "root" { 2 } else { 0 };
    } else if label.starts_with(needle) {
        score = 10;
    } else if label.contains(needle) {
        score = 30;
    } else if name.contains(needle) {
        score = 40;
    } else if !needle.is_empty()
        && description
            .split_whitespace()
            .all(|w| term_in_words(w, &description))
        && needle
            .split_whitespace()
            .all(|t| term_in_words(t, &description))
    {
        score = 60;
    }
    if entry.kind == Kind::Menu || entry.kind == Kind::Link {
        score -= 2;
    }
    score * 1000 + depth_for(menu, &entry.id) as i64 * 25 + entry.order as i64
}

/// Subsequence match used only when the contract matcher returns nothing.
pub fn fuzzy_hit(menu: &Menu, entry: &Item, query: &str) -> bool {
    if entry.id == "root" {
        return false;
    }
    let mut hay = name_search_text(menu, entry);
    hay.retain(|c| !c.is_whitespace());
    let q: String = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if q.is_empty() {
        return false;
    }
    let mut rest = hay.as_str();
    for ch in q.chars() {
        let Some(idx) = rest.find(ch) else {
            return false;
        };
        rest = &rest[idx + ch.len_utf8()..];
    }
    true
}
