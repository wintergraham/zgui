//! Windows counters use process handles and creation FILETIME as the start token.
//! No debug privilege, command-line reads or GPU estimates.
use std::{collections::HashMap, ffi::c_void};

type Handle = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}
impl FileTime {
    fn ticks(&self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    executable: [u16; 260],
}

#[repr(C)]
#[derive(Default)]
struct MemoryCounters {
    size: u32,
    faults: u32,
    peak_working_set: usize,
    working_set: usize,
    peak_paged_pool: usize,
    paged_pool: usize,
    peak_nonpaged_pool: usize,
    nonpaged_pool: usize,
    pagefile: usize,
    peak_pagefile: usize,
}

// SAFETY: kernel32's PROCESSENTRY32W, FILETIME and PROCESS_MEMORY_COUNTERS
// layouts and system calling convention, with handles closed by OwnedHandle.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetProcessTimes(
        process: Handle,
        created: *mut FileTime,
        exited: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn K32GetProcessMemoryInfo(process: Handle, counters: *mut MemoryCounters, size: u32) -> i32;
}

struct OwnedHandle(Handle);
impl OwnedHandle {
    fn new(handle: Handle) -> Option<Self> {
        (!handle.is_null() && handle as isize != -1).then(|| Self(handle))
    }
}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this handle was returned by kernel32 and is owned here.
        unsafe { CloseHandle(self.0) };
    }
}

/// Current PID -> parent PID and executable basename, from one Toolhelp snapshot.
pub fn metadata() -> HashMap<i32, (i32, String)> {
    // SAFETY: TH32CS_SNAPPROCESS has no pointer parameters.
    let Some(snapshot) = OwnedHandle::new(unsafe { CreateToolhelp32Snapshot(0x2, 0) }) else {
        return HashMap::new();
    };
    // SAFETY: zero is valid for this integer-only structure.
    let mut entry: ProcessEntry = unsafe { std::mem::zeroed() };
    entry.size = size_of::<ProcessEntry>() as u32;
    let mut rows = HashMap::new();
    // SAFETY: a live snapshot and a correctly sized writable PROCESSENTRY32W.
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while more {
        if let Ok(pid) = i32::try_from(entry.pid)
            && pid > 0
        {
            let length = entry.executable.iter().position(|c| *c == 0).unwrap_or(260);
            rows.insert(
                pid,
                (
                    i32::try_from(entry.parent).unwrap_or(0),
                    String::from_utf16_lossy(&entry.executable[..length]),
                ),
            );
        }
        // SAFETY: same snapshot and writable entry as above.
        more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    rows
}

/// Currently enumerated process IDs; empty when enumeration fails.
pub fn pids() -> Vec<i32> {
    metadata().into_keys().collect()
}
/// Parent PID, or zero when it cannot be read.
pub fn parent(pid: i32) -> i32 {
    metadata().get(&pid).map_or(0, |row| row.0)
}
/// Executable basename; command-line arguments are never read.
pub fn name(pid: i32) -> String {
    metadata()
        .remove(&pid)
        .map_or_else(|| format!("pid {pid}"), |row| row.1)
}
/// Windows command-line classification is unavailable in this reader.
pub fn arguments(_: i32) -> Option<Vec<String>> {
    None
}

/// Cumulative counters for one live process handle.
pub struct Usage {
    /// User plus kernel CPU nanoseconds, converted from 100 ns FILETIME ticks.
    pub cpu_ns: u64,
    /// Reserved for the macOS footprint reader; Windows reports no footprint.
    pub footprint: u64,
    /// Working-set bytes, including shared pages.
    pub rss: u64,
    /// Creation FILETIME in 100 ns ticks, used only as an identity token.
    pub start: u64,
}

/// Read the same process handle's times and working set; inaccessible is None.
pub fn read(pid: i32) -> Option<Usage> {
    let pid = u32::try_from(pid).ok().filter(|pid| *pid > 0)?;
    // SAFETY: PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, non-inheritable.
    let process = OwnedHandle::new(unsafe { OpenProcess(0x400 | 0x10, 0, pid) })?;
    let (mut created, mut exited, mut kernel, mut user) = (
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
        FileTime::default(),
    );
    // SAFETY: a live handle and four writable FILETIME buffers.
    if unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
        || exited.ticks() != 0
    {
        return None;
    }
    let mut memory = MemoryCounters::default();
    let size = size_of::<MemoryCounters>() as u32;
    memory.size = size;
    // SAFETY: a live handle and correctly sized writable PROCESS_MEMORY_COUNTERS.
    if unsafe { K32GetProcessMemoryInfo(process.0, &mut memory, size) } == 0 {
        return None;
    }
    Some(Usage {
        cpu_ns: kernel.ticks().checked_add(user.ticks())?.checked_mul(100)?,
        footprint: 0,
        rss: memory.working_set as u64,
        start: created.ticks(),
    })
}

/// Per-process GPU accounting is unavailable on Windows.
pub fn gpu_times() -> HashMap<i32, u64> {
    HashMap::new()
}
/// Device GPU utilization is unavailable on Windows.
pub fn device_gpu() -> Option<f32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_reader_keeps_handle_identity_and_working_set_units() {
        let pid = std::process::id() as i32;
        let before = read(pid).unwrap();
        let after = read(pid).unwrap();
        assert!(before.start > 0);
        assert_eq!(before.start, after.start);
        assert!(after.cpu_ns >= before.cpu_ns);
        assert!(after.rss > 0);
        assert!(metadata().contains_key(&pid));
        assert!(parent(pid) > 0);
        assert!(!name(pid).is_empty());
        assert!(read(-1).is_none());
        assert!(gpu_times().is_empty());
    }
}
