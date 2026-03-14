use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, IsZoomed, HTCAPTION, HTCLIENT, WM_NCDESTROY, WM_NCHITTEST,
};

/// Height in pixels of the invisible drag strip at the top of the window
/// when maximized. This lets the OS handle dragging natively from the
/// topmost screen edge, which egui cannot receive events for.
const CAPTION_GRAB_HEIGHT: i32 = 8;

const SUBCLASS_ID: usize = 1;

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid_subclass: usize,
    _ref_data: usize,
) -> LRESULT {
    if msg == WM_NCHITTEST {
        let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        if result == HTCLIENT as LRESULT && unsafe { IsZoomed(hwnd) } != 0 {
            let mut pt = POINT {
                x: (lparam & 0xFFFF) as i16 as i32,
                y: ((lparam >> 16) & 0xFFFF) as i16 as i32,
            };
            unsafe { ScreenToClient(hwnd, &mut pt) };
            if pt.y < CAPTION_GRAB_HEIGHT {
                return HTCAPTION as LRESULT;
            }
        }
        return result;
    }

    if msg == WM_NCDESTROY {
        unsafe { RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID) };
    }

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Install a window subclass that returns HTCAPTION for the top few pixels
/// when maximized, enabling native drag from the screen edge.
pub fn install_drag_subclass(hwnd: isize) {
    unsafe {
        SetWindowSubclass(hwnd as HWND, Some(subclass_proc), SUBCLASS_ID, 0);
    }
}

/// Query the cursor's Y position in client coordinates using the native API.
/// Returns the Y value, or -1 if the query fails.
/// This works even when the cursor is in the HTCAPTION zone where egui
/// doesn't receive mouse events.
pub fn get_cursor_client_y(hwnd: isize) -> i32 {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return -1;
        }
        ScreenToClient(hwnd as HWND, &mut pt);
        pt.y
    }
}
