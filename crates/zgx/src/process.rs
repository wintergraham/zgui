//! On-demand process-family accounting shared by apps and the monitor.
//! No timer or thread is installed. CPU is percent of one core over the actual
//! interval; RSS sums include shared pages more than once. Missing is not zero.
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Low-level readers also used by the interactive monitor.
#[path = "process/platform.rs"]
pub mod platform;

/// A single process lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Identity {
    /// OS process ID.
    pub pid: i32,
    /// OS start token, comparable only within this boot and platform.
    pub start: u64,
}

/// Resource counters and metadata from one snapshot.
#[derive(Clone, Debug)]
pub struct Reading {
    /// OS process ID.
    pub pid: i32,
    /// Parent PID, or zero when unavailable.
    pub parent: i32,
    /// None if this lifetime could not be read consistently.
    pub identity: Option<Identity>,
    /// Executable basename, never command-line arguments.
    pub name: String,
    /// Accumulated user plus system CPU nanoseconds.
    pub cpu_ns: Option<u64>,
    /// Resident bytes; shared pages may appear in several processes.
    pub rss_bytes: Option<u64>,
    /// Physical-footprint charge on macOS; unavailable elsewhere.
    pub footprint_bytes: Option<u64>,
    /// Accumulated GPU client nanoseconds, when reported.
    pub gpu_ns: Option<u64>,
}

/// Enumerate once; recheck identity after metadata reads to avoid joining two
/// lifetimes of a PID. Command lines and application labels are never read here.
pub fn snapshot(gpu: bool) -> Vec<Reading> {
    let gpu_times = if gpu {
        platform::gpu_times()
    } else {
        HashMap::new()
    };
    platform::pids()
        .into_iter()
        .map(|pid| {
            let before = platform::read(pid);
            let parent = platform::parent(pid);
            let name = platform::name(pid);
            let usage = before
                .and_then(|before| platform::read(pid).filter(|after| before.start == after.start));
            Reading {
                pid,
                parent,
                name,
                identity: usage.as_ref().map(|u| Identity {
                    pid,
                    start: u.start,
                }),
                cpu_ns: usage.as_ref().map(|u| u.cpu_ns),
                rss_bytes: usage.as_ref().map(|u| u.rss),
                footprint_bytes: usage
                    .as_ref()
                    .and_then(|u| cfg!(target_os = "macos").then_some(u.footprint)),
                gpu_ns: gpu_times.get(&pid).copied(),
            }
        })
        .collect()
}

/// A reading and deltas against the previous snapshot.
#[derive(Clone, Debug)]
pub struct Sample {
    /// Current counters and process metadata.
    pub reading: Reading,
    /// Percent of one CPU core; may exceed 100.
    pub cpu_percent: Option<f64>,
    /// GPU client time divided by elapsed wall time, times 100.
    pub gpu_percent: Option<f64>,
}

/// A deduplicated process family at one sampling boundary.
#[derive(Debug)]
pub struct FamilySample {
    /// The original root lifetime was readable in this snapshot.
    pub root_available: bool,
    /// None until a previous sample exists.
    pub interval_ms: Option<f64>,
    /// Observed root and descendants, including known reparented children.
    pub processes: Vec<Sample>,
}

/// A sampler is permanently bound to one root lifetime. Descendants observed
/// before reparenting remain owned while that exact identity is still alive.
/// A detached process never observed under the root cannot be attributed.
pub struct FamilySampler {
    root: Identity,
    owned: HashSet<Identity>,
    previous: HashMap<Identity, Reading>,
    at: Option<Instant>,
}
impl FamilySampler {
    /// Bind to the current lifetime of `pid`, or fail if unavailable.
    pub fn new(pid: i32) -> Option<Self> {
        let root = Identity {
            pid,
            start: platform::read(pid)?.start,
        };
        Some(Self::for_identity(root))
    }
    /// Bind to an already captured identity, useful with supplied snapshots.
    pub fn for_identity(root: Identity) -> Self {
        Self {
            root,
            owned: HashSet::new(),
            previous: HashMap::new(),
            at: None,
        }
    }
    /// Read a new OS snapshot; optionally include GPU client accounting.
    pub fn sample(&mut self, gpu: bool) -> FamilySample {
        self.update(snapshot(gpu), Instant::now())
    }
    /// Consume a supplied snapshot using its monotonic sampling boundary.
    pub fn update(&mut self, rows: Vec<Reading>, now: Instant) -> FamilySample {
        let mut seen = HashSet::new();
        let rows: Vec<_> = rows.into_iter().filter(|r| seen.insert(r.pid)).collect();
        let interval = self.at.map(|at| now.saturating_duration_since(at));
        let by_pid: HashMap<_, _> = rows.iter().map(|r| (r.pid, r)).collect();
        let root_available = by_pid
            .get(&self.root.pid)
            .is_some_and(|r| r.identity == Some(self.root));
        let retained: HashSet<_> = self
            .owned
            .iter()
            .copied()
            .filter(|id| {
                by_pid
                    .get(&id.pid)
                    .is_some_and(|r| r.identity.is_none_or(|current| current == *id))
            })
            .collect();
        let mut selected = HashSet::new();
        if root_available {
            selected.insert(self.root.pid);
        }
        // Never attach to a new lifetime at the root PID, even after exit.
        for row in &rows {
            if retained.iter().any(|id| id.pid == row.pid) {
                selected.insert(row.pid);
            }
        }
        loop {
            let before = selected.len();
            for row in &rows {
                if selected.contains(&row.parent) && row.pid != self.root.pid {
                    let parent = by_pid[&row.parent];
                    // Unknown identity may be reported as unavailable, but cannot
                    // establish ownership of grandchildren or survive reparenting.
                    if parent
                        .identity
                        .is_some_and(|p| row.identity.is_none_or(|c| c.start >= p.start))
                    {
                        selected.insert(row.pid);
                    }
                }
            }
            if selected.len() == before {
                break;
            }
        }
        let processes: Vec<_> = rows
            .into_iter()
            .filter(|r| selected.contains(&r.pid))
            .map(|reading| {
                let previous = reading.identity.and_then(|id| self.previous.get(&id));
                let percent = |current: Option<u64>, old: Option<u64>| {
                    let elapsed = interval?.as_nanos();
                    if elapsed == 0 {
                        return None;
                    }
                    Some(current?.checked_sub(old?)? as f64 / elapsed as f64 * 100.)
                };
                Sample {
                    cpu_percent: percent(reading.cpu_ns, previous.and_then(|p| p.cpu_ns)),
                    gpu_percent: percent(reading.gpu_ns, previous.and_then(|p| p.gpu_ns)),
                    reading,
                }
            })
            .collect();
        self.previous = processes
            .iter()
            .filter_map(|p| Some((p.reading.identity?, p.reading.clone())))
            .collect();
        self.owned = retained;
        self.owned.extend(self.previous.keys().copied());
        self.at = Some(now);
        FamilySample {
            root_available,
            interval_ms: interval.map(|d| d.as_secs_f64() * 1000.),
            processes,
        }
    }
}

/// Complete sum only. Callers can separately show coverage and partial totals.
/// Empty and partially unreadable families have no aggregate.
pub fn complete_sum(values: impl IntoIterator<Item = Option<u64>>) -> Option<u64> {
    let mut values = values.into_iter().peekable();
    values.peek()?;
    values.try_fold(0_u64, |sum, value| sum.checked_add(value?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn row(pid: i32, parent: i32, start: u64, cpu: u64) -> Reading {
        Reading {
            pid,
            parent,
            identity: Some(Identity { pid, start }),
            name: String::new(),
            cpu_ns: Some(cpu),
            rss_bytes: Some(10),
            footprint_bytes: None,
            gpu_ns: None,
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn native_reader_tracks_self_and_cpu_units() {
        unsafe extern "C" {
            fn clock() -> std::ffi::c_long;
        }
        let pid = std::process::id() as i32;
        let before = platform::read(pid).unwrap();
        let cpu_before = unsafe { clock() };
        assert!(cpu_before >= 0);
        // clock() uses one million ticks/second on macOS and Linux. Burn 50ms
        // of CPU, not wall time, so a busy host does not invalidate the check.
        while unsafe { clock() } - cpu_before < 50_000 {
            for n in 0..1000 {
                std::hint::black_box(n * n);
            }
        }
        let cpu_after = unsafe { clock() };
        let after = platform::read(pid).unwrap();
        let reference = (cpu_after - cpu_before) as f64 * 1000.;
        let ratio = (after.cpu_ns - before.cpu_ns) as f64 / reference;
        // The two API reads bracket the same work, with a little sampling skew.
        assert!((0.8..1.2).contains(&ratio), "CPU unit ratio {ratio}");
        assert_eq!(before.start, after.start);
        assert!(after.rss > 0);
        assert_eq!(
            platform::parent(pid),
            std::os::unix::process::parent_id() as i32
        );
        assert!(platform::pids().contains(&pid));
    }
    #[test]
    fn family_tracks_churn_reparenting_and_reuse_without_other_instances() {
        let mut s = FamilySampler::for_identity(Identity { pid: 10, start: 1 });
        let at = Instant::now();
        let first = s.update(
            vec![
                row(10, 1, 1, 0),
                row(11, 10, 2, 0),
                row(12, 11, 3, 0),
                row(20, 1, 4, 0),
                row(21, 20, 5, 0),
            ],
            at,
        );
        assert_eq!(first.processes.len(), 3);
        assert!(first.processes.iter().all(|p| p.cpu_percent.is_none()));
        let next = s.update(
            vec![
                row(10, 1, 1, 100_000_000),
                row(11, 1, 2, 200_000_000),
                row(12, 20, 8, 0),
                row(13, 10, 9, 0),
            ],
            at + Duration::from_secs(1),
        );
        assert_eq!(
            next.processes
                .iter()
                .map(|p| p.reading.pid)
                .collect::<Vec<_>>(),
            [10, 11, 13]
        );
        assert_eq!(next.processes[0].cpu_percent, Some(10.));
        assert_eq!(next.processes[1].cpu_percent, Some(20.));
        assert_eq!(next.processes[2].cpu_percent, None);
        let reused = s.update(
            vec![row(10, 1, 12, 0), row(14, 10, 13, 0)],
            at + Duration::from_secs(2),
        );
        assert!(!reused.root_available);
        assert!(reused.processes.is_empty());
    }
    #[test]
    fn unreadable_reparented_child_can_recover_without_accepting_reuse() {
        let mut s = FamilySampler::for_identity(Identity { pid: 10, start: 1 });
        let at = Instant::now();
        s.update(vec![row(10, 1, 1, 0), row(11, 10, 2, 0)], at);
        let mut unavailable = row(11, 1, 2, 0);
        unavailable.identity = None;
        unavailable.cpu_ns = None;
        unavailable.rss_bytes = None;
        let missing = s.update(
            vec![row(10, 1, 1, 0), unavailable],
            at + Duration::from_secs(1),
        );
        assert_eq!(missing.processes.len(), 2);
        assert_eq!(
            complete_sum(missing.processes.iter().map(|s| s.reading.rss_bytes)),
            None
        );
        let back = s.update(
            vec![row(10, 1, 1, 0), row(11, 1, 2, 100)],
            at + Duration::from_secs(2),
        );
        assert_eq!(back.processes.len(), 2);
        assert_eq!(back.processes[1].cpu_percent, None);
        let reused = s.update(
            vec![row(10, 1, 1, 0), row(11, 1, 3, 100)],
            at + Duration::from_secs(3),
        );
        assert_eq!(reused.processes.len(), 1);
    }
    #[test]
    fn unreadable_duplicates_and_counter_resets_are_not_zero() {
        let mut s = FamilySampler::for_identity(Identity { pid: 10, start: 1 });
        let at = Instant::now();
        s.update(vec![row(10, 1, 1, 99)], at);
        let mut missing = row(11, 10, 2, 0);
        missing.identity = None;
        missing.cpu_ns = None;
        missing.rss_bytes = None;
        let sample = s.update(
            vec![row(10, 1, 1, 1), row(10, 1, 1, 1), missing],
            at + Duration::from_secs(1),
        );
        assert_eq!(sample.processes.len(), 2);
        assert!(sample.processes.iter().all(|p| p.cpu_percent.is_none()));
        assert_eq!(
            complete_sum(sample.processes.iter().map(|p| p.reading.rss_bytes)),
            None
        );
        assert_eq!(complete_sum([]), None);
        assert_eq!(complete_sum([Some(3), Some(4)]), Some(7));
        assert_eq!(complete_sum([Some(u64::MAX), Some(1)]), None);
    }
}
