use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, oneshot};

use crate::model::WatcherEvent;
use crate::parser::csv_parser::extract_file_key;
use crate::parser::parse_run_pair;
use crate::storage::db::Database;

/// Configuration for the KovaaK's directory watcher and pair debouncer.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub stats_dir: PathBuf,
    pub performances_dir: PathBuf,
    pub db_path: PathBuf,
    /// Stability wait after both `.csv` and `.perf` arrive (default: 180 ms).
    pub stability_wait_ms: u64,
    /// Fallback wait before ingesting a `.csv` alone if no `.perf` arrives (default: 1200 ms).
    pub perf_fallback_ms: u64,
}

impl WatcherConfig {
    pub fn new(stats_dir: PathBuf, performances_dir: PathBuf, db_path: PathBuf) -> Self {
        Self {
            stats_dir,
            performances_dir,
            db_path,
            stability_wait_ms: 180,
            perf_fallback_ms: 1200,
        }
    }
}

/// Handle that keeps the file watcher alive. Dropping it stops the watcher cleanly.
pub struct WatcherHandle {
    _shutdown_tx: Option<oneshot::Sender<()>>,
    _watcher: Option<RecommendedWatcher>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        if let Some(tx) = self._shutdown_tx.take() {
            let _ = tx.send(());
        }
        self._watcher.take();
    }
}

struct PendingPair {
    csv_path: Option<PathBuf>,
    perf_path: Option<PathBuf>,
    first_seen: Instant,
    last_change: Instant,
    last_csv_size: u64,
    last_perf_size: u64,
}

/// Spawns a background file watcher on `stats_dir` and `performances_dir`.
/// Returns an unbounded receiver of `WatcherEvent` and a `WatcherHandle`.
/// Requires being called inside a Tokio runtime context.
pub fn spawn_watcher(
    cfg: WatcherConfig,
) -> Result<(mpsc::UnboundedReceiver<WatcherEvent>, WatcherHandle), String> {
    let (out_tx, out_rx) = mpsc::unbounded_channel::<WatcherEvent>();
    let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<PathBuf>();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => {
                        for path in event.paths {
                            let _ = fs_tx.send(path);
                        }
                    }
                    _ => {}
                }
            }
        },
        NotifyConfig::default(),
    )
    .map_err(|e| format!("Failed to create notify watcher: {e}"))?;

    if cfg.stats_dir.exists() {
        watcher
            .watch(&cfg.stats_dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch stats dir {:?}: {e}", cfg.stats_dir))?;
    }
    if cfg.performances_dir.exists() && cfg.performances_dir != cfg.stats_dir {
        watcher
            .watch(&cfg.performances_dir, RecursiveMode::NonRecursive)
            .map_err(|e| {
                format!(
                    "Failed to watch performances dir {:?}: {e}",
                    cfg.performances_dir
                )
            })?;
    }

    let _ = out_tx.send(WatcherEvent::WatcherReady {
        stats_dir: cfg.stats_dir.to_string_lossy().into_owned(),
        performances_dir: cfg.performances_dir.to_string_lossy().into_owned(),
    });

    let stability_wait = Duration::from_millis(cfg.stability_wait_ms);
    let perf_fallback = Duration::from_millis(cfg.perf_fallback_ms);
    let stats_dir = cfg.stats_dir.clone();
    let perf_dir = cfg.performances_dir.clone();
    let db_path = cfg.db_path.clone();

    tokio::spawn(async move {
        let mut pending: HashMap<String, PendingPair> = HashMap::new();
        let poll_step = Duration::from_millis(40);

        loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            // Wait up to 40ms for the next filesystem event, then drain any queued events
            match tokio::time::timeout(poll_step, fs_rx.recv()).await {
                Ok(Some(first_path)) => {
                    enqueue_fs_path(first_path, &mut pending, &stats_dir, &perf_dir);
                    while let Ok(more_path) = fs_rx.try_recv() {
                        enqueue_fs_path(more_path, &mut pending, &stats_dir, &perf_dir);
                    }
                }
                Ok(None) => break, // Channel closed
                Err(_) => {}       // Timeout tick
            }

            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            if pending.is_empty() {
                continue;
            }

            let now = Instant::now();
            let mut ready_keys: Vec<String> = Vec::new();

            for (key, pair) in pending.iter_mut() {
                // Refresh file sizes to detect in-progress writes
                if let Some(ref cp) = pair.csv_path {
                    let sz = file_size(cp);
                    if sz != pair.last_csv_size {
                        pair.last_csv_size = sz;
                        pair.last_change = now;
                    }
                }
                if let Some(ref pp) = pair.perf_path {
                    let sz = file_size(pp);
                    if sz != pair.last_perf_size {
                        pair.last_perf_size = sz;
                        pair.last_change = now;
                    }
                }

                let has_csv = pair.csv_path.is_some() && pair.last_csv_size > 0;
                let has_perf = pair.perf_path.is_some() && pair.last_perf_size > 0;
                let stable = now.duration_since(pair.last_change) >= stability_wait;

                if has_csv && has_perf && stable {
                    ready_keys.push(key.clone());
                } else if has_csv
                    && !has_perf
                    && stable
                    && now.duration_since(pair.first_seen) >= perf_fallback
                {
                    ready_keys.push(key.clone());
                } else if !has_csv && now.duration_since(pair.first_seen) >= Duration::from_secs(5)
                {
                    // Orphaned `.perf` without `.csv` — discard after 5s
                    ready_keys.push(key.clone());
                }
            }

            for key in ready_keys {
                if let Some(pair) = pending.remove(&key) {
                    let Some(csv_path) = pair.csv_path else {
                        continue;
                    };
                    let perf_path = pair.perf_path;
                    let started = pair.first_seen;
                    let db_path_clone = db_path.clone();
                    let out_tx_clone = out_tx.clone();

                    let res = tokio::task::spawn_blocking(move || {
                        ingest_pair_sync(&db_path_clone, &csv_path, perf_path.as_deref(), started)
                    })
                    .await;

                    match res {
                        Ok(Ok(event)) => {
                            let _ = out_tx_clone.send(event);
                        }
                        Ok(Err(err)) => {
                            let _ = out_tx_clone.send(WatcherEvent::Error(err));
                        }
                        Err(join_err) => {
                            let _ = out_tx_clone.send(WatcherEvent::Error(format!(
                                "Watcher ingest task panicked: {join_err}"
                            )));
                        }
                    }
                }
            }
        }
    });

    let handle = WatcherHandle {
        _shutdown_tx: Some(shutdown_tx),
        _watcher: Some(watcher),
    };
    Ok((out_rx, handle))
}

fn enqueue_fs_path(
    path: PathBuf,
    pending: &mut HashMap<String, PendingPair>,
    stats_dir: &Path,
    perf_dir: &Path,
) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    let Some(ext) = ext else { return };
    if ext != "csv" && ext != "perf" {
        return;
    }
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    let key = extract_file_key(file_name);
    let now = Instant::now();

    let entry = pending.entry(key.clone()).or_insert_with(|| PendingPair {
        csv_path: None,
        perf_path: None,
        first_seen: now,
        last_change: now,
        last_csv_size: 0,
        last_perf_size: 0,
    });

    if ext == "csv" {
        entry.csv_path = Some(path.clone());
        entry.last_csv_size = file_size(&path);
        if entry.perf_path.is_none() {
            let candidate = perf_dir.join(format!("{key} Performance.perf"));
            if candidate.exists() {
                entry.last_perf_size = file_size(&candidate);
                entry.perf_path = Some(candidate);
            }
        }
    } else if ext == "perf" {
        entry.perf_path = Some(path.clone());
        entry.last_perf_size = file_size(&path);
        if entry.csv_path.is_none() {
            let candidate = stats_dir.join(format!("{key} Stats.csv"));
            if candidate.exists() {
                entry.last_csv_size = file_size(&candidate);
                entry.csv_path = Some(candidate);
            }
        }
    }
    entry.last_change = now;
}

fn ingest_pair_sync(
    db_path: &Path,
    csv_path: &Path,
    perf_path: Option<&Path>,
    started: Instant,
) -> Result<WatcherEvent, String> {
    let parsed = parse_run_pair(csv_path, perf_path)?;
    let db = Database::open(db_path)?;
    let (run_id, is_new_pb) = db.ingest_parsed_run(&parsed)?;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(WatcherEvent::RunIngested {
        run_id,
        scenario: parsed.run.scenario,
        score: parsed.run.score,
        accuracy: parsed.run.accuracy,
        kps: parsed.run.avg_kps,
        is_new_pb,
        elapsed_ms,
    })
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
