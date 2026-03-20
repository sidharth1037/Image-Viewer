use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, IsZoomed, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
    HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, WM_NCDESTROY, WM_NCHITTEST,
};

// Safely link the DWM API to avoid Cargo.toml feature flag headaches
#[link(name = "dwmapi")]
unsafe extern "system" {
    pub fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwattribute: u32,
        pvattribute: *const core::ffi::c_void,
        cbattribute: u32,
    ) -> i32;
}

const RESIZE_BORDER: i32 = 10;
const CORNER_BORDER: i32 = 14;
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

    // --- BORDERLESS DRAG & RESIZE ---
    // Notice: We completely removed WM_SIZE, WM_NCACTIVATE, and WM_NCPAINT!
    // We let the modern Windows DWM handle all drawing natively, eliminating flashes.

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
            if pt.y < CAPTION_GRAB_HEIGHT {
                return HTCAPTION as LRESULT;
            }
        } else {
            let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let w = rc.right;
            let h = rc.bottom;

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

pub fn install_drag_subclass(hwnd: isize) {
    unsafe {
        SetWindowSubclass(hwnd as HWND, Some(subclass_proc), SUBCLASS_ID, 0);

        // --- THE MODERN NATIVE FIX ---
        // Tell the Windows 11 Desktop Window Manager to round the corners natively.
        // This happens on the GPU, keeps your dropshadow, and NEVER flashes!
        const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
        const DWMWCP_ROUND: u32 = 2; // 2 = Round, 3 = Small Round
        
        DwmSetWindowAttribute(
            hwnd as HWND,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &DWMWCP_ROUND as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

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