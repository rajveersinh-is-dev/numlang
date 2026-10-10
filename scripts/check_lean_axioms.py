#!/usr/bin/env python3
"""
scripts/check_lean_axioms.py

Runs #print axioms via AxiomCheck.lean in lean/ and proof/,
and verifies that every headline theorem depends only on allowed standard Lean foundational axioms:
  - propext
  - Quot.sound
  - Classical.choice
Fails if sorryAx or any custom/unproven axiom appears.
"""

import os
import subprocess
import sys
import re

ALLOWED_AXIOMS = {"propext", "Quot.sound", "Classical.choice"}

def run_axiom_check(directory, file_name):
    cmd = ["lake", "env", "lean", file_name]
    result = subprocess.run(cmd, cwd=directory, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error running {' '.join(cmd)} in {directory}:")
        print(result.stderr)
        return False, []
    
    lines = result.stdout.strip().splitlines()
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    rel_path = os.path.relpath(os.path.join(directory, file_name), root).replace("\\", "/")
    print(f"--- Axiom output for {rel_path} ---")
    for line in lines:
        print(line)
    return True, lines

def parse_and_validate(lines):
    errors = []
    # Match: 'TheoremName' depends on axioms: [ax1, ax2]
    # or: 'TheoremName' does not depend on any axioms
    pattern = re.compile(r"'([^']+)'\s+(?:depends on axioms:\s*\[([^\]]*)\]|does not depend on any axioms)")
    for line in lines:
        line = line.strip()
        if not line:
            continue
        m = pattern.search(line)
        if not m:
            continue
        thm_name = m.group(1)
        axioms_str = m.group(2)
        if axioms_str is None:
            # Does not depend on any axioms (constructive)
            continue
        axioms = [a.strip() for a in axioms_str.split(",") if a.strip()]
        for ax in axioms:
            if ax not in ALLOWED_AXIOMS:
                errors.append(f"Theorem '{thm_name}' depends on disallowed axiom: '{ax}'")
            if "sorry" in ax.lower():
                errors.append(f"Theorem '{thm_name}' uses 'sorry'!")
    return errors

def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    lean_dir = os.path.join(root, "lean")
    proof_dir = os.path.join(root, "proof")

    all_errors = []
    
    # 1. lean/AxiomCheck.lean
    ok1, lines1 = run_axiom_check(lean_dir, "AxiomCheck.lean")
    if not ok1:
        all_errors.append("lean/AxiomCheck.lean failed to execute")
    else:
        all_errors.extend(parse_and_validate(lines1))

    # 2. proof/AxiomCheck.lean
    ok2, lines2 = run_axiom_check(proof_dir, "AxiomCheck.lean")
    if not ok2:
        all_errors.append("proof/AxiomCheck.lean failed to execute")
    else:
        all_errors.extend(parse_and_validate(lines2))

    if all_errors:
        print("\nAXIOM CHECK FAILED with errors:")
        for err in all_errors:
            print(f"  - {err}")
        sys.exit(1)
    
    print("\nAXIOM CHECK PASSED: All headline theorems depend only on foundational Lean axioms.")
    sys.exit(0)

if __name__ == "__main__":
    main()
