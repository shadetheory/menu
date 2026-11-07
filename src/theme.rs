//! Palette tokens. Missing files keep the bundled snapshot.
//! `[font] base-size` is applied only when that key is present.
//! The window stays opaque. This module does not claim compositor blur.

use std::path::PathBuf;

/// One opaque sRGB channel triple. Alpha is not stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeTokens {
    pub bg: Rgb,
    pub fg: Rgb,
    pub accent: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub font_px: u32,
    pub card: Rgb,
    pub search_bg: Rgb,
    pub selection: Rgb,
    pub elevated: Rgb,
    pub warn: Rgb,
    pub font_family: String,
    pub font_file: Option<PathBuf>,
    pub font_fallback: bool,
    /// Where the tokens came from. Paint does not draw this string.
    pub source: String,
}

impl ThemeTokens {
    pub fn search_px(&self) -> f32 {
        self.font_px as f32
    }
    pub fn list_px(&self) -> f32 {
        (self.font_px as f32 * 0.9).max(12.0)
    }
    pub fn hint_px(&self) -> f32 {
        (self.font_px as f32 * 0.72).max(11.0)
    }
    pub fn header_px(&self) -> f32 {
        (self.font_px as f32 * 0.8).max(12.0)
    }
    pub fn body_px(&self) -> f32 {
        self.font_px as f32
    }
    /// Row pitch. Size 15 keeps the original 52px pitch so hit tests stay put.
    pub fn row_h(&self) -> f32 {
        52.0 * (self.font_px as f32) / 15.0
    }
}

/// Theme files. A missing path keeps the bundled value for that field.
#[derive(Clone, Debug, Default)]
pub struct ThemePaths {
    pub colors_toml: Option<PathBuf>,
    pub shell_toml: Option<PathBuf>,
    pub user_shell: Option<PathBuf>,
    pub menu_file: Option<PathBuf>,
    pub ghostty_theme: Option<PathBuf>,
    pub ghostty_user: Option<PathBuf>,
    pub font_file: Option<PathBuf>,
}

/// Halloween Hacker snapshot the window already paints.
///
/// `bg` `#050805`, `fg` `#6dff57`, `accent` `#ff6a00`. `text` `#e6f3e2` and
/// `muted` `#a9c2ad` are the body ink already used for labels. `font_px` is
/// 15. This function does not read disk.
pub fn bundled_snapshot() -> ThemeTokens {
    let mut theme = ThemeTokens {
        bg: Rgb::new(0x05, 0x08, 0x05),
        fg: Rgb::new(0x6d, 0xff, 0x57),
        accent: Rgb::new(0xff, 0x6a, 0x00),
        text: Rgb::new(0xe6, 0xf3, 0xe2),
        muted: Rgb::new(0xa9, 0xc2, 0xad),
        font_px: 15,
        card: Rgb::new(0, 0, 0),
        search_bg: Rgb::new(0, 0, 0),
        selection: Rgb::new(0, 0, 0),
        elevated: Rgb::new(0, 0, 0),
        warn: Rgb::new(0, 0, 0),
        font_family: "JetBrains Mono".into(),
        font_file: None,
        font_fallback: true,
        source: "Halloween Hacker".into(),
    };
    derive_surfaces(&mut theme);
    // Neon foreground stays the snapshot token. Body ink is the lighter pair
    // already used for labels, and the plate is a neutral dark so alpha does
    // not composite as a green slab.
    theme.text = Rgb::new(0xe6, 0xf3, 0xe2);
    theme.muted = Rgb::new(0xa9, 0xc2, 0xad);
    theme.card = Rgb::new(0x16, 0x1c, 0x16);
    theme.elevated = Rgb::new(0x2c, 0x36, 0x2c);
    theme.search_bg = theme.card;
    theme.selection = Rgb::new(0x2e, 0x2a, 0x20);
    theme.warn = theme.text;
    theme
}

pub fn default_paths() -> ThemePaths {
    let Ok(home) = std::env::var("HOME") else {
        return ThemePaths::default();
    };
    let home = PathBuf::from(home);
    let theme = home.join(".local/state/omarchy/current/theme");
    ThemePaths {
        colors_toml: Some(theme.join("colors.toml")),
        shell_toml: Some(theme.join("shell.toml")),
        user_shell: Some(home.join(".config/omarchy/shell.toml")),
        menu_file: Some(theme.join("shell.menu.toml")),
        ghostty_theme: Some(theme.join("ghostty.conf")),
        ghostty_user: Some(home.join(".config/ghostty/config")),
        font_file: None,
    }
}

/// Reads color and font files when they exist. Does not start a shell.
/// Size 20 is used only when `[font] base-size = 20` is present.
pub fn load_theme(paths: Option<&ThemePaths>) -> ThemeTokens {
    let Some(paths) = paths else {
        let mut theme = bundled_snapshot();
        theme.source = "bundled snapshot".into();
        return theme;
    };
    let mut theme = bundled_snapshot();
    let mut notes = Vec::new();
    if let Some(text) = paths.colors_toml.as_ref().and_then(read_if_file) {
        let map = flat_keys(&text);
        if let Some(bg) = map.get("background").and_then(|value| parse_hex(value)) {
            theme.bg = bg;
            notes.push("colors.toml background".into());
        }
        if let Some(fg) = map.get("foreground").and_then(|value| parse_hex(value)) {
            theme.fg = fg;
            notes.push("colors.toml foreground".into());
        }
        if let Some(accent) = map.get("accent").and_then(|value| parse_hex(value)) {
            theme.accent = accent;
            notes.push("colors.toml accent".into());
        }
    }
    if let Some(px) = paths.shell_toml.as_ref().and_then(font_base_size) {
        theme.font_px = px;
        notes.push(format!("theme shell.toml base-size {px}"));
    }
    if let Some(px) = paths.user_shell.as_ref().and_then(font_base_size) {
        theme.font_px = px;
        notes.push(format!("user shell.toml base-size {px}"));
    }
    let colors_locked = notes.iter().any(|note| note.starts_with("colors.toml"));
    apply_ghostty(
        paths.ghostty_theme.as_deref(),
        &mut theme,
        &mut notes,
        !colors_locked,
    );
    apply_ghostty(
        paths.ghostty_user.as_deref(),
        &mut theme,
        &mut notes,
        !colors_locked,
    );
    if let Some(px) = paths.shell_toml.as_ref().and_then(font_base_size) {
        theme.font_px = px;
    }
    if let Some(px) = paths.user_shell.as_ref().and_then(font_base_size) {
        theme.font_px = px;
    }
    let asked_for_family = notes
        .iter()
        .any(|note| note.starts_with("ghostty font-family"));
    if let Some(file) = paths.font_file.clone().filter(|path| path.is_file()) {
        theme.font_file = Some(file);
        theme.font_fallback = false;
        notes.push("explicit font file".into());
    } else if asked_for_family {
        if let Some(found) = find_font_file(&theme.font_family) {
            theme.font_file = Some(found);
            theme.font_fallback = false;
            notes.push(format!("font file for {}", theme.font_family));
        } else {
            theme.font_fallback = true;
            theme.font_file = None;
            notes.push(format!(
                "font {} not found; bundled JetBrains Mono",
                theme.font_family
            ));
        }
    }
    let mut menu_card = None;
    let mut menu_text = None;
    let mut menu_selection = None;
    if let Some(text) = paths.shell_toml.as_ref().and_then(read_if_file) {
        apply_menu_section(
            &text,
            &theme,
            &mut menu_card,
            &mut menu_text,
            &mut menu_selection,
        );
    }
    if let Some(text) = paths.menu_file.as_ref().and_then(read_if_file) {
        apply_menu_body(
            &text,
            &theme,
            &mut menu_card,
            &mut menu_text,
            &mut menu_selection,
        );
    }
    if let Some(text) = paths.user_shell.as_ref().and_then(read_if_file) {
        apply_menu_section(
            &text,
            &theme,
            &mut menu_card,
            &mut menu_text,
            &mut menu_selection,
        );
    }
    finish_surfaces(&mut theme, menu_card, menu_text, menu_selection);
    let color_note = notes.iter().any(|note| {
        note.contains("background") || note.contains("foreground") || note.contains("accent")
    });
    if menu_text.is_none() && menu_card.is_none() && !color_note {
        let snap = bundled_snapshot();
        theme.text = snap.text;
        theme.muted = snap.muted;
        theme.card = snap.card;
        theme.elevated = snap.elevated;
        theme.search_bg = snap.search_bg;
        theme.selection = snap.selection;
        theme.warn = snap.warn;
    }
    theme.source = if notes.is_empty() {
        "bundled snapshot".into()
    } else {
        notes.join(", ")
    };
    theme
}

fn apply_ghostty(
    path: Option<&std::path::Path>,
    theme: &mut ThemeTokens,
    notes: &mut Vec<String>,
    allow_color: bool,
) {
    let Some(path) = path else { return };
    let Some(text) = read_if_file(&path.to_path_buf()) else {
        return;
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim();
        match key {
            "font-family" if !value.is_empty() => {
                theme.font_family = value.to_string();
                notes.push(format!("ghostty font-family {value}"));
            }
            "font-size" => {
                let n: f32 = value.parse().unwrap_or(0.0);
                let px = n.round() as u32;
                if (9..=48).contains(&px) {
                    theme.font_px = px;
                    notes.push(format!("ghostty font-size {px}"));
                }
            }
            "background" if allow_color => {
                if let Some(bg) = parse_hex(value) {
                    theme.bg = bg;
                    notes.push("ghostty background".into());
                }
            }
            "foreground" if allow_color => {
                if let Some(fg) = parse_hex(value) {
                    theme.fg = fg;
                    notes.push("ghostty foreground".into());
                }
            }
            _ => {}
        }
    }
}

fn derive_surfaces(theme: &mut ThemeTokens) {
    finish_surfaces(theme, None, None, None);
}

fn finish_surfaces(
    theme: &mut ThemeTokens,
    card: Option<Rgb>,
    text: Option<Rgb>,
    selection: Option<Rgb>,
) {
    theme.text = text.unwrap_or(theme.fg);
    theme.card = card.unwrap_or_else(|| mix(theme.bg, theme.fg, 0.08));
    theme.selection = selection.unwrap_or_else(|| mix(theme.card, theme.accent, 0.42));
    theme.muted = mix(theme.text, theme.card, 0.45);
    theme.search_bg = mix(theme.card, Rgb::new(0, 0, 0), 0.22);
    theme.elevated = mix(theme.card, theme.text, 0.08);
    theme.warn = mix(theme.accent, theme.text, 0.25);
}

fn apply_menu_section(
    text: &str,
    theme: &ThemeTokens,
    card: &mut Option<Rgb>,
    text_color: &mut Option<Rgb>,
    selection: &mut Option<Rgb>,
) {
    let mut in_menu = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            in_menu = rest.trim_end_matches(']').trim() == "menu";
            continue;
        }
        if in_menu {
            apply_menu_line(trimmed, theme, card, text_color, selection);
        }
    }
}

fn apply_menu_body(
    text: &str,
    theme: &ThemeTokens,
    card: &mut Option<Rgb>,
    text_color: &mut Option<Rgb>,
    selection: &mut Option<Rgb>,
) {
    let mut in_menu = true;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            in_menu = rest.trim_end_matches(']').trim() == "menu";
            continue;
        }
        if in_menu {
            apply_menu_line(trimmed, theme, card, text_color, selection);
        }
    }
}

fn apply_menu_line(
    line: &str,
    theme: &ThemeTokens,
    card: &mut Option<Rgb>,
    text_color: &mut Option<Rgb>,
    selection: &mut Option<Rgb>,
) {
    let Some((key, value)) = line.split_once('=') else {
        return;
    };
    let key = key.trim();
    let value = value.trim().trim_matches('"');
    let Some(color) = resolve_color(value, theme) else {
        return;
    };
    match key {
        "background" => *card = Some(color),
        "text" => *text_color = Some(color),
        "selected-background" => *selection = Some(color),
        _ => {}
    }
}

fn resolve_color(value: &str, theme: &ThemeTokens) -> Option<Rgb> {
    if let Some(hex) = parse_hex(value) {
        return Some(hex);
    }
    match value.trim().to_lowercase().as_str() {
        "background" => Some(theme.bg),
        "foreground" | "text" => Some(theme.fg),
        "accent" => Some(theme.accent),
        _ => None,
    }
}

/// Keeps the last good colors when `colors.toml` is non-empty but has no usable background.
pub fn reload_theme(previous: &ThemeTokens, paths: &ThemePaths) -> ThemeTokens {
    if let Some(text) = paths.colors_toml.as_ref().and_then(read_if_file) {
        let broken = !text.trim().is_empty()
            && flat_keys(&text)
                .get("background")
                .and_then(|value| parse_hex(value))
                .is_none();
        if broken {
            let mut kept = previous.clone();
            let fresh = load_theme(Some(paths));
            kept.font_px = fresh.font_px;
            kept.font_family = fresh.font_family;
            kept.font_file = fresh.font_file;
            kept.font_fallback = fresh.font_fallback;
            kept.source = format!("kept colors; {}", fresh.source);
            return kept;
        }
    }
    load_theme(Some(paths))
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    Rgb::new(
        (a.r as f32 * (1.0 - t) + b.r as f32 * t).round() as u8,
        (a.g as f32 * (1.0 - t) + b.g as f32 * t).round() as u8,
        (a.b as f32 * (1.0 - t) + b.b as f32 * t).round() as u8,
    )
}

fn find_font_file(family: &str) -> Option<PathBuf> {
    let needle = family
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(|part| part.to_lowercase())
        .max_by_key(|part| part.len())
        .filter(|part| part.len() >= 4)?;
    let needles = [needle];
    let mut roots = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    for root in roots {
        if let Some(hit) = walk_font(&root, &needles, 0) {
            return Some(hit);
        }
    }
    None
}

fn walk_font(dir: &std::path::Path, needles: &[String], depth: u32) -> Option<PathBuf> {
    if depth > 4 || !dir.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(hit) = walk_font(&path, needles, depth + 1) {
                return Some(hit);
            }
            continue;
        }
        let ext = path.extension()?.to_string_lossy().to_lowercase();
        if ext != "ttf" && ext != "otf" {
            continue;
        }
        let name = path.file_name()?.to_string_lossy().to_lowercase();
        if needles.iter().any(|needle| name.contains(needle)) {
            return Some(path);
        }
    }
    None
}

fn read_if_file(path: &PathBuf) -> Option<String> {
    if path.is_file() {
        std::fs::read_to_string(path).ok()
    } else {
        None
    }
}

fn font_base_size(path: &PathBuf) -> Option<u32> {
    let text = read_if_file(path)?;
    let mut in_font = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            in_font = rest.trim_end_matches(']').trim() == "font";
            continue;
        }
        if !in_font {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "base-size" {
            continue;
        }
        let n: u32 = value.trim().parse().ok()?;
        if (9..=48).contains(&n) {
            return Some(n);
        }
    }
    None
}

fn flat_keys(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let mut value = value.trim().trim_matches('"');
        if let Some((body, _)) = value.split_once('#') {
            if !value.starts_with('#') {
                value = body.trim();
            }
        }
        if !key.trim().is_empty() {
            out.insert(key.trim().to_string(), value.to_string());
        }
    }
    out
}

fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(hex, 16).ok()?;
    Some(Rgb::new(
        ((n >> 16) & 0xff) as u8,
        ((n >> 8) & 0xff) as u8,
        (n & 0xff) as u8,
    ))
}

/// Blocks on filesystem events. It does not spin. The callback runs after a short quiet period.
pub fn spawn_theme_watcher(
    on_change: impl Fn() + Send + 'static,
) -> Option<notify::RecommendedWatcher> {
    use notify::{EventKind, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    let (tx, rx) = mpsc::channel();
    let mut watcher =
        notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
            if let Ok(event) = result {
                if matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                ) && event.paths.iter().any(|path| path_is_theme_input(path))
                {
                    let _ = tx.send(());
                }
            }
        })
        .ok()?;
    let mut armed = false;
    for root in watch_roots() {
        if root.exists() && watcher.watch(&root, RecursiveMode::Recursive).is_ok() {
            armed = true;
        }
    }
    if !armed {
        return None;
    }
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            let start = Instant::now();
            while start.elapsed() < Duration::from_millis(200) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(()) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            on_change();
        }
    });
    Some(watcher)
}

fn path_is_theme_input(path: &std::path::Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if matches!(
        name,
        "colors.toml" | "shell.toml" | "shell.menu.toml" | "ghostty.conf" | "config" | "current"
    ) {
        return true;
    }
    path.components()
        .any(|part| part.as_os_str() == "theme" || part.as_os_str() == "themes")
}

fn watch_roots() -> Vec<PathBuf> {
    let Ok(home) = std::env::var("HOME") else {
        return Vec::new();
    };
    let home = PathBuf::from(home);
    vec![
        home.join(".local/state/omarchy"),
        home.join(".config/omarchy"),
        home.join(".config/ghostty"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_symlink_and_color_file_events_are_reloads() {
        assert!(path_is_theme_input(std::path::Path::new(
            "/home/x/.local/state/omarchy/current"
        )));
        assert!(path_is_theme_input(std::path::Path::new(
            "/home/x/.config/omarchy/themes/blue/colors.toml"
        )));
        assert!(!path_is_theme_input(std::path::Path::new(
            "/home/x/.config/omarchy/extensions/omarchy-menu.jsonc"
        )));
    }

    #[test]
    fn bundled_snapshot_font_px_is_not_20() {
        let theme = bundled_snapshot();
        assert_ne!(theme.font_px, 20);
        assert_eq!(theme.font_px, 15);
        assert_eq!(theme.bg, Rgb::new(0x05, 0x08, 0x05));
        assert_eq!(theme.fg, Rgb::new(0x6d, 0xff, 0x57));
        assert_eq!(theme.accent, Rgb::new(0xff, 0x6a, 0x00));
        assert_eq!(theme.text, Rgb::new(0xe6, 0xf3, 0xe2));
        assert_ne!(theme.text, theme.fg);
        assert!(theme.card.g.abs_diff(theme.card.r) < 16);
        let paths = ThemePaths {
            colors_toml: Some(PathBuf::from("/omarchy-menu-rust-theme-unread/colors.toml")),
            shell_toml: Some(PathBuf::from("/omarchy-menu-rust-theme-unread/shell.toml")),
            user_shell: None,
            ghostty_theme: None,
            ghostty_user: None,
            font_file: None,
            menu_file: None,
        };
        let loaded = load_theme(Some(&paths));
        assert_eq!(loaded.source, "bundled snapshot");
        assert_eq!(loaded.font_px, theme.font_px);
        assert_eq!(loaded.bg, theme.bg);
        assert_eq!(loaded.fg, theme.fg);
        assert_eq!(loaded.accent, theme.accent);
        assert_eq!(load_theme(None).source, "bundled snapshot");
    }

    #[test]
    fn configured_base_size_20_is_read_and_not_assumed() {
        let dir = std::env::temp_dir().join(format!("omr-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("colors.toml"),
            "background = \"#0A1420\"\nforeground = \"#F2F7FC\"\naccent = \"#5AC8FA\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("shell.toml"),
            "[font]\nbase-size = 12\n[menu]\nbackground = \"#0A1420\"\ntext = \"#F2F7FC\"\nselected-background = \"#113355\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("shell.menu.toml"),
            "background = \"#0A1420\"\nselected-background = \"#0A3A6A\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("user.toml"), "[font]\nbase-size = 20\n").unwrap();
        std::fs::write(
            dir.join("ghostty.conf"),
            "font-family = Fixture Face\nfont-size = 12\nbackground = #111111\nforeground = #222222\n",
        )
        .unwrap();
        let loaded = load_theme(Some(&ThemePaths {
            colors_toml: Some(dir.join("colors.toml")),
            shell_toml: Some(dir.join("shell.toml")),
            user_shell: Some(dir.join("user.toml")),
            ghostty_theme: Some(dir.join("ghostty.conf")),
            ghostty_user: None,
            font_file: Some(PathBuf::from(
                "preview/assets/fonts/JetBrainsMono-Regular.ttf",
            )),
            menu_file: Some(dir.join("shell.menu.toml")),
        }));
        assert_eq!(loaded.bg, Rgb::new(0x0a, 0x14, 0x20));
        assert_eq!(loaded.fg, Rgb::new(0xf2, 0xf7, 0xfc));
        assert_eq!(loaded.accent, Rgb::new(0x5a, 0xc8, 0xfa));
        assert_eq!(loaded.card, Rgb::new(0x0a, 0x14, 0x20));
        assert_eq!(loaded.selection, Rgb::new(0x0a, 0x3a, 0x6a));
        assert_eq!(loaded.text, Rgb::new(0xf2, 0xf7, 0xfc));
        assert_ne!(loaded.muted, Rgb::new(0xa9, 0xc2, 0xad));
        assert_ne!(loaded.search_bg, Rgb::new(0x08, 0x10, 0x09));
        assert_ne!(loaded.elevated, Rgb::new(0x10, 0x18, 0x12));
        assert_eq!(loaded.font_px, 20);
        assert_eq!(loaded.font_family, "Fixture Face");
        assert!(!loaded.font_fallback);
        assert_ne!(loaded.selection, Rgb::new(0x3a, 0x22, 0x10));
        assert_ne!(loaded.search_bg, loaded.bg);
        assert_ne!(loaded.muted, loaded.fg);
        let broken = dir.join("colors.toml");
        std::fs::write(&broken, "background = \"#0A").unwrap();
        let kept = reload_theme(
            &loaded,
            &ThemePaths {
                colors_toml: Some(broken),
                shell_toml: Some(dir.join("shell.toml")),
                user_shell: Some(dir.join("user.toml")),
                ghostty_theme: Some(dir.join("ghostty.conf")),
                ghostty_user: None,
                font_file: Some(PathBuf::from(
                    "preview/assets/fonts/JetBrainsMono-Regular.ttf",
                )),
                menu_file: Some(dir.join("shell.menu.toml")),
            },
        );
        assert_eq!(kept.bg, Rgb::new(0x0a, 0x14, 0x20));
        std::fs::write(
            dir.join("colors.toml"),
            "background = \"#112233\"\nforeground = \"#F2F7FC\"\naccent = \"#5AC8FA\"\n",
        )
        .unwrap();
        let next = reload_theme(
            &kept,
            &ThemePaths {
                colors_toml: Some(dir.join("colors.toml")),
                shell_toml: Some(dir.join("shell.toml")),
                user_shell: Some(dir.join("user.toml")),
                ghostty_theme: Some(dir.join("ghostty.conf")),
                ghostty_user: None,
                font_file: Some(PathBuf::from(
                    "preview/assets/fonts/JetBrainsMono-Regular.ttf",
                )),
                menu_file: Some(dir.join("shell.menu.toml")),
            },
        );
        assert_eq!(next.bg, Rgb::new(0x11, 0x22, 0x33));
        assert_eq!(next.selection, Rgb::new(0x0a, 0x3a, 0x6a));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
