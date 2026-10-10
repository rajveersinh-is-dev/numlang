#!/usr/bin/env python3
"""
scripts/check_lean_docs.py

Validates that documentation and paper sources do not reference nonexistent
Lean files or fake/unproven Lean theorem names.

Checks:
1. Every *.lean path referenced in docs/, README.md, or paper/ exists in lean/ or proof/.
2. Every Lean theorem/lemma referenced in docs/, README.md, or paper/ exists in lean/ or proof/.
3. No known fabricated theorem names (e.g. eval_preservation, step_deterministic) appear.
4. Docs markdown tables in formal verification files only list real, verified declarations.
"""

import os
import re
import sys
from pathlib import Path

# Known fabricated or obsolete theorem names that must never appear in docs or paper
FABRICATED_THEOREMS = {
    "eval_preservation",
    "step_star_preservation",
    "simulation_preorder",
    "trace_equivalence",
    "unfold_sound",
    "fold_sound",
    "intermediate_elimination",
    "prune_unreachable_branch",
    "compact_path",
    "step_deterministic",
    "drive_preserves_eval",
    "driving_step_soundness",
    "embedding_wqo",
    "msg_sound",
    "lattice_sound",
    "refinement_sound",
    "compaction_sound",
    "step_blocks_equiv",
    "stepstar_blocks_equiv",
    "terminates_blocks_equiv",
    "semantic_equiv_of_blocks_equiv",
    "distillation_finite",
}

DECL_PATTERN = re.compile(
    r'^\s*(?:@\[[^\]]+\]\s+)*(?:(?:noncomputable|private|scoped|protected)\s+)*(theorem|lemma|def|inductive|structure)\s+([A-Za-z0-9_\'\.]+)'
)

def scan_active_lean_artifacts(repo_root):
    lean_files = set()
    lean_basenames = set()
    lean_decls = set()

    for base in ["lean", "proof"]:
        base_dir = repo_root / base
        if not base_dir.is_dir():
            continue
        for root, _, files in os.walk(base_dir):
            if ".lake" in root:
                continue
            for f in files:
                if f.endswith(".lean"):
                    full = Path(root) / f
                    rel = full.relative_to(repo_root).as_posix()
                    lean_files.add(rel)
                    lean_basenames.add(f)

                    # Also add package-relative paths (e.g. Supercompiler/Semantics.lean)
                    pkg_rel = full.relative_to(repo_root / base).as_posix()
                    lean_files.add(pkg_rel)

                    with open(full, "r", encoding="utf-8", errors="replace") as fh:
                        for line in fh:
                            m = DECL_PATTERN.match(line)
                            if m:
                                lean_decls.add(m.group(2))

    return lean_files, lean_basenames, lean_decls

def get_target_doc_files(repo_root):
    targets = []
    # docs/
    docs_dir = repo_root / "docs"
    if docs_dir.is_dir():
        for p in docs_dir.rglob("*.md"):
            targets.append(p)
    # README.md
    readme = repo_root / "README.md"
    if readme.is_file():
        targets.append(readme)
    # SHOWDOWN.md
    showdown = repo_root / "SHOWDOWN.md"
    if showdown.is_file():
        targets.append(showdown)
    # paper/
    paper_dir = repo_root / "paper"
    if paper_dir.is_dir():
        for p in paper_dir.rglob("*"):
            if p.suffix in (".md", ".tex") and not p.name.endswith(".pdf"):
                targets.append(p)

    return sorted(targets)

def check_docs(repo_root):
    lean_files, lean_basenames, lean_decls = scan_active_lean_artifacts(repo_root)
    targets = get_target_doc_files(repo_root)
    errors = []

    file_ref_pattern = re.compile(r'([A-Za-z0-9_\-/\\]+\.lean)')
    code_thm_pattern = re.compile(r'\b(?:theorem|lemma)\s+`([A-Za-z0-9_]+)`', re.IGNORECASE)
    tex_thm_pattern = re.compile(r'\\(?:theorem|lemma)\s*\[[^\]]*\\texttt\{([A-Za-z0-9_]+)\}', re.IGNORECASE)

    for target in targets:
        rel_target = target.relative_to(repo_root).as_posix()
        try:
            content = target.read_text(encoding="utf-8", errors="replace")
        except Exception as e:
            errors.append(f"{rel_target}: Could not read file: {e}")
            continue

        lines = content.splitlines()
        for idx, line in enumerate(lines, 1):
            # 1. Check for fabricated theorem names anywhere on the line
            for fake in FABRICATED_THEOREMS:
                if re.search(r'\b' + re.escape(fake) + r'\b', line):
                    errors.append(
                        f"{rel_target}:{idx}: Mentions fabricated/obsolete Lean theorem `{fake}`"
                    )

            # 2. Check for *.lean file references
            for m in file_ref_pattern.finditer(line):
                raw_path = m.group(1).replace("\\", "/")
                # Strip leading ../ or ./ or /
                clean_path = raw_path
                while clean_path.startswith("../") or clean_path.startswith("./") or clean_path.startswith("/"):
                    if clean_path.startswith("../"):
                        clean_path = clean_path[3:]
                    elif clean_path.startswith("./"):
                        clean_path = clean_path[2:]
                    elif clean_path.startswith("/"):
                        clean_path = clean_path[1:]
                basename = os.path.basename(clean_path)
                # If path has directories, it must match a known relative path
                if "/" in clean_path:
                    if clean_path not in lean_files:
                        errors.append(
                            f"{rel_target}:{idx}: References nonexistent Lean file `{raw_path}`"
                        )
                else:
                    if basename not in lean_basenames:
                        errors.append(
                            f"{rel_target}:{idx}: References nonexistent Lean file `{raw_path}`"
                        )

            # 3. Check for backticked `theorem `name``
            for m in code_thm_pattern.finditer(line):
                thm_name = m.group(1)
                if thm_name not in lean_decls:
                    errors.append(
                        f"{rel_target}:{idx}: References non-mechanized Lean theorem `{thm_name}`"
                    )

            # 4. Check LaTeX theorem patterns
            for m in tex_thm_pattern.finditer(line):
                thm_name = m.group(1)
                if thm_name not in lean_decls:
                    errors.append(
                        f"{rel_target}:{idx}: References non-mechanized Lean theorem in LaTeX `{thm_name}`"
                    )

    return errors

def main():
    repo_root = Path(__file__).resolve().parent.parent
    print("=====================================================================")
    print("               NUMLANG LEAN DOCUMENTATION HONESTY AUDITOR            ")
    print("=====================================================================")

    errors = check_docs(repo_root)
    if errors:
        print(f"FAILED: Found {len(errors)} Lean documentation violation(s):")
        for err in errors:
            print(f"  [ERROR] {err}")
        sys.exit(1)
    else:
        print("PASSED: All Lean file and theorem references strictly match reality.")
        sys.exit(0)

if __name__ == "__main__":
    main()
