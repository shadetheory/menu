//! Spotlight pill. Wayland uses a panel-sized layer-shell surface. X11 is an opaque fallback.
//! A left click selects a visible row for this process only. It is not saved.
//!
//! Clicks outside the panel are not delivered to this process. This build does not
//! claim that those clicks are swallowed before they reach the window underneath.

#[path = "present_wayland.rs"]
mod present_wayland;

use crate::bots::{self, Bot};
use crate::dispatch::{self, PlanError, SpawnPlan};
use crate::model::{self, fuzzy_hit, is_visible, matches_query, search_score, Item, Kind, Menu};
use crate::providers;
use crate::theme;
use fontdue::Font;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tiny_skia::{Color, Pixmap};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{Window, WindowId};

const FONT_BYTES: &[u8] = include_bytes!("../preview/assets/fonts/JetBrainsMono-Regular.ttf");

pub struct Options {
    pub screenshot: Option<PathBuf>,
    /// When true, confirm does not spawn shell actions. The CLI default is true.
    pub no_exec: bool,
    /// When true, a confirmed non-dangerous app argv may start. Shell stays off.
    pub live: bool,
    /// When true, an admitted desktop Exec may start. Shell, updates, and guards stay off.
    pub safe_apps: bool,
    pub exit_after_first_frame: bool,
    /// Exact allow-file grants may run only when this is set. Not implied by --live or --exec.
    pub run_guards: bool,
    /// When true, key and click steps print one state line on stderr. Off unless requested.
    pub trace_state: bool,
}

const PILL_W: f32 = 680.0;
const SEARCH_H: f32 = 54.0;
const PILL_PAD: f32 = 10.0;
const MAX_PILL_H: f32 = 440.0;
const PILL_RADIUS: f32 = 18.0;
/// Panel plates only. Glyph interiors are written at alpha 255.
const PANEL_ALPHA: u8 = 196;
const ROW_PAD: f32 = 4.0;
const LIST_X: f32 = PILL_PAD;

fn list_top() -> f32 {
    PILL_PAD + SEARCH_H + 8.0
}

fn hidden_future(entry: &Item) -> bool {
    entry.id.starts_with("samples")
        || entry.panel.starts_with("bot:")
        || matches!(
            entry.panel.as_str(),
            "groceries"
                | "inbox"
                | "recent"
                | "decisions"
                | "example-a"
                | "example-b"
                | "music"
                | "store"
        )
}

#[derive(Clone)]
struct Confirm {
    summary: String,
    /// Plain effect we actually know. Unknown effects stay unknown.
    lead: String,
    effect: String,
    plan: SpawnPlan,
    dangerous: bool,
    /// 0 is Cancel, the default. 1 is Confirm.
    focus: usize,
    source_key: String,
    source_gen: u64,
}

#[derive(Clone, Default)]
struct Row {
    id: String,
    title: String,
    hint: String,
    verb: String,
    panel: String,
    action: String,
    argv: Vec<String>,
    input_at: Option<usize>,
    fallback_route: String,
    /// Desktop filename when this row came from a scanned desktop entry.
    desktop_id: String,
    desktop_file: PathBuf,
    fuzzy: bool,
    unavailable: bool,
    dangerous: bool,
    needs_input: bool,
    terminal: bool,
}

impl Row {
    fn key(&self) -> String {
        if !self.id.is_empty() {
            self.id.clone()
        } else {
            format!("{}|{}", self.title, self.argv.join("\t"))
        }
    }
}

#[derive(Clone)]
enum InputPurpose {
    Filter {
        title: String,
        rows: Vec<Row>,
    },
    Command {
        argv: Vec<String>,
        input_at: usize,
        dangerous: bool,
        saved_query: String,
        saved_cursor: usize,
        saved_view: Box<View>,
    },
}

#[derive(Clone)]
enum View {
    Tree,
    Select {
        title: String,
        rows: Vec<Row>,
    },
    Input {
        purpose: InputPurpose,
        buffer: String,
    },
}

#[derive(Clone)]
struct Place {
    active: String,
    cursor: usize,
    query: String,
    view: View,
}

struct Model {
    menu: Menu,
    bots: Vec<Bot>,
    query: String,
    cursor: usize,
    active: String,
    panel: String,
    confirm: Option<Confirm>,
    status: String,
    no_exec: bool,
    live: bool,
    safe_apps: bool,
    view: View,
    app_dirs: Vec<PathBuf>,
    command_dirs: Vec<PathBuf>,
    app_rows: Vec<Row>,
    terminal: Vec<String>,
    when_results: BTreeMap<String, bool>,
    disabled_results: BTreeMap<String, bool>,
    checked_results: BTreeMap<String, bool>,
    guard_blocks: BTreeMap<String, String>,
    notice: Option<String>,
    theme: theme::ThemeTokens,
    stack: Vec<Place>,
    results_gen: u64,
    stale: bool,
    should_close: bool,
    answer: Option<String>,
    copy_focus: bool,
    copied: Option<String>,
}

impl Model {
    fn new(menu: Menu, no_exec: bool) -> Self {
        Self {
            menu,
            bots: bots::embedded_bots(),
            query: String::new(),
            cursor: 0,
            active: "root".into(),
            panel: String::new(),
            confirm: None,
            status:
                "Search does not run commands. --safe-apps starts only allowlisted desktop apps. --exec launches stock actions."
                    .into(),
            no_exec,
            live: false,
            safe_apps: false,
            view: View::Tree,
            app_dirs: Vec::new(),
            command_dirs: Vec::new(),
            app_rows: Vec::new(),
            terminal: Vec::new(),
            when_results: BTreeMap::new(),
            disabled_results: BTreeMap::new(),
            checked_results: BTreeMap::new(),
            guard_blocks: BTreeMap::new(),
            notice: None,
            theme: theme::bundled_snapshot(),
            stack: Vec::new(),
            results_gen: 1,
            stale: false,
            should_close: false,
            answer: None,
            copy_focus: false,
            copied: None,
        }
    }

    fn apply_guards(&mut self, facts: &crate::guards::GuardFacts) {
        self.when_results = facts.when_results.clone();
        self.disabled_results = facts.disabled.clone();
        self.checked_results = facts.checked.clone();
        self.guard_blocks = facts.blocks.clone();
        if facts.skipped_shell > 0 {
            self.status = format!(
                "{} shell guard(s) were not run. Those rows have no action.",
                facts.skipped_shell
            );
        }
    }

    fn rows(&self) -> Vec<Row> {
        match &self.view {
            View::Input { .. } => Vec::new(),
            View::Select { rows, .. } => {
                let q = self.query.trim().to_lowercase();
                let mut out = Vec::new();
                for row in rows {
                    if row.id == "__filter__" {
                        out.push(row.clone());
                        continue;
                    }
                    if q.is_empty()
                        || row.title.to_lowercase().contains(&q)
                        || row.hint.to_lowercase().contains(&q)
                    {
                        out.push(row.clone());
                    }
                }
                out
            }
            View::Tree => {
                let q = self.query.trim();
                if q.is_empty()
                    && self.active == "root"
                    && self.confirm.is_none()
                    && self.notice.is_none()
                    && self.answer.is_none()
                    && self.panel.is_empty()
                {
                    self.shortcut_rows()
                } else if !q.is_empty() {
                    self.search_rows(q)
                } else {
                    self.child_rows(&self.active)
                }
            }
        }
    }

    fn child_rows(&self, parent: &str) -> Vec<Row> {
        let mut rows = Vec::new();
        let none = &self.when_results;
        for id in &self.menu.order {
            let Some(entry) = self.menu.items.get(id) else {
                continue;
            };
            if entry.parent != parent || entry.id == "root" || hidden_future(entry) {
                continue;
            }
            if !is_visible(&self.menu, none, entry, 0) {
                continue;
            }
            rows.push(self.row_for(entry, false));
        }
        if let Some(entry) = self.menu.items.get(parent) {
            if !entry.provider.is_empty() {
                let kind = providers::classify(&entry.provider);
                rows.push(Row {
                    id: String::new(),
                    title: format!("Open stock {} list", entry.provider),
                    hint: providers::fallback_row_label(&kind),
                    verb: "Confirm…".into(),
                    panel: String::new(),
                    action: String::new(),
                    argv: Vec::new(),
                    fallback_route: entry.id.clone(),
                    fuzzy: false,
                    unavailable: false,
                    dangerous: false,
                    ..Row::default()
                });
            }
        }
        rows
    }

    fn search_rows(&self, query: &str) -> Vec<Row> {
        let none = &self.when_results;
        let mut hits: Vec<&Item> = self
            .menu
            .items
            .values()
            .filter(|e| {
                !hidden_future(e)
                    && matches_query(&self.menu, e, query, is_visible(&self.menu, none, e, 0))
            })
            .collect();
        hits.sort_by(|a, b| {
            search_score(&self.menu, a, query)
                .cmp(&search_score(&self.menu, b, query))
                .then(a.order.cmp(&b.order))
        });
        let mut rows: Vec<Row> = if !hits.is_empty() {
            hits.into_iter()
                .take(40)
                .map(|entry| self.row_for(entry, false))
                .collect()
        } else {
            let mut fuzzy: Vec<&Item> = self
                .menu
                .items
                .values()
                .filter(|entry| {
                    !hidden_future(entry)
                        && fuzzy_hit(&self.menu, entry, query)
                        && is_visible(&self.menu, none, entry, 0)
                })
                .collect();
            fuzzy.sort_by_key(|entry| entry.order);
            fuzzy
                .into_iter()
                .take(20)
                .map(|entry| self.row_for(entry, true))
                .collect()
        };
        let needle = query.to_lowercase();
        for app in &self.app_rows {
            if app.title.to_lowercase().contains(&needle)
                || app.hint.to_lowercase().contains(&needle)
            {
                rows.push(app.clone());
            }
        }
        rows.truncate(40);
        rows
    }

    fn shortcut_rows(&self) -> Vec<Row> {
        shortcut_rows_from(&self.app_rows, |path| std::path::Path::new(path).is_file())
    }

    fn load_apps(&mut self) {
        let dirs = if self.app_dirs.is_empty() {
            providers::default_app_dirs()
        } else {
            self.app_dirs.clone()
        };
        let listed = providers::discover_apps(&dirs, &self.terminal);
        let no_exec = self.no_exec;
        let live = self.live;
        let safe_apps = self.safe_apps;
        self.app_rows = listed
            .rows
            .into_iter()
            .map(|item| listed_row(item, no_exec, live, safe_apps))
            .collect();
    }

    fn row_for(&self, entry: &Item, fuzzy: bool) -> Row {
        let blocked = self.guard_blocks.get(&entry.id).cloned();
        let disabled = model::is_disabled(&self.disabled_results, entry);
        let verb = if entry.panel == "answer" {
            "Answer"
        } else if disabled {
            "Disabled"
        } else if blocked.is_some() || !entry.panel.is_empty() {
            "Unavailable"
        } else if !entry.provider.is_empty() || entry.kind == Kind::Menu || entry.kind == Kind::Link
        {
            "Browse"
        } else if !entry.action.is_empty() {
            "Confirm…"
        } else {
            "Open"
        };
        let hint = if let Some(reason) = &blocked {
            reason.clone()
        } else if fuzzy {
            "Fuzzy fallback. Not the stock ranker. Enter does not run it.".into()
        } else if entry.panel == "answer" {
            if entry.description.is_empty() {
                "Answer. Enter expands it. Copy is a separate control.".into()
            } else {
                entry.description.clone()
            }
        } else if !entry.panel.is_empty() {
            "Unavailable. Not connected. Nothing is loaded.".into()
        } else if disabled {
            "Disabled by its guard. Nothing will start.".into()
        } else if !entry.description.is_empty()
            && !entry.description.eq_ignore_ascii_case(&entry.label)
        {
            entry.description.clone()
        } else if !entry.action.is_empty() {
            entry.action.clone()
        } else if !entry.provider.is_empty() {
            providers::fallback_row_label(&providers::classify(&entry.provider))
        } else if model::path_for(&self.menu, &entry.id).eq_ignore_ascii_case(&entry.label) {
            "Opens the next list.".into()
        } else {
            model::path_for(&self.menu, &entry.id)
        };
        let checked = self.checked_results.get(&entry.id) == Some(&true);
        let disabled = model::is_disabled(&self.disabled_results, entry);
        Row {
            id: entry.id.clone(),
            title: model::label_for(entry, checked, disabled),
            hint,
            verb: verb.into(),
            panel: entry.panel.clone(),
            action: entry.action.clone(),
            argv: Vec::new(),
            fallback_route: String::new(),
            fuzzy,
            unavailable: disabled
                || blocked.is_some()
                || (!entry.panel.is_empty() && entry.panel != "answer"),
            dangerous: dispatch::is_dangerous(&entry.action),
            ..Row::default()
        }
    }

    fn clamp(&mut self) {
        let n = self.rows().len();
        if n == 0 {
            self.cursor = 0;
        } else if self.cursor >= n {
            self.cursor = n - 1;
        }
    }

    /// Moves the highlight only. It does not activate a row or start a command.
    fn highlight_at(&mut self, x: f32, y: f32, width: f32, height: f32) {
        if let Some(focus) = confirm_button_at(self, x, y, width, height) {
            if let Some(confirm) = &mut self.confirm {
                confirm.focus = focus;
            }
            return;
        }
        if self.confirm.is_some() {
            return;
        }
        if let Some(index) = row_index_at(self, x, y, width, height) {
            self.cursor = index;
        }
    }

    fn push_place(&mut self) {
        self.stack.push(Place {
            active: self.active.clone(),
            cursor: self.cursor,
            query: self.query.clone(),
            view: self.view.clone(),
        });
    }

    fn note_results(&mut self) {
        self.results_gen = self.results_gen.wrapping_add(1);
    }

    /// One activation step. Tests use this. The window passes a status callback to `enter`.
    #[allow(dead_code)]
    fn activate(&mut self) {
        self.enter(|_| {});
    }

    fn wayland_key(&mut self, keysym: &str, utf8: Option<&str>, repeat: bool) {
        match keysym {
            "Escape" => {
                self.go_back();
            }
            "Return" | "KP_Enter" => {
                if repeat {
                    return;
                }
                self.enter(|_| {});
            }
            "Up" => {
                if self.confirm.is_some() {
                    if let Some(confirm) = &mut self.confirm {
                        confirm.focus = 0;
                    }
                } else if self.copy_focus {
                    self.copy_focus = false;
                } else {
                    self.cursor = self.cursor.saturating_sub(1);
                }
            }
            "Down" => {
                if self.confirm.is_some() {
                    if let Some(confirm) = &mut self.confirm {
                        confirm.focus = 1;
                    }
                } else if self.answer.is_some() && !self.copy_focus {
                    self.copy_focus = true;
                } else {
                    self.cursor = self.cursor.saturating_add(1);
                    self.clamp();
                }
            }
            "BackSpace" => {
                if self.confirm.is_none() && self.panel.is_empty() {
                    if let View::Input { buffer, .. } = &mut self.view {
                        buffer.pop();
                    } else {
                        self.query.pop();
                        self.cursor = 0;
                    }
                }
            }
            _ => {
                if repeat || self.confirm.is_some() || !self.panel.is_empty() {
                    return;
                }
                let Some(text) = utf8 else { return };
                if let View::Input { buffer, .. } = &mut self.view {
                    if buffer.len() < 64 {
                        buffer.push_str(text);
                    }
                } else {
                    self.query.push_str(text);
                    self.cursor = 0;
                    self.note_results();
                }
            }
        }
        self.clamp();
    }

    fn enter(&mut self, report: impl Fn(String) + Send + 'static) {
        if self.stale {
            self.status = "Results are outdated. Activation ignored. Nothing was started.".into();
            return;
        }
        if self.copy_focus {
            if let Some(body) = &self.answer {
                if self.no_exec && !self.live {
                    self.copied = None;
                    self.status =
                        "Clipboard tool was not started. --no-exec does not launch programs."
                            .into();
                    return;
                }
                match crate::clipboard::copy_text(body, None) {
                    Ok(tool) => {
                        self.copied = Some(body.clone());
                        self.status = format!(
                            "Copied to the system clipboard with {tool}. Nothing was started."
                        );
                    }
                    Err(err) => {
                        self.copied = None;
                        self.status = format!("Not copied to the system clipboard: {err}");
                    }
                }
            }
            return;
        }
        if self.confirm.is_some() {
            let focus = self.confirm.as_ref().map(|c| c.focus).unwrap_or(0);
            if focus == 0 {
                self.confirm = None;
                self.status = "Cancelled. Nothing was started.".into();
            } else {
                self.confirm_run(report);
            }
            return;
        }
        if matches!(self.view, View::Input { .. }) {
            self.commit_input(report);
            return;
        }
        let gen = self.results_gen;
        let Some(row) = self.rows().get(self.cursor).cloned() else {
            return;
        };
        let key = row.key();
        self.activate_key(gen, &key, report);
    }

    /// Applies an activation captured for one result generation and one row.
    /// A newer list never inherits that activation.
    fn activate_key(&mut self, gen: u64, key: &str, report: impl Fn(String) + Send + 'static) {
        if self.stale || gen != self.results_gen {
            self.status =
                "Stale activation ignored. It was not applied to a new row. Nothing was started."
                    .into();
            return;
        }
        let Some(row) = self.rows().into_iter().find(|row| row.key() == key) else {
            self.status = "That row is no longer listed. Nothing was started.".into();
            return;
        };
        if row.fuzzy {
            self.status = "Fuzzy row is not activated and is not run.".into();
            return;
        }
        if row.id == "__filter__" {
            self.open_filter_input();
            return;
        }
        if row.unavailable || row.verb == "Unavailable" || row.verb == "Disabled" {
            self.confirm = None;
            self.status = format!("Unavailable. {} Nothing was started.", row.hint);
            return;
        }
        if row.verb == "Answer" || row.panel == "answer" {
            let body = if row.hint.is_empty() {
                row.title.clone()
            } else {
                row.hint.clone()
            };
            self.answer = Some(body);
            self.copy_focus = false;
            self.confirm = None;
            self.status = "Answer expanded. Copy is separate. Nothing was started.".into();
            return;
        }
        if let Some(entry) = self.menu.items.get(&row.id) {
            if model::is_disabled(&self.disabled_results, entry) {
                self.status = "This row is disabled by its guard. Nothing was started.".into();
                return;
            }
            if !entry.provider.is_empty() {
                let provider = entry.provider.clone();
                let action = entry.action.clone();
                let id = entry.id.clone();
                self.open_provider(&provider, &action, &id);
                return;
            }
        }
        if !row.panel.is_empty() {
            self.panel = row.panel.clone();
            self.confirm = None;
            self.answer = None;
            self.status =
                "Unavailable. Not connected. No message, list, or account call was made.".into();
            return;
        }
        if row.needs_input {
            self.open_command_input(&row);
            return;
        }
        if row.dangerous || !row.action.is_empty() || !row.fallback_route.is_empty() {
            self.open_risk(row, report);
            return;
        }
        if !row.argv.is_empty() {
            match dispatch::plan_argv(&row.argv) {
                Ok(mut plan) => {
                    let admitted = if let Some(admission) = self.admission(&row) {
                        plan.program = admission.program.to_string_lossy().into_owned();
                        plan.args = admission.args;
                        true
                    } else {
                        false
                    };
                    self.launch_or_dry(plan, false, admitted, report);
                }
                Err(_) => {
                    self.status =
                        "Refused: that Exec line is a shell wrapper. Nothing was started.".into()
                }
            }
            return;
        }
        if row.action.is_empty() {
            if let Some(entry) = self.menu.items.get(&row.id) {
                if entry.kind == Kind::Menu || entry.kind == Kind::Link {
                    let target = if entry.kind == Kind::Link {
                        entry.target.clone()
                    } else {
                        entry.id.clone()
                    };
                    if !target.is_empty() {
                        self.push_place();
                        self.active = target;
                        self.query.clear();
                        self.cursor = 0;
                        self.panel.clear();
                        self.answer = None;
                        self.view = View::Tree;
                        self.note_results();
                    }
                }
            }
            return;
        }
        self.open_risk(row, report);
    }

    fn open_risk(&mut self, row: Row, _report: impl Fn(String) + Send + 'static) {
        let plan = if !row.argv.is_empty() {
            dispatch::plan_argv(&row.argv)
        } else if !row.fallback_route.is_empty() {
            dispatch::plan_stock_fallback(&row.fallback_route)
        } else {
            dispatch::plan_action(&row.action, &self.query)
        };
        match plan {
            Ok(plan) => {
                let command = if row.action.is_empty() {
                    format!("{} {}", plan.program, plan.args.join(" "))
                } else {
                    row.action.clone()
                };
                let dangerous = row.dangerous || dispatch::is_dangerous(&row.action);
                let will_run = self.will_spawn(&plan, dangerous, false);
                let lead = if !will_run {
                    "This will not run. Its effects haven’t been verified.".to_string()
                } else if dangerous {
                    "This command is classified as dangerous. The effect is the command below."
                        .to_string()
                } else if matches!(
                    plan.kind,
                    dispatch::PlanKind::ShellAction | dispatch::PlanKind::StockFallback
                ) {
                    "Runs this command. Its effects haven’t been verified.".to_string()
                } else {
                    format!("Start {}", plan.program)
                };
                let source_key = row.key();
                self.answer = None;
                self.panel.clear();
                self.notice = None;
                self.confirm = Some(Confirm {
                    summary: row.title,
                    lead,
                    effect: command,
                    plan,
                    dangerous,
                    focus: 0,
                    source_key,
                    source_gen: self.results_gen,
                });
                self.status = if will_run {
                    "Cancel is selected. Choose Confirm, then activate it.".into()
                } else {
                    "Cancel is selected. Confirm does not start it.".into()
                };
            }
            Err(PlanError::QueryLeak) => {
                self.status = "Refused: the question text appears inside that command, so it was not started.".into();
            }
            Err(_) => self.status = "Nothing to run.".into(),
        }
    }

    fn open_filter_input(&mut self) {
        let View::Select { title, rows } = self.view.clone() else {
            return;
        };
        self.view = View::Input {
            purpose: InputPurpose::Filter { title, rows },
            buffer: self.query.clone(),
        };
        self.confirm = None;
        self.status = "Filter only. This text is not a command. Esc goes back.".into();
    }

    fn open_command_input(&mut self, row: &Row) {
        self.view = View::Input {
            purpose: InputPurpose::Command {
                argv: row.argv.clone(),
                input_at: row.input_at.unwrap_or(row.argv.len()),
                dangerous: row.dangerous,
                saved_query: self.query.clone(),
                saved_cursor: self.cursor,
                saved_view: Box::new(self.view.clone()),
            },
            buffer: String::new(),
        };
        self.confirm = None;
        self.status = if self.no_exec && !self.live {
            "A file or URL is required. Enter does not start it. Esc goes back.".into()
        } else {
            "Input required before this can run. Empty text cannot be confirmed. Esc goes back."
                .into()
        };
    }

    fn commit_input(&mut self, report: impl Fn(String) + Send + 'static) {
        let View::Input { purpose, buffer } = self.view.clone() else {
            return;
        };
        match purpose {
            InputPurpose::Filter { title, rows } => {
                self.query = buffer;
                self.view = View::Select { title, rows };
                self.cursor = 0;
                self.note_results();
                self.status = "Filter applied. The text was not executed.".into();
            }
            InputPurpose::Command {
                mut argv,
                input_at,
                dangerous,
                saved_query,
                saved_cursor,
                saved_view,
            } => {
                let value = buffer.trim();
                if value.is_empty() || value.contains('\n') || value.contains('\0') {
                    self.status = "Required input is empty or invalid. Nothing was started.".into();
                    return;
                }
                let at = input_at.min(argv.len());
                argv.insert(at, value.to_string());
                self.view = *saved_view;
                self.query = saved_query;
                self.cursor = saved_cursor;
                match dispatch::plan_argv(&argv) {
                    Ok(plan) if dangerous => {
                        let effect = format!("{} {}", plan.program, plan.args.join(" "));
                        self.confirm = Some(Confirm {
                            summary: plan.program.clone(),
                            lead: "This command is classified as dangerous. The effect is the command below.".into(),
                            effect: effect.clone(),
                            plan,
                            dangerous: true,
                            focus: 0,
                            source_key: String::new(),
                            source_gen: self.results_gen,
                        });
                        self.status = format!("Effect: {effect}. Cancel is selected.");
                    }
                    Ok(plan) => self.launch_or_dry(plan, false, false, report),
                    Err(_) => {
                        self.status =
                            "Refused: that command is a shell wrapper. Nothing was started.".into()
                    }
                }
            }
        }
    }

    fn open_provider(&mut self, name: &str, stock_action: &str, route: &str) {
        let kind = providers::classify(name);
        let app_dirs = if self.app_dirs.is_empty() {
            providers::default_app_dirs()
        } else {
            self.app_dirs.clone()
        };
        let command_dirs = if self.command_dirs.is_empty() {
            providers::default_command_dirs()
        } else {
            self.command_dirs.clone()
        };
        let listed = match &kind {
            providers::ProviderKind::Apps => providers::discover_apps(&app_dirs, &self.terminal),
            providers::ProviderKind::Commands => providers::discover_commands(&command_dirs),
            other => providers::unavailable_list(other),
        };
        let mut rows = vec![Row {
            id: "__filter__".into(),
            title: "Filter this list".into(),
            hint: "Filter only. The text is not a command.".into(),
            verb: "Filter".into(),
            ..Row::default()
        }];
        if listed.rows.is_empty() {
            rows.push(Row {
                title: format!("No {name} entries"),
                hint: "Nothing was found in the local directories.".into(),
                verb: "Unavailable".into(),
                unavailable: true,
                ..Row::default()
            });
        }
        for item in listed.rows {
            let verb = if item.unavailable {
                "Unavailable"
            } else if item.input_at.is_some() {
                "Run…"
            } else if item.dangerous {
                "Confirm…"
            } else {
                "Open"
            };
            let (hint, verb) = refused_preview(item.hint, verb, self.no_exec && !self.live);
            rows.push(Row {
                title: item.title,
                hint,
                verb,
                argv: item.argv,
                input_at: item.input_at,
                needs_input: item.input_at.is_some(),
                unavailable: item.unavailable,
                dangerous: item.dangerous,
                ..Row::default()
            });
        }
        if !stock_action.is_empty() {
            rows.push(Row {
                title: "Stock menu action".into(),
                hint: "Trusted menu action. Cancel is the default.".into(),
                verb: "Confirm…".into(),
                action: stock_action.to_string(),
                dangerous: dispatch::is_dangerous(stock_action),
                ..Row::default()
            });
        }
        rows.push(Row {
            title: "Open in the stock menu".into(),
            hint: "Hands this route to omarchy-menu. Cancel is the default.".into(),
            verb: "Confirm…".into(),
            fallback_route: route.to_string(),
            ..Row::default()
        });
        let count = rows.len();
        self.push_place();
        self.view = View::Select {
            title: providers::kind_name(&kind),
            rows,
        };
        self.query.clear();
        self.cursor = 0;
        self.panel.clear();
        self.confirm = None;
        self.note_results();
        self.status = format!(
            "Select {name}. {count} rows. Malformed entries skipped: {}. Unreadable: {}.",
            listed.malformed, listed.unreadable
        );
    }

    fn go_back(&mut self) {
        if self.confirm.is_some() {
            self.confirm = None;
            self.status = "Cancelled. Nothing was started.".into();
            return;
        }
        if self.copy_focus {
            self.copy_focus = false;
            self.status = "Left Copy. The answer stays open. Nothing was started.".into();
            return;
        }
        if self.answer.is_some() {
            self.answer = None;
            self.status = "Closed the answer. Nothing was copied or started.".into();
            return;
        }
        if let View::Input { purpose, .. } = self.view.clone() {
            match purpose {
                InputPurpose::Filter { title, rows } => {
                    self.view = View::Select { title, rows };
                    self.status = "Input cancelled. The query and highlight are unchanged. Nothing was started.".into();
                }
                InputPurpose::Command {
                    saved_query,
                    saved_cursor,
                    saved_view,
                    ..
                } => {
                    self.view = *saved_view;
                    self.query = saved_query;
                    self.cursor = saved_cursor;
                    self.status =
                        "Input cancelled. Query and highlight restored. Nothing was started."
                            .into();
                }
            }
            return;
        }
        if let Some(place) = self.stack.pop() {
            self.active = place.active;
            self.cursor = place.cursor;
            self.query = place.query;
            self.view = place.view;
            self.panel.clear();
            self.note_results();
            self.status =
                "Back one level. Query and highlight restored. Nothing was started.".into();
            return;
        }
        if !self.panel.is_empty() {
            self.panel.clear();
            self.status = "Closed the unavailable panel. Nothing was contacted.".into();
            return;
        }
        self.should_close = true;
        self.status = "Closing. Nothing was started.".into();
    }

    fn admission(&self, row: &Row) -> Option<crate::safe_apps::Admission> {
        if !self.safe_apps {
            return None;
        }
        crate::safe_apps::admit_configured(crate::safe_apps::AppFact {
            desktop_id: &row.desktop_id,
            desktop_file: &row.desktop_file,
            argv: &row.argv,
            terminal: row.terminal,
            needs_input: row.needs_input,
            dangerous: row.dangerous,
            unavailable: row.unavailable,
        })
        .ok()
    }

    fn will_spawn(&self, plan: &SpawnPlan, dangerous: bool, admitted: bool) -> bool {
        dispatch::gate_spawn(
            self.no_exec,
            self.live,
            self.safe_apps,
            admitted,
            plan,
            dangerous,
        )
        .is_ok()
    }

    fn launch_or_dry(
        &mut self,
        plan: SpawnPlan,
        dangerous: bool,
        admitted: bool,
        report: impl Fn(String) + Send + 'static,
    ) {
        let effect = format!("{} {}", plan.program, plan.args.join(" "));
        if dispatch::gate_spawn(
            self.no_exec,
            self.live,
            self.safe_apps,
            admitted,
            &plan,
            dangerous,
        )
        .is_err()
        {
            self.confirm = None;
            self.notice = Some(format!("Dry run. Nothing was started.\n{effect}"));
            self.status = "Dry run. Nothing was started.".into();
            return;
        }
        match dispatch::spawn_report(&plan, report) {
            Ok(_) => {
                self.status = format!("Started {}. Closing the menu.", plan.program);
                self.should_close = true;
                self.confirm = None;
            }
            Err(msg) => {
                self.confirm = None;
                self.notice = Some(format!("Did not start.\n{msg}"));
                self.status = format!("{msg} Nothing was started.");
            }
        }
    }

    fn confirm_run(&mut self, report: impl Fn(String) + Send + 'static) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        if confirm.source_gen != self.results_gen && !confirm.source_key.is_empty() {
            self.confirm = None;
            self.status = "Stale confirmation ignored. Nothing was started.".into();
            return;
        }
        self.launch_or_dry(confirm.plan, confirm.dangerous, false, report);
    }
}

fn load_guard_allow() -> crate::guards::GuardGrant {
    let path = std::env::var_os("OMARCHY_MENU_GUARD_ALLOW")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                std::path::PathBuf::from(home).join(".config/omarchy/menu-guard-allow.txt")
            })
        });
    let Some(path) = path else {
        return crate::guards::GuardGrant::default();
    };
    match std::fs::read_to_string(path) {
        Ok(text) => crate::guards::parse_allow(&text),
        Err(_) => crate::guards::GuardGrant::default(),
    }
}

fn load_ui_font(theme: &theme::ThemeTokens) -> Font {
    if let Some(path) = &theme.font_file {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(font) = Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                return font;
            }
        }
    }
    Font::from_bytes(FONT_BYTES, fontdue::FontSettings::default()).expect("bundled font")
}

pub fn paint(menu: &Menu, query: &str, cursor: usize, width: u32, height: u32) -> Pixmap {
    let mut model = Model::new(menu.clone(), true);
    model.query = query.to_string();
    model.cursor = cursor;
    model.clamp();
    paint_model(&model, width, height)
}

fn side_open(model: &Model) -> bool {
    model.confirm.is_some()
        || model.notice.is_some()
        || model.answer.is_some()
        || !model.panel.is_empty()
}

fn list_width(_model: &Model, width: f32) -> f32 {
    (width.min(PILL_W) - PILL_PAD * 2.0).max(120.0)
}

fn row_window(model: &Model, height: f32) -> (usize, usize, f32) {
    let pitch = model.theme.row_h().max(1.0);
    let bottom = if showing_shortcuts(model) {
        SEARCH_H + PILL_PAD + 8.0
    } else {
        12.0
    };
    let available = (height.min(MAX_PILL_H) - rows_origin(model) - bottom).max(pitch);
    let fit = ((available / pitch).floor() as usize).clamp(1, 8);
    let count = model.rows().len();
    let start = model
        .cursor
        .saturating_sub(6)
        .min(count.saturating_sub(fit));
    (start, fit, pitch)
}

const GHOSTTY_ID: &str = "com.mitchellh.ghostty.desktop";
const FLEA_ID: &str = "com.thisisgm.flea.desktop";
const CURSOR_ID: &str = "cursor.desktop";
const GROK_ID: &str = "grok-bot.desktop";
const MATTERMOST_ID: &str = "mattermost-desktop.desktop";
const TERMINAL_WRAPPER: &str = "/usr/share/omarchy/bin/omarchy-launch-terminal";
const BROWSER_WRAPPER: &str = "/usr/share/omarchy/bin/omarchy-launch-browser";
const FLEA_WRAPPER: &str = "/usr/bin/flea";

/// Shortcut-derived rows. A desktop entry wins over a wrapper for the same app.
/// Chromium is not used as the browser. Nothing here is a usage count.
fn shortcut_rows_from(apps: &[Row], mut exists: impl FnMut(&str) -> bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let find = |id: &str| apps.iter().find(|row| row.desktop_id == id);
    if let Some(app) = find(GHOSTTY_ID) {
        rows.push(shortcut_from_desktop(app));
    } else if exists(TERMINAL_WRAPPER) {
        rows.push(wrapper_row(
            "shortcut:terminal",
            "Open terminal",
            "Shortcut. omarchy-launch-terminal.",
            &[TERMINAL_WRAPPER],
        ));
    }
    if let Some(app) = find(FLEA_ID) {
        rows.push(shortcut_from_desktop(app));
    } else if exists(FLEA_WRAPPER) {
        rows.push(wrapper_row(
            "shortcut:files",
            "Open file manager",
            "Shortcut. flea --gui.",
            &[FLEA_WRAPPER, "--gui"],
        ));
    }
    if exists(BROWSER_WRAPPER) {
        rows.push(wrapper_row(
            "shortcut:browser",
            "Open browser",
            "Shortcut. omarchy-launch-browser.",
            &[BROWSER_WRAPPER],
        ));
    }
    for id in [CURSOR_ID, GROK_ID, MATTERMOST_ID] {
        if let Some(app) = find(id) {
            rows.push(shortcut_from_desktop(app));
        }
    }
    rows
}

fn shortcut_from_desktop(app: &Row) -> Row {
    let mut row = app.clone();
    if row.id.is_empty() {
        row.id = row.desktop_id.clone();
    }
    row
}

fn wrapper_row(id: &str, title: &str, hint: &str, argv: &[&str]) -> Row {
    Row {
        id: id.into(),
        title: title.into(),
        hint: hint.into(),
        verb: "Open".into(),
        argv: argv.iter().map(|part| (*part).to_string()).collect(),
        ..Row::default()
    }
}

fn showing_shortcuts(model: &Model) -> bool {
    model.query.trim().is_empty()
        && model.active == "root"
        && !side_open(model)
        && matches!(model.view, View::Tree)
        && !model.shortcut_rows().is_empty()
}

fn rows_origin(model: &Model) -> f32 {
    if showing_shortcuts(model) {
        PILL_PAD + 24.0
    } else {
        list_top()
    }
}

/// `--no-exec` refuses the launch. The preview must not say Enter opens it.
fn refused_preview(hint: String, verb: &str, refused: bool) -> (String, String) {
    if !refused {
        return (hint, verb.to_string());
    }
    let hint = hint.replace("One Enter opens it.", "Enter does not start it.");
    let verb = if verb == "Open" || verb == "Run…" {
        "Dry run"
    } else {
        verb
    };
    (hint, verb.to_string())
}

fn listed_row(item: providers::ListedRow, no_exec: bool, live: bool, safe_apps: bool) -> Row {
    let verb = if item.unavailable {
        "Unavailable"
    } else if item.input_at.is_some() {
        "Run…"
    } else if item.dangerous {
        "Confirm…"
    } else {
        "Open"
    };
    let needs_input = item.input_at.is_some();
    let admitted = safe_apps
        && crate::safe_apps::admit_configured(crate::safe_apps::AppFact {
            desktop_id: &item.desktop_id,
            desktop_file: &item.desktop_file,
            argv: &item.argv,
            terminal: item.terminal,
            needs_input,
            dangerous: item.dangerous,
            unavailable: item.unavailable,
        })
        .is_ok();
    let refused = if item.argv.is_empty() {
        no_exec && !live
    } else if let Ok(plan) = dispatch::plan_argv(&item.argv) {
        dispatch::gate_spawn(no_exec, live, safe_apps, admitted, &plan, item.dangerous).is_err()
    } else {
        true
    };
    let (hint, verb) = refused_preview(item.hint, verb, refused);
    Row {
        id: item.desktop_id.clone(),
        title: item.title,
        hint,
        verb,
        desktop_id: item.desktop_id,
        desktop_file: item.desktop_file,
        argv: item.argv,
        input_at: item.input_at,
        needs_input,
        unavailable: item.unavailable,
        dangerous: item.dangerous,
        terminal: item.terminal,
        ..Row::default()
    }
}

fn accept_activation(repeat: bool) -> bool {
    !repeat
}

fn trace_field(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            ch if ch.is_control() => out.push(' '),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn refusal_code(status: &str) -> &'static str {
    if status.contains("Dry run") {
        "dry-run"
    } else if status.contains("Cancelled") {
        "cancelled"
    } else if status.contains("Stale") || status.contains("outdated") {
        "stale"
    } else if status.contains("empty") {
        "empty"
    } else if status.contains("Nothing was started") {
        "refused"
    } else {
        "none"
    }
}

/// One line of internal state. Printed only when `--trace-state` is passed.
fn state_trace(model: &Model) -> String {
    let rows = model.rows();
    let (selected, verb) = rows
        .get(model.cursor)
        .map(|row| (row.title.as_str(), row.verb.as_str()))
        .unwrap_or(("", ""));
    let (confirm, focus) = match &model.confirm {
        None => ("none", "none"),
        Some(choice) if choice.focus == 0 => ("open", "cancel"),
        Some(_) => ("open", "confirm"),
    };
    format!(
        "query={} selected={} verb={} confirm={} focus={} close={} refusal={}",
        trace_field(&model.query),
        trace_field(selected),
        trace_field(verb),
        confirm,
        focus,
        if model.should_close { "1" } else { "0" },
        refusal_code(&model.status),
    )
}

fn emit_state_trace(model: &Model) {
    eprintln!("omarchy-menu-rust: state {}", state_trace(model));
}

const VERB_X: f32 = 48.0;
const TITLE_X: f32 = 188.0;
const CHOICE_W: f32 = 168.0;
const CHOICE_H: f32 = 32.0;

/// True when the point is inside the rounded pill. The surface itself stays a
/// rectangle, so a corner click is delivered here and then ignored.
fn inside_pill(x: f32, y: f32, width: f32, height: f32) -> bool {
    let w = width.min(PILL_W);
    let h = height.clamp(1.0, MAX_PILL_H);
    if x < 0.0 || y < 0.0 || x >= w || y >= h {
        return false;
    }
    let r = PILL_RADIUS.min(w / 2.0).min(h / 2.0);
    let dx = if x < r {
        r - x
    } else if x > w - r {
        x - (w - r)
    } else {
        0.0
    };
    let dy = if y < r {
        r - y
    } else if y > h - r {
        y - (h - r)
    } else {
        0.0
    };
    dx * dx + dy * dy <= r * r
}

fn confirm_button_at(model: &Model, x: f32, y: f32, width: f32, height: f32) -> Option<usize> {
    if !inside_pill(x, y, width, height) {
        return None;
    }
    model.confirm.as_ref()?;
    let x0 = LIST_X;
    let y0 = list_top() + 132.0;
    if x < x0 || x > x0 + CHOICE_W {
        return None;
    }
    if (y0..y0 + CHOICE_H).contains(&y) {
        Some(0)
    } else if (y0 + 40.0..y0 + 40.0 + CHOICE_H).contains(&y) {
        Some(1)
    } else {
        None
    }
}

/// Index of a painted row under a physical pixel, if the point lands on one.
fn row_index_at(model: &Model, x: f32, y: f32, width: f32, height: f32) -> Option<usize> {
    if !inside_pill(x, y, width, height) {
        return None;
    }
    let rows = model.rows();
    let list_w = list_width(model, width);
    let (start, fit, pitch) = row_window(model, height);
    let row_h = pitch - ROW_PAD;
    for (i, _) in rows.iter().enumerate().skip(start).take(fit) {
        let row_y = rows_origin(model) + (i - start) as f32 * pitch;
        if x >= LIST_X && x < LIST_X + list_w && y >= row_y && y < row_y + row_h {
            return Some(i);
        }
    }
    None
}

fn paint_model(model: &Model, width: u32, height: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(width.max(1), height.max(1)).expect("pixmap");
    let font = load_ui_font(&model.theme);
    let theme = model.theme.clone();
    pixmap.fill(color(0, 0, 0, 0));
    let w = width as f32;
    let h = height as f32;
    let pill_w = w.min(PILL_W);
    let pill_h = h.min(MAX_PILL_H);
    fill_round_rect(
        &mut pixmap,
        0.0,
        0.0,
        pill_w,
        pill_h,
        PILL_RADIUS,
        color(theme.card.r, theme.card.g, theme.card.b, PANEL_ALPHA),
    );
    let search_top = if showing_shortcuts(model) {
        (pill_h - PILL_PAD - SEARCH_H).max(PILL_PAD)
    } else {
        PILL_PAD
    };
    shade_mark(
        &mut pixmap,
        PILL_PAD + 8.0,
        search_top + 16.0,
        color(theme.accent.r, theme.accent.g, theme.accent.b, 255),
    );
    let text = (theme.text.r, theme.text.g, theme.text.b);
    let muted = (theme.muted.r, theme.muted.g, theme.muted.b);
    let accent = (theme.accent.r, theme.accent.g, theme.accent.b);
    let (shown, qcolor) = if let Some(confirm) = &model.confirm {
        (confirm.summary.clone(), text)
    } else {
        match &model.view {
            View::Input {
                purpose: InputPurpose::Command { .. },
                buffer,
            } if buffer.is_empty() => ("File or URL".to_string(), muted),
            View::Input { buffer, .. } if buffer.is_empty() => ("Filter".to_string(), muted),
            View::Input { buffer, .. } => (buffer.clone(), text),
            _ if model.query.is_empty() => (String::new(), text),
            _ => (model.query.clone(), text),
        }
    };
    draw_text(
        &mut pixmap,
        &font,
        &shown,
        PILL_PAD + 40.0,
        search_top + 36.0,
        theme.search_px(),
        qcolor,
    );
    if showing_shortcuts(model) {
        draw_text(
            &mut pixmap,
            &font,
            "Shortcuts",
            LIST_X,
            PILL_PAD + 16.0,
            theme.hint_px(),
            muted,
        );
    }

    let rows = model.rows();
    let top = rows_origin(model);
    let (start, fit, row_h) = row_window(model, h);
    let list_w = list_width(model, w);
    if start > 0 {
        draw_text(
            &mut pixmap,
            &font,
            "↑ more above",
            w - 210.0,
            168.0,
            theme.hint_px(),
            accent,
        );
    }
    if start + fit < rows.len() {
        draw_text(
            &mut pixmap,
            &font,
            "↓ more below",
            w - 210.0,
            h - 48.0,
            theme.hint_px(),
            muted,
        );
    }
    if !side_open(model) {
        for (i, row) in rows.iter().enumerate().skip(start).take(fit) {
            let y = top + (i - start) as f32 * row_h;
            if i == model.cursor {
                stroke_rect(
                    &mut pixmap,
                    LIST_X,
                    y,
                    list_w,
                    row_h - ROW_PAD,
                    color(text.0, text.1, text.2, 255),
                );
            }
            draw_text(
                &mut pixmap,
                &font,
                &clip(&font, &row.verb, theme.hint_px(), TITLE_X - VERB_X - 12.0),
                VERB_X,
                y + 30.0,
                theme.hint_px(),
                text,
            );
            draw_text(
                &mut pixmap,
                &font,
                &clip(
                    &font,
                    &row.title,
                    theme.list_px(),
                    list_w - (TITLE_X - 20.0),
                ),
                TITLE_X,
                y + 22.0,
                theme.list_px(),
                text,
            );
            draw_text(
                &mut pixmap,
                &font,
                &clip(&font, &row.hint, theme.hint_px(), list_w - (TITLE_X - 20.0)),
                TITLE_X,
                y + 40.0,
                theme.hint_px(),
                muted,
            );
        }
    }
    if rows.is_empty() && !model.query.trim().is_empty() && !side_open(model) {
        draw_text(
            &mut pixmap,
            &font,
            "No row matches.",
            LIST_X + 12.0,
            list_top() + 28.0,
            theme.list_px(),
            muted,
        );
    }

    if side_open(model) {
        paint_side(
            &mut pixmap,
            &font,
            model,
            &theme,
            LIST_X,
            top,
            pill_w - PILL_PAD * 2.0,
        );
    }
    pixmap
}

struct Choice<'a> {
    label: &'a str,
    x: f32,
    y: f32,
    focused: bool,
}

fn draw_choice(pixmap: &mut Pixmap, font: &Font, theme: &theme::ThemeTokens, choice: Choice<'_>) {
    let fill = if choice.focused {
        theme.selection
    } else {
        theme.elevated
    };
    rect(
        pixmap,
        choice.x,
        choice.y,
        CHOICE_W,
        CHOICE_H,
        color(fill.r, fill.g, fill.b, 255),
    );
    if choice.focused {
        stroke_rect(
            pixmap,
            choice.x,
            choice.y,
            CHOICE_W,
            CHOICE_H,
            color(theme.text.r, theme.text.g, theme.text.b, 255),
        );
    }
    let ink = theme.text;
    draw_text(
        pixmap,
        font,
        choice.label,
        choice.x + 14.0,
        choice.y + 22.0,
        theme.list_px(),
        (ink.r, ink.g, ink.b),
    );
}

fn paint_side(
    pixmap: &mut Pixmap,
    font: &Font,
    model: &Model,
    theme: &theme::ThemeTokens,
    x: f32,
    y: f32,
    width: f32,
) {
    let text = (theme.text.r, theme.text.g, theme.text.b);
    let muted = (theme.muted.r, theme.muted.g, theme.muted.b);
    let accent = (theme.accent.r, theme.accent.g, theme.accent.b);
    let hint = theme.hint_px();
    let header = theme.header_px();
    let list = theme.list_px();
    if let Some(confirm) = &model.confirm {
        let mut dy = 22.0;
        for line in wrap(&confirm.lead, 42) {
            draw_text(pixmap, font, &line, x, y + dy, list, text);
            dy += 20.0;
        }
        draw_text(pixmap, font, "Command", x, y + dy + 6.0, hint, muted);
        dy += 24.0;
        for line in wrap(&confirm.effect, 42).into_iter().take(3) {
            draw_text(pixmap, font, &line, x, y + dy, hint, muted);
            dy += 16.0;
        }
        draw_choice(
            pixmap,
            font,
            theme,
            Choice {
                label: "Cancel",
                x,
                y: y + 132.0,
                focused: confirm.focus == 0,
            },
        );
        draw_choice(
            pixmap,
            font,
            theme,
            Choice {
                label: "Confirm",
                x,
                y: y + 172.0,
                focused: confirm.focus == 1,
            },
        );
        return;
    }
    if let Some(notice) = &model.notice {
        draw_text(pixmap, font, "DRY RUN", x, y, header, accent);
        let mut dy = 32.0;
        for line in wrap(notice, 42) {
            draw_text(pixmap, font, &line, x, y + dy, list, text);
            dy += 20.0;
        }
        return;
    }
    if let Some(body) = &model.answer {
        draw_text(pixmap, font, "ANSWER", x, y, header, accent);
        let mut dy = 28.0;
        for line in wrap(body, 42) {
            draw_text(pixmap, font, &line, x, y + dy, list, text);
            dy += 20.0;
        }
        draw_choice(
            pixmap,
            font,
            theme,
            Choice {
                label: "Copy",
                x,
                y: y + dy + 8.0,
                focused: model.copy_focus,
            },
        );
        draw_text(
            pixmap,
            font,
            "Copy uses a system clipboard tool. If none is found, nothing is copied.",
            x,
            y + dy + 48.0,
            hint,
            muted,
        );
        return;
    }
    draw_text(pixmap, font, "NOT CONNECTED", x, y, header, accent);
    let title = panel_title(model);
    draw_text(
        pixmap,
        font,
        &clip(font, &title, theme.search_px(), width),
        x,
        y + 28.0,
        theme.search_px(),
        text,
    );
    draw_text(
        pixmap,
        font,
        "Unavailable. This panel is not a live inbox, grocery list, or contact list.",
        x,
        y + 58.0,
        hint,
        muted,
    );
    draw_text(
        pixmap,
        font,
        "No account was read and no message was sent.",
        x,
        y + 80.0,
        theme.body_px(),
        text,
    );
    if let Some(id) = model.panel.strip_prefix("bot:") {
        let name = bots::find(&model.bots, id)
            .map(|bot| bot.name.as_str())
            .unwrap_or("Bot");
        draw_text(
            pixmap,
            font,
            &format!("{name} is a local roster id."),
            x,
            y + 108.0,
            list,
            text,
        );
        draw_text(pixmap, font, id, x, y + 132.0, header, muted);
    }
}

fn panel_title(model: &Model) -> String {
    match model.panel.as_str() {
        "groceries" => "Groceries".into(),
        "inbox" => "Messages".into(),
        "recent" => "Recent work".into(),
        "decisions" => "Open decisions".into(),
        "example-a" => "Example A".into(),
        "example-b" => "Example B".into(),
        other if other.starts_with("bot:") => "Bot".into(),
        other => other.to_string(),
    }
}

fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn fill_round_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    let mut path = tiny_skia::PathBuilder::new();
    let r = radius.min(w / 2.0).min(h / 2.0);
    path.move_to(x + r, y);
    path.line_to(x + w - r, y);
    path.quad_to(x + w, y, x + w, y + r);
    path.line_to(x + w, y + h - r);
    path.quad_to(x + w, y + h, x + w - r, y + h);
    path.line_to(x + r, y + h);
    path.quad_to(x, y + h, x, y + h - r);
    path.line_to(x, y + r);
    path.quad_to(x, y, x + r, y);
    path.close();
    let Some(path) = path.finish() else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(color);
    pixmap.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
}

fn shade_mark(pixmap: &mut Pixmap, x: f32, y: f32, color: Color) {
    fill_round_rect(pixmap, x, y, 4.0, 16.0, 2.0, color);
    fill_round_rect(pixmap, x + 7.0, y + 6.0, 4.0, 16.0, 2.0, color);
}

fn stroke_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    rect(pixmap, x, y, w, 1.0, color);
    rect(pixmap, x, y + h - 1.0, w, 1.0, color);
    rect(pixmap, x, y, 1.0, h, color);
    rect(pixmap, x + w - 1.0, y, 1.0, h, color);
}

fn rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(color);
    pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
}

fn draw_text(
    pixmap: &mut Pixmap,
    font: &Font,
    text: &str,
    x: f32,
    baseline: f32,
    size: f32,
    rgb: (u8, u8, u8),
) {
    let mut pen = x;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        blit(
            pixmap,
            &bitmap,
            metrics.width,
            metrics.height,
            pen + metrics.xmin as f32,
            baseline - metrics.height as f32 - metrics.ymin as f32,
            rgb,
        );
        pen += metrics.advance_width;
    }
}

fn text_width(font: &Font, text: &str, size: f32) -> f32 {
    text.chars()
        .map(|ch| font.metrics(ch, size).advance_width)
        .sum()
}

fn clip(font: &Font, text: &str, size: f32, max: f32) -> String {
    if text_width(font, text, size) <= max {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let next = format!("{out}{ch}");
        if text_width(font, &format!("{next}…"), size) > max {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.len() + word.len() + 1 > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn blit(pixmap: &mut Pixmap, bitmap: &[u8], w: usize, h: usize, x: f32, y: f32, rgb: (u8, u8, u8)) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    for row in 0..h {
        for col in 0..w {
            let cov = bitmap[row * w + col] as f32 / 255.0;
            if cov < 0.01 {
                continue;
            }
            let px = x as i32 + col as i32;
            let py = y as i32 + row as i32;
            if px < 0 || py < 0 || px >= pw || py >= ph {
                continue;
            }
            let idx = (py as u32 * pixmap.width() + px as u32) as usize;
            let dst = pixmap.pixels()[idx];
            let dr = dst.red() as f32;
            let dg = dst.green() as f32;
            let db = dst.blue() as f32;
            let inv = 1.0 - cov;
            let r = (rgb.0 as f32 * cov + dr * inv).round() as u8;
            let g = (rgb.1 as f32 * cov + dg * inv).round() as u8;
            let b = (rgb.2 as f32 * cov + db * inv).round() as u8;
            let a = (255.0 * cov + dst.alpha() as f32 * (1.0 - cov)).round() as u8;
            let r = r.min(a);
            let g = g.min(a);
            let b = b.min(a);
            pixmap.pixels_mut()[idx] = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
                .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
        }
    }
}

pub fn write_png(path: &std::path::Path, pixmap: &Pixmap) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        pixmap.width(),
        pixmap.height(),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(pixmap.data())
        .map_err(|e| e.to_string())?;
    Ok(())
}

enum UserMsg {
    Child(String),
    Theme,
}

struct App {
    model: Model,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    opts: Options,
    wrote_shot: bool,
    traced: bool,
    pointer: Option<(f64, f64)>,
    proxy: winit::event_loop::EventLoopProxy<UserMsg>,
    _theme_watch: Option<notify::RecommendedWatcher>,
}

impl winit::application::ApplicationHandler<UserMsg> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (pill_w, pill_h) = pill_size(&self.model);
        let attrs = Window::default_attributes()
            .with_title("shadetheory")
            .with_name("omarchy-menu-rust", "omarchy-menu-rust")
            .with_inner_size(LogicalSize::new(pill_w as f64, pill_h as f64))
            .with_resizable(false)
            .with_visible(true);
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("omarchy-menu-rust: window was not created: {err}");
                event_loop.exit();
                return;
            }
        };
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(ctx) => ctx,
            Err(err) => {
                eprintln!("omarchy-menu-rust: framebuffer was not created: {err}");
                event_loop.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                eprintln!("omarchy-menu-rust: surface was not created: {err}");
                event_loop.exit();
                return;
            }
        };
        self.surface = Some(surface);
        center_x11_window(&window, pill_w, pill_h);
        window.request_redraw();
        self.window = Some(window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserMsg) {
        match event {
            UserMsg::Child(text) => self.model.status = text,
            UserMsg::Theme => {
                self.model.theme = theme::reload_theme(&self.model.theme, &theme::default_paths());
                self.model.status = "Theme refreshed.".into();
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface) = self.surface.as_mut() {
                    let _ = surface.resize(
                        std::num::NonZeroU32::new(size.width.max(1)).unwrap(),
                        std::num::NonZeroU32::new(size.height.max(1)).unwrap(),
                    );
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = Some((position.x, position.y));
                let size = self
                    .window
                    .as_ref()
                    .map(|window| window.inner_size())
                    .unwrap_or_default();
                self.model.highlight_at(
                    position.x as f32,
                    position.y as f32,
                    size.width.max(1) as f32,
                    size.height.max(1) as f32,
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.on_left_click();
                if self.model.should_close {
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let proxy = self.proxy.clone();
                self.on_key(event_loop, event.logical_key, event.repeat, proxy);
            }
            WindowEvent::RedrawRequested => {
                self.redraw(event_loop);
            }
            _ => {}
        }
    }
}

impl App {
    fn trace_state(&mut self) {
        if self.opts.trace_state {
            emit_state_trace(&self.model);
        }
    }

    /// Hover only highlights. One click activates the row or the Cancel/Confirm control under the pointer.
    fn on_left_click(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some((px, py)) = self.pointer else {
            return;
        };
        let size = window.inner_size();
        let width = size.width.max(1) as f32;
        let height = size.height.max(1) as f32;
        let proxy = self.proxy.clone();
        if let Some(focus) = confirm_button_at(&self.model, px as f32, py as f32, width, height) {
            if let Some(confirm) = &mut self.model.confirm {
                confirm.focus = focus;
            }
            self.model.enter(move |text| {
                let _ = proxy.send_event(UserMsg::Child(text));
            });
        } else if let Some(index) = row_index_at(&self.model, px as f32, py as f32, width, height) {
            self.model.cursor = index;
            self.model.confirm = None;
            self.model.copy_focus = false;
            self.model.enter(move |text| {
                let _ = proxy.send_event(UserMsg::Child(text));
            });
        }
        self.model.clamp();
        self.trace_state();
        window.request_redraw();
    }

    fn on_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        key: Key,
        repeat: bool,
        proxy: winit::event_loop::EventLoopProxy<UserMsg>,
    ) {
        match key {
            Key::Named(NamedKey::Escape) => {
                self.model.go_back();
                if self.model.should_close {
                    self.trace_state();
                    event_loop.exit();
                    return;
                }
            }
            Key::Named(NamedKey::Enter) => {
                if !accept_activation(repeat) {
                    self.trace_state();
                    return;
                }
                self.model.enter(move |text| {
                    let _ = proxy.send_event(UserMsg::Child(text));
                });
                if self.model.should_close {
                    self.trace_state();
                    event_loop.exit();
                    return;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.model.confirm.is_some() {
                    if let Some(confirm) = &mut self.model.confirm {
                        confirm.focus = 0;
                    }
                } else if self.model.copy_focus {
                    self.model.copy_focus = false;
                } else {
                    self.model.cursor = self.model.cursor.saturating_sub(1);
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.model.confirm.is_some() {
                    if let Some(confirm) = &mut self.model.confirm {
                        confirm.focus = 1;
                    }
                } else if self.model.answer.is_some() && !self.model.copy_focus {
                    self.model.copy_focus = true;
                } else {
                    self.model.cursor = self.model.cursor.saturating_add(1);
                    self.model.clamp();
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if self.model.confirm.is_some() || !self.model.panel.is_empty() {
                    // Confirm and unavailable panels do not take typed text.
                } else if let View::Input { buffer, .. } = &mut self.model.view {
                    buffer.pop();
                } else {
                    self.model.query.pop();
                    self.model.cursor = 0;
                }
            }
            Key::Character(ch) => {
                if self.model.confirm.is_some() || !self.model.panel.is_empty() {
                    self.trace_state();
                    return;
                }
                if let View::Input { buffer, .. } = &mut self.model.view {
                    let text = ch.as_str();
                    if text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
                        && buffer.len() < 64
                    {
                        buffer.push_str(text);
                    }
                } else {
                    self.model.query.push_str(ch.as_str());
                    self.model.cursor = 0;
                    self.model.note_results();
                }
            }
            _ => {}
        }
        self.model.clamp();
        self.trace_state();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.opts.trace_state && !self.traced {
            self.traced = true;
            emit_state_trace(&self.model);
        }
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let (pill_w, pill_h) = pill_size(&self.model);
        center_x11_window(&window, pill_w, pill_h);
        let size = window
            .request_inner_size(LogicalSize::new(pill_w as f64, pill_h as f64))
            .unwrap_or_else(|| window.inner_size());
        let width = size.width.max(1);
        let height = size.height.max(1);
        let _ = surface.resize(
            std::num::NonZeroU32::new(width).unwrap(),
            std::num::NonZeroU32::new(height).unwrap(),
        );
        let pixmap = paint_model(&self.model, width, height);
        if let Some(path) = &self.opts.screenshot {
            if !self.wrote_shot {
                match write_png(path, &pixmap) {
                    Ok(()) => eprintln!("omarchy-menu-rust: wrote {}", path.display()),
                    Err(err) => eprintln!("omarchy-menu-rust: screenshot failed: {err}"),
                }
                self.wrote_shot = true;
            }
        }
        let mut buffer = match surface.buffer_mut() {
            Ok(buf) => buf,
            Err(err) => {
                eprintln!("omarchy-menu-rust: buffer failed: {err}");
                return;
            }
        };
        let pixels = pixmap.pixels();
        let plate = self.model.theme.card;
        for (i, px) in pixels.iter().enumerate() {
            if i >= buffer.len() {
                break;
            }
            let (r, g, b) = if px.alpha() < 16 {
                (plate.r, plate.g, plate.b)
            } else {
                let a = px.alpha() as u32;
                (
                    (px.red() as u32 * 255 / a).min(255) as u8,
                    (px.green() as u32 * 255 / a).min(255) as u8,
                    (px.blue() as u32 * 255 / a).min(255) as u8,
                )
            };
            buffer[i] = b as u32 | ((g as u32) << 8) | ((r as u32) << 16);
        }
        if buffer.present().is_err() {
            eprintln!("omarchy-menu-rust: present failed");
        }
        let shot_done = self.opts.screenshot.is_none() || self.wrote_shot;
        if self.opts.exit_after_first_frame && shot_done {
            event_loop.exit();
        }
    }
}

fn prepare_model(menu: Menu, opts: &Options) -> Model {
    let mut model = Model::new(menu, opts.no_exec);
    model.live = opts.live;
    model.safe_apps = opts.safe_apps;
    model.theme = theme::load_theme(Some(&theme::default_paths()));
    if let Ok(raw) = std::env::var("OMARCHY_MENU_TERMINAL") {
        match crate::desktop::parse_terminal_argv(&raw) {
            Ok(argv) => model.terminal = argv,
            Err(err) => {
                model.status =
                    format!("Terminal argv was refused ({err}). Terminal apps stay unavailable.");
            }
        }
    }
    model.load_apps();
    let guard_items: Vec<_> = model
        .menu
        .order
        .iter()
        .filter_map(|id| model.menu.items.get(id))
        .cloned()
        .collect();
    let facts = if opts.run_guards {
        crate::guards::evaluate_items_allowed(&guard_items, &load_guard_allow())
    } else {
        crate::guards::evaluate_items(&guard_items)
    };
    model.apply_guards(&facts);
    model
}

#[cfg(test)]
fn paint_pill(model: &Model) -> Pixmap {
    let (w, h) = pill_size(model);
    paint_model(model, w, h)
}

#[cfg(test)]
fn composite_over(pixmap: &Pixmap, base: impl Fn(u32, u32) -> (u8, u8, u8)) -> Pixmap {
    let mut out = Pixmap::new(pixmap.width(), pixmap.height()).expect("pixmap");
    for (i, px) in pixmap.pixels().iter().enumerate() {
        let x = i as u32 % pixmap.width();
        let y = i as u32 / pixmap.width();
        let (br, bg, bb) = base(x, y);
        let a = px.alpha() as u32;
        let mix = |src: u8, dst: u8| -> u8 {
            let Some(straight) = (src as u32).checked_mul(255).and_then(|v| v.checked_div(a))
            else {
                return dst;
            };
            ((straight * a + dst as u32 * (255 - a)) / 255) as u8
        };
        let (r, g, b) = (mix(px.red(), br), mix(px.green(), bg), mix(px.blue(), bb));
        out.pixels_mut()[i] = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255)
            .unwrap_or(tiny_skia::PremultipliedColorU8::from_rgba(255, 255, 255, 255).unwrap());
    }
    out
}

fn center_x11_window(window: &Window, panel_w: u32, panel_h: u32) {
    let Some(monitor) = window
        .current_monitor()
        .or_else(|| window.available_monitors().next())
    else {
        return;
    };
    let scale = monitor.scale_factor();
    let size = monitor.size();
    let pos = monitor.position();
    let output = crate::place::OutputBox {
        origin_x: crate::place::logical_px(pos.x, scale),
        origin_y: crate::place::logical_px(pos.y, scale),
        logical_w: crate::place::logical_len(size.width as i32, scale),
        logical_h: crate::place::logical_len(size.height as i32, scale),
    };
    let placed = crate::place::place_panel(output, panel_w, panel_h);
    window.set_outer_position(winit::dpi::LogicalPosition::new(
        placed.x as f64,
        placed.y as f64,
    ));
}

fn pill_size(model: &Model) -> (u32, u32) {
    let row_h = model.theme.row_h();
    let rows = model.rows().len();
    let (_, fit, _) = row_window(model, MAX_PILL_H);
    let shown = rows.min(fit);
    let h = if side_open(model) {
        (list_top() + 220.0).min(MAX_PILL_H)
    } else if showing_shortcuts(model) {
        (PILL_PAD + 24.0 + shown as f32 * row_h + 8.0 + SEARCH_H + PILL_PAD).min(MAX_PILL_H)
    } else if shown == 0 {
        PILL_PAD * 2.0 + SEARCH_H
    } else {
        (list_top() + shown as f32 * row_h + 12.0).min(MAX_PILL_H)
    };
    (PILL_W.round() as u32, h.round().max(72.0) as u32)
}

pub fn run(menu: Menu, opts: Options) -> Result<(), String> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return present_wayland::run(menu, opts);
    }
    if std::env::var_os("DISPLAY").is_some() {
        eprintln!(
            "omarchy-menu-rust: X11 opaque fallback. The desktop does not show through this window."
        );
    }
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Some(path) = &opts.screenshot {
            let pixmap = paint(&menu, "", 0, 1200, 760);
            write_png(path, &pixmap)?;
            eprintln!("omarchy-menu-rust: no display. Wrote an offscreen frame only.");
            return Ok(());
        }
        return Err("no DISPLAY or WAYLAND_DISPLAY. The window was not opened.".into());
    }
    let event_loop = EventLoop::<UserMsg>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    let mut app = App {
        model: prepare_model(menu, &opts),
        window: None,
        surface: None,
        wrote_shot: false,
        traced: false,
        pointer: None,
        proxy: event_loop.create_proxy(),
        opts,
        _theme_watch: {
            let proxy = event_loop.create_proxy();
            theme::spawn_theme_watcher(move || {
                let _ = proxy.send_event(UserMsg::Theme);
            })
        },
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::stock_menu;

    #[test]
    fn paint_uses_halloween_background() {
        let pixmap = paint(&stock_menu(), "check on my groceries", 0, 800, 500);
        let corner = pixmap.pixels()[0];
        assert_eq!(corner.alpha(), 0);
        let plate = pixmap.pixels()[(40 * pixmap.width() as usize) + 80];
        assert!(plate.alpha() > 20 && plate.alpha() < 255);
        assert!(pixmap
            .pixels()
            .iter()
            .any(|p| p.alpha() == 255 && p.red() > 80));
        let bg = pixmap
            .pixels()
            .iter()
            .find(|p| p.alpha() > 20 && p.alpha() < 255)
            .unwrap();
        let over = |base: u8| -> u8 {
            let a = bg.alpha() as u32;
            let straight = bg.red() as u32 * 255 / a;
            ((straight * a + base as u32 * (255 - a)) / 255) as u8
        };
        assert_ne!(over(0), over(255));
        let ink = pixmap
            .pixels()
            .iter()
            .find(|p| p.alpha() == 255 && p.red() > 40)
            .unwrap();
        let ink_over = |base: u8| -> u8 {
            let a = ink.alpha() as u32;
            let straight = ink.red() as u32 * 255 / a.max(1);
            ((straight * a + base as u32 * (255 - a)) / 255) as u8
        };
        assert_eq!(ink_over(0), ink_over(255));
    }

    #[test]
    fn left_click_on_a_visible_row_activates_like_enter() {
        let menu = stock_menu();
        let mut by_click = Model::new(menu.clone(), true);
        by_click.query = "learn".into();
        let y = list_top() + 6.0;
        let index = row_index_at(&by_click, 80.0, y, 1200.0, 760.0).unwrap();
        assert_eq!(index, 0);
        by_click.cursor = index;
        by_click.confirm = None;
        by_click.activate();

        let mut by_enter = Model::new(menu, true);
        by_enter.query = "learn".into();
        by_enter.cursor = 0;
        by_enter.activate();
        assert_eq!(by_click.active, "learn");
        assert_eq!(by_click.active, by_enter.active);
        assert!(by_click.confirm.is_none());
        assert_eq!(by_click.panel, by_enter.panel);
        assert!(row_index_at(&by_click, 2.0, list_top() + 6.0, 1200.0, 760.0).is_none());
    }

    #[test]
    fn fixture_theme_paints_panel_colors_not_the_halloween_card() {
        let dir = std::env::temp_dir().join(format!("omr-paint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("colors.toml"),
            "background = \"#0A1420\"\nforeground = \"#F2F7FC\"\naccent = \"#5AC8FA\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("shell.toml"),
            "[font]\nbase-size = 12\n[menu]\nbackground = \"#0A1420\"\ntext = \"#F2F7FC\"\nselected-background = \"#0A3A6A\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("user.toml"), "[font]\nbase-size = 20\n").unwrap();
        std::fs::write(
            dir.join("ghostty.conf"),
            "font-family = Fixture Face\nfont-size = 12\n",
        )
        .unwrap();
        let loaded = theme::load_theme(Some(&theme::ThemePaths {
            colors_toml: Some(dir.join("colors.toml")),
            shell_toml: Some(dir.join("shell.toml")),
            user_shell: Some(dir.join("user.toml")),
            ghostty_theme: Some(dir.join("ghostty.conf")),
            ghostty_user: None,
            font_file: Some(PathBuf::from(
                "preview/assets/fonts/JetBrainsMono-Regular.ttf",
            )),
            menu_file: None,
        }));
        assert_eq!(loaded.font_px, 20);
        assert_eq!(loaded.search_px(), 20.0);
        let mut model = Model::new(stock_menu(), true);
        model.theme = loaded;
        let pixmap = paint_model(&model, 800, 500);
        let fill = pixmap.pixels()[(40 * pixmap.width() as usize) + 80];
        assert!(fill.alpha() > 20 && fill.alpha() < 255);
        std::fs::write(
            dir.join("colors.toml"),
            "background = \"#112233\"\nforeground = \"#F2F7FC\"\naccent = \"#5AC8FA\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("shell.toml"),
            "[font]\nbase-size = 12\n[menu]\nbackground = \"#112233\"\ntext = \"#F2F7FC\"\nselected-background = \"#0A3A6A\"\n",
        )
        .unwrap();
        model.theme = theme::reload_theme(
            &model.theme,
            &theme::ThemePaths {
                colors_toml: Some(dir.join("colors.toml")),
                shell_toml: Some(dir.join("shell.toml")),
                user_shell: Some(dir.join("user.toml")),
                ghostty_theme: Some(dir.join("ghostty.conf")),
                ghostty_user: None,
                font_file: Some(PathBuf::from(
                    "preview/assets/fonts/JetBrainsMono-Regular.ttf",
                )),
                menu_file: None,
            },
        );
        let again = paint_model(&model, 800, 500);
        let fill2 = again.pixels()[(100 * again.width() as usize) + 200];
        assert!(fill2.alpha() > 20 && fill2.alpha() < 255);
        let straight = |p: tiny_skia::PremultipliedColorU8| -> (u8, u8, u8) {
            let a = p.alpha() as u32;
            (
                (p.red() as u32 * 255 / a) as u8,
                (p.green() as u32 * 255 / a) as u8,
                (p.blue() as u32 * 255 / a) as u8,
            )
        };
        let before = pixmap.pixels()[(100 * pixmap.width() as usize) + 200];
        assert_ne!(straight(before), straight(fill2));
        assert!(straight(fill2).0 > straight(before).0);
        let card = pixmap.pixels()[(40 * 800 + 40) as usize];
        assert_ne!((card.red(), card.green(), card.blue()), (0x0c, 0x14, 0x0e));
        if let Some(parent) = std::path::Path::new("/opt/cursor/artifacts").parent() {
            if parent.exists() || std::path::Path::new("/opt/cursor/artifacts").exists() {
                let _ = write_png(
                    std::path::Path::new(
                        "/opt/cursor/artifacts/omarchy-menu-rust-theme-fixture.png",
                    ),
                    &pixmap,
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_exec_confirm_does_not_spawn() {
        let mut model = Model::new(stock_menu(), true);
        model.query = "About".into();
        let index = model
            .rows()
            .iter()
            .position(|row| row.id == "about")
            .unwrap();
        assert_eq!(model.rows()[index].verb, "Confirm…");
        model.cursor = index;
        model.activate();
        assert_eq!(model.confirm.as_ref().unwrap().focus, 0);
        model.activate();
        assert!(model.confirm.is_none());
        assert!(model.status.contains("Cancelled"));
        model.cursor = index;
        model.activate();
        model.confirm.as_mut().unwrap().focus = 1;
        model.activate();
        assert!(model.status.contains("Dry run"));
        assert!(model
            .notice
            .as_ref()
            .unwrap()
            .contains("Nothing was started"));
        assert!(model.status.contains("Nothing was started"));
        assert!(!model.status.to_lowercase().contains("success"));
        assert!(!model.status.contains("finished ok"));
        assert!(!model.should_close);
    }

    #[test]
    fn interaction_highlight_input_cancel_stale_and_live_argv() {
        let dir = std::env::temp_dir().join(format!("omr-flow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("ran");
        let script = dir.join("demo-app");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" > '{}'\n",
                marker.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        std::fs::write(
            dir.join("demo.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Demo App\nExec=\"{}\" \"a;b$(reboot)\"\n",
                script.display()
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("needs.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Needs Arg\nExec={} %u\n",
                script.display()
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("broken.desktop"),
            "[Desktop Entry]\nType=Application\nName=Broken\nExec=broken a;rm\n",
        )
        .unwrap();

        let mut model = Model::new(stock_menu(), true);
        model.live = true;
        model.app_dirs = vec![dir.clone()];
        let saved_query = "learn".to_string();
        model.query = saved_query.clone();
        model.cursor = 4;
        model.highlight_at(80.0, list_top() + 8.0, 1200.0, 760.0);
        assert_eq!(model.cursor, 0);
        assert!(model.confirm.is_none());
        assert!(!model.should_close);
        assert!(!marker.exists());
        assert!(!accept_activation(true));
        assert!(accept_activation(false));

        model.query = "Apps".into();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "apps")
            .unwrap();
        let before = model.cursor;
        model.activate();
        assert!(matches!(model.view, View::Select { .. }));
        assert_eq!(model.stack.last().unwrap().cursor, before);
        assert!(!model.rows().iter().any(|row| row.title == "Broken"));
        let verbs: Vec<String> = model.rows().iter().map(|row| row.verb.clone()).collect();
        assert!(verbs.iter().any(|verb| verb == "Open"));
        assert!(verbs.iter().any(|verb| verb.contains("Run")));

        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.title == "Demo App")
            .unwrap();
        model.activate();
        let start = std::time::Instant::now();
        let mut written = String::new();
        while start.elapsed() < std::time::Duration::from_secs(2) {
            if let Ok(text) = std::fs::read_to_string(&marker) {
                if !text.is_empty() {
                    written = text;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(written, "a;b$(reboot)\n");
        assert!(model.should_close);

        let mut dry = Model::new(stock_menu(), true);
        dry.app_dirs = vec![dir.clone()];
        dry.query = "Apps".into();
        dry.cursor = dry.rows().iter().position(|row| row.id == "apps").unwrap();
        dry.activate();
        dry.cursor = dry
            .rows()
            .iter()
            .position(|row| row.title == "Demo App")
            .unwrap();
        dry.activate();
        assert!(dry.status.contains("Dry run"));
        assert!(!dry.status.to_lowercase().contains("success"));
        assert!(!dry.should_close);
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "a;b$(reboot)\n");

        model.should_close = false;
        model.view = View::Tree;
        model.stack.clear();
        model.query = "Apps".into();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "apps")
            .unwrap();
        model.activate();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.title == "Needs Arg")
            .unwrap();
        assert!(model.rows()[model.cursor].verb.contains("Run"));
        model.activate();
        assert!(matches!(
            model.view,
            View::Input {
                purpose: InputPurpose::Command { .. },
                ..
            }
        ));
        model.activate();
        assert!(model.status.contains("empty"));
        assert!(matches!(model.view, View::Input { .. }));
        if let View::Input { buffer, .. } = &mut model.view {
            buffer.push_str("hello");
        }
        let _ = std::fs::remove_file(&marker);
        model.activate();
        let start = std::time::Instant::now();
        written.clear();
        while start.elapsed() < std::time::Duration::from_secs(2) {
            if let Ok(text) = std::fs::read_to_string(&marker) {
                written = text;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(written, "hello\n");

        let mut risky = Model::new(stock_menu(), true);
        risky.query = "About".into();
        let about = risky
            .rows()
            .iter()
            .position(|row| row.id == "about")
            .unwrap();
        risky.cursor = about;
        risky.activate();
        assert_eq!(risky.confirm.as_ref().unwrap().focus, 0);
        assert!(risky
            .confirm
            .as_ref()
            .unwrap()
            .effect
            .contains("omarchy-launch-about"));
        risky.activate();
        assert!(risky.confirm.is_none());
        assert!(risky.status.contains("Cancelled"));
        risky.cursor = about;
        risky.activate();
        risky.confirm.as_mut().unwrap().focus = 1;
        risky.note_results();
        risky.activate();
        assert!(risky.status.contains("Stale"));
        assert!(risky.confirm.is_none());

        risky.query = saved_query.clone();
        let learn = risky
            .rows()
            .iter()
            .position(|row| row.id == "learn")
            .unwrap();
        risky.cursor = learn;
        risky.activate();
        assert_eq!(risky.active, "learn");
        assert!(risky.query.is_empty());
        risky.go_back();
        assert_eq!(risky.query, saved_query);
        assert_eq!(risky.cursor, learn);
        assert_eq!(risky.active, "root");

        risky.query = "Groceries".into();
        assert!(risky
            .rows()
            .iter()
            .all(|row| !row.id.starts_with("samples") && row.panel != "groceries"));
        risky.answer = Some("local note".into());
        risky.copy_focus = false;
        risky.activate();
        assert!(risky.copied.is_none());
        risky.copy_focus = true;
        risky.activate();
        assert!(risky.status.contains("Clipboard tool was not started"));
        assert!(!risky.status.to_lowercase().contains("success"));
        assert!(!risky.status.contains("Copied in this window"));
        assert!(risky.confirm.is_none());

        risky.copy_focus = false;
        risky.answer = None;
        risky.query = "Font".into();
        risky.cursor = risky
            .rows()
            .iter()
            .position(|row| row.id == "appearance.font")
            .unwrap();
        risky.activate();
        assert!(risky.rows().iter().any(|row| row.verb == "Unavailable"));
        risky.cursor = risky
            .rows()
            .iter()
            .position(|row| row.verb == "Unavailable")
            .unwrap();
        risky.activate();
        assert!(risky.status.contains("Unavailable"));
        assert!(risky.confirm.is_none());

        risky.stale = true;
        risky.activate();
        assert!(risky.status.contains("outdated"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fixture_palette_render_is_not_an_installed_host_screenshot() {
        let dir = std::env::temp_dir().join(format!("omr-fixture-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("colors.toml"),
            "background = \"#07090c\"\nforeground = \"#d5e6df\"\naccent = \"#7ec8ff\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("shell.toml"),
            "[font]\nbase-size = 15\n[menu]\nbackground = \"#10161c\"\ntext = \"#d5e6df\"\nselected-background = \"#163044\"\n",
        )
        .unwrap();
        let theme = theme::load_theme(Some(&theme::ThemePaths {
            colors_toml: Some(dir.join("colors.toml")),
            shell_toml: Some(dir.join("shell.toml")),
            user_shell: None,
            menu_file: None,
            ghostty_theme: None,
            ghostty_user: None,
            font_file: None,
        }));
        assert_eq!(theme.font_px, 15);
        assert_eq!(theme.bg, theme::Rgb::new(0x07, 0x09, 0x0c));
        assert_eq!(theme.fg, theme::Rgb::new(0xd5, 0xe6, 0xdf));
        assert_eq!(theme.accent, theme::Rgb::new(0x7e, 0xc8, 0xff));
        let mut model = Model::new(stock_menu(), true);
        model.theme = theme;
        model.query = "About".into();
        let root = paint_pill(&model);
        let px = root.pixels()[(40 * root.width() as usize) + 80];
        assert!(px.alpha() > 20 && px.alpha() < 255);
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "about")
            .unwrap();
        model.activate();
        assert_eq!(model.confirm.as_ref().unwrap().focus, 0);
        let lead = model.confirm.as_ref().unwrap().lead.to_lowercase();
        assert!(lead.contains("haven’t been verified"));
        assert!(!lead.contains("about window"));
        let confirm_px = paint_pill(&model);
        let cancel =
            confirm_px.pixels()[((list_top() as u32 + 148) * confirm_px.width()) as usize + 24];
        let other =
            confirm_px.pixels()[((list_top() as u32 + 188) * confirm_px.width()) as usize + 24];
        assert_eq!(cancel.alpha(), 255);
        assert_eq!(other.alpha(), 255);
        assert_ne!(
            (cancel.red(), cancel.green(), cancel.blue()),
            (other.red(), other.green(), other.blue())
        );
        let confirm = paint_pill(&model);
        model.confirm.as_mut().unwrap().focus = 1;
        model.activate();
        let dry = paint_pill(&model);
        assert!(model.status.contains("Dry run"));
        assert!(model.notice.is_some());
        model.confirm = None;
        model.notice = None;
        model.view = View::Input {
            purpose: InputPurpose::Command {
                argv: vec!["/usr/bin/true".into()],
                input_at: 1,
                dangerous: false,
                saved_query: String::new(),
                saved_cursor: 0,
                saved_view: Box::new(View::Tree),
            },
            buffer: String::new(),
        };
        let input = paint_pill(&model);
        model.view = View::Tree;
        let about = model
            .rows()
            .iter()
            .position(|row| row.id == "about")
            .unwrap();
        model.menu.items.get_mut("about").unwrap().disabled = "true".into();
        model.disabled_results.insert("about".into(), true);
        model.cursor = about;
        let disabled = paint_pill(&model);
        assert_eq!(model.rows()[about].verb, "Disabled");
        model.query = "Appearance".into();
        let appearance = model
            .rows()
            .into_iter()
            .find(|row| row.id == "appearance")
            .unwrap();
        assert_eq!(appearance.verb, "Browse");
        assert_ne!(appearance.hint, "Appearance");
        let out = std::path::Path::new("/opt/cursor/artifacts");
        if out.exists() {
            let _ = write_png(&out.join("omarchy-menu-rust-fixture-root.png"), &root);
            let _ = write_png(&out.join("omarchy-menu-rust-fixture-confirm.png"), &confirm);
            let _ = write_png(&out.join("omarchy-menu-rust-fixture-dry-run.png"), &dry);
            let _ = write_png(&out.join("omarchy-menu-rust-fixture-input.png"), &input);
            let _ = write_png(
                &out.join("omarchy-menu-rust-fixture-disabled.png"),
                &disabled,
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn root_query_finds_an_app_and_terminal_dry_run_does_not_start_it() {
        let dir = std::env::temp_dir().join(format!("omr-root-{}", std::process::id()));
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
                "[Desktop Entry]\nType=Application\nName=Harmless Finder\nExec={}\n",
                script.display()
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("term.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Term Finder\nExec={}\nTerminal=true\n",
                script.display()
            ),
        )
        .unwrap();
        let mut model = Model::new(stock_menu(), true);
        model.app_dirs = vec![dir.clone()];
        model.terminal = crate::desktop::parse_terminal_argv("ghostty -e").unwrap();
        model.load_apps();
        model.query = "Harmless Finder".into();
        assert!(matches!(model.view, View::Tree));
        let found = model
            .rows()
            .into_iter()
            .find(|row| row.title == "Harmless Finder")
            .unwrap();
        assert_eq!(found.verb, "Dry run");
        assert!(!found.hint.to_lowercase().contains("open"));
        assert!(found.hint.contains("Enter does not start it."));
        model.theme = theme::bundled_snapshot();
        let search = paint_pill(&model);
        model.query = "Term Finder".into();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.title == "Term Finder")
            .unwrap();
        let term = model
            .rows()
            .into_iter()
            .find(|row| row.title == "Term Finder")
            .unwrap();
        assert!(term.argv.starts_with(&["ghostty".into(), "-e".into()]));
        assert!(!term.argv.iter().any(|arg| arg == "-c" || arg == "bash"));
        model.activate();
        assert!(model.notice.as_ref().unwrap().contains("ghostty"));
        assert!(model.notice.as_ref().unwrap().contains("Dry run"));
        assert!(!marker.exists());
        let dry = paint_pill(&model);
        let out = std::path::Path::new("/opt/cursor/artifacts");
        if out.exists() {
            let _ = write_png(
                &out.join("omarchy-menu-rust-fixture-root-search.png"),
                &search,
            );
            let _ = write_png(
                &out.join("omarchy-menu-rust-fixture-terminal-dry-run.png"),
                &dry,
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_and_theme_reload_do_not_run_a_guard() {
        let dir = std::env::temp_dir().join(format!("omr-guard-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("pwned");
        let bin = dir.join("touch-marker");
        std::fs::write(&bin, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin, perms).unwrap();
        let items = crate::model::parse_menu_jsonc(&format!(
            r#"{{"row": {{"label": "Row", "when": "{}", "action": "true"}}}}"#,
            bin.display()
        ));
        let menu = crate::model::merge_menu_sources(&items, &[]);
        let mut model = Model::new(menu, true);
        model.live = true;
        model.query = "Row".into();
        let _ = model.rows();
        model.theme = theme::reload_theme(&model.theme, &theme::default_paths());
        let facts = crate::guards::evaluate_items(&items);
        model.apply_guards(&facts);
        assert!(facts.blocks["row"].contains("not run"));
        assert!(model
            .rows()
            .iter()
            .any(|row| row.verb == "Unavailable" || row.verb == "Disabled"));
        assert!(!marker.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn idle_search_hides_future_panels() {
        let model = Model::new(stock_menu(), true);
        let (w, h) = pill_size(&model);
        assert!((640..=720).contains(&w));
        if model.rows().is_empty() {
            assert!((52..=56).contains(&h) || (70..=80).contains(&h));
        } else {
            assert!(h <= 440);
            assert!(model.rows().iter().all(|row| {
                !row.title.to_lowercase().contains("frequent")
                    && !row.hint.to_lowercase().contains("frequent")
            }));
        }
        for query in ["inbox", "groceries", "music", "store", "bots"] {
            let mut hidden = Model::new(stock_menu(), true);
            hidden.query = query.into();
            assert!(
                hidden.rows().iter().all(|row| {
                    !row.id.starts_with("samples")
                        && !row.panel.starts_with("bot:")
                        && !matches!(
                            row.panel.as_str(),
                            "groceries"
                                | "inbox"
                                | "recent"
                                | "decisions"
                                | "example-a"
                                | "example-b"
                                | "music"
                                | "store"
                        )
                }),
                "{query}"
            );
        }
    }

    #[test]
    fn state_trace_follows_search_confirm_cancel_and_escape() {
        let mut model = Model::new(stock_menu(), true);
        assert!(state_trace(&model).contains("query=\"\""));
        assert!(state_trace(&model).contains("confirm=none"));
        assert!(state_trace(&model).contains("close=0"));
        model.wayland_key("", Some("A"), false);
        model.wayland_key("", Some("b"), false);
        model.wayland_key("", Some("out"), false);
        let about = model
            .rows()
            .iter()
            .position(|row| row.id == "about")
            .unwrap();
        model.cursor = about;
        let before = state_trace(&model);
        model.wayland_key("Return", None, true);
        assert_eq!(state_trace(&model), before);
        assert!(model.confirm.is_none());
        model.wayland_key("Return", None, false);
        let opened = state_trace(&model);
        assert!(opened.contains("query=\"About\""));
        assert!(opened.contains("selected=\"About\""));
        assert!(opened.contains("confirm=open"));
        assert!(opened.contains("focus=cancel"));
        assert!(opened.contains("close=0"));
        model.wayland_key("Return", None, false);
        let cancelled = state_trace(&model);
        assert!(cancelled.contains("confirm=none"));
        assert!(cancelled.contains("refusal=cancelled"));
        assert!(cancelled.contains("close=0"));
        model.wayland_key("Return", None, false);
        model.wayland_key("Down", None, false);
        assert!(state_trace(&model).contains("focus=confirm"));
        model.wayland_key("Return", None, false);
        let refused = state_trace(&model);
        assert!(refused.contains("refusal=dry-run"));
        assert!(refused.contains("close=0"));
        assert!(model
            .notice
            .as_ref()
            .unwrap()
            .contains("omarchy-launch-about"));
        model.notice = None;
        model.query = "About".into();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "appearance.branding.about")
            .unwrap();
        model.wayland_key("Return", None, false);
        assert_eq!(model.active, "appearance.branding.about");
        assert!(model.query.is_empty());
        model.wayland_key("Escape", None, false);
        let back = state_trace(&model);
        assert!(back.contains("query=\"About\""));
        assert!(back.contains("close=0"));
        assert!(back.contains("refusal=refused"));
        model.wayland_key("Escape", None, false);
        assert!(state_trace(&model).contains("close=1"));
    }

    #[test]
    fn held_enter_does_not_repeat_and_corner_click_is_ignored() {
        let mut model = Model::new(stock_menu(), true);
        model.query = "About".into();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "about")
            .unwrap();
        model.wayland_key("Return", None, false);
        assert!(model.confirm.is_some());
        assert_eq!(model.confirm.as_ref().unwrap().focus, 0);
        model.wayland_key("Return", None, true);
        assert_eq!(model.confirm.as_ref().unwrap().focus, 0);
        assert!(model.confirm.is_some());
        model.cursor = 4;
        model.highlight_at(1.0, 1.0, 680.0, 440.0);
        assert_eq!(model.cursor, 4);
        assert!(row_index_at(&model, 1.0, 1.0, 680.0, 440.0).is_none());
        assert!(confirm_button_at(&model, 1.0, 1.0, 680.0, 440.0).is_none());
        assert_eq!(
            confirm_button_at(&model, LIST_X + 8.0, list_top() + 136.0, 680.0, 440.0),
            Some(0)
        );
        assert_eq!(
            confirm_button_at(&model, LIST_X + 8.0, list_top() + 176.0, 680.0, 440.0),
            Some(1)
        );
    }

    #[test]
    fn plate_is_translucent_and_glyph_interior_is_opaque() {
        let mut model = Model::new(stock_menu(), true);
        model.query = "About".into();
        let pixmap = paint_pill(&model);
        let plate = pixmap.pixels()[(6 * pixmap.width() as usize) + 300];
        assert_eq!(plate.alpha(), PANEL_ALPHA);
        let corner = pixmap.pixels()[0];
        assert_eq!(corner.alpha(), 0);
        let mut opaque = false;
        let mut edge = false;
        for y in 18..46 {
            for x in 48..220 {
                let px = pixmap.pixels()[(y * pixmap.width() as usize) + x];
                if px.alpha() == 255 && px.red() > 20 {
                    opaque = true;
                }
                if px.alpha() > 8
                    && px.alpha() < 250
                    && px.alpha() != PANEL_ALPHA
                    && px.alpha() != 220
                {
                    edge = true;
                }
            }
        }
        assert!(opaque);
        assert!(edge);
        let over = |px: tiny_skia::PremultipliedColorU8, base: u8| -> u8 {
            let a = px.alpha() as u32;
            let straight = px.red() as u32 * 255 / a.max(1);
            ((straight * a + base as u32 * (255 - a)) / 255) as u8
        };
        assert_ne!(over(plate, 0), over(plate, 255));
        let ink = pixmap
            .pixels()
            .iter()
            .copied()
            .find(|px| px.alpha() == 255 && px.red() > 20)
            .unwrap();
        assert_eq!(over(ink, 0), over(ink, 255));
        let out = std::path::Path::new("/opt/cursor/artifacts");
        if out.exists() {
            let _ = write_png(&out.join("omarchy-menu-rust-fixture-pill.png"), &pixmap);
            let _ = write_png(
                &out.join("omarchy-menu-rust-fixture-over-black.png"),
                &composite_over(&pixmap, |_, _| (0, 0, 0)),
            );
            let _ = write_png(
                &out.join("omarchy-menu-rust-fixture-over-white.png"),
                &composite_over(&pixmap, |_, _| (255, 255, 255)),
            );
            let _ = write_png(
                &out.join("omarchy-menu-rust-fixture-over-checker.png"),
                &composite_over(&pixmap, |x, y| {
                    if ((x / 16) + (y / 16)) % 2 == 0 {
                        (236, 236, 236)
                    } else {
                        (32, 32, 32)
                    }
                }),
            );
        }
    }

    fn desktop_row(id: &str, title: &str, argv: &[&str], input_at: Option<usize>) -> Row {
        Row {
            id: id.into(),
            desktop_id: id.into(),
            title: title.into(),
            hint: "Desktop entry.".into(),
            verb: if input_at.is_some() {
                "Run…".into()
            } else {
                "Open".into()
            },
            argv: argv.iter().map(|part| (*part).to_string()).collect(),
            input_at,
            needs_input: input_at.is_some(),
            ..Row::default()
        }
    }

    #[test]
    fn shortcut_rows_use_desktop_entries_and_not_frequency() {
        let apps = vec![
            desktop_row(
                GHOSTTY_ID,
                "Ghostty",
                &["ghostty", "--gtk-single-instance=true"],
                None,
            ),
            desktop_row(FLEA_ID, "Flea", &["flea", "--gui"], Some(2)),
            desktop_row("chromium.desktop", "Chromium", &["chromium"], None),
            desktop_row(CURSOR_ID, "Cursor", &["/usr/share/cursor/cursor"], None),
            desktop_row(GROK_ID, "Grok Bot", &["grok-bot"], None),
            desktop_row(MATTERMOST_ID, "Mattermost", &["mattermost-desktop"], None),
            desktop_row(
                "music.desktop",
                "Music",
                &["/tmp/not-a-shortcut/music-summon"],
                None,
            ),
        ];
        let present = [TERMINAL_WRAPPER, BROWSER_WRAPPER, FLEA_WRAPPER];
        let rows = shortcut_rows_from(&apps, |path| present.contains(&path));
        let titles: Vec<_> = rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(
            titles,
            vec![
                "Ghostty",
                "Flea",
                "Open browser",
                "Cursor",
                "Grok Bot",
                "Mattermost"
            ]
        );
        assert!(rows.iter().all(|row| {
            let text = format!("{} {}", row.title, row.hint).to_lowercase();
            !text.contains("frequent") && !text.contains("most used")
        }));
        assert!(rows.iter().all(|row| row.desktop_id != "chromium.desktop"));
        let cursor = rows.iter().find(|row| row.desktop_id == CURSOR_ID).unwrap();
        assert_eq!(cursor.argv, vec!["/usr/share/cursor/cursor".to_string()]);
        let grok = rows.iter().find(|row| row.desktop_id == GROK_ID).unwrap();
        assert_eq!(grok.argv, vec!["grok-bot".to_string()]);
        let flea = rows.iter().find(|row| row.desktop_id == FLEA_ID).unwrap();
        assert_eq!(flea.input_at, Some(2));
        assert!(flea.needs_input);
        let browser = rows
            .iter()
            .find(|row| row.id == "shortcut:browser")
            .unwrap();
        assert_eq!(browser.argv, vec![BROWSER_WRAPPER.to_string()]);
        assert!(!rows
            .iter()
            .any(|row| row.title == "Music" || row.title == "Open terminal"));
    }

    #[test]
    fn missing_desktop_files_use_terminal_and_file_wrappers_without_filler() {
        let rows = shortcut_rows_from(&[], |path| path == TERMINAL_WRAPPER || path == FLEA_WRAPPER);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "Open terminal");
        assert_eq!(rows[0].argv, vec![TERMINAL_WRAPPER.to_string()]);
        assert_eq!(rows[1].title, "Open file manager");
        assert_eq!(
            rows[1].argv,
            vec![FLEA_WRAPPER.to_string(), "--gui".to_string()]
        );
        assert!(shortcut_rows_from(&[], |_| false).is_empty());
    }

    #[test]
    fn shortcut_activation_stays_a_dry_run() {
        let mut model = Model::new(stock_menu(), true);
        model.app_rows = vec![desktop_row(
            GHOSTTY_ID,
            "Ghostty",
            &["ghostty", "--gtk-single-instance=true"],
            None,
        )];
        assert_eq!(model.rows()[0].title, "Ghostty");
        model.enter(|_| panic!("dry run must not report a start"));
        assert!(model.status.contains("Dry run"));
        assert!(!model.should_close);
        let flea = desktop_row(FLEA_ID, "Flea", &["flea", "--gui"], Some(2));
        model.app_rows = vec![flea];
        model.notice = None;
        model.enter(|_| panic!("input must not start"));
        assert!(matches!(model.view, View::Input { .. }));
        assert!(model.confirm.is_none());
    }

    #[test]
    fn omarchy_and_omarchi_update_find_the_existing_leaf() {
        let menu = stock_menu();
        let leaf = &menu.items["update.omarchy"];
        assert_eq!(
            leaf.action,
            "omarchy-launch-floating-terminal-with-presentation omarchy-update"
        );
        assert!(matches_query(&menu, leaf, "Omarchy update", true));
        assert!(matches_query(&menu, leaf, "Omarchi update", true));
        let mut model = Model::new(menu, true);
        for query in ["Omarchy update", "Omarchi update"] {
            model.query = query.into();
            model.cursor = 0;
            let hits: Vec<_> = model
                .rows()
                .iter()
                .filter(|row| row.id == "update.omarchy")
                .map(|row| row.action.clone())
                .collect();
            assert_eq!(hits.len(), 1, "{query}");
            assert_eq!(
                hits[0],
                "omarchy-launch-floating-terminal-with-presentation omarchy-update"
            );
        }
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "update.omarchy")
            .unwrap();
        model.enter(|_| panic!("opening confirm must not start the updater"));
        let confirm = model.confirm.as_ref().unwrap();
        assert_eq!(confirm.focus, 0);
        assert!(confirm.effect.contains("omarchy-update"));
        assert!(!confirm.effect.contains(" -y"));
        model.enter(|_| panic!("cancel must not start the updater"));
        assert!(model.confirm.is_none());
        assert!(model.status.contains("Cancelled"));
        assert!(!model.should_close);
        let out = std::path::Path::new("/opt/cursor/artifacts");
        if out.exists() {
            model.query = "Omarchy update".into();
            model.cursor = model
                .rows()
                .iter()
                .position(|row| row.id == "update.omarchy")
                .unwrap();
            let _ = write_png(
                &out.join("omarchy-menu-rust-review-update-search.png"),
                &paint_pill(&model),
            );
            model.query = "Omarchi update".into();
            model.cursor = model
                .rows()
                .iter()
                .position(|row| row.id == "update.omarchy")
                .unwrap();
            let _ = write_png(
                &out.join("omarchy-menu-rust-review-omarchi-search.png"),
                &paint_pill(&model),
            );
        }
    }

    #[test]
    fn safe_apps_starts_only_the_documented_desktop_exec() {
        let dir = std::env::temp_dir().join(format!("omr-safe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("usr/share/applications");
        let user = dir.join("home/applications");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        let real = dir.join("usr/bin/flea");
        let mark = dir.join("ran");
        let shadow = dir.join("shadow/flea");
        let shadow_mark = dir.join("shadow-ran");
        let write_script = |path: &std::path::Path, dest: &std::path::Path| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(
                path,
                format!(
                    "#!/bin/sh\n{{ printf '%s\\n' \"$0\" \"$@\"; pwd -P; printf '%s\\n' \"$HOME\" \"${{OMARCHY_MENU_QUERY-unset}}\"; }} > '{}'\n",
                    dest.display()
                ),
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).unwrap();
        };
        write_script(&real, &mark);
        write_script(&shadow, &shadow_mark);
        let exec_link = dir.join("flea-link");
        std::os::unix::fs::symlink(&real, &exec_link).unwrap();
        let desktop = root.join("com.thisisgm.flea.desktop");
        std::fs::write(
            &desktop,
            "[Desktop Entry]\nType=Application\nName=Flea\nExec=flea --gui\n",
        )
        .unwrap();
        let user_desktop = user.join("com.thisisgm.flea.desktop");
        std::fs::write(
            &user_desktop,
            "[Desktop Entry]\nType=Application\nName=Flea\nExec=flea --gui\n",
        )
        .unwrap();
        let _trust = crate::safe_apps::install_test_trust(
            vec![root],
            vec![crate::safe_apps::Spec {
                desktop_id: "com.thisisgm.flea.desktop".into(),
                program: real.clone(),
                args: vec!["--gui".into()],
                provenance: "fixture".into(),
            }],
        );
        let shown = listed_row(
            providers::ListedRow {
                title: "Flea".into(),
                hint: "Desktop entry. One Enter opens it.".into(),
                desktop_id: "com.thisisgm.flea.desktop".into(),
                desktop_file: desktop.clone(),
                argv: vec![exec_link.to_string_lossy().into_owned(), "--gui".into()],
                input_at: None,
                terminal: false,
                unavailable: false,
                dangerous: false,
            },
            true,
            false,
            true,
        );
        assert_eq!(shown.verb, "Open");
        assert!(shown.hint.contains("opens it"));
        let hidden = listed_row(
            providers::ListedRow {
                title: "Flea".into(),
                hint: "Desktop entry. One Enter opens it.".into(),
                desktop_id: "com.thisisgm.flea.desktop".into(),
                desktop_file: user_desktop,
                argv: vec!["flea".into(), "--gui".into()],
                input_at: None,
                terminal: false,
                unavailable: false,
                dangerous: false,
            },
            true,
            false,
            true,
        );
        assert_eq!(hidden.verb, "Dry run");
        assert!(hidden.hint.contains("does not start"));

        let mut model = Model::new(stock_menu(), true);
        model.safe_apps = true;
        model.app_rows = vec![shown];
        let (tx, rx) = std::sync::mpsc::channel();
        model.enter(move |result| tx.send(result).unwrap());
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(),
            "exited 0."
        );
        let recorded = std::fs::read_to_string(&mark).expect("canonical launcher log");
        assert_eq!(
            recorded.lines().collect::<Vec<_>>(),
            vec![
                std::fs::canonicalize(&real)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                "--gui".into(),
                std::env::current_dir()
                    .unwrap()
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                std::env::var("HOME").unwrap(),
                "unset".into(),
            ]
        );
        assert!(!shadow_mark.exists());
        let _ = std::fs::remove_file(&mark);

        // The row was admitted for display, but Enter must recheck both paths.
        model.should_close = false;
        std::fs::remove_file(&exec_link).unwrap();
        std::os::unix::fs::symlink(&shadow, &exec_link).unwrap();
        model.enter(|_| panic!("a substituted executable symlink must not start"));
        assert!(model.status.contains("Dry run"));
        assert!(!model.should_close);
        assert!(!mark.exists());
        assert!(!shadow_mark.exists());
        std::fs::remove_file(&exec_link).unwrap();
        std::os::unix::fs::symlink(&real, &exec_link).unwrap();

        let saved_desktop = dir.join("saved.desktop");
        std::fs::rename(&desktop, &saved_desktop).unwrap();
        std::os::unix::fs::symlink(&saved_desktop, &desktop).unwrap();
        model.notice = None;
        model.enter(|_| panic!("a desktop symlink outside the trusted root must not start"));
        assert!(model.status.contains("Dry run"));
        assert!(!model.should_close);
        assert!(!mark.exists());
        assert!(!shadow_mark.exists());
        std::fs::remove_file(&desktop).unwrap();
        std::fs::rename(&saved_desktop, &desktop).unwrap();

        model.notice = None;
        model.app_rows[0].argv[0] = shadow.to_string_lossy().into_owned();
        model.enter(|_| panic!("a substitute executable must not start"));
        assert!(model.status.contains("Dry run"));
        assert!(!mark.exists());
        assert!(!shadow_mark.exists());

        model.should_close = false;
        model.app_rows = vec![hidden];
        model.notice = None;
        model.enter(|_| panic!("a shadowed desktop file must not start"));
        assert!(model.status.contains("Dry run"));
        assert!(!mark.exists());
        assert!(!shadow_mark.exists());

        model.notice = None;
        model.query = "Omarchy update".into();
        model.cursor = model
            .rows()
            .iter()
            .position(|row| row.id == "update.omarchy")
            .unwrap();
        model.enter(|_| panic!("opening confirm must not start the updater"));
        let confirm = model.confirm.as_ref().unwrap();
        assert_eq!(confirm.focus, 0);
        assert!(confirm.lead.contains("will not run"));
        model.confirm.as_mut().unwrap().focus = 1;
        model.enter(|_| panic!("confirm must not start the updater"));
        assert!(model.status.contains("Dry run") || model.status.contains("not started"));
        assert!(!mark.exists());
        assert!(!shadow_mark.exists());

        model.query.clear();
        model.confirm = None;
        model.notice = None;
        model.should_close = false;
        model.app_rows = vec![Row {
            desktop_id: "ask-bot.desktop".into(),
            title: "Ask".into(),
            argv: vec![
                "ask-bot".into(),
                "example".into(),
                "handoff".into(),
                "hello".into(),
            ],
            verb: "Open".into(),
            ..Row::default()
        }];
        model.enter(|_| panic!("a messaging desktop entry must not start"));
        assert!(model.status.contains("Dry run"));
        assert!(!mark.exists());

        model.notice = None;
        model.app_rows[0].argv = vec!["bash".into(), "-c".into(), "omarchy-update".into()];
        model.enter(|_| panic!("a shell desktop entry must not start"));
        assert!(model.status.contains("Nothing was started"));
        assert!(!mark.exists());

        model.safe_apps = false;
        model.notice = None;
        model.should_close = false;
        model.app_rows = vec![listed_row(
            providers::ListedRow {
                title: "Flea".into(),
                hint: "Desktop entry. One Enter opens it.".into(),
                desktop_id: "com.thisisgm.flea.desktop".into(),
                desktop_file: dir.join("usr/share/applications/com.thisisgm.flea.desktop"),
                argv: vec!["flea".into(), "--gui".into()],
                input_at: None,
                terminal: false,
                unavailable: false,
                dangerous: false,
            },
            true,
            false,
            false,
        )];
        assert_eq!(model.app_rows[0].verb, "Dry run");
        model.enter(|_| panic!("default no-exec must not start the allowlisted app"));
        assert!(model.status.contains("Dry run"));
        assert!(!mark.exists());

        model.safe_apps = true;
        model.notice = None;
        model.app_rows[0].terminal = true;
        model.enter(|_| panic!("Terminal=true must not start"));
        assert!(model.status.contains("Dry run"));
        assert!(!mark.exists());

        model.copy_focus = true;
        model.answer = Some("note".into());
        model.notice = None;
        model.enter(|_| {});
        assert!(model.status.contains("Clipboard"));
        assert!(!mark.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
