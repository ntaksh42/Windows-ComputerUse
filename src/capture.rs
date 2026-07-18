//! Screen capture backends.
//!
//! Only the GDI backend is implemented today. `Backend` is an enum (not a
//! trait) since there is exactly one implementation; a `Dxgi` variant is
//! reserved for a future, faster backend but has no capture logic behind it.

use std::env;
use std::ffi::c_void;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

/// Screen capture backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Gdi,
    /// Reserved for a future capture backend; no implementation yet.
    #[allow(dead_code)]
    Dxgi,
}

impl Backend {
    pub fn name(&self) -> &'static str {
        match self {
            Backend::Gdi => "gdi",
            Backend::Dxgi => "dxgi",
        }
    }
}

/// Resolves the capture backend from `WINDOWS_MCP_SCREENSHOT_BACKEND`
/// (`auto`/`gdi`; unrecognized or unset values are treated as `auto`).
/// Only GDI is implemented, so every value currently resolves to `Backend::Gdi`.
pub fn resolve_backend() -> Backend {
    let _ = env::var("WINDOWS_MCP_SCREENSHOT_BACKEND").unwrap_or_default();
    Backend::Gdi
}

/// Returns the bounding rectangle of the full virtual desktop (all monitors).
pub fn virtual_screen_rect() -> RECT {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let cx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let cy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        RECT { left: x, top: y, right: x + cx, bottom: y + cy }
    }
}

/// Captures `rect` (virtual-desktop coordinates) as an RGBA image.
pub fn capture_rect(rect: RECT, backend: Backend) -> Result<image::RgbaImage, String> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(format!("Invalid capture region: {width}x{height}"));
    }

    let pixels = match backend {
        Backend::Gdi => unsafe { capture_rect_gdi(rect, width, height)? },
        Backend::Dxgi => return Err("dxgi backend is not implemented".to_string()),
    };

    image::RgbaImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| "Failed to build image buffer from captured pixels".to_string())
}

/// Captures `rect` via GDI `BitBlt` + `GetDIBits`, returning RGBA bytes.
unsafe fn capture_rect_gdi(rect: RECT, width: i32, height: i32) -> Result<Vec<u8>, String> {
    unsafe {
        let hdc_screen = GetDC(None);
        if hdc_screen.is_invalid() {
            return Err("GetDC returned a null device context".to_string());
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        if hdc_mem.is_invalid() {
            ReleaseDC(None, hdc_screen);
            return Err("CreateCompatibleDC failed".to_string());
        }

        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.is_invalid() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(None, hdc_screen);
            return Err("CreateCompatibleBitmap failed".to_string());
        }

        let old_obj = SelectObject(hdc_mem, hbitmap.into());
        let blit_result =
            BitBlt(hdc_mem, 0, 0, width, height, Some(hdc_screen), rect.left, rect.top, SRCCOPY);

        let mut buffer = vec![0u8; width as usize * height as usize * 4];
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // negative height requests a top-down DIB
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let lines_copied = if blit_result.is_ok() {
            GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                height as u32,
                Some(buffer.as_mut_ptr() as *mut c_void),
                &mut bmi,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };

        SelectObject(hdc_mem, old_obj);
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        if let Err(e) = blit_result {
            return Err(format!("BitBlt failed: {e}"));
        }
        if lines_copied == 0 {
            return Err("GetDIBits failed to read captured pixels".to_string());
        }

        // GDI writes 32bpp pixels as BGRA (with a meaningless alpha byte for
        // an opaque desktop capture); swap to RGBA and force full opacity.
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_backend_is_always_gdi_today() {
        assert_eq!(resolve_backend(), Backend::Gdi);
        assert_eq!(resolve_backend().name(), "gdi");
    }

    #[test]
    fn capture_rect_rejects_empty_region() {
        let rect = RECT { left: 0, top: 0, right: 0, bottom: 100 };
        let err = capture_rect(rect, Backend::Gdi).unwrap_err();
        assert!(err.contains("Invalid capture region"));
    }
}
