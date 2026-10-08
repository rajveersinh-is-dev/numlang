#!/usr/bin/env python3
"""Audit NumLang Monograph PDF.

Aliases and executes verify_all.py to validate:
- Total page count (>= 300 pages)
- Zero broken cross-references (??)
- Zero broken citation tags ([?])
- Table of contents integrity
- Hyperlink consistency
- Formal Lean 4 semantic listings
- Bibliography population
"""

import os
import sys

# Ensure execution path is workspace root
repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
os.chdir(repo_root)

# Delegate directly to verify_all.py
verify_script = os.path.join(os.path.dirname(__file__), "verify_all.py")
if not os.path.exists(verify_script):
    print(f"Error: {verify_script} not found.", file=sys.stderr)
    sys.exit(1)

with open(verify_script, "r", encoding="utf-8") as f:
    code = f.read()

exec(compile(code, verify_script, "exec"), globals())
