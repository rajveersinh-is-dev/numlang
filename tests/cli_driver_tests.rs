use std::fs;
use std::process::Command;

#[test]
fn test_cli_subcommand_check() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli_subcommands");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("check_test.nl");
    fs::write(
        &src_file,
        "fn main() -> i64 { let x: i64 = 100; return x + 50; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("check")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("type check passed"));
}

#[test]
fn test_cli_subcommand_build_with_output() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli_subcommands");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("build_test.nl");
    let out_exe = test_dir.join("custom_calc.exe");
    if out_exe.exists() {
        let _ = fs::remove_file(&out_exe);
    }

    fs::write(
        &src_file,
        "fn main() -> i64 { let a: i64 = 20; let b: i64 = 22; return a + b; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&src_file)
        .arg("-o")
        .arg(&out_exe)
        .output()
        .expect("Failed to execute numlang build");

    assert!(output.status.success());
    assert!(out_exe.exists(), "Executable must be created at custom path");

    // Execute generated binary
    let exec_out = Command::new(&out_exe)
        .status()
        .expect("Failed to execute compiled binary");
    assert_eq!(exec_out.code(), Some(42));
}

#[test]
fn test_cli_subcommand_build_default_output() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli_subcommands");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("default_build.nl");
    let expected_exe = test_dir.join("default_build.exe");
    if expected_exe.exists() {
        let _ = fs::remove_file(&expected_exe);
    }

    fs::write(
        &src_file,
        "fn main() -> i64 { return 17; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("build")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang build with default output");

    assert!(output.status.success());
    assert!(
        expected_exe.exists(),
        "Executable must be created with default .exe stem"
    );

    let exec_out = Command::new(&expected_exe)
        .status()
        .expect("Failed to execute compiled binary");
    assert_eq!(exec_out.code(), Some(17));
}

#[test]
fn test_cli_subcommand_run() {
    let test_dir = std::env::temp_dir().join("numlang_test_cli_subcommands");
    fs::create_dir_all(&test_dir).unwrap();
    let src_file = test_dir.join("run_test.nl");

    fs::write(
        &src_file,
        "fn main() -> i64 { let mut i: i64 = 0; let mut acc: i64 = 0; while i < 5 { acc = acc + 10; i = i + 1; } return acc; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_numlang"))
        .arg("run")
        .arg(&src_file)
        .output()
        .expect("Failed to execute numlang run");

    // Exit code should match main return value 50
    assert_eq!(output.status.code(), Some(50));
}
