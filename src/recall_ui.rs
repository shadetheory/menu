//! Native, read-only work recall state. Disk and semantic retrieval run on one worker.
use crate::recall::{self, Hit, Record};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

pub const TOPICS: [&str; 6] = [
    "Music",
    "Automation",
    "Gate",
    "Receiver",
    "Listener",
    "Pub/Sub",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Topics,
    Results,
    Preview,
}

#[derive(Clone)]
struct Request {
    generation: u64,
    query: String,
}

enum Reply {
    Loaded {
        warning: Option<String>,
    },
    Results {
        generation: u64,
        hits: Vec<Hit>,
        status: String,
    },
}

pub struct WorkUi {
    pub stage: Stage,
    pub query: String,
    pub cursor: usize,
    pub hits: Vec<Hit>,
    pub status: String,
    pub preview_scroll: usize,
    preview_record: Option<Record>,
    generation: u64,
    pending: Arc<Mutex<Option<Request>>>,
    wake: Option<mpsc::SyncSender<()>>,
    replies: mpsc::Receiver<Reply>,
}

impl WorkUi {
    /// Only checking path existence is done on the UI thread. Config parsing and
    /// source reconciliation are bounded work on the worker.
    pub fn start_if_configured() -> Option<Self> {
        let path = recall::config_path();
        if !path.is_file() {
            return None;
        }
        let pending = Arc::new(Mutex::new(None));
        let (wake_tx, wake_rx) = mpsc::sync_channel(1);
        let (reply_tx, reply_rx) = mpsc::sync_channel(4);
        let worker_pending = Arc::clone(&pending);
        std::thread::Builder::new()
            .name("shade-work-recall".into())
            .spawn(move || {
                worker(path, worker_pending, wake_rx, reply_tx);
            })
            .ok()?;
        Some(Self {
            stage: Stage::Topics,
            query: String::new(),
            cursor: 0,
            hits: Vec::new(),
            status: "Loading your work…".into(),
            preview_scroll: 0,
            preview_record: None,
            generation: 0,
            pending,
            wake: Some(wake_tx),
            replies: reply_rx,
        })
    }

    #[cfg(test)]
    fn test() -> Self {
        let (_, replies) = mpsc::channel();
        Self {
            stage: Stage::Topics,
            query: String::new(),
            cursor: 0,
            hits: Vec::new(),
            status: "Loading your work…".into(),
            preview_scroll: 0,
            preview_record: None,
            generation: 0,
            pending: Arc::new(Mutex::new(None)),
            wake: None,
            replies,
        }
    }

    pub fn pick_topic(&mut self, index: usize) {
        let Some(topic) = TOPICS.get(index) else {
            return;
        };
        self.set_query((*topic).into());
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.stage = Stage::Results;
        self.preview_record = None;
        self.cursor = 0;
        self.hits.clear();
        self.generation = self.generation.wrapping_add(1);
        self.status = if self.query.trim().is_empty() {
            "Type to search your work.".into()
        } else {
            "Searching…".into()
        };
        if let Ok(mut slot) = self.pending.lock() {
            *slot = Some(Request {
                generation: self.generation,
                query: self.query.clone(),
            });
        }
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(reply) = self.replies.try_recv() {
            changed |= self.apply(reply);
        }
        changed
    }

    fn apply(&mut self, reply: Reply) -> bool {
        match reply {
            Reply::Loaded { warning } => {
                if self.generation == 0 {
                    self.status =
                        warning.unwrap_or_else(|| "Choose a topic or type to search.".into());
                    true
                } else {
                    false
                }
            }
            Reply::Results {
                generation,
                hits,
                status,
            } if generation == self.generation => {
                let selected_id = self.hits.get(self.cursor).map(|h| h.record.id.as_str());
                if !self.hits.is_empty()
                    && self.stage == Stage::Results
                    && selected_id.is_some_and(|id| !hits.iter().any(|h| h.record.id == id))
                {
                    self.status = format!("{status}; current selection retained");
                    return true;
                }
                let selected_id = selected_id.map(str::to_owned);
                self.hits = hits;
                if let Some(id) = selected_id {
                    self.cursor = self
                        .hits
                        .iter()
                        .position(|h| h.record.id == id)
                        .unwrap_or(0);
                } else {
                    self.cursor = self.cursor.min(self.hits.len().saturating_sub(1));
                }
                self.status = status;
                true
            }
            _ => false,
        }
    }

    pub fn enter(&mut self) {
        match self.stage {
            Stage::Topics => self.pick_topic(self.cursor),
            Stage::Results if self.cursor < self.hits.len() => {
                self.preview_record = self.hits.get(self.cursor).map(|h| h.record.clone());
                self.stage = Stage::Preview;
                self.preview_scroll = 0;
            }
            _ => {}
        }
    }

    /// Returns true when the Work view should close to the root menu.
    pub fn back(&mut self) -> bool {
        match self.stage {
            Stage::Preview => {
                self.stage = Stage::Results;
                self.preview_record = None;
            }
            Stage::Results => {
                self.stage = Stage::Topics;
                self.cursor = 0;
            }
            Stage::Topics => return true,
        }
        false
    }

    pub fn selected(&self) -> Option<&Record> {
        if self.stage == Stage::Preview {
            self.preview_record.as_ref()
        } else {
            self.hits.get(self.cursor).map(|h| &h.record)
        }
    }
    pub fn move_up(&mut self) {
        if self.stage == Stage::Preview {
            self.preview_scroll = self.preview_scroll.saturating_sub(1);
        } else {
            self.cursor = self.cursor.saturating_sub(1);
        }
    }
    pub fn move_down(&mut self) {
        if self.stage == Stage::Preview {
            let count = self
                .preview_record
                .as_ref()
                .map_or(0, |r| r.text.lines().take(400).count());
            self.preview_scroll = (self.preview_scroll + 1).min(count.saturating_sub(10));
        } else {
            let count = if self.stage == Stage::Topics {
                TOPICS.len()
            } else {
                self.hits.len()
            };
            self.cursor = (self.cursor + 1).min(count.saturating_sub(1));
        }
    }
}

fn worker(
    path: PathBuf,
    pending: Arc<Mutex<Option<Request>>>,
    wake: mpsc::Receiver<()>,
    reply: mpsc::SyncSender<Reply>,
) {
    let loaded = recall::load_config(&path).and_then(|cfg| {
        if !cfg.enabled {
            return Err("Work recall is disabled in its config.".into());
        }
        recall::reconcile_with_status(&cfg, &recall::cache_path())
            .map(|(records, _, warnings)| (cfg, records, warnings))
    });
    let warning = match &loaded {
        Ok((_, _, warnings)) if warnings.is_empty() => None,
        Ok((_, _, warnings)) => Some(format!(
            "{} source issue(s): {}",
            warnings.len(),
            warnings.join("; ")
        )),
        Err(err) => Some(format!("Work unavailable: {err}")),
    };
    let _ = reply.send(Reply::Loaded { warning });
    while wake.recv().is_ok() {
        let request = pending.lock().ok().and_then(|mut slot| slot.take());
        let Some(request) = request else {
            continue;
        };
        let Ok((_, _, _)) = &loaded else {
            let _ = reply.send(Reply::Results {
                generation: request.generation,
                hits: Vec::new(),
                status: "Work unavailable; check recall configuration and sources.".into(),
            });
            continue;
        };
        let refreshed = recall::load_config(&path).and_then(|cfg| {
            if !cfg.enabled {
                return Err("Work recall is disabled.".into());
            }
            recall::reconcile_with_status(&cfg, &recall::cache_path())
                .map(|(records, _, warnings)| (cfg, records, warnings))
        });
        let Ok((cfg, records, warnings)) = refreshed else {
            let reason = refreshed.err().unwrap_or_default();
            let _ = reply.send(Reply::Results {
                generation: request.generation,
                hits: Vec::new(),
                status: format!("Work unavailable: {reason}"),
            });
            continue;
        };
        let health = if warnings.is_empty() {
            String::new()
        } else {
            format!("; {} source issue(s)", warnings.len())
        };
        let lexical = recall::search(&records, &request.query, 20);
        let _ = reply.send(Reply::Results {
            generation: request.generation,
            hits: lexical,
            status: format!("Lexical results; checking local retrieval…{health}"),
        });
        let result = crate::fetch::retrieve(&records, &cfg, &request.query, 20);
        let _ = reply.send(Reply::Results {
            generation: request.generation,
            hits: result.hits,
            status: format!("{}{health}", result.status),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_preserves_query_and_selection() {
        let mut ui = WorkUi::test();
        ui.pick_topic(0);
        assert_eq!(ui.query, "Music");
        assert_eq!(ui.stage, Stage::Results);
        assert!(!ui.back());
        assert_eq!(ui.stage, Stage::Topics);
        assert_eq!(ui.query, "Music");
        assert!(ui.back());
    }
    #[test]
    fn stale_reply_cannot_replace_new_query() {
        let mut ui = WorkUi::test();
        ui.set_query("Music".into());
        let old = ui.generation;
        ui.set_query("Gate".into());
        assert!(!ui.apply(Reply::Results {
            generation: old,
            hits: Vec::new(),
            status: "old".into()
        }));
        assert_eq!(ui.status, "Searching…");
    }

    #[test]
    fn preview_is_pinned_and_back_restores_results_without_dispatch() {
        let mut ui = WorkUi::test();
        ui.set_query("Music".into());
        let source = Record {
            schema_version: 1,
            id: "music".into(),
            title: "Music plan".into(),
            project: "Shade".into(),
            source_path: PathBuf::from("/tmp/music.md"),
            text: "Original source".into(),
            modified_unix: 0,
            fingerprint: "example".into(),
            kind: "idea".into(),
            topics: vec!["Music".into()],
        };
        assert!(ui.apply(Reply::Results {
            generation: ui.generation,
            hits: vec![Hit {
                record: source.clone(),
                score: 1
            }],
            status: "Lexical".into(),
        }));
        ui.enter();
        assert_eq!(ui.stage, Stage::Preview);
        assert_eq!(ui.selected().unwrap().text, "Original source");
        assert!(ui.apply(Reply::Results {
            generation: ui.generation,
            hits: vec![Hit {
                record: Record {
                    text: "Changed index".into(),
                    ..source
                },
                score: 2
            }],
            status: "Semantic".into(),
        }));
        assert_eq!(ui.selected().unwrap().text, "Original source");
        assert!(!ui.back());
        assert_eq!(ui.stage, Stage::Results);
        assert_eq!(ui.query, "Music");
        assert_eq!(ui.cursor, 0);
    }
}
