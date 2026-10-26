//! Clipboard selection follows the actual native display, never environment guesses.
//!
//! Wayland uses the application's data worker (`wayland_data`), which owns the
//! connection's only data devices; X11, macOS and Windows retain arboard. The primary
//! selection (middle-click paste) exists on X11 and Wayland only.
use std::error::Error;
use winit::event_loop::OwnedDisplayHandle;
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Text as the platform's apps take it from the clipboard. Windows': lines
/// ending in CR LF, as browsers write them there (a classic edit control
/// shows a lone LF as no break at all). Pasting gives editors either.
fn platform_text(text: String) -> String {
    #[cfg(windows)]
    if text.contains('\n') {
        return crlf(&text);
    }
    text
}

/// `text` with a CR before each LF that has none.
#[cfg(any(windows, test))]
fn crlf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut previous = None;
    for c in text.chars() {
        if c == '\n' && previous != Some('\r') {
            out.push('\r');
        }
        out.push(c);
        previous = Some(c);
    }
    out
}

enum Backend {
    Native(arboard::Clipboard),
    #[cfg(target_os = "linux")]
    Wayland(crate::wayland_data::WaylandClipboard),
}

pub(crate) struct Clipboard {
    backend: Backend,
}

impl Clipboard {
    /// `wayland` is the data worker's clipboard, required on a Wayland display.
    pub(crate) fn new(
        display: OwnedDisplayHandle,
        #[cfg(target_os = "linux")] wayland: Option<crate::wayland_data::WaylandClipboard>,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        if let RawDisplayHandle::Wayland(_) = display.display_handle()?.as_raw() {
            let wayland = wayland.ok_or("the Wayland data worker is unavailable")?;
            return Ok(Self {
                backend: Backend::Wayland(wayland),
            });
        }
        let _ = display;
        Ok(Self {
            backend: Backend::Native(arboard::Clipboard::new()?),
        })
    }

    /// Put `text` on the clipboard with `html`, its rich form, beside it
    /// where the platform takes HTML (macOS, Windows, X11); text alone on
    /// Wayland.
    pub(crate) fn set_rich_text(&mut self, text: String, html: String) -> Result<()> {
        match &mut self.backend {
            Backend::Native(clipboard) => clipboard.set_html(html, Some(platform_text(text)))?,
            #[cfg(target_os = "linux")]
            Backend::Wayland(clipboard) => clipboard.store_rich(text, html)?,
        }
        Ok(())
    }

    pub(crate) fn get_text(&mut self) -> Result<String> {
        match &mut self.backend {
            Backend::Native(clipboard) => Ok(clipboard.get_text()?),
            #[cfg(target_os = "linux")]
            Backend::Wayland(clipboard) => Ok(clipboard.load(false)?),
        }
    }

    /// Wayland publication is asynchronous: success means the selection was
    /// offered, not that another process has accepted or persisted its data.
    pub(crate) fn set_text(&mut self, text: String) -> Result<()> {
        match &mut self.backend {
            Backend::Native(clipboard) => clipboard.set_text(platform_text(text))?,
            #[cfg(target_os = "linux")]
            Backend::Wayland(clipboard) => clipboard.store(false, text)?,
        }
        Ok(())
    }

    /// Where a worker thread reads and writes its images (`read_image`,
    /// `write_image`): arboard's clipboards, each its own, while this one,
    /// kept for the app's life, keeps X11's selection served; on Wayland
    /// the data worker, as PNG.
    pub(crate) fn images(&self) -> Images {
        match &self.backend {
            Backend::Native(_) => Images::Native,
            #[cfg(target_os = "linux")]
            Backend::Wayland(clipboard) => Images::Wayland(clipboard.clone()),
        }
    }

    /// The primary selection (X11, Wayland): the text last selected
    /// anywhere, which a middle click pastes.
    #[cfg(target_os = "linux")]
    pub(crate) fn get_primary_text(&mut self) -> Result<String> {
        match &mut self.backend {
            Backend::Native(clipboard) => {
                use arboard::{GetExtLinux, LinuxClipboardKind};
                Ok(clipboard
                    .get()
                    .clipboard(LinuxClipboardKind::Primary)
                    .text()?)
            }
            Backend::Wayland(clipboard) => Ok(clipboard.load(true)?),
        }
    }

    /// Publish the primary selection (X11, Wayland).
    #[cfg(target_os = "linux")]
    pub(crate) fn set_primary_text(&mut self, text: String) -> Result<()> {
        match &mut self.backend {
            Backend::Native(clipboard) => {
                use arboard::{LinuxClipboardKind, SetExtLinux};
                clipboard
                    .set()
                    .clipboard(LinuxClipboardKind::Primary)
                    .text(text)?;
            }
            Backend::Wayland(clipboard) => clipboard.store(true, text)?,
        }
        Ok(())
    }
}

/// How a worker thread reaches a clipboard's images (`Clipboard::images`).
pub(crate) enum Images {
    Native,
    #[cfg(target_os = "linux")]
    Wayland(crate::wayland_data::WaylandClipboard),
}

/// The clipboard's image, decoded on the calling thread (a worker's: a
/// large one takes tens of milliseconds) from what the platform's apps put
/// there: on Windows a PNG, else a DIB (what Paint, browsers and
/// screenshots copy); on Wayland its `image/png`.
pub(crate) fn read_image(images: &Images) -> Result<zgx::image::ImageData> {
    #[cfg(target_os = "linux")]
    if let Images::Wayland(clipboard) = images {
        let png = clipboard.load_image()?;
        let image =
            image::load_from_memory_with_format(&png, image::ImageFormat::Png)?.into_rgba8();
        let (width, height) = image.dimensions();
        return Ok(zgx::image::ImageData::new(width, height, image.into_raw())?);
    }
    let _ = images;
    #[cfg(windows)]
    if let Some(image) = windows_dib() {
        return Ok(image);
    }
    let image = arboard::Clipboard::new()?.get_image()?;
    Ok(zgx::image::ImageData::new(
        image.width.try_into()?,
        image.height.try_into()?,
        image.bytes.into_owned(),
    )?)
}

/// Put `image` on the clipboard, encoded on the calling thread: on Windows
/// as a PNG and a DIB, which apps that take either paste; on Wayland as a
/// PNG.
pub(crate) fn write_image(images: &Images, image: &zgx::image::ImageData) -> Result<()> {
    #[cfg(target_os = "linux")]
    if let Images::Wayland(clipboard) = images {
        let mut png = Vec::new();
        image::write_buffer_with_format(
            &mut std::io::Cursor::new(&mut png),
            image.pixels(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )?;
        return Ok(clipboard.store_image(png)?);
    }
    let _ = images;
    arboard::Clipboard::new()?.set_image(arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: image.pixels().into(),
    })?;
    Ok(())
}

/// The clipboard's DIB on Windows, decoded by `dib`; none where a PNG is
/// there too (the better copy, which arboard reads) or the DIB is of a kind
/// `dib` leaves to arboard.
#[cfg(windows)]
fn windows_dib() -> Option<zgx::image::ImageData> {
    use windows::{
        Win32::{
            Foundation::HGLOBAL,
            System::{
                DataExchange::{
                    CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
                    RegisterClipboardFormatW,
                },
                Memory::{GlobalLock, GlobalSize, GlobalUnlock},
                Ole::CF_DIBV5,
            },
        },
        core::w,
    };
    // SAFETY: the clipboard is opened and closed around the read; the block
    // is read within its locked size and copied before it is unlocked.
    unsafe {
        if IsClipboardFormatAvailable(RegisterClipboardFormatW(w!("PNG"))).is_ok()
            || IsClipboardFormatAvailable(u32::from(CF_DIBV5.0)).is_err()
        {
            return None;
        }
        // Another app may hold the clipboard a moment, as arboard allows for.
        (0..10).find(|_| {
            OpenClipboard(None).is_ok() || {
                std::thread::sleep(std::time::Duration::from_millis(5));
                false
            }
        })?;
        let data = GetClipboardData(u32::from(CF_DIBV5.0))
            .ok()
            .and_then(|handle| {
                let global = HGLOBAL(handle.0);
                let pointer = GlobalLock(global);
                (!pointer.is_null()).then(|| {
                    let data = std::slice::from_raw_parts(pointer.cast::<u8>(), GlobalSize(global))
                        .to_vec();
                    let _ = GlobalUnlock(global);
                    data
                })
            });
        let _ = CloseClipboard();
        let (width, height, rgba) = dib::decode(&data?)?;
        zgx::image::ImageData::new(width, height, rgba).ok()
    }
}

/// Windows' DIBs as apps put them on the clipboard, decoded here rather than
/// by arboard's decoder, which takes a V4 or V5 header with bit fields to be
/// followed by its colour masks again: where they aren't (Firefox's DIBs,
/// arboard's own), it failed, or shifted every pixel where the block had
/// slack at its end.
#[cfg(any(windows, test))]
mod dib {
    const BI_RGB: u32 = 0;
    const BI_BITFIELDS: u32 = 3;

    /// Straight RGBA rows top to bottom from a packed DIB (`CF_DIBV5`,
    /// `CF_DIB`): a BITMAPINFOHEADER, V4 or V5 header and 24- or 32-bit
    /// rows, plain or in bit fields. None for other kinds (palettes, 16-bit,
    /// RLE, JPEG), which arboard reads.
    pub(super) fn decode(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        let at = |offset: usize| {
            let bytes = data.get(offset..offset.checked_add(4)?)?;
            Some(u32::from_le_bytes(bytes.try_into().ok()?))
        };
        let size = at(0)? as usize;
        let (width, height) = (at(4)? as i32, at(8)? as i32);
        let bits = at(12)? >> 16;
        let (compression, colours) = (at(16)?, at(32)? as usize);
        if !matches!(size, 40 | 108 | 124) || width <= 0 || height == 0 || height == i32::MIN {
            return None;
        }
        let (width, rows) = (width as usize, height.unsigned_abs() as usize);
        let stride = (width.checked_mul(bits as usize)?.checked_add(31)? / 32).checked_mul(4)?;
        let length = stride.checked_mul(rows)?;
        let mut offset = size;
        let masks = match (bits, compression) {
            (24, BI_RGB) => [0xff_0000, 0xff00, 0xff, 0],
            // The fourth byte is unused (GDI bitmaps', Paint's, leave it 0)
            // unless a V4 or V5 header masks it as alpha, as browsers' do.
            (32, BI_RGB) => {
                let alpha = if size > 40 { at(52)? } else { 0 };
                [0xff_0000, 0xff00, 0xff, alpha & 0xff00_0000]
            }
            (32, BI_BITFIELDS) if size == 40 => {
                offset += 12;
                [at(40)?, at(44)?, at(48)?, 0]
            }
            (32, BI_BITFIELDS) => {
                let masks = [at(40)?, at(44)?, at(48)?, at(52)?];
                // Some writers repeat the colour masks after a V4 or V5
                // header, where a BITMAPINFOHEADER's go.
                if (0..3).all(|i| at(size + 4 * i) == Some(masks[i]))
                    && data.len() >= size + 12 + length
                {
                    offset += 12;
                }
                masks
            }
            _ => return None,
        };
        // A colour table, which these depths need not have.
        offset = offset.checked_add(colours.checked_mul(4)?)?;
        let pixels = data.get(offset..offset.checked_add(length)?)?;
        let channel = |pixel: u32, mask: u32| {
            if mask == 0 {
                return 255;
            }
            let shift = mask.trailing_zeros();
            let value = u64::from((pixel & mask) >> shift);
            (value * 255 / u64::from(mask >> shift)) as u8
        };
        let bytes = bits as usize / 8;
        let mut rgba = Vec::with_capacity(width * rows * 4);
        for row in 0..rows {
            let source = if height > 0 { rows - 1 - row } else { row };
            let line = &pixels[source * stride..][..width * bytes];
            for pixel in line.chunks_exact(bytes) {
                let pixel = u32::from_le_bytes([
                    pixel[0],
                    pixel[1],
                    pixel[2],
                    pixel.get(3).copied().unwrap_or(0),
                ]);
                rgba.extend(masks.map(|mask| channel(pixel, mask)));
            }
        }
        Some((width as u32, rows as u32, rgba))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lines_copied_end_in_cr_lf_once() {
        assert_eq!(
            super::crlf("one\ntwo\r\nthree\n"),
            "one\r\ntwo\r\nthree\r\n"
        );
        assert_eq!(super::crlf("\n\n"), "\r\n\r\n");
        assert_eq!(super::crlf("no break \r here"), "no break \r here");
        assert_eq!(super::crlf("é\nü"), "é\r\nü");
    }

    /// A DIB's header: `size` 40 (BITMAPINFOHEADER) or 124 (V5, its masks
    /// red, green, blue and alpha in bytes 2, 1, 0 and 3 of a pixel).
    fn header(size: u32, width: i32, height: i32, bits: u16, compression: u32) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend(size.to_le_bytes());
        header.extend(width.to_le_bytes());
        header.extend(height.to_le_bytes());
        header.extend(1u16.to_le_bytes());
        header.extend(bits.to_le_bytes());
        header.extend(compression.to_le_bytes());
        header.resize(40, 0);
        if size == 124 {
            for mask in [0xff_0000u32, 0xff00, 0xff, 0xff00_0000] {
                header.extend(mask.to_le_bytes());
            }
            header.resize(124, 0);
        }
        header
    }

    /// Two rows of two pixels, as a DIB stores them (BGRA, bottom row
    /// first), and as RGBA rows top to bottom.
    const BOTTOM_UP: [u8; 16] = [
        30, 20, 10, 255, 70, 60, 50, 128, // the bottom row
        3, 2, 1, 0, 7, 6, 5, 64, // the top row
    ];
    const RGBA: [u8; 16] = [1, 2, 3, 0, 5, 6, 7, 64, 10, 20, 30, 255, 50, 60, 70, 128];

    #[test]
    fn a_v5_dib_with_an_alpha_mask_decodes_whole_with_or_without_masks_after() {
        let plain = [header(124, 2, 2, 32, 3), BOTTOM_UP.to_vec()].concat();
        assert_eq!(super::dib::decode(&plain), Some((2, 2, RGBA.to_vec())));
        let masks: Vec<u8> = [0xff_0000u32, 0xff00, 0xff]
            .iter()
            .flat_map(|m| m.to_le_bytes())
            .collect();
        let repeated = [header(124, 2, 2, 32, 3), masks, BOTTOM_UP.to_vec()].concat();
        assert_eq!(super::dib::decode(&repeated), Some((2, 2, RGBA.to_vec())));
        // Slack at the end of the block shifts nothing.
        let slack = [plain, vec![0; 12]].concat();
        assert_eq!(super::dib::decode(&slack), Some((2, 2, RGBA.to_vec())));
    }

    #[test]
    fn a_gdi_bitmaps_unused_fourth_byte_is_opaque_not_transparent() {
        let dib = [header(40, 2, 2, 32, 0), BOTTOM_UP.to_vec()].concat();
        let (_, _, rgba) = super::dib::decode(&dib).unwrap();
        assert!(rgba.chunks(4).all(|p| p[3] == 255), "{rgba:?}");
        assert_eq!(&rgba[..3], &[1, 2, 3]);
    }

    #[test]
    fn rows_of_24_bits_are_padded_to_whole_words_and_may_run_top_down() {
        // One pixel a row: three bytes and one of padding.
        let rows = [9, 8, 7, 0, 3, 2, 1, 0];
        let bottom_up = [header(40, 1, 2, 24, 0), rows.to_vec()].concat();
        let top = [1, 2, 3, 255, 7, 8, 9, 255];
        assert_eq!(super::dib::decode(&bottom_up), Some((1, 2, top.to_vec())));
        let top_down = [header(40, 1, -2, 24, 0), rows.to_vec()].concat();
        let flipped = [7, 8, 9, 255, 1, 2, 3, 255];
        assert_eq!(
            super::dib::decode(&top_down),
            Some((1, 2, flipped.to_vec()))
        );
    }

    #[test]
    fn dibs_of_other_kinds_or_cut_short_are_left_alone() {
        let palette = [header(40, 2, 2, 8, 0), vec![0; 1024 + 8]].concat();
        assert_eq!(super::dib::decode(&palette), None);
        let short = [header(124, 2, 2, 32, 3), BOTTOM_UP[..12].to_vec()].concat();
        assert_eq!(super::dib::decode(&short), None);
        assert_eq!(super::dib::decode(&[0; 12]), None);
    }

    /// On Windows, what zgx copies has Windows' line ends; elsewhere it is
    /// as it was.
    #[test]
    fn copied_text_has_the_platforms_line_ends() {
        let copied = super::platform_text("a\nb".into());
        if cfg!(windows) {
            assert_eq!(copied, "a\r\nb");
        } else {
            assert_eq!(copied, "a\nb");
        }
    }
}
