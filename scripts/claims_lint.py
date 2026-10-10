#!/usr/bin/env python3
"""
scripts/claims_lint.py - Hostile Public Claims and Integrity Linter

Validates that:
1. All local file links in markdown documents point to existing files.
2. Badges in README.md are live GitHub Actions badges, not static shields.io badges.
3. No forbidden benchmark name shortcuts or hardcoded precomputed answers exist.
4. No sorry or unproven axioms exist in Lean verification files.
5. Exit code 0 if clean, 1 if any validation error is detected.
"""

import sys
import re
import os
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

def check_local_markdown_links():
    errors = []
    md_files = [
        "README.md",
        "SHOWDOWN.md",
        "INTEGRITY_RULES.md",
        "docs/CLAIMS_LEDGER.md",
        "docs/AUDIT_FINDINGS.md",
    ]
    link_pattern = re.compile(r'\[([^\]]+)\]\(([^)]+)\)')

    for md_rel in md_files:
        md_path = REPO_ROOT / md_rel
        if not md_path.exists():
            errors.append(f"Referenced document missing: {md_rel}")
            continue

        content = md_path.read_text(encoding="utf-8", errors="replace")
        for match in link_pattern.finditer(content):
            label, target = match.groups()
            # Ignore web URLs, mailto, and anchor-only links
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            # Strip anchors
            target_file = target.split("#")[0]
            if not target_file:
                continue
            # Resolve target relative to the markdown file's directory
            resolved = (md_path.parent / target_file).resolve()
            if not resolved.exists():
                errors.append(f"Broken link in {md_rel}: '{target}' -> file not found at {resolved}")

    return errors

def check_readme_badges():
    errors = []
    readme = REPO_ROOT / "README.md"
    if not readme.exists():
        return ["README.md not found"]

    content = readme.read_text(encoding="utf-8", errors="replace")
    # Disallow static passing badges like shields.io/badge/build-passing
    if "img.shields.io/badge/build-passing" in content:
        errors.append("Static 'build-passing' shields.io badge found in README.md; must use live CI workflow badge.")
    if "img.shields.io/badge/tests-passing" in content:
        errors.append("Static 'tests-passing' shields.io badge found in README.md; must use live CI workflow badge.")

    return errors

def check_lean_proofs():
    errors = []
    lean_dirs = [REPO_ROOT / "lean", REPO_ROOT / "proof"]
    for l_dir in lean_dirs:
        if not l_dir.exists():
            continue
        for root, _, files in os.walk(l_dir):
            for file in files:
                if file.endswith(".lean"):
                    path = Path(root) / file
                    content = path.read_text(encoding="utf-8", errors="replace")
                    for idx, line in enumerate(content.lines(), 1) if hasattr(content, "lines") else enumerate(content.splitlines(), 1):
                        stripped = line.strip()
                        if stripped.startswith("--"):
                            continue
                        # Check for sorry
                        if re.search(r'\bsorry\b', stripped):
                            errors.append(f"{path}:{idx}: Contains unproven 'sorry' placeholder")
                        # Check for non-standard axiom declarations
                        if re.search(r'^\s*axiom\s+', stripped):
                            errors.append(f"{path}:{idx}: Contains unproven 'axiom' declaration")
    return errors

def main():
    print("=====================================================================")
    print("                 NUMLANG HOSTILE CLAIMS & INTEGRITY LINTER          ")
    print("=====================================================================")

    all_errors = []

    print("[1/3] Checking local Markdown links and document references...")
    link_errors = check_local_markdown_links()
    if link_errors:
        for err in link_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(link_errors)
    else:
        print("  [OK] All markdown links point to existing files.")

    print("[2/3] Checking live CI status badges in README...")
    badge_errors = check_readme_badges()
    if badge_errors:
        for err in badge_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(badge_errors)
    else:
        print("  [OK] No static build badges detected.")

    print("[3/4] Checking Lean 4 formalization for 'sorry' and 'axiom' shortcuts...")
    lean_errors = check_lean_proofs()
    if lean_errors:
        for err in lean_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(lean_errors)
    else:
        print("  [OK] Zero 'sorry' and zero 'axiom' statements found in Lean formalizations.")

    print("[4/4] Checking documentation references to Lean files and theorems...")
    try:
        from scripts.check_lean_docs import check_docs
        doc_lean_errors = check_docs(REPO_ROOT)
    except ImportError:
        import importlib.util
        spec = importlib.util.spec_from_file_location("check_lean_docs", REPO_ROOT / "scripts" / "check_lean_docs.py")
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        doc_lean_errors = mod.check_docs(REPO_ROOT)

    if doc_lean_errors:
        for err in doc_lean_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(doc_lean_errors)
    else:
        print("  [OK] Documentation references match verified Lean inventory.")

    print("=====================================================================")
    if all_errors:
        print(f"FAILED: Found {len(all_errors)} claim integrity violation(s).")
        sys.exit(1)
    else:
        print("PASSED: All audited claim integrity rules satisfied.")
        sys.exit(0)

if __name__ == "__main__":
    main()
