//! Low-level input simulation built on `SendInput`.
//!
//! This intentionally replaces the Python reference implementation's use of
//! the legacy `mouse_event`/`keybd_event` APIs (`windows_mcp.uia.core`) with
//! `SendInput`, and normalizes absolute mouse coordinates against the
//! *virtual* screen (all monitors) instead of the primary monitor only, so
//! clicks on secondary monitors placed left of or above the primary monitor
//! land correctly.

use std::thread::sleep;
use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, POINT};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetDoubleClickTime, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_ABSOLUTE,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    SendInput, VIRTUAL_KEY, VK_RETURN, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SetCursorPos,
};

/// WHEEL_DELTA from winuser.h: one "notch" of mouse wheel rotation.
const WHEEL_DELTA: i32 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Current cursor position in screen coordinates.
pub fn get_cursor_pos() -> (i32, i32) {
    let mut point = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    (point.x, point.y)
}

/// Moves the cursor directly to `(x, y)` with no intermediate steps.
pub fn set_cursor_pos(x: i32, y: i32) {
    unsafe {
        let _ = SetCursorPos(x, y);
    }
}

/// The system double-click time, in milliseconds.
pub fn get_double_click_time_ms() -> u32 {
    unsafe { GetDoubleClickTime() }
}

/// Bounding rectangle of the virtual screen (the union of all monitors), as
/// `(x, y, width, height)`.
fn virtual_screen_rect() -> (i32, i32, i32, i32) {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        (x, y, w.max(1), h.max(1))
    }
}

/// Normalizes a screen coordinate to the 0..=65535 range `SendInput` expects
/// for `MOUSEEVENTF_ABSOLUTE`, relative to the virtual screen origin.
fn normalize_absolute(x: i32, y: i32) -> (i32, i32) {
    let (vx, vy, vw, vh) = virtual_screen_rect();
    let nx = (x - vx) as i64 * 65535 / (vw - 1).max(1) as i64;
    let ny = (y - vy) as i64 * 65535 / (vh - 1).max(1) as i64;
    let nx = nx.clamp(0, 65535);
    let ny = ny.clamp(0, 65535);
    (nx as i32, ny as i32)
}

fn send_mouse_input(flags: MOUSE_EVENT_FLAGS, dx: i32, dy: i32, mouse_data: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: mouse_data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn mouse_button_flags(button: MouseButton, down: bool) -> MOUSE_EVENT_FLAGS {
    match (button, down) {
        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
    }
}

/// Presses `button` down at the cursor's current position.
pub fn mouse_down(button: MouseButton) {
    let (x, y) = get_cursor_pos();
    let (nx, ny) = normalize_absolute(x, y);
    send_mouse_input(
        mouse_button_flags(button, true) | MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
        nx,
        ny,
        0,
    );
}

/// Releases `button` at the cursor's current position.
pub fn mouse_up(button: MouseButton) {
    let (x, y) = get_cursor_pos();
    let (nx, ny) = normalize_absolute(x, y);
    send_mouse_input(
        mouse_button_flags(button, false) | MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
        nx,
        ny,
        0,
    );
}

/// A single click cycle at `(x, y)`: move, button down, a short gap, button
/// up, then `wait_after`.
pub fn click_once(x: i32, y: i32, button: MouseButton, wait_after: Duration) {
    set_cursor_pos(x, y);
    mouse_down(button);
    sleep(Duration::from_millis(50));
    mouse_up(button);
    sleep(wait_after);
}

/// Maximum duration (seconds) `move_smooth` allows itself at `move_speed == 1`.
const MAX_MOVE_SECOND: f64 = 1.0;

/// Smoothly moves the cursor to `(x, y)` in stepped `SetCursorPos` calls,
/// porting the pacing algorithm from the Python reference's `uia.MoveTo`.
pub fn move_smooth(x: i32, y: i32, move_speed: f64, wait_after: Duration) {
    let mut move_time = if move_speed > 0.0 {
        MAX_MOVE_SECOND / move_speed
    } else {
        0.0
    };
    let (cur_x, cur_y) = get_cursor_pos();
    let x_count = (x - cur_x).unsigned_abs();
    let y_count = (y - cur_y).unsigned_abs();
    let mut max_point = x_count.max(y_count) as i64;

    let (_, _, vw, vh) = virtual_screen_rect();
    let max_side = vw.max(vh) as i64;
    let min_side = vw.min(vh) as i64;

    if max_point > min_side {
        max_point = min_side;
    }
    if max_point < max_side {
        max_point = 100 + ((max_side - 100) as f64 / max_side as f64 * max_point as f64) as i64;
        move_time = move_time * max_point as f64 / max_side as f64;
    }
    let step_count = max_point / 20;
    if step_count > 1 {
        let x_step = (x - cur_x) as f64 / step_count as f64;
        let y_step = (y - cur_y) as f64 / step_count as f64;
        let interval = move_time / step_count as f64;
        for i in 0..step_count {
            let cx = cur_x + (x_step * i as f64) as i32;
            let cy = cur_y + (y_step * i as f64) as i32;
            set_cursor_pos(cx, cy);
            if interval > 0.0 {
                sleep(Duration::from_secs_f64(interval));
            }
        }
    }
    set_cursor_pos(x, y);
    sleep(wait_after);
}

/// Moves the cursor to `(x, y)` over exactly `duration` seconds via linear
/// interpolation, capped at 200 steps (10ms each). Ports `uia.MoveToDuration`.
pub fn move_smooth_duration(x: i32, y: i32, duration: f64, wait_after: Duration) {
    let (cur_x, cur_y) = get_cursor_pos();
    if duration <= 0.0 {
        set_cursor_pos(x, y);
        sleep(wait_after);
        return;
    }
    let step_count = ((duration / 0.01).ceil() as i64).clamp(2, 200);
    let interval = duration / step_count as f64;
    for i in 1..=step_count {
        let ratio = i as f64 / step_count as f64;
        let cx = cur_x + ((x - cur_x) as f64 * ratio).round() as i32;
        let cy = cur_y + ((y - cur_y) as f64 * ratio).round() as i32;
        set_cursor_pos(cx, cy);
        sleep(Duration::from_secs_f64(interval));
    }
    sleep(wait_after);
}

/// Spins the mouse wheel `notches` times (positive = up, negative = down),
/// waiting `interval` between notches and `wait_after` once all notches have
/// been sent.
pub fn wheel(notches: i32, interval: Duration, wait_after: Duration) {
    let delta = if notches >= 0 {
        WHEEL_DELTA
    } else {
        -WHEEL_DELTA
    };
    for _ in 0..notches.unsigned_abs() {
        send_mouse_input(MOUSEEVENTF_WHEEL, 0, 0, delta);
        sleep(interval);
    }
    sleep(wait_after);
}

fn send_keyboard_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

/// Presses a virtual-key code down (does not release it).
pub fn key_down(vk: u16) {
    send_keyboard_input(VIRTUAL_KEY(vk), KEYBD_EVENT_FLAGS(0));
}

/// Releases a virtual-key code.
pub fn key_up(vk: u16) {
    send_keyboard_input(VIRTUAL_KEY(vk), KEYEVENTF_KEYUP);
}

/// Presses and releases a virtual-key code, waiting `wait_after` afterward.
pub fn key_tap(vk: u16, wait_after: Duration) {
    key_down(vk);
    key_up(vk);
    sleep(wait_after);
}

/// Presses `vks` down together, in order, holds briefly, then releases them
/// in reverse order — a simultaneous chord (e.g. Ctrl+Shift+Esc) — waiting
/// `wait_after` once everything has been released.
pub fn chord(vks: &[u16], wait_after: Duration) {
    for &vk in vks {
        key_down(vk);
        sleep(Duration::from_millis(10));
    }
    sleep(Duration::from_millis(10));
    for &vk in vks.iter().rev() {
        key_up(vk);
        sleep(Duration::from_millis(10));
    }
    sleep(wait_after);
}

/// Sends a single Unicode character via `KEYEVENTF_UNICODE`, bypassing
/// keyboard-layout translation entirely.
pub fn send_unicode_char(ch: char) {
    let mut buf = [0u16; 2];
    for unit in ch.encode_utf16(&mut buf) {
        send_keyboard_input(VIRTUAL_KEY(*unit), KEYEVENTF_UNICODE);
        send_keyboard_input(VIRTUAL_KEY(*unit), KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
    }
}

/// Types `text` one character at a time, waiting `interval` between
/// characters. `\n` and `\t` are sent as Enter/Tab key taps (so form
/// navigation still works); `\r` is skipped; everything else goes through
/// `send_unicode_char`.
pub fn type_text_char_by_char(text: &str, interval: Duration, wait_after: Duration) {
    for ch in text.chars() {
        match ch {
            '\n' => key_tap(VK_RETURN.0, Duration::ZERO),
            '\t' => key_tap(VK_TAB.0, Duration::ZERO),
            '\r' => {}
            other => send_unicode_char(other),
        }
        sleep(interval);
    }
    sleep(wait_after);
}

/// Reads CF_UNICODETEXT from the clipboard, if present.
pub fn get_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let result = (|| {
            let handle = GetClipboardData(13 /* CF_UNICODETEXT */).ok()?;
            let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0 as *mut _));
            if ptr.is_null() {
                return None;
            }
            let wide = std::slice::from_raw_parts(ptr as *const u16, wcslen(ptr as *const u16));
            let text = String::from_utf16_lossy(wide);
            let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0 as *mut _));
            Some(text)
        })();
        let _ = CloseClipboard();
        result
    }
}

/// Writes `text` to the clipboard as CF_UNICODETEXT. Returns `true` on
/// success.
pub fn set_clipboard_text(text: &str) -> bool {
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let ok = (|| {
            let _ = EmptyClipboard();
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let byte_len = wide.len() * std::mem::size_of::<u16>();
            let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len).ok()?;
            let ptr = GlobalLock(hmem);
            if ptr.is_null() {
                let _ = GlobalFree(Some(hmem));
                return None;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
            let _ = GlobalUnlock(hmem);
            if SetClipboardData(
                13, /* CF_UNICODETEXT */
                Some(windows::Win32::Foundation::HANDLE(hmem.0)),
            )
            .is_err()
            {
                let _ = GlobalFree(Some(hmem));
                return None;
            }
            Some(())
        })()
        .is_some();
        let _ = CloseClipboard();
        ok
    }
}

/// Restores the clipboard to an empty state.
pub fn clear_clipboard() {
    unsafe {
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();
            let _ = CloseClipboard();
        }
    }
}

/// Minimal `wcslen` for reading a NUL-terminated UTF-16 buffer.
unsafe fn wcslen(mut ptr: *const u16) -> usize {
    let mut len = 0usize;
    unsafe {
        while *ptr != 0 {
            len += 1;
            ptr = ptr.add(1);
        }
    }
    len
}
