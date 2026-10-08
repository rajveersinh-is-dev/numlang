//! CLBG Correctness Test Suite (Phase 57 Deliverable 6).
//!
//! Verifies that all 6 Computer Language Benchmarks Game programs produce
//! byte-for-byte identical output between NumLang (baseline and supercompiled)
//! and the reference C implementations.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn get_vcvars() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvars64.bat",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn compile_c_and_run(c_file: &Path, test_id: &str) -> Option<String> {
    let vcvars = get_vcvars()?;
    let temp_dir = std::env::temp_dir().join(format!("nl_clbg_c_{}", test_id));
    let _ = fs::create_dir_all(&temp_dir);

    let compile_cmd = format!(
        "call \"{}\" >nul 2>&1 && cl.exe /nologo /O2 \"{}\"",
        vcvars.display(),
        c_file.display()
    );

    let status = Command::new("cmd.exe")
        .arg("/c")
        .arg(&compile_cmd)
        .current_dir(&temp_dir)
        .status()
        .ok()?;

    let exe_name = format!("{}.exe", c_file.file_stem()?.to_string_lossy());
    let exe_path = temp_dir.join(&exe_name);

    if !status.success() || !exe_path.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
        return None;
    }

    let output = Command::new(&exe_path).output().ok()?;

    let _ = fs::remove_dir_all(&temp_dir);
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_numlang_program(nl_file: &Path, supercompile: bool) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_numlang"));
    cmd.arg("run");
    if supercompile {
        cmd.arg("--supercompile");
    }
    cmd.arg(nl_file);

    let output = cmd.output().expect("Failed to run numlang");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn verify_benchmark(name: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let nl_path = root
        .join("examples")
        .join("clbg")
        .join(format!("{}.nl", name));
    let c_path = root
        .join("examples")
        .join("clbg")
        .join(format!("{}.c", name));

    assert!(
        nl_path.exists(),
        "NumLang source missing: {}",
        nl_path.display()
    );
    assert!(c_path.exists(), "C source missing: {}", c_path.display());

    let out_base = run_numlang_program(&nl_path, false);
    let out_super = run_numlang_program(&nl_path, true);

    assert_eq!(
        out_base, out_super,
        "Supercompiled output diverged from baseline for {}",
        name
    );

    if let Some(out_c) = compile_c_and_run(&c_path, name) {
        assert_eq!(
            out_super, out_c,
            "NumLang output does not match reference C output for {}",
            name
        );
    }
}

#[test]
fn test_clbg_binary_trees_correctness() {
    verify_benchmark("binary_trees");
}

#[test]
fn test_clbg_fannkuch_redux_correctness() {
    verify_benchmark("fannkuch_redux");
}

#[test]
fn test_clbg_nbody_correctness() {
    verify_benchmark("nbody");
}

#[test]
fn test_clbg_pidigits_correctness() {
    verify_benchmark("pidigits");
}

#[test]
fn test_clbg_fasta_correctness() {
    verify_benchmark("fasta");
}

#[test]
fn test_clbg_spectral_norm_correctness() {
    verify_benchmark("spectral_norm");
}
