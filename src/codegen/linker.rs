use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum LinkerError {
    #[error("Linker executable not found. Ensure rust-lld.exe, MSVC link.exe, or cc/clang is available.")]
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

pub fn find_msvc_link() -> Option<PathBuf> {
    // 1. Check PATH directly for link.exe
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("link.exe");
            if candidate.is_file() {
                // Verify it is the Microsoft linker, not GNU coreutils link
                if let Ok(output) = Command::new(&candidate).arg("/?").output() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    if text.contains("Microsoft") && text.contains("Incremental Linker") {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    // 2. Check VCINSTALLDIR if set
    if let Ok(vc_dir) = std::env::var("VCINSTALLDIR") {
        let candidate = PathBuf::from(vc_dir)
            .join("bin")
            .join("Hostx64")
            .join("x64")
            .join("link.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

#[derive(Debug, Clone)]
pub enum WindowsLinker {
    RustLld(PathBuf),
    MsvcLink(PathBuf),
}

pub fn find_windows_linker() -> Option<WindowsLinker> {
    if let Some(lld) = find_rust_lld() {
        Some(WindowsLinker::RustLld(lld))
    } else { find_msvc_link().map(WindowsLinker::MsvcLink) }
}

pub fn find_windows_sdk_lib_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Inspect LIB environment variable
    if let Ok(lib_env) = std::env::var("LIB") {
        for part in lib_env.split(';') {
            let p = PathBuf::from(part.trim());
            if p.exists() && p.is_dir() && !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }

    // 2. Inspect WindowsSdkDir and WindowsSDKLibVersion
    let sdk_dir = std::env::var("WindowsSdkDir").ok().map(PathBuf::from);
    let sdk_ver = std::env::var("WindowsSDKLibVersion")
        .ok()
        .map(|s| s.trim_end_matches('\\').to_string());

    if let (Some(sdk_dir), Some(sdk_ver)) = (&sdk_dir, &sdk_ver) {
        let um_x64 = sdk_dir.join("Lib").join(sdk_ver).join("um").join("x64");
        let ucrt_x64 = sdk_dir.join("Lib").join(sdk_ver).join("ucrt").join("x64");
        if um_x64.exists() && !dirs.contains(&um_x64) {
            dirs.push(um_x64);
        }
        if ucrt_x64.exists() && !dirs.contains(&ucrt_x64) {
            dirs.push(ucrt_x64);
        }
    } else if let Some(sdk_dir) = &sdk_dir {
        let lib_base = sdk_dir.join("Lib");
        scan_and_append_sdk_libs(&lib_base, &mut dirs);
    }

    // 3. Fallback filesystem scanning across standard Windows Kits locations
    if dirs.is_empty() {
        let candidates = [
            PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Lib"),
            PathBuf::from(r"C:\Program Files\Windows Kits\10\Lib"),
            PathBuf::from(r"D:\Program Files (x86)\Windows Kits\10\Lib"),
            PathBuf::from(r"D:\Program Files\Windows Kits\10\Lib"),
        ];

        for base in &candidates {
            if base.exists() {
                scan_and_append_sdk_libs(base, &mut dirs);
                if !dirs.is_empty() {
                    break;
                }
            }
        }
    }

    dirs
}

fn scan_and_append_sdk_libs(base: &Path, dirs: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(base) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();

        versions.sort();
        if let Some(latest) = versions.last() {
            let um_x64 = latest.join("um").join("x64");
            let ucrt_x64 = latest.join("ucrt").join("x64");
            if um_x64.exists() && !dirs.contains(&um_x64) {
                dirs.push(um_x64);
            }
            if ucrt_x64.exists() && !dirs.contains(&ucrt_x64) {
                dirs.push(ucrt_x64);
            }
        }
    }
}

static ENTRY_BENCH_OBJ: &[u8] = include_bytes!("entry_bench.obj");

pub fn link_windows(obj_path: &Path, exe_path: &Path) -> Result<(), LinkerError> {
    let linker = find_windows_linker().ok_or(LinkerError::LinkerNotFound)?;
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

    let mut cmd = match linker {
        WindowsLinker::RustLld(ref path) => {
            let mut c = Command::new(path);
            c.arg("-flavor").arg("link");
            c
        }
        WindowsLinker::MsvcLink(ref path) => Command::new(path),
    };

    cmd.arg("/nologo");
    cmd.arg("/subsystem:console");
    cmd.arg("/entry:mainCRTStartup");
    cmd.arg("/opt:ref");
    cmd.arg("/opt:icf");
    cmd.arg("/incremental:no");
    cmd.arg("/STACK:16777216,1048576");
    if let Some(ref bp) = bench_obj_path {
        cmd.arg(bp);
    }
    cmd.arg(obj_path);
    cmd.arg(format!("/out:{}", exe_path.display()));

    for dir in sdk_dirs {
        cmd.arg(format!("/libpath:{}", dir.display()));
    }

    cmd.arg("kernel32.lib");
    cmd.arg("ucrt.lib");

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
            message: format!("{}\\n{}", stderr, stdout),
        });
    }

    Ok(())
}

static ENTRY_BENCH_C: &str = include_str!("entry_bench.c");

pub fn link_unix(obj_path: &Path, exe_path: &Path) -> Result<(), LinkerError> {
    let compiler = ["cc", "clang", "gcc"]
        .iter()
        .find(|&&c| Command::new(c).arg("--version").output().is_ok())
        .copied()
        .ok_or(LinkerError::LinkerNotFound)?;

    let bench_mode = std::env::var("NUMLANG_BENCH").is_ok();
    let bench_c_path = if bench_mode {
        let p = obj_path.with_file_name(format!("entry_bench_{}.c", std::process::id()));
        let _ = std::fs::write(&p, ENTRY_BENCH_C);
        Some(p)
    } else {
        None
    };

    let mut cmd = Command::new(compiler);
    cmd.arg(obj_path);
    if let Some(ref bp) = bench_c_path {
        cmd.arg(bp);
    }
    cmd.arg("-o")
        .arg(exe_path)
        .arg("-lm")
        .arg("-no-pie");

    let output = cmd
        .output()
        .map_err(|e| LinkerError::LinkFailed {
            message: e.to_string(),
        })?;

    if let Some(ref bp) = bench_c_path {
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

pub fn link_executable(obj_path: &Path, exe_path: &Path) -> Result<(), LinkerError> {
    #[cfg(target_os = "windows")]
    {
        link_windows(obj_path, exe_path)
    }

    #[cfg(not(target_os = "windows"))]
    {
        link_unix(obj_path, exe_path)
    }
}
