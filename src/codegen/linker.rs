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
    } else {
        find_msvc_link().map(WindowsLinker::MsvcLink)
    }
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

static ENTRY_BENCH_C: &str = include_str!("entry_bench.c");
static ENTRY_BENCH_RS: &str = r#"#![no_std]
#![no_main]

#[link(name = "kernel32")]
extern "system" {
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
    fn QueryPerformanceCounter(lpPerformanceCount: *mut i64) -> i32;
    fn GetStdHandle(nStdHandle: u32) -> *mut core::ffi::c_void;
    fn WriteFile(
        hFile: *mut core::ffi::c_void,
        lpBuffer: *const u8,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut core::ffi::c_void,
    ) -> i32;
    fn ExitProcess(uExitCode: u32) -> !;
    fn GetCommandLineA() -> *const u8;
}

extern "C" {
    fn main() -> i64;
}

#[no_mangle]
pub unsafe extern "C" fn __nl_read_i64() -> i64 {
    let mut p = GetCommandLineA();
    if p.is_null() { return 0; }
    if *p == b'"' {
        p = p.add(1);
        while *p != 0 && *p != b'"' { p = p.add(1); }
        if *p == b'"' { p = p.add(1); }
    } else {
        while *p != 0 && *p != b' ' && *p != b'\t' { p = p.add(1); }
    }
    while *p == b' ' || *p == b'\t' { p = p.add(1); }
    let mut neg = false;
    if *p == b'-' { neg = true; p = p.add(1); }
    let mut v: i64 = 0;
    while *p >= b'0' && *p <= b'9' {
        v = v.wrapping_mul(10).wrapping_add((*p - b'0') as i64);
        p = p.add(1);
    }
    if neg { -v } else { v }
}

#[no_mangle]
pub unsafe extern "C" fn mainCRTStartup() -> ! {
    let mut freq: i64 = 0;
    let mut t0: i64 = 0;
    let mut t1: i64 = 0;
    QueryPerformanceFrequency(&mut freq);
    QueryPerformanceCounter(&mut t0);
    let ret = main();
    QueryPerformanceCounter(&mut t1);

    let ns = if freq > 0 {
        ((t1.wrapping_sub(t0)).wrapping_mul(1_000_000_000)) / freq
    } else {
        0
    };

    let mut buf = [0u8; 64];
    let prefix = b"COMPUTE_NS: ";
    core::ptr::copy_nonoverlapping(prefix.as_ptr(), buf.as_mut_ptr(), 12);
    let mut len = 12;

    let mut digits = [0u8; 32];
    let mut dlen = 0;
    let mut temp = ns;
    while temp > 0 {
        *digits.as_mut_ptr().add(dlen) = b'0' + (temp % 10) as u8;
        dlen += 1;
        temp /= 10;
    }
    if dlen == 0 {
        *digits.as_mut_ptr() = b'0';
        dlen = 1;
    }
    for i in (0..dlen).rev() {
        *buf.as_mut_ptr().add(len) = *digits.as_ptr().add(i);
        len += 1;
    }
    *buf.as_mut_ptr().add(len) = b'\n';
    len += 1;

    let mut written: u32 = 0;
    WriteFile(
        GetStdHandle(0xFFFFFFF5),
        buf.as_ptr(),
        len as u32,
        &mut written,
        core::ptr::null_mut(),
    );
    ExitProcess(ret as u32);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
"#;

fn get_runtime_stub_obj() -> Option<&'static Path> {
    static STUB_PATH: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    STUB_PATH
        .get_or_init(|| {
            let temp_dir = std::env::temp_dir();
            let p = temp_dir.join(format!("numlang_runtime_stub_{}.obj", std::process::id()));
            let rs_path = temp_dir.join(format!("numlang_runtime_stub_{}.rs", std::process::id()));
            if std::fs::write(
                &rs_path,
                "#![no_std]\n#![no_main]\n#[no_mangle]\npub extern \"C\" fn __nl_read_i64() -> i64 { 0 }\n#[panic_handler]\nfn panic(_: &core::panic::PanicInfo) -> ! { loop {} }\n",
            )
            .is_err()
            {
                return None;
            }
            let ok = Command::new("rustc")
                .args([
                    "-O",
                    "-C",
                    "panic=abort",
                    "--crate-type=staticlib",
                    "--emit=obj",
                ])
                .arg(&rs_path)
                .arg("-o")
                .arg(&p)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            let _ = std::fs::remove_file(&rs_path);
            if ok && p.exists() {
                Some(p)
            } else {
                None
            }
        })
        .as_deref()
}

fn get_entry_bench_obj() -> Option<&'static Path> {
    static BENCH_PATH: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    BENCH_PATH
        .get_or_init(|| {
            let temp_dir = std::env::temp_dir();
            let p = temp_dir.join(format!("numlang_entry_bench_{}.obj", std::process::id()));
            let c_path = temp_dir.join(format!("numlang_entry_bench_{}.c", std::process::id()));
            if std::fs::write(&c_path, ENTRY_BENCH_C).is_ok() {
                let ok = Command::new("clang")
                    .args(["-c", "-O2"])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&p)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                let _ = std::fs::remove_file(&c_path);
                if ok && p.exists() {
                    return Some(p);
                }
            }

            let rs_path = temp_dir.join(format!("numlang_entry_bench_{}.rs", std::process::id()));
            if std::fs::write(&rs_path, ENTRY_BENCH_RS).is_ok() {
                let rustc_ok = Command::new("rustc")
                    .args([
                        "-O",
                        "-C",
                        "panic=abort",
                        "--crate-type=staticlib",
                        "--emit=obj",
                    ])
                    .arg(&rs_path)
                    .arg("-o")
                    .arg(&p)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                let _ = std::fs::remove_file(&rs_path);
                if rustc_ok && p.exists() {
                    return Some(p);
                }
            }

            None
        })
        .as_deref()
}

pub fn link_windows(obj_path: &Path, exe_path: &Path) -> Result<(), LinkerError> {
    let linker = find_windows_linker().ok_or(LinkerError::LinkerNotFound)?;
    let sdk_dirs = find_windows_sdk_lib_dirs();
    if sdk_dirs.is_empty() {
        return Err(LinkerError::WindowsSdkNotFound);
    }

    let bench_mode = std::env::var("NUMLANG_BENCH").is_ok();
    let runtime_obj_path = if bench_mode {
        get_entry_bench_obj()
    } else {
        get_runtime_stub_obj()
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
    if let Some(ro) = runtime_obj_path {
        cmd.arg(ro);
    }
    cmd.arg(obj_path);
    cmd.arg(format!("/out:{}", exe_path.display()));

    for dir in sdk_dirs {
        cmd.arg(format!("/libpath:{}", dir.display()));
    }

    cmd.arg("kernel32.lib");
    cmd.arg("ucrt.lib");

    let output = cmd.output().map_err(|e| LinkerError::LinkFailed {
        message: e.to_string(),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(LinkerError::LinkFailed {
            message: format!("{}\\n{}", stderr, stdout),
        });
    }

    Ok(())
}

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
    if let Some(ref bp) = bench_c_path {
        cmd.arg(bp);
    }
    cmd.arg(obj_path).arg("-o").arg(exe_path).arg("-lm");

    #[cfg(target_os = "linux")]
    cmd.arg("-no-pie");

    let output = cmd.output().map_err(|e| LinkerError::LinkFailed {
        message: e.to_string(),
    })?;

    if let Some(ref bp) = bench_c_path {
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
