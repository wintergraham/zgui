//! Process-family bookkeeping and native sampling cost, without GUI dependencies.
//! `rustc --edition 2024 -O crates/zgx/examples/process_bench.rs -o process-bench`
//! Prints raw repeated samples as JSON lines. Synthetic rows contain no local metadata.
#[allow(dead_code)]
#[path = "../src/process.rs"]
mod process;

use process::{FamilySampler, Identity, Reading};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn instructions() -> u64 {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn thread_selfcounts(kind: i32, buffer: *mut u64, size: usize) -> i32;
        }
        let mut counts = [0; 2];
        // SAFETY: THSC_CPI writes two u64 counters.
        assert_eq!(unsafe { thread_selfcounts(1, counts.as_mut_ptr(), 16) }, 0);
        counts[0]
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

fn rows(total: i32, family: i32, deep: bool) -> Vec<Reading> {
    (1..=total)
        .rev()
        .map(|pid| Reading {
            pid,
            parent: if pid == 1 || pid > family {
                0
            } else if deep {
                pid - 1
            } else {
                1
            },
            identity: Some(Identity {
                pid,
                start: pid as u64,
            }),
            name: "synthetic".into(),
            cpu_ns: Some(1000),
            rss_bytes: Some(4096),
            footprint_bytes: Some(4096),
            gpu_ns: None,
        })
        .collect()
}

fn synthetic(name: &str, total: i32, family: i32, deep: bool) {
    let input = rows(total, family, deep);
    for run in 0..7 {
        let mut sampler = FamilySampler::for_identity(Identity { pid: 1, start: 1 });
        let at = Instant::now();
        assert_eq!(
            sampler.update(input.clone(), at).processes.len(),
            family as usize
        );
        let start = instructions();
        let wall = Instant::now();
        for sample in 1..=100 {
            let result = sampler.update(black_box(input.clone()), at + Duration::from_secs(sample));
            assert!(result.root_available);
            assert_eq!(result.processes.len(), family as usize);
            assert!(result.processes.iter().all(|p| p.cpu_percent == Some(0.)));
            black_box(result);
        }
        let nanos = wall.elapsed().as_nanos();
        let instr = instructions() - start;
        println!(
            "{{\"scenario\":\"{name}\",\"run\":{run},\"samples\":100,\"instr\":{instr},\"ns\":{nanos}}}"
        );
    }
}

fn main() {
    synthetic("1000 processes, 32 family, shallow", 1000, 32, false);
    synthetic("1000 processes, 64 family, reversed chain", 1000, 64, true);
    synthetic("100 processes, 4 family, shallow", 100, 4, false);
    // Bind only to this benchmark's lifetime. Never attach to a running app.
    for gpu in [false, true] {
        let mut sampler = FamilySampler::new(std::process::id() as i32).unwrap();
        for run in 0..7 {
            let start = instructions();
            let wall = Instant::now();
            let sample = sampler.sample(gpu);
            let nanos = wall.elapsed().as_nanos();
            let instr = instructions() - start;
            assert!(sample.root_available);
            assert_eq!(sample.processes.len(), 1);
            println!(
                "{{\"scenario\":\"native self, gpu={gpu}\",\"run\":{run},\"samples\":1,\"instr\":{instr},\"ns\":{nanos}}}"
            );
        }
    }
}
