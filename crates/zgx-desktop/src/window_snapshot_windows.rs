//! A window's snapshot on Windows (`Platform::window_snapshot`): the
//! window's client area as the user sees it, child windows (web pages,
//! video) included, which zgx's own frame doesn't hold.
//!
//! `PrintWindow` with `PW_RENDERFULLCONTENT` has the desktop window manager
//! render the window's composed content, Direct3D and DirectComposition
//! children included, into a memory bitmap, whatever covers the window on
//! screen. It runs on the main thread and takes a few tens of milliseconds
//! for a full window. A window that is minimized, or a capture that fails,
//! gives none.
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GetDC, HGDIOBJ, ReleaseDC, SelectObject,
    },
    UI::WindowsAndMessaging::{GetClientRect, IsIconic},
};
use zgx::{image::ImageData, platform::Pending};

// user32's, declared here rather than through the `windows` crate's
// `Win32_Storage_Xps` feature, which brings in nothing else this needs.
#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC, flags: u32) -> i32;
}

/// `PW_CLIENTONLY | PW_RENDERFULLCONTENT`.
const FLAGS: u32 = 1 | 2;

/// The window's snapshot (see the module docs).
pub(crate) fn take(window: &Window) -> Pending<Option<ImageData>> {
    let image = capture(window);
    Box::pin(async move { image })
}

fn capture(window: &Window) -> Option<ImageData> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // SAFETY: winit's live window, on the main thread; every GDI object made
    // here is released before returning, and the DIB's pixels are read
    // within its size while it is selected.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        if w <= 0 || h <= 0 {
            return None;
        }
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Top-down rows.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(memory), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        let mut image = None;
        if let Ok(bitmap) = bitmap {
            let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
            if !bits.is_null() && PrintWindow(hwnd, memory, FLAGS) != 0 {
                let (w, h) = (w as usize, h as usize);
                let bgra = std::slice::from_raw_parts(bits.cast::<u8>(), w * h * 4);
                // GDI leaves the fourth byte undefined: the window is opaque.
                let rgba: Vec<u8> = bgra
                    .chunks_exact(4)
                    .flat_map(|p| [p[2], p[1], p[0], 255])
                    .collect();
                image = ImageData::new(w as u32, h as u32, rgba).ok();
            }
            SelectObject(memory, previous);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
        }
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        image
    }
}
