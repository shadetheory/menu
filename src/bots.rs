//! Seed bot roster. Launchers come only from configured argv lines.
//! An empty launcher is unavailable. Nothing here is contacted.

use crate::model::{Item, Kind};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Bot {
    pub id: String,
    pub name: String,
    pub argv: Vec<String>,
}

pub fn parse_bots(raw: &str) -> Vec<Bot> {
    let mut by_id: BTreeMap<String, Bot> = BTreeMap::new();
    let mut order = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let kind = parts.next().unwrap_or("");
        if kind == "bot" {
            let Some(id) = parts.next() else { continue };
            let name = parts.collect::<Vec<_>>().join(" ");
            if name.is_empty() {
                continue;
            }
            if !by_id.contains_key(id) {
                order.push(id.to_string());
            }
            by_id.insert(
                id.to_string(),
                Bot {
                    id: id.to_string(),
                    name,
                    argv: Vec::new(),
                },
            );
        } else if kind == "launch" {
            let Some(id) = parts.next() else { continue };
            let argv: Vec<String> = parts.map(str::to_string).collect();
            if let Some(bot) = by_id.get_mut(id) {
                bot.argv = argv;
            }
        }
    }
    order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect()
}

pub fn embedded_bots() -> Vec<Bot> {
    parse_bots(include_str!("../config/bots.txt"))
}

pub fn bot_menu_items(bots: &[Bot]) -> Vec<Item> {
    let mut items = vec![Item {
        id: "samples.bots".into(),
        parent: "samples".into(),
        kind: Kind::Menu,
        icon: String::new(),
        icon_font: String::new(),
        label: "Bots".into(),
        title: String::new(),
        target: String::new(),
        description: "EXAMPLE seed roster, not exhaustive".into(),
        action: String::new(),
        provider: String::new(),
        aliases: Vec::new(),
        search_terms: vec!["bots".into()],
        when_guard: String::new(),
        checked: String::new(),
        disabled: String::new(),
        order: 0,
        panel: String::new(),
    }];
    for bot in bots {
        let slug: String = bot
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let slug = slug.trim_matches('-').to_string();
        items.push(Item {
            id: format!("samples.bots.{slug}"),
            parent: "samples.bots".into(),
            kind: Kind::Panel,
            icon: String::new(),
            icon_font: String::new(),
            label: format!("Call {}", bot.name),
            title: String::new(),
            target: String::new(),
            description: "EXAMPLE unavailable until a launcher argv is configured".into(),
            action: String::new(),
            provider: String::new(),
            aliases: Vec::new(),
            search_terms: vec![bot.name.clone(), bot.id.clone()],
            when_guard: String::new(),
            checked: String::new(),
            disabled: String::new(),
            order: 0,
            panel: format!("bot:{}", bot.id),
        });
    }
    items
}

pub fn find<'a>(bots: &'a [Bot], id: &str) -> Option<&'a Bot> {
    bots.iter().find(|b| b.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_roster_is_synthetic_and_has_no_launchers() {
        let bots = embedded_bots();
        assert_eq!(bots.len(), 2);
        let first = find(&bots, "example-a").unwrap();
        assert_eq!(first.name, "Example A");
        assert!(first.argv.is_empty());
        let second = find(&bots, "example-b").unwrap();
        assert_eq!(second.name, "Example B");
        assert!(bots.iter().all(|bot| bot.argv.is_empty()));
        assert!(bots.iter().all(|bot| !bot.id.contains('-') || bot.id.starts_with("example-")));
    }
}
