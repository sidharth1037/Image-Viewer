#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

/// Copy one or more files to the Windows clipboard so they can be pasted in
/// Explorer or other file managers (exactly like Ctrl+C in Explorer).
pub fn copy_files_to_clipboard(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    copy_via_dropfiles(paths)
}

/// Use the classic DROPFILES clipboard format (CF_HDROP = 15).
/// This works for files from any combination of folders.
fn copy_via_dropfiles(paths: &[PathBuf]) -> Result<(), String> {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GLOBAL_ALLOC_FLAGS,
    };

    unsafe {
        let hr_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr_init.is_err() && hr_init != RPC_E_CHANGED_MODE {
            return Err(format!(
                "Failed to initialize COM ({:#010X})",
                hr_init.0 as u32
            ));
        }
        let should_uninitialize = hr_init.is_ok();

        // Build the list of wide-string paths, each null-terminated, with a
        // final double-null terminator.
        let mut wide_paths: Vec<u16> = Vec::new();
        for path in paths {
            let abs = absolute_path(path);
            let wide: Vec<u16> = abs.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            wide_paths.extend_from_slice(&wide);
        }
        wide_paths.push(0); // Double null terminator.

        // DROPFILES structure: 20 bytes header + wide string data.
        let header_size = std::mem::size_of::<DropfilesHeader>();
        let data_size = wide_paths.len() * std::mem::size_of::<u16>();
        let total_size = header_size + data_size;

        // GMEM_MOVEABLE | GMEM_ZEROINIT = 0x0042
        let hglobal = GlobalAlloc(GLOBAL_ALLOC_FLAGS(0x0042), total_size)
            .map_err(|e| format!("GlobalAlloc failed: {}", e))?;

        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err("GlobalLock failed".to_string());
        }

        // Write the DROPFILES header.
        let header = ptr as *mut DropfilesHeader;
        (*header).p_files = header_size as u32;
        (*header).pt_x = 0;
        (*header).pt_y = 0;
        (*header).f_nc = 0;
        (*header).f_wide = 1; // Unicode paths.

        // Write the path data after the header.
        let data_dst = (ptr as *mut u8).add(header_size) as *mut u16;
        std::ptr::copy_nonoverlapping(wide_paths.as_ptr(), data_dst, wide_paths.len());

        let _ = GlobalUnlock(hglobal);

        // Open clipboard, empty it, set our data.
        OpenClipboard(None)
            .map_err(|e| format!("OpenClipboard failed: {}", e))?;

        let _ = EmptyClipboard();

        // CF_HDROP = 15
        let handle = windows::Win32::Foundation::HANDLE(hglobal.0);
        let result = SetClipboardData(15, Some(handle));
        let _ = CloseClipboard();

        if result.is_err() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err("SetClipboardData failed".to_string());
        }

        if should_uninitialize {
            CoUninitialize();
        }
    }

    Ok(())
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Mirrors the Win32 DROPFILES structure layout (20 bytes).
#[repr(C, packed)]
struct DropfilesHeader {
    p_files: u32,
    pt_x: i32,
    pt_y: i32,
    f_nc: i32,
    f_wide: i32,
}
