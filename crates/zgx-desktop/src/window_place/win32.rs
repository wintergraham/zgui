//! Where a new window opens on Windows (see the module's notes): a saved
//! frame put back while it is hidden, kept as it changes, and a point of its
//! content put under the pointer.
use super::Placed;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL,
            MONITORINFO, MonitorFromRect,
        },
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS,
            REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegGetValueW, RegSetValueExW,
        },
        UI::{
            HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GWL_STYLE, GetCursorPos, GetWindowLongW, GetWindowPlacement, GetWindowRect,
                SW_HIDE, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SWP_NOACTIVATE, SWP_NOSIZE,
                SWP_NOZORDER, SetWindowPlacement, SetWindowPos, WINDOWPLACEMENT, WM_DESTROY,
                WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NCDESTROY, WM_WINDOWPOSCHANGED,
                WPF_RESTORETOMAXIMIZED, WS_CAPTION,
            },
        },
    },
    core::{HSTRING, PCWSTR},
};

const SUBCLASS_ID: usize = 0x7a67_7066; // "zgpf"
/// A title bar's height, in pixels at 100%: what of a saved frame a monitor
/// must still show for it to reopen there.
const TITLE: i32 = 32;

/// A saved frame: the window's restored frame (workspace coordinates, as
/// `WINDOWPLACEMENT` keeps them: the screen's pixels), whether it was
/// maximized, and the DPI it was saved at (0 for a frame saved before it
/// was kept).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Frame {
    rect: (i32, i32, i32, i32),
    maximized: bool,
    dpi: u32,
}
impl Frame {
    fn encode(self) -> String {
        let (left, top, right, bottom) = self.rect;
        let maximized = u8::from(self.maximized);
        format!("{left} {top} {right} {bottom} {maximized} {}", self.dpi)
    }
    fn decode(text: &str) -> Option<Self> {
        let mut numbers = text.split_whitespace().map(str::parse::<i32>);
        let mut next = || numbers.next()?.ok();
        let rect = (next()?, next()?, next()?, next()?);
        let maximized = next()? != 0;
        let dpi = next().and_then(|dpi| u32::try_from(dpi).ok()).unwrap_or(0);
        (rect.2 > rect.0 && rect.3 > rect.1).then_some(Self {
            rect,
            maximized,
            dpi,
        })
    }
    /// Its title bar, which a monitor must still show.
    fn title(self) -> RECT {
        let (left, top, right, _) = self.rect;
        RECT {
            left,
            top,
            right,
            bottom: top + TITLE,
        }
    }
}

fn hwnd(window: &winit::window::Window) -> Option<HWND> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
        _ => None,
    }
}

/// The registry key frames are kept under: one per executable.
fn key_path() -> HSTRING {
    let app = std::env::current_exe()
        .ok()
        .and_then(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "zgx".into());
    HSTRING::from(format!("Software\\zgx\\{app}\\Frames"))
}
fn key(access: REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    // SAFETY: opens (or makes) a key of the user's, into a local.
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &key_path(),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            access,
            None,
            &mut key,
            None,
        )
    }
    .ok()
    .ok()?;
    Some(key)
}
fn load(name: &str) -> Option<Frame> {
    let key = key(KEY_READ)?;
    let mut buffer = [0u16; 128];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    // SAFETY: reads a string value into a local buffer, given its size in
    // bytes; `RRF_RT_REG_SZ` has it NUL-terminated, or fails if it does not fit.
    let read = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    // SAFETY: our key.
    let _ = unsafe { RegCloseKey(key) };
    read.ok().ok()?;
    let length = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    Frame::decode(&String::from_utf16_lossy(&buffer[..length]))
}
fn store(name: &str, frame: Frame) {
    let Some(key) = key(KEY_WRITE) else {
        return;
    };
    let text: Vec<u8> = frame
        .encode()
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    // SAFETY: writes a string value of our key, then closes it.
    unsafe {
        let _ = RegSetValueExW(key, &HSTRING::from(name), None, REG_SZ, Some(&text));
        let _ = RegCloseKey(key);
    }
}
/// The window's frame as it is now: its restored frame, minimized or
/// maximized; none while it is fullscreen, whose frame is the monitor's (the
/// frame saved before stands).
fn frame(hwnd: HWND) -> Option<Frame> {
    // Fullscreen (borderless or exclusive), winit takes the overlapped
    // window's styles away, which its windows otherwise always have,
    // decorated or not; minimized from fullscreen, it keeps them off.
    // SAFETY: reads our own window's style.
    if unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_CAPTION.0 == 0 {
        return None;
    }
    let mut placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // SAFETY: reads our own window's placement into a local.
    unsafe { GetWindowPlacement(hwnd, &mut placement) }.ok()?;
    let show = placement.showCmd as i32;
    let maximized = show == SW_SHOWMAXIMIZED.0
        || (show == SW_SHOWMINIMIZED.0 && placement.flags.contains(WPF_RESTORETOMAXIMIZED));
    let rect = placement.rcNormalPosition;
    Some(Frame {
        rect: (rect.left, rect.top, rect.right, rect.bottom),
        maximized,
        // SAFETY: our own window's DPI.
        dpi: unsafe { GetDpiForWindow(hwnd) },
    })
}
/// Where `frame` reopens, given the work area (`area`, workspace
/// coordinates as the frame's) and DPI of the monitor showing the most of
/// it or nearest it, and whether a monitor still shows its title bar
/// (`shown`): as it was saved, where it is shown and fits; shown but bigger
/// than the work area (a monitor's resolution lowered), shrunk to fit and
/// kept inside; shown nowhere (its monitor disconnected, or moved), as big
/// as it looked where it was saved, at this monitor's DPI, and fully inside
/// the work area.
fn fit(frame: Frame, area: RECT, dpi: u32, shown: bool) -> (i32, i32, i32, i32) {
    let (left, top, right, bottom) = frame.rect;
    let (mut width, mut height) = (right - left, bottom - top);
    let (room_width, room_height) = (area.right - area.left, area.bottom - area.top);
    if shown && width <= room_width && height <= room_height {
        return frame.rect;
    }
    if !shown && frame.dpi > 0 && dpi > 0 && frame.dpi != dpi {
        let scale = |pixels: i32| (i64::from(pixels) * i64::from(dpi) / i64::from(frame.dpi)) as i32;
        (width, height) = (scale(width), scale(height));
    }
    let (width, height) = (width.min(room_width).max(0), height.min(room_height).max(0));
    let left = left.clamp(area.left, (area.right - width).max(area.left));
    let top = top.clamp(area.top, (area.bottom - height).max(area.top));
    (left, top, left + width, top + height)
}
/// Put the hidden window back at `frame`, fitted to a monitor (`fit`):
/// whether it went.
fn restore(hwnd: HWND, frame: Frame) -> bool {
    let (left, top, right, bottom) = frame.rect;
    let saved = RECT {
        left,
        top,
        right,
        bottom,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let (mut dpi, mut dpi_y) = (0, 0);
    // SAFETY: asks which monitors show rectangles, and reads one's geometry
    // and DPI into locals.
    let shown = unsafe {
        let shown = !MonitorFromRect(&frame.title(), MONITOR_DEFAULTTONULL).is_invalid();
        let monitor = MonitorFromRect(&saved, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
        shown
    };
    // The work area in workspace coordinates: those run from the work
    // area's top-left, put at the monitor's.
    let (work, screen) = (info.rcWork, info.rcMonitor);
    let area = RECT {
        left: screen.left,
        top: screen.top,
        right: screen.left + (work.right - work.left),
        bottom: screen.top + (work.bottom - work.top),
    };
    let (left, top, right, bottom) = fit(frame, area, dpi, shown);
    let placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        // Still hidden: it shows (maximized, if it was) as it would.
        showCmd: SW_HIDE.0 as u32,
        rcNormalPosition: RECT {
            left,
            top,
            right,
            bottom,
        },
        ..Default::default()
    };
    // Moved onto a monitor of another DPI than the one it was made on, the
    // window hears `WM_DPICHANGED`, and winit resizes it to keep its
    // logical size: placed again, now on that monitor, it keeps the frame's.
    // SAFETY: places our own (hidden) window.
    unsafe {
        SetWindowPlacement(hwnd, &placement).is_ok()
            && SetWindowPlacement(hwnd, &placement).is_ok()
    }
}

/// The autosave subclass's state: the name, and whether a move or resize
/// loop runs (saved once, as it ends).
struct Autosave {
    name: String,
    looping: std::cell::Cell<bool>,
}
/// # Safety
///
/// Called by comctl32 alone, as the subclass `autosave` installs: `data` is
/// the `Autosave` it leaked for it.
unsafe extern "system" fn autosave_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: `data` is the `Autosave` `autosave` leaked for this subclass,
    // alive until `WM_NCDESTROY` takes it back.
    let state = unsafe { &*(data as *const Autosave) };
    let save = || {
        if let Some(frame) = frame(hwnd) {
            store(&state.name, frame);
        }
    };
    match message {
        WM_ENTERSIZEMOVE => state.looping.set(true),
        WM_EXITSIZEMOVE => {
            state.looping.set(false);
            save();
        }
        // Snapped, maximized, restored or moved by the app: outside a move
        // loop, each is a frame to keep.
        WM_WINDOWPOSCHANGED if !state.looping.get() => save(),
        WM_DESTROY => save(),
        WM_NCDESTROY => {
            // SAFETY: removes this subclass and takes back its state.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(autosave_proc), SUBCLASS_ID);
                drop(Box::from_raw(data as *mut Autosave));
            }
            // SAFETY: the next procedure in the window's chain.
            return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
        }
        _ => {}
    }
    // SAFETY: the next procedure in the window's chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
/// Keep `hwnd`'s frame under `name` as it changes, and as it closes.
fn autosave(hwnd: HWND, name: &str) {
    let state = Box::into_raw(Box::new(Autosave {
        name: name.to_owned(),
        looping: std::cell::Cell::new(false),
    }));
    // SAFETY: a subclass of our own window on its thread; its state is taken
    // back as the window goes (or now, if not installed).
    unsafe {
        if !SetWindowSubclass(hwnd, Some(autosave_proc), SUBCLASS_ID, state as usize).as_bool() {
            drop(Box::from_raw(state));
        }
    }
}

/// Put content point `(x, y)` (logical) of the hidden window under the
/// pointer.
fn under_pointer(hwnd: HWND, (x, y): (f64, f64)) {
    let (mut pointer, mut client) = (POINT::default(), POINT::default());
    let mut frame = RECT::default();
    // SAFETY: reads the pointer and our own window's geometry into locals,
    // then moves the window.
    unsafe {
        if GetCursorPos(&mut pointer).is_err()
            || !ClientToScreen(hwnd, &mut client).as_bool()
            || GetWindowRect(hwnd, &mut frame).is_err()
        {
            return;
        }
        let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.;
        let left = pointer.x - (x * scale).round() as i32 - (client.x - frame.left);
        let top = pointer.y - (y * scale).round() as i32 - (client.y - frame.top);
        let _ = SetWindowPos(
            hwnd,
            None,
            left,
            top,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Put `window`'s content top-left at global point `to` (the screen's
/// pixels), as a drag of its only tab does (`View::drag_moves_window`): at
/// once, so there is nothing for `settle` to do.
pub(crate) fn move_to(window: &winit::window::Window, to: (f64, f64), _: &mut Option<(f64, f64)>) {
    let Some(hwnd) = hwnd(window) else {
        return;
    };
    let (mut client, mut frame) = (POINT::default(), RECT::default());
    // SAFETY: reads our own window's geometry into locals, then moves it.
    unsafe {
        if !ClientToScreen(hwnd, &mut client).as_bool() || GetWindowRect(hwnd, &mut frame).is_err()
        {
            return;
        }
        let left = to.0.round() as i32 - (client.x - frame.left);
        let top = to.1.round() as i32 - (client.y - frame.top);
        let _ = SetWindowPos(
            hwnd,
            None,
            left,
            top,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// The pointer in global coordinates (the screen's pixels): a window
/// following the pointer is placed from it, never from window-relative
/// points that its own moves skew.
pub(crate) fn pointer() -> Option<(f64, f64)> {
    let mut point = POINT::default();
    // SAFETY: writes the pointer's position into a local.
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some((f64::from(point.x), f64::from(point.y)))
}

/// `move_to` moved the window at once: nothing to tell Windows.
pub(crate) fn settle(_: &winit::window::Window, _: &mut Option<(f64, f64)>) {}

/// Place `window` (not shown yet), as `super::place` says. Placed nowhere in
/// particular, it stays where Windows put it (`CW_USEDEFAULT` cascades);
/// `Tabbing` has no counterpart here.
pub(crate) fn place(
    window: &winit::window::Window,
    under_pointer_at: Option<(f64, f64)>,
    _placed: bool,
    options: &crate::WindowOptions,
) -> Placed {
    let Some(hwnd) = hwnd(window) else {
        return Placed::default();
    };
    let mut placed = Placed::default();
    if let Some(name) = &options.frame_autosave {
        if under_pointer_at.is_none()
            && let Some(frame) = load(name)
            && restore(hwnd, frame)
        {
            placed = Placed {
                restored: true,
                maximized: frame.maximized,
            };
        }
        autosave(hwnd, name);
    }
    if let Some(at) = under_pointer_at {
        under_pointer(hwnd, at);
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_frame_reads_back_as_it_was_written() {
        let frame = Frame {
            rect: (-1200, 40, -300, 640),
            maximized: true,
            dpi: 144,
        };
        assert_eq!(Frame::decode(&frame.encode()), Some(frame));
        assert_eq!(
            Frame::decode("-1200 40 -300 640 1"),
            Some(Frame { dpi: 0, ..frame }),
            "a frame saved before its DPI was"
        );
        assert_eq!(Frame::decode("10 20 5 30 0"), None, "an empty frame");
        assert_eq!(Frame::decode("10 20"), None);
        assert_eq!(Frame::decode("garbage"), None);
    }

    #[test]
    fn a_saved_frame_fits_the_monitor_it_reopens_on() {
        // A 1080p monitor at 100% with a 48-pixel taskbar, right of a 4K
        // one at 150% the frame was saved on.
        let area = RECT {
            left: 3840,
            top: 0,
            right: 5760,
            bottom: 1032,
        };
        let frame = Frame {
            rect: (400, 200, 2380, 1520),
            maximized: false,
            dpi: 144,
        };
        let big = RECT {
            left: 0,
            top: 0,
            right: 3840,
            bottom: 2088,
        };
        assert_eq!(fit(frame, big, 144, true), frame.rect, "shown and fits: as saved");
        // Its monitor gone: as big as it looked (1980 × 1320 at 150% is
        // 1320 × 880 at 100%), fully inside the work area.
        assert_eq!(fit(frame, area, 96, false), (3840, 152, 5160, 1032));
        // Shown, on a monitor now too small for it: shrunk, kept inside.
        let small = Frame {
            rect: (3900, 100, 6000, 1300),
            dpi: 96,
            ..frame
        };
        assert_eq!(fit(small, area, 96, true), (3840, 0, 5760, 1032));
        // Saved before its DPI was: its pixels, moved inside.
        let old = Frame { dpi: 0, ..frame };
        assert_eq!(fit(old, area, 96, false), (3840, 0, 5760, 1032));
    }
}
