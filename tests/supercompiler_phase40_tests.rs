use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn get_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_paper_sections_exist() {
    let root = get_repo_root();
    let sections_dir = root.join("paper").join("sections");
    assert!(
        sections_dir.is_dir(),
        "Expected paper/sections directory to exist"
    );

    let required_sections = [
        "01_introduction.tex",
        "02_background.tex",
        "03_system.tex",
        "04_termination.tex",
        "05_optimization.tex",
        "06_correctness.tex",
        "07_evaluation.tex",
        "08_related.tex",
        "09_conclusion.tex",
    ];

    for sec in &required_sections {
        let path = sections_dir.join(sec);
        assert!(path.is_file(), "Expected section file {:?} to exist", path);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", path, e));
        assert!(
            !content.trim().is_empty(),
            "Expected section file {:?} to not be empty",
            path
        );
    }
}

#[test]
#[ignore = "Skip when pdflatex is not installed"]
fn test_paper_compiles_pdflatex() {
    let which_output = Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg("pdflatex")
        .output();
    let is_installed = which_output.map(|o| o.status.success()).unwrap_or(false);
    if !is_installed {
        println!("pdflatex not found on host, skipping test.");
        return;
    }

    let root = get_repo_root();
    let paper_dir = root.join("paper");

    let status = Command::new("pdflatex")
        .current_dir(&paper_dir)
        .arg("-interaction=nonstopmode")
        .arg("main.tex")
        .status()
        .expect("Failed to execute pdflatex");

    assert!(
        status.success(),
        "pdflatex compilation of paper/main.tex failed with status: {:?}",
        status
    );
}

#[test]
#[ignore = "Skip when docker is not installed"]
fn test_dockerfile_exists_and_is_valid() {
    let root = get_repo_root();
    let dockerfile = root.join("docker").join("Dockerfile");
    assert!(
        dockerfile.is_file(),
        "Expected docker/Dockerfile to exist at {:?}",
        dockerfile
    );

    let which_output = Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg("docker")
        .output();
    let is_installed = which_output.map(|o| o.status.success()).unwrap_or(false);
    if !is_installed {
        println!("docker not found on host, skipping docker build check.");
        return;
    }

    let output = Command::new("docker")
        .current_dir(&root)
        .args([
            "build",
            "--dry-run",
            "-f",
            "docker/Dockerfile",
            "-t",
            "numlang-artifact",
            ".",
        ])
        .output()
        .expect("Failed to invoke docker build --dry-run");

    assert!(
        output.status.success(),
        "Docker dry-run validation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_makefile_targets_present() {
    let root = get_repo_root();
    let makefile_path = root.join("Makefile");
    assert!(
        makefile_path.is_file(),
        "Expected Makefile to exist at {:?}",
        makefile_path
    );

    let content = fs::read_to_string(&makefile_path).expect("Read Makefile");
    let required_targets = ["build", "test", "reproduce", "paper", "artifact-docker"];

    for target in &required_targets {
        let pattern = format!("{}:", target);
        assert!(
            content.contains(&pattern),
            "Expected Makefile to contain target pattern {:?}, but it was missing",
            pattern
        );
    }
}

#[test]
fn test_rebuttal_objections_complete() {
    let root = get_repo_root();
    let rebuttal_path = if root
        .join("docs/archive/rebuttal/likely_objections.md")
        .is_file()
    {
        root.join("docs/archive/rebuttal/likely_objections.md")
    } else {
        root.join("rebuttal/likely_objections.md")
    };
    assert!(
        rebuttal_path.is_file(),
        "Expected rebuttal objections file to exist at {:?}",
        rebuttal_path
    );

    let content = fs::read_to_string(&rebuttal_path).expect("Read likely_objections.md");
    let required_keywords = ["cherry-picked", "Lean proofs", "parallel", "GHC", "cache"];

    for kw in &required_keywords {
        assert!(
            content.contains(kw),
            "Expected likely_objections.md to contain keyword {:?}, but it was missing",
            kw
        );
    }
}
