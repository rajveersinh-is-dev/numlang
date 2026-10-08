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
fn test_lake_build_zero_errors() {
    let lean_dir = get_lean_dir();
    assert!(lean_dir.exists(), "lean directory must exist");

    let status = Command::new("lake")
        .arg("build")
        .current_dir(&lean_dir)
        .status()
        .expect("Failed to execute lake build");

    assert!(
        status.success(),
        "lake build failed with status: {:?}",
        status
    );
}

#[test]
fn test_no_sorry_in_entire_lean_codebase() {
    let supercompiler_dir = get_supercompiler_dir();
    let mut lean_files = Vec::new();
    visit_lean_files(&supercompiler_dir, &mut lean_files);
    assert!(!lean_files.is_empty(), "No lean files found");

    for file in lean_files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        assert!(
            !content.contains("sorry"),
            "Found 'sorry' in Lean file: {}",
            file.display()
        );
    }
}

#[test]
fn test_no_unproven_axioms_in_supercompiler() {
    let supercompiler_dir = get_supercompiler_dir();
    let mut lean_files = Vec::new();
    visit_lean_files(&supercompiler_dir, &mut lean_files);

    for file in lean_files {
        let content = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("axiom ") {
                panic!(
                    "Found unexpected unproven axiom in {}: {}",
                    file.display(),
                    trimmed
                );
            }
        }
    }
}

#[test]
fn test_evaluates_is_constructive_with_stepstar() {
    let semantics_path = get_supercompiler_dir().join("Semantics.lean");
    let content = fs::read_to_string(&semantics_path).expect("Failed to read Semantics.lean");

    // Must define StepStar
    assert!(
        content.contains("inductive StepStar"),
        "Semantics.lean must define StepStar"
    );

    // Must define TerminatesWith
    assert!(
        content.contains("inductive TerminatesWith"),
        "Semantics.lean must define TerminatesWith"
    );

    // Evaluates must constructively combine StepStar and TerminatesWith
    assert!(
        content.contains("StepStar fn (initState fn args) s_final ∧ TerminatesWith fn s_final res"),
        "Evaluates must be defined constructively via StepStar and TerminatesWith"
    );

    // Must NOT contain the old vacuous base case that only checked lookup env retVar
    assert!(
        !content.contains("lookup env retVar = some v →\n      Evaluates fn env v"),
        "Semantics.lean must not contain vacuous Evaluates base constructor"
    );
}

#[test]
fn test_no_circular_tautology_premises_in_theorems() {
    let dir = get_supercompiler_dir();
    let main_content = fs::read_to_string(dir.join("Main.lean")).expect("Failed to read Main.lean");
    let dist_content = fs::read_to_string(dir.join("Distillation.lean"))
        .expect("Failed to read Distillation.lean");
    let comp_content =
        fs::read_to_string(dir.join("Compaction.lean")).expect("Failed to read Compaction.lean");
    let pres_content = fs::read_to_string(dir.join("Preservation.lean"))
        .expect("Failed to read Preservation.lean");

    // Ensure SupercompilerProduces does NOT require SemanticEquivalent in constructor
    assert!(
        !main_content.contains("SemanticEquivalent f1 f2)\n      SupercompilerProduces"),
        "Main.lean must not take SemanticEquivalent as premise in SupercompilerProduces"
    );

    // Ensure FoldStep does NOT require SemanticEquivalent in constructor
    assert!(
        !dist_content.contains("SemanticEquivalent f1 f2 →\n      FoldStep f1 f2"),
        "Distillation.lean must not take SemanticEquivalent as constructor premise"
    );

    // Ensure NoopRemoval does NOT require SemanticEquivalent in constructor
    assert!(
        !comp_content.contains("SemanticEquivalent f1 f2 →\n      NoopRemoval f1 f2"),
        "Compaction.lean must not take SemanticEquivalent as constructor premise"
    );

    // Ensure DriveStep does NOT require SemanticEquivalent in constructor
    assert!(
        !pres_content.contains("SemanticEquivalent fn fn' →\n      DriveStep fn fn'"),
        "Preservation.lean must not take SemanticEquivalent as constructor premise"
    );

    // Ensure theorems exist and have non-vacuous signatures
    assert!(
        main_content.contains("theorem supercompiler_sound"),
        "Main.lean must contain supercompiler_sound"
    );
    assert!(
        dist_content.contains("theorem distillation_preserves_semantics"),
        "Distillation.lean must contain distillation_preserves_semantics"
    );
    assert!(
        comp_content.contains("theorem noop_removal_preserves_semantics"),
        "Compaction.lean must contain noop_removal_preserves_semantics"
    );
    assert!(
        pres_content.contains("theorem driving_preserves_semantics"),
        "Preservation.lean must contain driving_preserves_semantics"
    );
}
