use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn get_lean_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean")
}

fn get_supercompiler_dir() -> PathBuf {
    get_lean_dir().join("Supercompiler")
}

fn visit_lean_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_lean_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "lean") {
                files.push(path);
            }
        }
    }
}

#[test]
fn test_lean_build_succeeds() {
    let lean_dir = get_lean_dir();
    assert!(lean_dir.exists(), "lean/ directory must exist");

    let status = Command::new("lake")
        .arg("build")
        .current_dir(&lean_dir)
        .status();

    match status {
        Ok(s) => assert!(
            s.success(),
            "lake build failed in lean/ with status: {:?}",
            s
        ),
        Err(e) => panic!("Failed to invoke lake: {}", e),
    }
}

#[test]
fn test_lean_no_sorry() {
    let supercompiler_dir = get_supercompiler_dir();
    assert!(
        supercompiler_dir.exists(),
        "lean/Supercompiler directory must exist"
    );

    let mut lean_files = Vec::new();
    visit_lean_files(&supercompiler_dir, &mut lean_files);
    assert!(
        !lean_files.is_empty(),
        "Expected .lean files in Supercompiler directory"
    );

    for file in lean_files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        assert!(
            !content.contains("sorry"),
            "Found 'sorry' in {}",
            file.display()
        );
    }
}

#[test]
fn test_lean_no_unproven_axiom() {
    let supercompiler_dir = get_supercompiler_dir();
    let mut lean_files = Vec::new();
    visit_lean_files(&supercompiler_dir, &mut lean_files);
    assert!(
        !lean_files.is_empty(),
        "Expected .lean files in Supercompiler directory"
    );

    for file in lean_files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("axiom ") && !trimmed.starts_with("axiom kruskal_tree_theorem") {
                panic!("Found unproven axiom in {}: '{}'", file.display(), trimmed);
            }
        }
    }
}

#[test]
fn test_distillation_theorem_present() {
    let dist_file = get_supercompiler_dir().join("Distillation.lean");
    assert!(dist_file.exists(), "Distillation.lean must exist");

    let content = fs::read_to_string(&dist_file).expect("Failed to read Distillation.lean");
    assert!(
        content.contains("theorem distillation_preserves_semantics"),
        "Distillation.lean must contain 'theorem distillation_preserves_semantics'"
    );
}

#[test]
fn test_end_to_end_theorem_present() {
    let main_file = get_supercompiler_dir().join("Main.lean");
    assert!(main_file.exists(), "Main.lean must exist");

    let content = fs::read_to_string(&main_file).expect("Failed to read Main.lean");
    assert!(
        content.contains("theorem supercompiler_sound"),
        "Main.lean must contain 'theorem supercompiler_sound'"
    );
}
