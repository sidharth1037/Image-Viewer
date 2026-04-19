#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{DROPEFFECT_COPY, IDropSource};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{ILClone, ILFindLastID, ILRemoveLastID, SHCreateDataObject, SHDoDragDrop, SHParseDisplayName};
use windows::core::PCWSTR;

struct PidlGuard(*mut ITEMIDLIST);

impl Drop for PidlGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CoTaskMemFree(Some(self.0.cast()));
            }
        }
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn error_with_code(prefix: &str, error: windows::core::Error) -> String {
    format!("{}: {} ({:#010X})", prefix, error, error.code().0 as u32)
}

pub fn begin_file_drag(path: &Path) -> Result<(), String> {
    let absolute = absolute_path(path);
    let wide_path: Vec<u16> = absolute
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hr_init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr_init.is_err() && hr_init != RPC_E_CHANGED_MODE {
            return Err(format!(
                "Failed to initialize COM for drag-and-drop ({:#010X})",
                hr_init.0 as u32
            ));
        }
        let should_uninitialize = hr_init.is_ok();

        let mut file_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        SHParseDisplayName(
            PCWSTR(wide_path.as_ptr()),
            None,
            &mut file_pidl,
            0,
            None,
        )
        .map_err(|e| error_with_code("Failed to parse file path for drag", e))?;

        if file_pidl.is_null() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err("Failed to create file PIDL".to_string());
        }

        let _file_guard = PidlGuard(file_pidl);

        let folder_pidl = ILClone(file_pidl);
        if folder_pidl.is_null() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err("Failed to clone folder PIDL".to_string());
        }
        let _folder_guard = PidlGuard(folder_pidl);

        if !ILRemoveLastID(Some(folder_pidl)).as_bool() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err("Failed to derive parent folder PIDL".to_string());
        }

        let child = ILFindLastID(file_pidl);
        if child.is_null() {
            if should_uninitialize {
                CoUninitialize();
            }
            return Err("Failed to derive child item PIDL".to_string());
        }

        let children = [child as *const ITEMIDLIST];
        let data_object: IDataObject = SHCreateDataObject(
            Some(folder_pidl as *const ITEMIDLIST),
            Some(&children),
            None::<&IDataObject>,
        )
        .map_err(|e| error_with_code("Failed to create shell data object", e))?;

        SHDoDragDrop(
            None,
            &data_object,
            Option::<&IDropSource>::None,
            DROPEFFECT_COPY,
        )
        .map_err(|e| error_with_code("Drag-and-drop operation failed", e))?;

        if should_uninitialize {
            CoUninitialize();
        }
    }

    Ok(())
}
