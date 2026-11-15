//! A window's snapshot on macOS (`Platform::window_snapshot`): the window
//! as the user sees it, native views (web pages, video) included, which
//! zgx's own frame doesn't hold.
//!
//! ScreenCaptureKit's screenshot of the app's own window: its
//! `getCurrentProcessShareableContent` (macOS 14.4) lists the windows a
//! process may capture without the user's consent, so no Screen Recording
//! permission is asked for, and a desktop-independent window filter takes
//! the window alone, as the window server composites it, whatever covers
//! it. On a MacBook's full window a capture takes about 40 ms (the first in
//! a process about 130), the window's filter is kept for the next one, and
//! the pixels are read off the main thread: the completion converts the
//! frame's premultiplied BGRA into straight RGBA, cut to the content view.
//! Older systems, and a capture that fails, give none.
use objc2::{
    msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject, Bool},
    sel,
};
use objc2_app_kit::NSView;
use objc2_foundation::NSArray;
use std::{
    ffi::c_void,
    sync::{Arc, Mutex},
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use zgx::{image::ImageData, platform::Pending};

/// Snapshots past this many pixels are taken smaller (the window server
/// scales them): a 5K display's full window would be 59 MB, near a
/// renderer's 64 MiB image budget. A MacBook's full window is under it.
const MAX_PIXELS: f64 = 8_400_000.;
/// `kCVPixelFormatType_32BGRA`.
const BGRA: u32 = u32::from_be_bytes(*b"BGRA");

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    fn CMSampleBufferGetImageBuffer(buffer: *mut c_void) -> *mut c_void;
}
#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVPixelBufferLockBaseAddress(buffer: *mut c_void, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(buffer: *mut c_void, flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddress(buffer: *mut c_void) -> *const u8;
    fn CVPixelBufferGetBytesPerRow(buffer: *mut c_void) -> usize;
    fn CVPixelBufferGetWidth(buffer: *mut c_void) -> usize;
    fn CVPixelBufferGetHeight(buffer: *mut c_void) -> usize;
    fn CVPixelBufferGetPixelFormatType(buffer: *mut c_void) -> u32;
}
unsafe extern "C" {
    fn dlopen(path: *const std::ffi::c_char, mode: i32) -> *mut c_void;
}

/// The last window's filter, kept for the next capture of it at that size.
struct Kept {
    number: isize,
    size: (i64, i64),
    filter: Retained<AnyObject>,
}
// SAFETY: an `SCContentFilter`, immutable once made; ScreenCaptureKit
// takes it on any queue.
unsafe impl Send for Kept {}
static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

/// What a capture needs from AppKit, read on the main thread.
#[derive(Clone, Copy)]
struct Shape {
    number: isize,
    /// The window's frame size, in points.
    frame: (f64, f64),
    /// The content view's box in the frame, in points from its top left.
    content: (f64, f64, f64, f64),
    /// Device pixels per point.
    scale: f64,
}

fn shape(window: &Window) -> Option<Shape> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: winit's AppKit handle is its live NSView, read on the main
    // thread, where the platform's calls come.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let ns_window = view.window()?;
    let frame = ns_window.frame();
    let shown = view.convertRect_toView(view.bounds(), None);
    Some(Shape {
        number: ns_window.windowNumber(),
        frame: (frame.size.width, frame.size.height),
        // AppKit's window space runs up from the bottom left.
        content: (
            shown.origin.x,
            frame.size.height - shown.origin.y - shown.size.height,
            shown.size.width,
            shown.size.height,
        ),
        scale: ns_window.backingScaleFactor(),
    })
}

fn class(name: &std::ffi::CStr) -> Option<&'static AnyClass> {
    static LOADED: std::sync::Once = std::sync::Once::new();
    LOADED.call_once(|| {
        // Loaded at first use rather than linked: it is missing before
        // macOS 12.3, where the app must still start.
        // SAFETY: a system framework's path; RTLD_LAZY.
        unsafe {
            dlopen(
                c"/System/Library/Frameworks/ScreenCaptureKit.framework/ScreenCaptureKit".as_ptr(),
                1,
            );
        }
    });
    AnyClass::get(name)
}

/// The window's snapshot, later (see the module docs).
pub(crate) fn take(window: &Window) -> Pending<Option<ImageData>> {
    let (tell, mut heard) = zgx::task::latest::<ImageData>();
    if let Some(shape) = shape(window) {
        let kept = KEPT.lock().unwrap().as_ref().and_then(|kept| {
            (kept.number == shape.number && kept.size == rounded(shape.frame))
                .then(|| kept.filter.clone())
        });
        let tell = Arc::new(Mutex::new(Some(tell)));
        match kept {
            Some(filter) => capture(&filter, shape, tell),
            None => find(shape, tell),
        }
    }
    // Every way a capture fails drops its sender: `recv` then gives none.
    Box::pin(async move { heard.recv().await })
}

type Tell = Arc<Mutex<Option<zgx::task::LatestSender<ImageData>>>>;

fn rounded((w, h): (f64, f64)) -> (i64, i64) {
    (w.round() as i64, h.round() as i64)
}

/// Find the window among those the process may capture, keep its filter,
/// and capture it.
fn find(shape: Shape, tell: Tell) {
    let Some(content) = class(c"SCShareableContent") else {
        return;
    };
    let ask = sel!(getCurrentProcessShareableContentWithCompletionHandler:);
    // SAFETY: a class method's `respondsToSelector:`.
    let can: Bool = unsafe { msg_send![content, respondsToSelector: ask] };
    if !can.as_bool() {
        return;
    }
    let block = block2::RcBlock::new(move |shareable: *mut AnyObject, _error: *mut AnyObject| {
        let tell = tell.clone();
        // SAFETY: the completion's content, alive for the call.
        let Some(shareable) = (unsafe { shareable.as_ref() }) else {
            return;
        };
        // SAFETY: `SCShareableContent.windows`, an array of `SCWindow`.
        let windows: Retained<NSArray<AnyObject>> = unsafe { msg_send![shareable, windows] };
        let ours = (0..windows.count())
            .map(|i| windows.objectAtIndex(i))
            .find(|window| {
                // SAFETY: `SCWindow.windowID`, a `CGWindowID`.
                let id: u32 = unsafe { msg_send![&**window, windowID] };
                id as isize == shape.number
            });
        let (Some(window), Some(filter_class)) = (ours, class(c"SCContentFilter")) else {
            return;
        };
        // SAFETY: `-[SCContentFilter initWithDesktopIndependentWindow:]`.
        let filter: Option<Retained<AnyObject>> = unsafe {
            let made: Allocated<AnyObject> = msg_send![filter_class, alloc];
            msg_send![made, initWithDesktopIndependentWindow: &*window]
        };
        let Some(filter) = filter else {
            return;
        };
        *KEPT.lock().unwrap() = Some(Kept {
            number: shape.number,
            size: rounded(shape.frame),
            filter: filter.clone(),
        });
        capture(&filter, shape, tell);
    });
    // SAFETY: the class method, with a completion block it copies.
    unsafe {
        let _: () =
            msg_send![content, getCurrentProcessShareableContentWithCompletionHandler: &*block];
    }
}

/// Capture through `filter` at the window's size in device pixels (no more
/// than `MAX_PIXELS`), and send what it gives.
fn capture(filter: &AnyObject, shape: Shape, tell: Tell) {
    let (Some(manager), Some(config_class)) = (
        class(c"SCScreenshotManager"),
        class(c"SCStreamConfiguration"),
    ) else {
        return;
    };
    let (w, h) = shape.frame;
    let fit = (MAX_PIXELS / (w * h * shape.scale * shape.scale))
        .sqrt()
        .min(1.);
    let scale = shape.scale * fit;
    let (pw, ph) = ((w * scale).round() as usize, (h * scale).round() as usize);
    if pw == 0 || ph == 0 {
        return;
    }
    // SAFETY: a new `SCStreamConfiguration` and its documented setters.
    let config: Retained<AnyObject> = unsafe { msg_send![config_class, new] };
    unsafe {
        let _: () = msg_send![&*config, setWidth: pw];
        let _: () = msg_send![&*config, setHeight: ph];
        let _: () = msg_send![&*config, setPixelFormat: BGRA];
        let _: () = msg_send![&*config, setShowsCursor: Bool::NO];
        let shadows = sel!(setIgnoreShadowsSingleWindow:);
        let can: Bool = msg_send![&*config, respondsToSelector: shadows];
        if can.as_bool() {
            let _: () = msg_send![&*config, setIgnoreShadowsSingleWindow: Bool::YES];
        }
    }
    let (cx, cy, cw, ch) = shape.content;
    let crop = (
        (cx * scale).round().max(0.) as usize,
        (cy * scale).round().max(0.) as usize,
        (cw * scale).round() as usize,
        (ch * scale).round() as usize,
    );
    let block = block2::RcBlock::new(move |sample: *mut c_void, _error: *mut AnyObject| {
        let Some(tell) = tell.lock().unwrap().take() else {
            return;
        };
        if sample.is_null() {
            return;
        }
        // SAFETY: the completion's sample buffer, alive for the call; its
        // image buffer is a CVPixelBuffer it owns.
        let image = unsafe { read(CMSampleBufferGetImageBuffer(sample), crop) };
        if let Some(image) = image {
            tell.send(image);
        }
    });
    // SAFETY: the class method, with a completion block it copies.
    unsafe {
        let _: () = msg_send![
            manager,
            captureSampleBufferWithFilter: filter,
            configuration: &*config,
            completionHandler: &*block
        ];
    }
}

/// `buffer`'s pixels in `crop` (x, y, width, height; held to the buffer) as
/// straight RGBA.
///
/// # Safety
/// `buffer` is null or a live CVPixelBuffer.
unsafe fn read(buffer: *mut c_void, crop: (usize, usize, usize, usize)) -> Option<ImageData> {
    if buffer.is_null() || unsafe { CVPixelBufferGetPixelFormatType(buffer) } != BGRA {
        return None;
    }
    const READ_ONLY: u64 = 1;
    if unsafe { CVPixelBufferLockBaseAddress(buffer, READ_ONLY) } != 0 {
        return None;
    }
    let (bw, bh) = unsafe {
        (
            CVPixelBufferGetWidth(buffer),
            CVPixelBufferGetHeight(buffer),
        )
    };
    let stride = unsafe { CVPixelBufferGetBytesPerRow(buffer) };
    let base = unsafe { CVPixelBufferGetBaseAddress(buffer) };
    let (x0, y0) = (crop.0.min(bw), crop.1.min(bh));
    let (w, h) = (crop.2.min(bw - x0), crop.3.min(bh - y0));
    let image = (!base.is_null() && w > 0 && h > 0).then(|| {
        let mut rgba = vec![0u8; w * h * 4];
        for (y, out) in rgba.chunks_exact_mut(w * 4).enumerate() {
            // SAFETY: row `y0 + y` of the locked buffer, from column `x0`,
            // `w` pixels (inside it, as clamped above).
            let row =
                unsafe { std::slice::from_raw_parts(base.add((y0 + y) * stride + x0 * 4), w * 4) };
            for (from, to) in row.chunks_exact(4).zip(out.chunks_exact_mut(4)) {
                let a = from[3];
                to.copy_from_slice(&match a {
                    255 => [from[2], from[1], from[0], 255],
                    0 => [0; 4],
                    a => {
                        let un = |c: u8| {
                            ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
                        };
                        [un(from[2]), un(from[1]), un(from[0]), a]
                    }
                });
            }
        }
        rgba
    });
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, READ_ONLY) };
    ImageData::new(w as u32, h as u32, image?).ok()
}
