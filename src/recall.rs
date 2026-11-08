//! Explicit-file work recall. Source files are authoritative; this JSON is a rebuildable cache.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

pub const MAX_SOURCE_BYTES: u64 = 512 * 1024;
const MAX_SOURCES: usize = 64;
const MAX_CACHE_BYTES: u64 = 48 * 1024 * 1024;
const MAX_HTTP_BYTES: usize = 16 * 1024 * 1024;
const MAX_EMBED_CHARS: usize = 12_000;
// Retrieval heuristic, not a probability or factual-confidence threshold.
const MIN_SEMANTIC_SIMILARITY: f64 = 0.35;
const CACHE_VERSION: u32 = 2;
static TMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSpec {
    pub id: String,
    pub title: String,
    pub project: String,
    pub path: PathBuf,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub topics: Vec<String>,
}
fn default_kind() -> String {
    "document".into()
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default)]
    pub planner: Option<OllamaConfig>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sources: Vec<SourceSpec>,
    #[serde(default)]
    pub ollama: Option<OllamaConfig>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OllamaConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub project: String,
    pub source_path: PathBuf,
    pub text: String,
    pub modified_unix: u64,
    pub fingerprint: String,
    pub kind: String,
    pub topics: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub operation: String,
    pub source_id: String,
    pub revision: String,
    pub fingerprint: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Embedding {
    fingerprint: String,
    endpoint: String,
    model: String,
    vector: Vec<f32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Cache {
    schema_version: u32,
    records: BTreeMap<String, Record>,
    #[serde(default)]
    embeddings: BTreeMap<String, Embedding>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            schema_version: CACHE_VERSION,
            records: BTreeMap::new(),
            embeddings: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub record: Record,
    pub score: usize,
}
#[derive(Clone, Debug)]
pub struct Retrieval {
    pub hits: Vec<Hit>,
    pub status: String,
}
pub type ReconcileResult = Result<(Vec<Record>, Vec<Event>, Vec<String>), String>;

pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("SHADE_RECALL_CONFIG") {
        return p.into();
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("shadetheory/recall.json")
}
pub fn cache_path() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("shadetheory/recall.json")
}
pub fn load_config(path: &Path) -> Result<Config, String> {
    let raw =
        fs::read(path).map_err(|e| format!("cannot read recall config {}: {e}", path.display()))?;
    if raw.len() > 256 * 1024 {
        return Err("recall config exceeds 256 KiB".into());
    }
    let cfg: Config =
        serde_json::from_slice(&raw).map_err(|e| format!("invalid recall config: {e}"))?;
    validate_config(&cfg)?;
    Ok(cfg)
}
fn validate_config(cfg: &Config) -> Result<(), String> {
    if cfg.sources.len() > MAX_SOURCES {
        return Err(format!("too many recall sources (maximum {MAX_SOURCES})"));
    }
    let mut ids = BTreeSet::new();
    for s in &cfg.sources {
        if s.id.trim().is_empty() || s.id.len() > 128 || !ids.insert(s.id.clone()) {
            return Err("source IDs must be nonempty, unique and at most 128 bytes".into());
        }
        if !["document", "decision", "idea", "implementation"].contains(&s.kind.as_str()) {
            return Err(format!("invalid kind for source {}", s.id));
        }
        if !s.path.is_absolute() {
            return Err(format!(
                "source path must be absolute: {}",
                s.path.display()
            ));
        }
    }
    if let Some(o) = &cfg.ollama {
        if o.enabled {
            parse_endpoint(&o.endpoint)?;
            if o.model.trim().is_empty() || o.model.len() > 128 {
                return Err("Ollama model must be nonempty and at most 128 bytes".into());
            }
        }
    }
    Ok(())
}
fn fingerprint(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in data {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{h:016x}")
}
fn revision(r: &Record) -> String {
    // Length-prefixed fields avoid ambiguous concatenations. This is a revision marker, not a security hash.
    let fields = [&r.id, &r.title, &r.project, &r.fingerprint, &r.kind];
    let mut bytes = Vec::new();
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    let path = r.source_path.to_string_lossy();
    bytes.extend_from_slice(&(path.len() as u64).to_le_bytes());
    bytes.extend_from_slice(path.as_bytes());
    bytes.extend_from_slice(&r.modified_unix.to_le_bytes());
    for topic in &r.topics {
        bytes.extend_from_slice(&(topic.len() as u64).to_le_bytes());
        bytes.extend_from_slice(topic.as_bytes());
    }
    fingerprint(&bytes)
}
#[derive(Debug)]
enum IngestError {
    Missing(String),
    Invalid(String),
}
fn ingest(spec: &SourceSpec) -> Result<Record, IngestError> {
    let display = spec.path.display();
    if !spec.path.is_absolute() {
        return Err(IngestError::Invalid(format!(
            "source path must be absolute: {display}"
        )));
    }
    if !matches!(
        spec.path
            .extension()
            .and_then(|x| x.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown" | "txt")
    ) {
        return Err(IngestError::Invalid(format!(
            "unsupported source type: {display}"
        )));
    }
    let mut part = PathBuf::new();
    for component in spec.path.components() {
        match component {
            std::path::Component::RootDir => part.push(component.as_os_str()),
            std::path::Component::Normal(p) => {
                part.push(p);
                let meta = fs::symlink_metadata(&part).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        IngestError::Missing(format!("source missing: {display}"))
                    } else {
                        IngestError::Invalid(format!("{}: {e}", part.display()))
                    }
                })?;
                if meta.file_type().is_symlink() {
                    return Err(IngestError::Invalid(format!(
                        "source path contains a symlink: {display}"
                    )));
                }
            }
            _ => {
                return Err(IngestError::Invalid(format!(
                    "source path must be canonical: {display}"
                )))
            }
        }
    }
    let canonical = fs::canonicalize(&spec.path)
        .map_err(|e| IngestError::Invalid(format!("{display}: {e}")))?;
    if canonical != spec.path {
        return Err(IngestError::Invalid(format!(
            "source path must be canonical: {display}"
        )));
    }
    let meta =
        fs::metadata(&spec.path).map_err(|e| IngestError::Invalid(format!("{display}: {e}")))?;
    if !meta.is_file() {
        return Err(IngestError::Invalid(format!(
            "source is not a regular file: {display}"
        )));
    }
    if meta.len() > MAX_SOURCE_BYTES {
        return Err(IngestError::Invalid(format!(
            "source exceeds {MAX_SOURCE_BYTES} bytes: {display}"
        )));
    }
    let mut file =
        fs::File::open(&spec.path).map_err(|e| IngestError::Invalid(format!("{display}: {e}")))?;
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| IngestError::Invalid(format!("{display}: {e}")))?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(IngestError::Invalid(format!(
            "source grew beyond size limit: {display}"
        )));
    }
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| IngestError::Invalid(format!("source is not UTF-8: {display}")))?;
    let modified_unix = meta
        .modified()
        .ok()
        .and_then(|x| x.duration_since(UNIX_EPOCH).ok())
        .map(|x| x.as_secs())
        .unwrap_or(0);
    Ok(Record {
        schema_version: 1,
        id: spec.id.clone(),
        title: spec.title.clone(),
        project: spec.project.clone(),
        source_path: canonical,
        text,
        modified_unix,
        fingerprint: fingerprint(&bytes),
        kind: spec.kind.clone(),
        topics: spec.topics.clone(),
    })
}
fn read_cache(path: &Path) -> Cache {
    let Ok(meta) = fs::metadata(path) else {
        return Cache::default();
    };
    if !meta.is_file() || meta.len() > MAX_CACHE_BYTES {
        return Cache::default();
    }
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Cache>(&b).ok())
        .filter(|c| {
            c.schema_version == CACHE_VERSION
                && c.records.len() <= MAX_SOURCES
                && c.embeddings.len() <= MAX_SOURCES
        })
        .unwrap_or_default()
}
fn write_cache(path: &Path, cache: &Cache) -> Result<(), String> {
    let parent = path.parent().ok_or("cache path has no parent")?;
    fs::create_dir_all(parent).map_err(|e| format!("cannot create cache directory: {e}"))?;
    let bytes = serde_json::to_vec(cache).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return Err("recall cache exceeds size limit".into());
    }
    let suffix = TMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp-{}-{suffix}", std::process::id()));
    let result = (|| -> Result<(), String> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
/// Idempotently applies a deterministic event stream to the derived cache.
fn apply_events(
    cache: &mut Cache,
    events: &[Event],
    candidates: &BTreeMap<String, Record>,
) -> Result<(), String> {
    for e in events {
        match e.operation.as_str() {
            "upsert" => {
                let r = candidates
                    .get(&e.source_id)
                    .ok_or_else(|| format!("missing candidate {}", e.source_id))?;
                if e.fingerprint.as_deref() != Some(&r.fingerprint) || e.revision != revision(r) {
                    return Err(format!("invalid upsert revision for {}", e.source_id));
                }
                if cache.records.get(&e.source_id).map(revision).as_deref()
                    != Some(e.revision.as_str())
                {
                    cache.records.insert(e.source_id.clone(), r.clone());
                    cache.embeddings.remove(&e.source_id);
                }
            }
            "remove" => {
                if e.fingerprint.is_some() || e.revision != format!("removed:{}", e.source_id) {
                    return Err(format!("invalid remove revision for {}", e.source_id));
                }
                cache.records.remove(&e.source_id);
                cache.embeddings.remove(&e.source_id);
            }
            _ => return Err(format!("invalid recall event operation {}", e.operation)),
        }
    }
    Ok(())
}
pub fn reconcile_with_status(cfg: &Config, path: &Path) -> ReconcileResult {
    validate_config(cfg)?;
    let mut cache = read_cache(path);
    let mut next = BTreeMap::new();
    let mut warnings = Vec::new();
    if cfg.enabled {
        for spec in &cfg.sources {
            match ingest(spec) {
                Ok(r) => {
                    next.insert(r.id.clone(), r);
                }
                Err(IngestError::Missing(e) | IngestError::Invalid(e)) => {
                    warnings.push(format!("{}: {e}", spec.id))
                }
            }
        }
    }
    let mut events = Vec::new();
    for (id, r) in &next {
        if cache.records.get(id).map(revision).as_deref() != Some(revision(r).as_str()) {
            events.push(Event {
                operation: "upsert".into(),
                source_id: id.clone(),
                revision: revision(r),
                fingerprint: Some(r.fingerprint.clone()),
            });
        }
    }
    for id in cache.records.keys().filter(|id| !next.contains_key(*id)) {
        events.push(Event {
            operation: "remove".into(),
            source_id: id.clone(),
            revision: format!("removed:{id}"),
            fingerprint: None,
        });
    }
    apply_events(&mut cache, &events, &next)?;
    cache.embeddings.retain(|id, e| {
        cache
            .records
            .get(id)
            .is_some_and(|r| r.fingerprint == e.fingerprint)
    });
    write_cache(path, &cache)?;
    Ok((cache.records.into_values().collect(), events, warnings))
}
// Kept for callers using the original two-value API.
#[allow(dead_code)]
pub fn reconcile(cfg: &Config, path: &Path) -> Result<(Vec<Record>, Vec<Event>), String> {
    let (records, events, _warnings) = reconcile_with_status(cfg, path)?;
    Ok((records, events))
}
pub fn search(records: &[Record], query: &str, limit: usize) -> Vec<Hit> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Hit> = records
        .iter()
        .filter_map(|r| {
            let title = r.title.to_lowercase();
            let project = r.project.to_lowercase();
            let body = r.text.to_lowercase();
            let topics = r.topics.join(" ").to_lowercase();
            let score: usize = terms
                .iter()
                .map(|t| {
                    if title.contains(t) {
                        8
                    } else if project.contains(t) {
                        5
                    } else if topics.contains(t) {
                        4
                    } else if body.contains(t) {
                        1
                    } else {
                        0
                    }
                })
                .sum();
            (score > 0).then(|| Hit {
                record: r.clone(),
                score,
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.record.id.cmp(&b.record.id))
    });
    hits.truncate(limit.min(50));
    hits
}

#[derive(Clone)]
struct Endpoint {
    host: String,
    address: SocketAddr,
}
fn parse_endpoint(value: &str) -> Result<Endpoint, String> {
    let rest = value
        .strip_prefix("http://")
        .ok_or("Ollama endpoint must use http:// on loopback")?;
    let authority = rest.trim_end_matches('/');
    if authority.contains('/')
        || authority.contains('@')
        || authority.contains('?')
        || authority.contains('#')
    {
        return Err("Ollama endpoint must be a loopback origin".into());
    }
    let (host, port) = if let Some(p) = authority.strip_prefix("127.0.0.1:") {
        (
            "127.0.0.1",
            p.parse::<u16>().map_err(|_| "invalid Ollama port")?,
        )
    } else if let Some(p) = authority.strip_prefix("localhost:") {
        (
            "localhost",
            p.parse::<u16>().map_err(|_| "invalid Ollama port")?,
        )
    } else if let Some(p) = authority.strip_prefix("[::1]:") {
        (
            "[::1]",
            p.parse::<u16>().map_err(|_| "invalid Ollama port")?,
        )
    } else if ["127.0.0.1", "localhost", "[::1]"].contains(&authority) {
        (authority, 11434)
    } else {
        return Err("Ollama endpoint must use localhost, 127.0.0.1, or [::1]".into());
    };
    if port == 0 {
        return Err("invalid Ollama port".into());
    }
    Ok(Endpoint {
        host: host.into(),
        address: if host == "[::1]" {
            SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], port))
        } else {
            SocketAddr::from(([127, 0, 0, 1], port))
        },
    })
}
fn decode_chunked(mut raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    loop {
        let end = raw
            .windows(2)
            .position(|x| x == b"\r\n")
            .ok_or("malformed chunked response")?;
        let size = std::str::from_utf8(&raw[..end])
            .map_err(|_| "malformed chunk size")?
            .split(';')
            .next()
            .unwrap_or("");
        let size = usize::from_str_radix(size.trim(), 16).map_err(|_| "malformed chunk size")?;
        raw = &raw[end + 2..];
        if size == 0 {
            return Ok(body);
        }
        if size > MAX_HTTP_BYTES.saturating_sub(body.len())
            || raw.len() < size + 2
            || &raw[size..size + 2] != b"\r\n"
        {
            return Err("malformed or oversized chunked response".into());
        }
        body.extend_from_slice(&raw[..size]);
        raw = &raw[size + 2..];
    }
}
pub(crate) fn local_post(
    endpoint_url: &str,
    route: &str,
    payload: &serde_json::Value,
    deadline: Instant,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let endpoint = parse_endpoint(endpoint_url)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("Ollama request timed out".into());
    }
    let mut stream =
        TcpStream::connect_timeout(&endpoint.address, remaining.min(Duration::from_millis(500)))
            .map_err(|e| format!("cannot connect to local Ollama: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|e| e.to_string())?;
    let body = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
    let request = format!("POST {route} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", endpoint.host, body.len());
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(&body))
        .map_err(|e| format!("Ollama request: {e}"))?;
    let mut response = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        if Instant::now() >= deadline {
            return Err("Ollama request timed out".into());
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if response.len() + n > max_bytes {
                    return Err("Ollama response too large".into());
                }
                response.extend_from_slice(&buf[..n]);
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue
            }
            Err(e) => return Err(format!("Ollama response: {e}")),
        }
    }
    let split = response
        .windows(4)
        .position(|x| x == b"\r\n\r\n")
        .ok_or("malformed Ollama HTTP response")?;
    let headers =
        std::str::from_utf8(&response[..split]).map_err(|_| "malformed Ollama headers")?;
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.contains(" 200 "))
    {
        return Err(format!(
            "Ollama HTTP error: {}",
            headers.lines().next().unwrap_or("unknown")
        ));
    }
    let body = if headers.lines().any(|h| {
        h.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    }) {
        decode_chunked(&response[split + 4..])?
    } else {
        response[split + 4..].to_vec()
    };
    Ok(body)
}
fn embed(
    endpoint: &Endpoint,
    model: &str,
    input: &[String],
    deadline: Instant,
) -> Result<Vec<Vec<f32>>, String> {
    let url = format!("http://{}:{}", endpoint.host, endpoint.address.port());
    let body = local_post(
        &url,
        "/api/embed",
        &serde_json::json!({"model":model,"input":input,"truncate":true}),
        deadline,
        MAX_HTTP_BYTES,
    )?;
    #[derive(Deserialize)]
    struct EmbedResponse {
        embeddings: Vec<Vec<f32>>,
    }
    let parsed: EmbedResponse = serde_json::from_slice(&body)
        .map_err(|e| format!("invalid Ollama embedding response: {e}"))?;
    if parsed.embeddings.len() != input.len() {
        return Err("Ollama embedding count mismatch".into());
    }
    let dimension = parsed.embeddings.first().map(Vec::len).unwrap_or(0);
    if dimension == 0
        || dimension > 4096
        || parsed
            .embeddings
            .iter()
            .any(|v| v.len() != dimension || !valid_vector(v))
    {
        return Err("invalid Ollama embedding dimensions or values".into());
    }
    Ok(parsed.embeddings)
}
fn valid_vector(v: &[f32]) -> bool {
    !v.is_empty()
        && v.len() <= 4096
        && v.iter().all(|x| x.is_finite())
        && v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>() > 0.0
}
fn cosine(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.len() != b.len() || !valid_vector(a) || !valid_vector(b) {
        return None;
    }
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let an: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let bn: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    Some((dot / (an * bn)).clamp(-1.0, 1.0))
}
fn embed_text(text: &str) -> String {
    text.chars().take(MAX_EMBED_CHARS).collect()
}
pub fn retrieve(records: &[Record], cfg: &Config, query: &str, limit: usize) -> Retrieval {
    retrieve_with_cache(records, cfg, query, limit, &cache_path())
}
pub fn retrieve_with_cache(
    records: &[Record],
    cfg: &Config,
    query: &str,
    limit: usize,
    cache_path: &Path,
) -> Retrieval {
    let lexical = || search(records, query, limit);
    if query.trim().is_empty() || limit == 0 {
        return Retrieval {
            hits: Vec::new(),
            status: "empty query".into(),
        };
    }
    let Some(o) = cfg.ollama.as_ref().filter(|o| o.enabled) else {
        return Retrieval {
            hits: lexical(),
            status: "lexical (semantic disabled)".into(),
        };
    };
    let fallback = |why: String| Retrieval {
        hits: lexical(),
        status: format!("lexical (semantic unavailable: {why})"),
    };
    if query.chars().count() > 4096 {
        return fallback("query exceeds 4096 characters".into());
    }
    let endpoint = match parse_endpoint(&o.endpoint) {
        Ok(e) => e,
        Err(e) => return fallback(e),
    };
    if records.len() > MAX_SOURCES {
        return fallback(format!("semantic source limit is {MAX_SOURCES}"));
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut cache = read_cache(cache_path);
    cache.records = records.iter().map(|r| (r.id.clone(), r.clone())).collect();
    cache
        .embeddings
        .retain(|id, _| cache.records.contains_key(id));
    let mut input = vec![query.to_string()];
    let mut missing = Vec::new();
    let mut cached_dimension = None;
    for r in records.iter().take(MAX_SOURCES) {
        let valid = cache.embeddings.get(&r.id).is_some_and(|e| {
            e.fingerprint == r.fingerprint
                && e.endpoint == o.endpoint
                && e.model == o.model
                && valid_vector(&e.vector)
        });
        if valid {
            let dimension = cache.embeddings[&r.id].vector.len();
            if cached_dimension.is_some_and(|d| d != dimension) {
                cache.embeddings.remove(&r.id);
                input.push(embed_text(&r.text));
                missing.push(r.id.clone());
            } else {
                cached_dimension = Some(dimension);
            }
        } else {
            cache.embeddings.remove(&r.id);
            input.push(embed_text(&r.text));
            missing.push(r.id.clone());
        }
    }
    let vectors = match embed(&endpoint, &o.model, &input, deadline) {
        Ok(v) => v,
        Err(e) => return fallback(e),
    };
    let query_vector = vectors[0].clone();
    if cached_dimension.is_some_and(|d| d != query_vector.len()) {
        // Model dimensions changed: re-embed every source before ranking.
        input = std::iter::once(query.to_string())
            .chain(
                records
                    .iter()
                    .take(MAX_SOURCES)
                    .map(|r| embed_text(&r.text)),
            )
            .collect();
        let all = match embed(&endpoint, &o.model, &input, deadline) {
            Ok(v) => v,
            Err(e) => return fallback(e),
        };
        cache.embeddings.clear();
        for (r, v) in records.iter().take(MAX_SOURCES).zip(all.iter().skip(1)) {
            cache.embeddings.insert(
                r.id.clone(),
                Embedding {
                    fingerprint: r.fingerprint.clone(),
                    endpoint: o.endpoint.clone(),
                    model: o.model.clone(),
                    vector: v.clone(),
                },
            );
        }
        return rank_semantic(records, query, &all[0], &cache, cache_path, limit, fallback);
    }
    for (id, v) in missing.into_iter().zip(vectors.into_iter().skip(1)) {
        let r = records
            .iter()
            .find(|r| r.id == id)
            .expect("missing record id came from records");
        cache.embeddings.insert(
            id,
            Embedding {
                fingerprint: r.fingerprint.clone(),
                endpoint: o.endpoint.clone(),
                model: o.model.clone(),
                vector: v,
            },
        );
    }
    rank_semantic(
        records,
        query,
        &query_vector,
        &cache,
        cache_path,
        limit,
        fallback,
    )
}
fn strong_lexical_match(record: &Record, terms: &[String]) -> bool {
    let fields = format!(
        "{} {} {}",
        record.title,
        record.project,
        record.topics.join(" ")
    )
    .to_lowercase();
    terms.iter().any(|term| fields.contains(term))
}
fn hybrid_hits(
    records: &[Record],
    query_text: &str,
    query_vector: &[f32],
    cache: &Cache,
    limit: usize,
) -> Result<Vec<Hit>, String> {
    let terms: Vec<String> = query_text
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let lexical = search(records, query_text, 50);
    let mut strong = Vec::new();
    let mut weak = Vec::new();
    let mut seen = BTreeSet::new();
    for hit in lexical {
        if strong_lexical_match(&hit.record, &terms) {
            seen.insert(hit.record.id.clone());
            strong.push(hit);
        } else {
            weak.push(hit);
        }
    }
    let mut semantic = Vec::new();
    for r in records {
        let e = cache
            .embeddings
            .get(&r.id)
            .ok_or("missing source embedding")?;
        let sim = cosine(query_vector, &e.vector).ok_or("embedding dimension mismatch")?;
        if !seen.contains(&r.id) && sim >= MIN_SEMANTIC_SIMILARITY {
            // Score is an internal ordering key, never factual confidence.
            semantic.push((
                sim,
                Hit {
                    record: r.clone(),
                    score: (sim * 1000.0).round() as usize,
                },
            ));
        }
    }
    semantic.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| a.1.record.id.cmp(&b.1.record.id))
    });
    let mut hits = strong;
    for (_, hit) in semantic {
        if seen.insert(hit.record.id.clone()) {
            hits.push(hit);
        }
    }
    for hit in weak {
        if seen.insert(hit.record.id.clone()) {
            hits.push(hit);
        }
    }
    hits.truncate(limit.min(50));
    Ok(hits)
}
fn rank_semantic(
    records: &[Record],
    query_text: &str,
    query_vector: &[f32],
    cache: &Cache,
    path: &Path,
    limit: usize,
    fallback: impl Fn(String) -> Retrieval,
) -> Retrieval {
    let hits = match hybrid_hits(records, query_text, query_vector, cache, limit) {
        Ok(h) => h,
        Err(e) => return fallback(e),
    };
    let status = match write_cache(path, cache) {
        Ok(()) => "hybrid (exact matches, local semantic similarity; 0.35 heuristic)".into(),
        Err(e) => format!("hybrid (local semantic similarity; cache unavailable: {e})"),
    };
    Retrieval { hits, status }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str, text: &str) -> Record {
        Record {
            schema_version: 1,
            id: id.into(),
            title: id.into(),
            project: "p".into(),
            source_path: format!("/tmp/{id}.md").into(),
            text: text.into(),
            modified_unix: 0,
            fingerprint: fingerprint(text.as_bytes()),
            kind: "document".into(),
            topics: vec![],
        }
    }
    fn scratch() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "recall-{}-{}",
            std::process::id(),
            TMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn spec(id: &str, path: PathBuf) -> SourceSpec {
        SourceSpec {
            id: id.into(),
            title: id.into(),
            project: "p".into(),
            path,
            kind: "document".into(),
            topics: vec![],
        }
    }
    #[test]
    fn lexical_rank_and_empty() {
        let a = record("music", "music");
        let b = record("other", "music");
        assert_eq!(search(&[b, a], "music", 10)[0].record.id, "music");
        assert!(search(&[], " ", 5).is_empty());
    }
    #[test]
    fn reconciliation_removes_missing_and_updates_metadata() {
        let dir = scratch();
        let a = dir.join("a.md");
        let b = dir.join("b.txt");
        fs::write(&a, "alpha").unwrap();
        fs::write(&b, "beta").unwrap();
        let cache = dir.join("cache.json");
        let mut cfg = Config {
            planner: None,
            enabled: true,
            sources: vec![spec("a", a.clone()), spec("b", b.clone())],
            ollama: None,
        };
        let (r, e, w) = reconcile_with_status(&cfg, &cache).unwrap();
        assert_eq!((r.len(), e.len(), w.len()), (2, 2, 0));
        assert!(reconcile_with_status(&cfg, &cache).unwrap().1.is_empty());
        cfg.sources[0].title = "new title".into();
        let (_, e, _) = reconcile_with_status(&cfg, &cache).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].source_id, "a");
        fs::remove_file(&b).unwrap();
        let (r, e, w) = reconcile_with_status(&cfg, &cache).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(e[0].operation, "remove");
        assert_eq!(w.len(), 1);
        assert!(reconcile_with_status(&cfg, &cache).unwrap().1.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn events_are_idempotent_and_validate_revision() {
        let r = record("a", "hello");
        let candidate = BTreeMap::from([("a".into(), r.clone())]);
        let event = Event {
            operation: "upsert".into(),
            source_id: "a".into(),
            revision: revision(&r),
            fingerprint: Some(r.fingerprint.clone()),
        };
        let mut cache = Cache::default();
        apply_events(&mut cache, &[event.clone(), event.clone()], &candidate).unwrap();
        assert_eq!(cache.records.len(), 1);
        let mut bad = event;
        bad.revision = "bad".into();
        assert!(apply_events(&mut cache, &[bad], &candidate).is_err());
        let remove = Event {
            operation: "remove".into(),
            source_id: "a".into(),
            revision: "removed:a".into(),
            fingerprint: None,
        };
        apply_events(&mut cache, &[remove.clone(), remove], &candidate).unwrap();
        assert!(cache.records.is_empty());
    }
    #[test]
    fn corrupt_cache_and_path_rejection() {
        let dir = scratch();
        let p = dir.join("cache.json");
        fs::write(&p, b"nope").unwrap();
        assert!(read_cache(&p).records.is_empty());
        let source = dir.join("real.md");
        fs::write(&source, "x").unwrap();
        let link = dir.join("link.md");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(matches!(
            ingest(&spec("x", link)),
            Err(IngestError::Invalid(_))
        ));
        let nested = dir.join("alias");
        std::os::unix::fs::symlink(&dir, &nested).unwrap();
        assert!(matches!(
            ingest(&spec("x", nested.join("real.md"))),
            Err(IngestError::Invalid(_))
        ));
        assert!(matches!(
            ingest(&spec("x", PathBuf::from("relative.md"))),
            Err(IngestError::Invalid(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn vector_validation_and_lexical_fallback() {
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), Some(0.0));
        assert!(cosine(&[1.0], &[1.0, 0.0]).is_none());
        assert!(!valid_vector(&[f32::NAN]));
        assert_eq!(decode_chunked(b"3\r\nabc\r\n0\r\n\r\n").unwrap(), b"abc");
        assert!(decode_chunked(b"3\r\nab\r\n0\r\n\r\n").is_err());
        let cfg = Config {
            planner: None,
            enabled: true,
            sources: vec![],
            ollama: Some(OllamaConfig {
                enabled: true,
                endpoint: "https://example.com".into(),
                model: "bge-large".into(),
            }),
        };
        let result = retrieve_with_cache(
            &[record("a", "hello")],
            &cfg,
            "hello",
            3,
            Path::new("/tmp/no-cache"),
        );
        assert_eq!(result.hits.len(), 1);
        assert!(result.status.contains("semantic unavailable"));
    }
    #[test]
    fn hybrid_keeps_exact_first_and_excludes_unrelated_vectors() {
        let mut exact = record("exact", "notes");
        exact.title = "Budget plan".into();
        let related = record("related", "expenses and planning");
        let weak = record("weak", "budget mentioned in passing");
        let unrelated = record("unrelated", "weather notes");
        let records = vec![related, unrelated, weak, exact];
        let mut cache = Cache::default();
        for (id, vector) in [
            ("exact", vec![0.0, 1.0]),
            ("related", vec![0.8, 0.6]),
            ("weak", vec![0.0, 1.0]),
            ("unrelated", vec![-1.0, 0.0]),
        ] {
            cache.embeddings.insert(
                id.into(),
                Embedding {
                    fingerprint: "x".into(),
                    endpoint: "local".into(),
                    model: "test".into(),
                    vector,
                },
            );
        }
        let hits = hybrid_hits(&records, "budget", &[1.0, 0.0], &cache, 10).unwrap();
        assert_eq!(
            hits.iter()
                .map(|h| h.record.id.as_str())
                .collect::<Vec<_>>(),
            vec!["exact", "related", "weak"]
        );
        assert!(hits.iter().all(|h| h.record.id != "unrelated"));
        assert_eq!(hits[1].score, 800); // Similarity sort key, not confidence.
    }
    #[test]
    fn local_embedding_ranks_and_reuses_source_vectors() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut input_counts = Vec::new();
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                let (split, length) = loop {
                    let n = socket.read(&mut buf).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buf[..n]);
                    if let Some(split) = request.windows(4).position(|x| x == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&request[..split]).unwrap();
                        assert!(headers.starts_with("POST /api/embed HTTP/1.1"));
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|n| n.parse::<usize>().ok())
                            })
                            .unwrap();
                        if request.len() >= split + 4 + length {
                            break (split, length);
                        }
                    }
                };
                let payload: serde_json::Value =
                    serde_json::from_slice(&request[split + 4..split + 4 + length]).unwrap();
                let inputs = payload["input"].as_array().unwrap();
                input_counts.push(inputs.len());
                let vectors: Vec<Vec<f32>> = inputs
                    .iter()
                    .map(|v| {
                        if v.as_str().unwrap().contains("beta") {
                            vec![0.0, 1.0]
                        } else {
                            vec![1.0, 0.0]
                        }
                    })
                    .collect();
                let body = serde_json::to_vec(&serde_json::json!({"embeddings":vectors})).unwrap();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).unwrap();
                socket.write_all(&body).unwrap();
            }
            input_counts
        });
        let dir = scratch();
        let path = dir.join("cache.json");
        let cfg = Config {
            planner: None,
            enabled: true,
            sources: vec![],
            ollama: Some(OllamaConfig {
                enabled: true,
                endpoint: format!("http://127.0.0.1:{port}"),
                model: "bge-large".into(),
            }),
        };
        let original = vec![record("a", "alpha"), record("b", "beta")];
        let first = retrieve_with_cache(&original, &cfg, "beta", 2, &path);
        assert_eq!(first.hits[0].record.id, "b");
        assert!(first.status.starts_with("hybrid"));
        let second = retrieve_with_cache(&original, &cfg, "beta", 2, &path);
        assert_eq!(second.hits[0].record.id, "b");
        let changed = vec![record("a", "alpha changed"), record("b", "beta")];
        let third = retrieve_with_cache(&changed, &cfg, "beta", 2, &path);
        assert_eq!(third.hits[0].record.id, "b");
        assert_eq!(server.join().unwrap(), vec![3, 1, 2]);
        assert_eq!(read_cache(&path).records.len(), 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
