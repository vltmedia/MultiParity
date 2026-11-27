//! How far along a long operation is (get latest, publish, connecting a folder, saving a draft),
//! for a progress bar in the desktop app and the command line.
//!
//! Whoever starts the operation wraps it in [`scope`] with a [`Progress`]; the operations report
//! into whichever one is in scope and do nothing when none is, so callers that do not show
//! progress need no changes. Another thread or request reads it with [`Progress::snapshot`].

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProgressState {
    /// What is running: "get-latest", "publish", "connect", "draft".
    pub op: String,
    /// The step within it, in plain words: "Checking your files", "Sending", "Downloading" ...
    pub phase: String,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// The file being worked on.
    pub current: Option<String>,
    /// When the operation started, milliseconds since 1970.
    pub started_ms: u64,
    /// Bytes a second over the current phase so far (0 until there is enough to tell).
    pub rate: u64,
    /// Set once the operation has finished, so a reader polling late sees the end state.
    pub finished: bool,
}

#[derive(Clone)]
pub struct Progress {
    state: Arc<Mutex<ProgressState>>,
    phase_started: Arc<Mutex<Instant>>,
}

impl Progress {
    pub fn new(op: &str) -> Self {
        let started_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        Progress {
            state: Arc::new(Mutex::new(ProgressState { op: op.into(), phase: "Starting".into(), started_ms, ..Default::default() })),
            phase_started: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn snapshot(&self) -> ProgressState {
        let mut s = self.state.lock().unwrap().clone();
        let secs = self.phase_started.lock().unwrap().elapsed().as_secs_f64();
        if secs > 0.5 && s.bytes_done > 0 {
            s.rate = (s.bytes_done as f64 / secs) as u64;
        }
        s
    }

    pub fn finish(&self) {
        let mut s = self.state.lock().unwrap();
        s.finished = true;
        s.current = None;
    }

    fn edit(&self, f: impl FnOnce(&mut ProgressState)) {
        f(&mut self.state.lock().unwrap());
    }
}

tokio::task_local! {
    static CURRENT: Progress;
}

/// Run `f` with `p` receiving its progress.
pub async fn scope<F: Future>(p: Progress, f: F) -> F::Output {
    CURRENT.scope(p, f).await
}

fn with(f: impl FnOnce(&Progress)) {
    let _ = CURRENT.try_with(|p| f(p));
}

/// Start a step: what it is and how much there is to do. Counts start again from zero.
pub fn phase(name: &str, files_total: u64, bytes_total: u64) {
    with(|p| {
        p.edit(|s| {
            s.phase = name.into();
            s.files_total = files_total;
            s.bytes_total = bytes_total;
            s.files_done = 0;
            s.bytes_done = 0;
            s.current = None;
        });
        *p.phase_started.lock().unwrap() = Instant::now();
    });
}

/// Working on this file now.
pub fn file(path: &str) {
    with(|p| p.edit(|s| s.current = Some(path.to_string())));
}

/// This much more of the step's bytes is done.
pub fn bytes(n: u64) {
    with(|p| p.edit(|s| s.bytes_done += n));
}

/// One more file of the step is done.
pub fn file_done() {
    with(|p| p.edit(|s| s.files_done += 1));
}

/// A file turned out to need nothing (already up to date, or kept as yours): count it as done
/// and take its bytes out of what is left, so the bar does not stall short of the end.
pub fn skip(size: u64) {
    with(|p| {
        p.edit(|s| {
            s.files_done += 1;
            s.bytes_total = s.bytes_total.saturating_sub(size).max(s.bytes_done);
        })
    });
}
