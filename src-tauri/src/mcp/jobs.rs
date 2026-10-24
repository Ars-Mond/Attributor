//! Long-running tool work (saving or attributing many images) as jobs with ids.
//! A tool waits a bounded time for its job and otherwise returns the id for `get_job` and `cancel_job`, which keeps every call within the clients' tool timeouts.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::sync::watch;

/// Finished jobs are forgotten after this long.
const RETENTION: Duration = Duration::from_secs(3600);
/// Most finished jobs kept.
const MAX_FINISHED: usize = 50;

/// One job: its per-item results so far and whether it has finished.
pub(crate) struct Job {
    pub id: String,
    pub kind: &'static str,
    pub total: usize,
    cancel: Arc<AtomicBool>,
    state: Mutex<State>,
    finished: watch::Sender<bool>,
}

#[derive(Default)]
struct State {
    items: Vec<Value>,
    finished_at: Option<Instant>,
    /// Whether the job had been cancelled when it finished.
    cancelled: bool,
}

impl Job {
    /// The flag the work checks between items (and passes to cancelable calls).
    pub(crate) fn cancel_flag(&self) -> &Arc<AtomicBool> {
        &self.cancel
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Asks a running job to stop; a finished job is left as it ended.
    pub(crate) fn cancel(&self) {
        let state = self.lock();
        if state.finished_at.is_none() {
            self.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Records the result of the next item.
    pub(crate) fn push(&self, item: Value) {
        self.lock().items.push(item);
    }

    fn finish(&self) {
        {
            let mut state = self.lock();
            state.finished_at = Some(Instant::now());
            state.cancelled = self.is_cancelled();
        }
        self.finished.send_replace(true);
    }

    fn finished_at(&self) -> Option<Instant> {
        self.lock().finished_at
    }

    /// Progress and results as the job tools return them.
    pub(crate) fn snapshot(&self) -> Value {
        let state = self.lock();
        let status = match (state.finished_at.is_some(), state.cancelled) {
            (false, _) => "running",
            (true, true) => "cancelled",
            (true, false) => "done",
        };
        json!({
            "jobId": self.id,
            "kind": self.kind,
            "status": status,
            "total": self.total,
            "done": state.items.len(),
            "items": state.items,
        })
    }

    /// Waits up to `timeout` for the job to finish, then returns its snapshot.
    pub(crate) async fn wait(&self, timeout: Duration) -> Value {
        let mut rx = self.finished.subscribe();
        let _ = tokio::time::timeout(timeout, rx.wait_for(|done| *done)).await;
        self.snapshot()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Marks the job finished when the work ends, even if it panics.
struct FinishOnDrop(Arc<Job>);

impl Drop for FinishOnDrop {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// All jobs of this app run.
#[derive(Default)]
pub(crate) struct Jobs {
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    counter: AtomicU64,
}

impl Jobs {
    /// Starts `work` for `total` items on the current Tokio runtime and returns its job.
    pub(crate) fn spawn<F, Fut>(&self, kind: &'static str, total: usize, work: F) -> Arc<Job>
    where
        F: FnOnce(Arc<Job>) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        // The random part keeps ids from a previous app run from naming a new job.
        let mut salt = [0u8; 3];
        let _ = ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut salt);
        let id = format!("job-{n}-{:02x}{:02x}{:02x}", salt[0], salt[1], salt[2]);
        let job = Arc::new(Job {
            id: id.clone(),
            kind,
            total,
            cancel: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(State::default()),
            finished: watch::channel(false).0,
        });
        {
            let mut jobs = self.lock();
            prune(&mut jobs);
            jobs.insert(id, job.clone());
        }
        let guard = FinishOnDrop(job.clone());
        let fut = work(job.clone());
        tokio::spawn(async move {
            fut.await;
            drop(guard);
        });
        job
    }

    pub(crate) fn get(&self, id: &str) -> Option<Arc<Job>> {
        self.lock().get(id).cloned()
    }

    /// Asks every running job to stop.
    pub(crate) fn cancel_all(&self) {
        for job in self.lock().values() {
            job.cancel();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Job>>> {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Drops expired finished jobs, then the oldest finished ones beyond [`MAX_FINISHED`].
fn prune(jobs: &mut HashMap<String, Arc<Job>>) {
    jobs.retain(|_, job| job.finished_at().is_none_or(|at| at.elapsed() < RETENTION));
    let mut finished: Vec<(Instant, String)> = jobs
        .iter()
        .filter_map(|(id, job)| job.finished_at().map(|at| (at, id.clone())))
        .collect();
    if finished.len() > MAX_FINISHED {
        finished.sort();
        for (_, id) in finished.iter().take(finished.len() - MAX_FINISHED) {
            jobs.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn jobs_report_items_and_finish() {
        let jobs = Jobs::default();
        let job = jobs.spawn("test", 2, |job| async move {
            job.push(json!({"n": 1}));
            tokio::task::yield_now().await;
            job.push(json!({"n": 2}));
        });
        let snap = job.wait(Duration::from_secs(5)).await;
        assert_eq!(snap["status"], "done");
        assert_eq!(snap["done"], 2);
        // Cancelling a finished job changes nothing.
        job.cancel();
        jobs.cancel_all();
        assert_eq!(job.snapshot()["status"], "done");
        assert!(!job.is_cancelled());
        assert!(jobs.get(&job.id).is_some());
        assert!(jobs.get("job-0-000000").is_none());
    }

    #[tokio::test]
    async fn cancelled_jobs_say_so_and_waits_time_out() {
        let jobs = Jobs::default();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let job = jobs.spawn("test", 1, |_job| async move {
            let _ = rx.await;
        });
        let snap = job.wait(Duration::from_millis(20)).await;
        assert_eq!(snap["status"], "running");
        job.cancel();
        let _ = tx.send(());
        let snap = job.wait(Duration::from_secs(5)).await;
        assert_eq!(snap["status"], "cancelled");
    }
}
