use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum LinkerError {
    #[error("Linker executable not found. Ensure rust-lld.exe or MSVC link.exe is available.")]
    LinkerNotFound,

    #[error("Windows SDK / kernel32.lib not found.")]
    WindowsSdkNotFound,

    #[error("Linking failed: {message}")]
    LinkFailed { message: String },
}

pub fn find_rust_lld() -> Option<PathBuf> {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .ok()?;

    if output.status.success() {
        let sysroot_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let sysroot = PathBuf::from(sysroot_str);
        let lld_path = sysroot
            .join("lib")
            .join("rustlib")
            .join("x86_64-pc-windows-msvc")
            .join("bin")
            .join("rust-lld.exe");

        if lld_path.exists() {
            return Some(lld_path);
        }
    }
    None
}

pub fn find_windows_sdk_lib_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let base = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Lib");
    if let Ok(entries) = std::fs::read_dir(&base) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();

        // Sort to get latest version
        versions.sort();
        if let Some(latest) = versions.last() {
            let um_x64 = latest.join("um").join("x64");
            let ucrt_x64 = latest.join("ucrt").join("x64");
            if um_x64.exists() {
                dirs.push(um_x64);
            }
            if ucrt_x64.exists() {
                dirs.push(ucrt_x64);
            }
        }
    }
    dirs
}

static ENTRY_BENCH_OBJ: &[u8] = include_bytes!("entry_bench.obj");

pub fn link_executable(obj_path: &Path, exe_path: &Path) -> Result<(), LinkerError> {
    let lld = find_rust_lld().ok_or(LinkerError::LinkerNotFound)?;
    let sdk_dirs = find_windows_sdk_lib_dirs();
    if sdk_dirs.is_empty() {
        return Err(LinkerError::WindowsSdkNotFound);
    }

    let bench_mode = std::env::var("NUMLANG_BENCH").is_ok();
    let bench_obj_path = if bench_mode {
        let p = obj_path.with_file_name(format!("entry_bench_{}.obj", std::process::id()));
        let _ = std::fs::write(&p, ENTRY_BENCH_OBJ);
        Some(p)
    } else {
        None
    };

    let mut cmd = Command::new(&lld);
    cmd.arg("-flavor").arg("link");
    cmd.arg("/nologo");
    cmd.arg("/subsystem:console");
    cmd.arg("/entry:mainCRTStartup");
    cmd.arg("/opt:ref");
    cmd.arg("/opt:icf");
    cmd.arg("/incremental:no");
    if let Some(ref bp) = bench_obj_path {
        cmd.arg(bp);
    }
    cmd.arg(obj_path);
    cmd.arg(format!("/out:{}", exe_path.display()));

    for dir in sdk_dirs {
        cmd.arg(format!("/libpath:{}", dir.display()));
    }

    cmd.arg("kernel32.lib");

    let output = cmd
        .output()
        .map_err(|e| LinkerError::LinkFailed {
            message: e.to_string(),
        })?;

    if let Some(bp) = bench_obj_path {
        let _ = std::fs::remove_file(bp);
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(LinkerError::LinkFailed {
            message: format!("{}\n{}", stderr, stdout),
        });
    }

    Ok(())
}
