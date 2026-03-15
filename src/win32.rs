use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, IsZoomed, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
    HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, WM_NCDESTROY, WM_NCHITTEST,
};

/// Width/height in pixels of the resize zone for edges.
const RESIZE_BORDER: i32 = 10;

/// Width/height in pixels of the resize zone for corners (larger for easier grabbing).
const CORNER_BORDER: i32 = 14;

/// Height in pixels of the invisible drag strip at the top of the window
/// when maximized. This lets the OS handle dragging natively from the
/// topmost screen edge, which egui cannot receive events for.
const CAPTION_GRAB_HEIGHT: i32 = 10;

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
        if result != HTCLIENT as LRESULT {
            return result;
        }

        let mut pt = POINT {
            x: (lparam & 0xFFFF) as i16 as i32,
            y: ((lparam >> 16) & 0xFFFF) as i16 as i32,
        };
        unsafe { ScreenToClient(hwnd, &mut pt) };

        let is_maximized = unsafe { IsZoomed(hwnd) } != 0;

        if is_maximized {
            // When maximized: only handle the caption grab strip at the top
            if pt.y < CAPTION_GRAB_HEIGHT {
                return HTCAPTION as LRESULT;
            }
        } else {
            // When windowed: handle resize from all edges and corners
            let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let w = rc.right;
            let h = rc.bottom;

            // Check corners first with larger hit zone
            let on_corner_left = pt.x < CORNER_BORDER;
            let on_corner_right = pt.x >= w - CORNER_BORDER;
            let on_corner_top = pt.y < CORNER_BORDER;
            let on_corner_bottom = pt.y >= h - CORNER_BORDER;

            let hit = match (on_corner_left, on_corner_right, on_corner_top, on_corner_bottom) {
                (true, _, true, _) => HTTOPLEFT,
                (true, _, _, true) => HTBOTTOMLEFT,
                (_, true, true, _) => HTTOPRIGHT,
                (_, true, _, true) => HTBOTTOMRIGHT,
                _ => {
                    // Check edges with smaller hit zone
                    let on_left = pt.x < RESIZE_BORDER;
                    let on_right = pt.x >= w - RESIZE_BORDER;
                    let on_top = pt.y < RESIZE_BORDER;
                    let on_bottom = pt.y >= h - RESIZE_BORDER;

                    match (on_left, on_right, on_top, on_bottom) {
                        (true, _, _, _) => HTLEFT,
                        (_, true, _, _) => HTRIGHT,
                        (_, _, true, _) => HTTOP,
                        (_, _, _, true) => HTBOTTOM,
                        _ => return result,
                    }
                }
            };
            return hit as LRESULT;
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
