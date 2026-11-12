//! Platform resource readers extracted from zgx_monitor.
/// Per-process CPU time and physical footprint (what Activity Monitor shows).
#[cfg(target_os = "macos")]
mod platform {
    use std::sync::OnceLock;
    #[repr(C)]
    #[derive(Default)]
    struct RusageInfoV2 {
        uuid: [u8; 16],
        user_time: u64,
        system_time: u64,
        pkg_idle_wkups: u64,
        interrupt_wkups: u64,
        pageins: u64,
        wired_size: u64,
        resident_size: u64,
        phys_footprint: u64,
        proc_start_abstime: u64,
        proc_exit_abstime: u64,
        child_user_time: u64,
        child_system_time: u64,
        child_pkg_idle_wkups: u64,
        child_interrupt_wkups: u64,
        child_pageins: u64,
        child_elapsed_abstime: u64,
        diskio_bytesread: u64,
        diskio_byteswritten: u64,
    }
    /// `proc_bsdshortinfo`, up to the parent's pid being what's read of it.
    #[repr(C)]
    #[derive(Default)]
    struct ShortInfo {
        pid: u32,
        ppid: u32,
        pgid: u32,
        status: u32,
        comm: [u8; 16],
        flags: u32,
        ids: [u32; 6],
        reserved: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }
    // SAFETY: libproc's and Mach's declarations (libproc.h, mach_time.h);
    // `RusageInfoV2` is `rusage_info_v2` and `ShortInfo` is `proc_bsdshortinfo`
    // field for field.
    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut std::ffi::c_void) -> i32;
        fn proc_listallpids(buffer: *mut i32, size: i32) -> i32;
        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut ShortInfo, size: i32) -> i32;
        fn proc_pidpath(pid: i32, buffer: *mut u8, size: u32) -> i32;
        fn proc_name(pid: i32, buffer: *mut u8, size: u32) -> i32;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
    }
    /// Read one of libproc's versioned rusage layouts.
    ///
    /// # Safety
    /// `buffer` must hold the complete layout selected by `flavor`.
    pub unsafe fn rusage(pid: i32, flavor: i32, buffer: *mut std::ffi::c_void) -> i32 {
        unsafe { proc_pid_rusage(pid, flavor, buffer) }
    }
    const RUSAGE_INFO_V2: i32 = 2;
    const PROC_PIDT_SHORTBSDINFO: i32 = 13;

    use std::ffi::{c_int, c_uint, c_void as Void};
    unsafe extern "C" {
        fn sysctl(
            name: *mut c_int,
            count: c_uint,
            old: *mut Void,
            old_size: *mut usize,
            new: *mut Void,
            new_size: usize,
        ) -> c_int;
    }
    /// A process's argv, from `KERN_PROCARGS2`: its count, the executable's path, then the arguments.
    pub fn arguments(pid: i32) -> Option<Vec<String>> {
        const CTL_KERN: c_int = 1;
        const KERN_PROCARGS2: c_int = 49;
        let mut name = [CTL_KERN, KERN_PROCARGS2, pid];
        let mut size = 0usize;
        let call = |buffer: *mut Void, size: &mut usize, name: &mut [c_int; 3]| unsafe {
            sysctl(name.as_mut_ptr(), 3, buffer, size, std::ptr::null_mut(), 0)
        };
        if call(std::ptr::null_mut(), &mut size, &mut name) != 0 {
            return None;
        }
        let mut bytes = vec![0u8; size];
        if call(bytes.as_mut_ptr().cast(), &mut size, &mut name) != 0 {
            return None;
        }
        bytes.truncate(size);
        let count = u32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
        let words = bytes[4..]
            .split(|byte| *byte == 0)
            .filter(|word| !word.is_empty())
            .skip(1);
        Some(
            words
                .take(count)
                .map(|word| String::from_utf8_lossy(word).into_owned())
                .collect(),
        )
    }

    /// Raw cumulative counters for one process.
    pub struct Usage {
        /// User plus system CPU nanoseconds.
        pub cpu_ns: u64,
        /// macOS physical footprint; legacy Linux monitor uses RSS here.
        pub footprint: u64,
        /// Resident bytes.
        pub rss: u64,
        /// Distinguishes a reused pid.
        pub start: u64,
    }
    /// Currently enumerated PIDs; empty on failure or unsupported platforms.
    pub fn pids() -> Vec<i32> {
        // SAFETY: a null buffer asks for the current count.
        let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) }.max(0) as usize;
        // Room for processes started between the two calls.
        let mut pids = vec![0_i32; count + 64];
        let bytes = (pids.len() * size_of::<i32>()) as i32;
        // SAFETY: the buffer holds `bytes` writable bytes.
        let count = unsafe { proc_listallpids(pids.as_mut_ptr(), bytes) }.max(0) as usize;
        pids.truncate(count.min(pids.len()));
        pids.retain(|pid| *pid > 0);
        pids
    }
    /// The parent's pid, or 0 if it can't be read.
    pub fn parent(pid: i32) -> i32 {
        let mut info = ShortInfo::default();
        let size = size_of::<ShortInfo>() as i32;
        // SAFETY: the buffer is a correctly sized, writable proc_bsdshortinfo.
        let read = unsafe { proc_pidinfo(pid, PROC_PIDT_SHORTBSDINFO, 0, &mut info, size) };
        if read == size { info.ppid as i32 } else { 0 }
    }
    /// The executable's file name (proc_name truncates at 32 bytes).
    pub fn name(pid: i32) -> String {
        let mut buffer = [0_u8; 4096];
        // SAFETY: the buffer is writable for its full length (4096 bytes,
        // PROC_PIDPATHINFO_MAXSIZE); the length returned is within it.
        let len = unsafe { proc_pidpath(pid, buffer.as_mut_ptr(), buffer.len() as u32) };
        if len > 0 {
            let path = String::from_utf8_lossy(&buffer[..len as usize]);
            if let Some(name) = path.rsplit('/').next().filter(|n| !n.is_empty()) {
                return name.trim_end_matches('\0').to_owned();
            }
        }
        // SAFETY: as above.
        let len = unsafe { proc_name(pid, buffer.as_mut_ptr(), buffer.len() as u32) };
        if len > 0 {
            return String::from_utf8_lossy(&buffer[..len as usize]).into_owned();
        }
        format!("pid {pid}")
    }
    /// Read resource counters, or return None when inaccessible or exited.
    pub fn read(pid: i32) -> Option<Usage> {
        static TIMEBASE: OnceLock<(u32, u32)> = OnceLock::new();
        let mut info = RusageInfoV2::default();
        // SAFETY: the buffer is a correctly sized, writable rusage_info_v2.
        if unsafe { rusage(pid, RUSAGE_INFO_V2, (&raw mut info).cast()) } != 0 {
            return None;
        }
        // CPU times are in Mach absolute-time units (not ns on Apple silicon).
        let &(numer, denom) = TIMEBASE.get_or_init(|| {
            let mut timebase = Timebase::default();
            // SAFETY: writes two integers into a local.
            unsafe { mach_timebase_info(&mut timebase) };
            (timebase.numer, timebase.denom.max(1))
        });
        let ticks = info.user_time + info.system_time;
        let cpu_ns = (ticks as u128 * numer as u128 / denom as u128) as u64;
        Some(Usage {
            cpu_ns,
            footprint: info.phys_footprint,
            rss: info.resident_size,
            start: info.proc_start_abstime,
        })
    }

    // GPU time per process -----------------------------------------------------
    //
    // The GPU driver keeps per-client accounting in the IORegistry: every
    // `AGXDeviceUserClient` names its creator ("pid 395, WindowServer") and
    // lists `AppUsage` entries with `accumulatedGPUTime` in nanoseconds.
    // `IOAccelerator`'s `PerformanceStatistics` hold the device-wide
    // "Device Utilization %". Both are readable without privileges.
    use std::{
        collections::HashMap,
        ffi::{CStr, c_char, c_void},
    };
    type CfType = *const c_void;
    // SAFETY: IOKit's and CoreFoundation's declarations (IOKitLib.h,
    // CFString.h, CFArray.h, CFDictionary.h, CFNumber.h): io_object_t and
    // mach ports are u32, kern_return_t i32, CFIndex isize, CFTypeID usize,
    // CFStringEncoding u32, Boolean a byte that is 0 or 1.
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOServiceMatching(name: *const c_char) -> *mut c_void;
        fn IOServiceGetMatchingServices(main: u32, matching: *mut c_void, it: *mut u32) -> i32;
        fn IOIteratorNext(it: u32) -> u32;
        fn IOObjectRelease(object: u32) -> i32;
        fn IORegistryEntryGetChildIterator(entry: u32, plane: *const c_char, it: *mut u32) -> i32;
        fn IORegistryEntryCreateCFProperty(
            entry: u32,
            key: CfType,
            allocator: CfType,
            options: u32,
        ) -> CfType;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: CfType,
            text: *const c_char,
            encoding: u32,
        ) -> CfType;
        fn CFStringGetCString(
            text: CfType,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> bool;
        fn CFGetTypeID(object: CfType) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFDictionaryGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
        fn CFArrayGetCount(array: CfType) -> isize;
        fn CFArrayGetValueAtIndex(array: CfType, index: isize) -> CfType;
        fn CFDictionaryGetValue(dictionary: CfType, key: CfType) -> CfType;
        fn CFNumberGetValue(number: CfType, kind: isize, value: *mut c_void) -> bool;
        fn CFRelease(object: CfType);
    }
    const UTF8: u32 = 0x0800_0100;
    const SINT64: isize = 4;

    /// An owned CFString key.
    struct Key(CfType);
    impl Key {
        fn new(text: &CStr) -> Self {
            // SAFETY: a NUL-terminated UTF-8 string; released in Drop.
            Self(unsafe { CFStringCreateWithCString(std::ptr::null(), text.as_ptr(), UTF8) })
        }
    }
    impl Drop for Key {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: created above and not shared.
                unsafe { CFRelease(self.0) };
            }
        }
    }

    /// Calls `visit` with each registry entry of `class`, then releases it.
    fn each_service(class: &CStr, mut visit: impl FnMut(u32)) {
        let mut iterator = 0_u32;
        // SAFETY: `IOServiceGetMatchingServices` consumes the matching
        // dictionary; the iterator and entries are released here.
        unsafe {
            let matching = IOServiceMatching(class.as_ptr());
            if matching.is_null() || IOServiceGetMatchingServices(0, matching, &mut iterator) != 0 {
                return;
            }
            loop {
                let entry = IOIteratorNext(iterator);
                if entry == 0 {
                    break;
                }
                visit(entry);
                IOObjectRelease(entry);
            }
            IOObjectRelease(iterator);
        }
    }

    /// Calls `visit` with each child of `entry` in the service plane. User
    /// clients are not registered services, so matching can't find them;
    /// they hang off their accelerator like `ioreg` shows.
    fn each_child(entry: u32, mut visit: impl FnMut(u32)) {
        let mut iterator = 0_u32;
        // SAFETY: a valid entry; the iterator and children are released.
        unsafe {
            if IORegistryEntryGetChildIterator(entry, c"IOService".as_ptr(), &mut iterator) != 0 {
                return;
            }
            loop {
                let child = IOIteratorNext(iterator);
                if child == 0 {
                    break;
                }
                visit(child);
                IOObjectRelease(child);
            }
            IOObjectRelease(iterator);
        }
    }

    /// A copied property of `entry`, released when dropped.
    struct Property(CfType);
    impl Property {
        fn of(entry: u32, key: &Key) -> Option<Self> {
            // SAFETY: a valid entry and key; the result is owned (Create rule).
            let value =
                unsafe { IORegistryEntryCreateCFProperty(entry, key.0, std::ptr::null(), 0) };
            // Lazily: an eager `Self(value)` would be dropped, releasing
            // null, when the entry lacks the property.
            (!value.is_null()).then(|| Self(value))
        }
    }
    impl Drop for Property {
        fn drop(&mut self) {
            // SAFETY: owned since `of`.
            unsafe { CFRelease(self.0) };
        }
    }

    /// # Safety
    ///
    /// `object` is null or a live CF object.
    unsafe fn is(object: CfType, type_id: usize) -> bool {
        // SAFETY: non-null, so a live CF object (the caller's promise).
        !object.is_null() && unsafe { CFGetTypeID(object) } == type_id
    }

    /// # Safety
    ///
    /// As `is`'s.
    unsafe fn number(object: CfType) -> Option<i64> {
        // SAFETY: the caller's object, read as a CFNumber only once it is
        // one, into an i64 (kCFNumberSInt64Type).
        unsafe {
            if !is(object, CFNumberGetTypeID()) {
                return None;
            }
            let mut value = 0_i64;
            CFNumberGetValue(object, SINT64, (&raw mut value).cast()).then_some(value)
        }
    }

    /// # Safety
    ///
    /// As `is`'s.
    unsafe fn string(object: CfType) -> Option<String> {
        // SAFETY: the caller's object, read as a CFString only once it is
        // one; CoreFoundation writes at most the buffer's length, NUL
        // included, and only a `true` (it fit) is read back.
        unsafe {
            if !is(object, CFStringGetTypeID()) {
                return None;
            }
            let mut buffer = [0 as c_char; 256];
            CFStringGetCString(object, buffer.as_mut_ptr(), buffer.len() as isize, UTF8).then(
                || {
                    CStr::from_ptr(buffer.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                },
            )
        }
    }

    /// "pid 395, WindowServer" → 395.
    pub fn creator_pid(creator: &str) -> Option<i32> {
        creator
            .strip_prefix("pid ")?
            .split(',')
            .next()?
            .trim()
            .parse()
            .ok()
    }

    /// Accumulated GPU nanoseconds per process with an open GPU client.
    pub fn gpu_times() -> HashMap<i32, u64> {
        let (creator_key, usage_key, time_key) = (
            Key::new(c"IOUserClientCreator"),
            Key::new(c"AppUsage"),
            Key::new(c"accumulatedGPUTime"),
        );
        let mut times = HashMap::new();
        let mut read = |entry: u32| {
            // SAFETY: a property is a live CF object while held.
            let Some(pid) = Property::of(entry, &creator_key)
                .and_then(|creator| unsafe { string(creator.0) })
                .and_then(|creator| creator_pid(&creator))
            else {
                return;
            };
            let Some(usage) = Property::of(entry, &usage_key) else {
                return;
            };
            // SAFETY: `usage` (live while held) is read as an array only once
            // it is one, at indices within its count; its items and their
            // values are Get-rule references it keeps alive meanwhile.
            let total: u64 = unsafe {
                if !is(usage.0, CFArrayGetTypeID()) {
                    return;
                }
                (0..CFArrayGetCount(usage.0))
                    .map(|i| CFArrayGetValueAtIndex(usage.0, i))
                    .filter(|item| is(*item, CFDictionaryGetTypeID()))
                    .filter_map(|item| number(CFDictionaryGetValue(item, time_key.0)))
                    .map(|ns| ns.max(0) as u64)
                    .sum()
            };
            *times.entry(pid).or_insert(0) += total;
        };
        each_service(c"IOAccelerator", |accelerator| {
            each_child(accelerator, &mut read)
        });
        times
    }

    /// The GPU's own "Device Utilization %", if the driver reports it.
    pub fn device_gpu() -> Option<f32> {
        let (stats_key, utilization_key) = (
            Key::new(c"PerformanceStatistics"),
            Key::new(c"Device Utilization %"),
        );
        let mut found = None;
        each_service(c"IOAccelerator", |entry| {
            if found.is_some() {
                return;
            }
            if let Some(stats) = Property::of(entry, &stats_key)
                // SAFETY: `stats` is a live CF object while held.
                && unsafe { is(stats.0, CFDictionaryGetTypeID()) }
            {
                // SAFETY: a dictionary, checked above; the value it holds is a
                // Get-rule reference, live while `stats` is.
                found = unsafe { number(CFDictionaryGetValue(stats.0, utilization_key.0)) }
                    .map(|percent| percent as f32);
            }
        });
        found
    }
}

#[cfg(target_os = "linux")]
mod platform {
    /// Read locally for role classification; do not publish raw arguments.
    pub fn arguments(pid: i32) -> Option<Vec<String>> {
        let bytes = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        Some(
            bytes
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect(),
        )
    }

    /// No per-process GPU accounting here yet.
    pub fn gpu_times() -> std::collections::HashMap<i32, u64> {
        std::collections::HashMap::new()
    }
    /// Device GPU utilization is unavailable on this platform.
    pub fn device_gpu() -> Option<f32> {
        None
    }
    /// Raw cumulative counters for one process.
    pub struct Usage {
        /// User plus system CPU nanoseconds.
        pub cpu_ns: u64,
        /// macOS physical footprint; legacy Linux monitor uses RSS here.
        pub footprint: u64,
        /// Resident bytes.
        pub rss: u64,
        /// OS process start token.
        pub start: u64,
    }
    /// Currently enumerated PIDs; empty on failure or unsupported platforms.
    pub fn pids() -> Vec<i32> {
        std::fs::read_dir("/proc")
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Process comm name from procfs.
    pub fn name(pid: i32) -> String {
        std::fs::read_to_string(format!("/proc/{pid}/comm"))
            .map(|name| name.trim_end().to_owned())
            .unwrap_or_else(|_| format!("pid {pid}"))
    }
    /// The parent's pid, or 0 if it can't be read.
    pub fn parent(pid: i32) -> i32 {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| {
                stat.rsplit(')')
                    .next()?
                    .split_whitespace()
                    .nth(1)?
                    .parse()
                    .ok()
            })
            .unwrap_or(0)
    }
    /// Linux CPU ticks and pages use this kernel's units.
    /// Read resource counters, or return None when inaccessible or exited.
    pub fn read(pid: i32) -> Option<Usage> {
        unsafe extern "C" {
            fn sysconf(name: i32) -> std::ffi::c_long;
        }
        // Linux unistd.h: _SC_CLK_TCK = 2, _SC_PAGESIZE = 30.
        let (clock_ticks, page_size) = unsafe { (sysconf(2), sysconf(30)) };
        if clock_ticks <= 0 || page_size <= 0 {
            return None;
        }
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let fields: Vec<&str> = stat.rsplit(')').next()?.split_whitespace().collect();
        let ticks: u64 =
            fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
        let pages: u64 = fields.get(21)?.parse().ok()?;
        Some(Usage {
            cpu_ns: (ticks as u128 * 1_000_000_000 / clock_ticks as u128) as u64,
            footprint: pages * page_size as u64,
            rss: pages * page_size as u64,
            start: fields.get(19)?.parse().ok()?,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    /// Command-line reading is unsupported.
    pub fn arguments(_: i32) -> Option<Vec<String>> {
        None
    }

    /// Raw cumulative counters for one process.
    pub struct Usage {
        /// User plus system CPU nanoseconds.
        pub cpu_ns: u64,
        /// macOS physical footprint; legacy Linux monitor uses RSS here.
        pub footprint: u64,
        /// Resident bytes.
        pub rss: u64,
        /// OS process start token.
        pub start: u64,
    }
    /// Currently enumerated PIDs; empty on failure or unsupported platforms.
    pub fn pids() -> Vec<i32> {
        Vec::new()
    }
    /// Parent reading is unsupported.
    pub fn parent(_: i32) -> i32 {
        0
    }
    /// A placeholder for an unreadable process name.
    pub fn name(pid: i32) -> String {
        format!("pid {pid}")
    }
    /// Resource reading is unsupported.
    pub fn read(_: i32) -> Option<Usage> {
        None
    }
    /// GPU accounting is unsupported.
    pub fn gpu_times() -> std::collections::HashMap<i32, u64> {
        Default::default()
    }
    /// Device GPU utilization is unavailable on this platform.
    pub fn device_gpu() -> Option<f32> {
        None
    }
}
pub use platform::*;
