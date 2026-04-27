use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_RUNTIME_DLLS: &[&str] = &[
    "libx265.dll",
    "heif.dll",
    "libde265.dll",
];

fn main() {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
    println!("cargo:rerun-if-env-changed=PROFILE");

    #[cfg(windows)]
    {
        if let Err(err) = copy_vcpkg_runtime_dlls() {
            println!("cargo:warning=DLL copy step skipped: {}", err);
        }
    }
}

#[cfg(windows)]
fn copy_vcpkg_runtime_dlls() -> Result<(), String> {
    let profile = env::var("PROFILE").map_err(|_| "PROFILE is not set".to_string())?;

    let target_root = if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        PathBuf::from(target_dir)
    } else {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_string())?;
        Path::new(&manifest_dir).join("target")
    };

    let out_dir = target_root.join(&profile);
    fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create output dir {}: {}", out_dir.display(), e))?;

    let vcpkg_root = env::var("VCPKG_ROOT").map_err(|_| {
        "VCPKG_ROOT is not set. Set it globally to copy runtime DLLs automatically.".to_string()
    })?;

    let vcpkg_bin = Path::new(&vcpkg_root)
        .join("installed")
        .join("x64-windows")
        .join("bin");

    if !vcpkg_bin.exists() {
        return Err(format!("Vcpkg runtime folder not found: {}", vcpkg_bin.display()));
    }

    let mut copied_count = 0usize;

    for dll_name in REQUIRED_RUNTIME_DLLS {
        let source = vcpkg_bin.join(dll_name);
        if !source.exists() {
            return Err(format!(
                "Required runtime DLL not found in {}: {}",
                vcpkg_bin.display(),
                dll_name
            ));
        }

        let dest = out_dir.join(dll_name);
        fs::copy(&source, &dest).map_err(|e| {
            format!(
                "Failed copying {} to {}: {}",
                source.display(),
                dest.display(),
                e
            )
        })?;
        copied_count += 1;
    }

    if copied_count == 0 {
        return Err(format!("No DLL files found in {}", vcpkg_bin.display()));
    }

    println!(
        "cargo:warning=Copied {} runtime DLL(s) to {}",
        copied_count,
        out_dir.display()
    );

    Ok(())
}
