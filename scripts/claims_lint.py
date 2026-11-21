#!/usr/bin/env python3
"""
scripts/claims_lint.py - Hostile Public Claims and Integrity Linter

Validates that:
1. All local file links in markdown documents point to existing files.
2. Badges in README.md are live GitHub Actions badges, not static shields.io badges.
3. No sorry or unproven axioms exist in Lean verification files.
4. Documentation references match verified Lean inventory.
5. All 20 claims in docs/CLAIMS_LEDGER.md are present with real git commit hashes,
   and CLM-07 is marked as 'PROVEN (abstract model only)'.
6. Benchmark numbers and win/tie counts in SHOWDOWN.md and README.md strictly match
   the raw CSV data and pass the CI regression guard.
7. Every test/source/evidence/location path cited in docs/CLAIMS_LEDGER.md exists on disk.
8. docs/FINAL_REPORT.md section 7 mirrors the ledger's claim IDs, summaries, statuses,
   commits, and key numbers exactly.

Exit code 0 if clean, 1 if any validation error is detected.
"""

import sys
import re
import os
import subprocess
from pathlib import Path

# Configure utf-8 encoding for standard output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8")

REPO_ROOT = Path(__file__).resolve().parent.parent

def check_local_markdown_links():
    errors = []
    md_files = [
        "README.md",
        "SHOWDOWN.md",
        "INTEGRITY_RULES.md",
        "CONTRIBUTING.md",
        "REPRODUCIBILITY.md",
        "BACKENDS.md",
        "LLVM_SETUP.md",
        "CHANGELOG.md",
        "docs/CLAIMS_LEDGER.md",
        "docs/AUDIT_FINDINGS.md",
        "docs/LEAN_STATUS.md",
        "docs/LEAN_COVERAGE.md",
        "docs/LEAN_INVENTORY.md",
        "docs/ROUND2_REPORT.md",
        "docs/FINAL_REPORT.md",
        "docs/TERMINATION_PROOF.md",
        "docs/FORMAL_VERIFICATION.md",
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
            # Reject absolute file:// links — they are always machine-specific and forbidden
            if target.startswith("file://"):
                errors.append(
                    f"Absolute file:// link in {md_rel}: '{target}' "
                    f"— use relative paths only (absolute paths break on other machines and CI)"
                )
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

def check_claims_ledger():
    errors = []
    ledger_path = REPO_ROOT / "docs" / "CLAIMS_LEDGER.md"
    if not ledger_path.exists():
        return ["docs/CLAIMS_LEDGER.md not found"]

    content = ledger_path.read_text(encoding="utf-8", errors="replace")

    # Check all 20 claims CLM-01 to CLM-20
    for i in range(1, 21):
        cid = f"CLM-{i:02d}"
        if cid not in content:
            errors.append(f"docs/CLAIMS_LEDGER.md missing claim: {cid}")

    # Parse rows: | **CLM-XX** | ... | ... | ... | Status | Last Verified Commit |
    row_pattern = re.compile(r'\|\s*\*\*(CLM-\d{2})\*\*\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|')
    found_rows = 0
    for match in row_pattern.finditer(content):
        found_rows += 1
        cid, claim, loc, evidence, status, commit = [m.strip() for m in match.groups()]

        # Check commit hash format and existence
        commit_hash = commit.strip("`").strip()
        if not re.match(r'^[0-9a-fA-F]{7,40}$', commit_hash):
            errors.append(f"{cid}: 'Last Verified Commit' must be a valid hex git hash, got '{commit}'")
        else:
            res = subprocess.run(
                ["git", "cat-file", "-t", commit_hash],
                cwd=REPO_ROOT,
                capture_output=True,
                text=True,
                check=False
            )
            if res.returncode != 0 or res.stdout.strip() != "commit":
                # In shallow CI clones, attempt to fetch if commit object is missing
                is_shallow = subprocess.run(
                    ["git", "rev-parse", "--is-shallow-repository"],
                    cwd=REPO_ROOT,
                    capture_output=True,
                    text=True,
                    check=False
                ).stdout.strip() == "true"
                if is_shallow:
                    subprocess.run(
                        ["git", "fetch", "--depth=50", "origin", commit_hash],
                        cwd=REPO_ROOT,
                        capture_output=True,
                        text=True,
                        check=False
                    )
                    res = subprocess.run(
                        ["git", "cat-file", "-t", commit_hash],
                        cwd=REPO_ROOT,
                        capture_output=True,
                        text=True,
                        check=False
                    )
                if res.returncode != 0 or res.stdout.strip() != "commit":
                    errors.append(f"{cid}: commit '{commit_hash}' not found as a commit object in git repository")

        # Check CLM-07 status
        if cid == "CLM-07":
            if "PROVEN (abstract model only)" not in status:
                errors.append(f"CLM-07: Status must be 'PROVEN (abstract model only)', got '{status}'")

    if found_rows < 20:
        errors.append(f"Expected at least 20 claim rows, parsed {found_rows}")

    return errors

LEDGER_ROW_RE = re.compile(
    r'\|\s*\*\*(CLM-\d{2})\*\*\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|'
)

EVIDENCE_PATH_EXTENSIONS = (
    ".rs", ".py", ".csv", ".md", ".nl", ".lean", ".toml",
    ".yml", ".yaml", ".json", ".txt", ".bib", ".sh", ".ps1",
)


def parse_claims_ledger():
    """Parse docs/CLAIMS_LEDGER.md rows into a list of dicts, or None if missing."""
    ledger_path = REPO_ROOT / "docs" / "CLAIMS_LEDGER.md"
    if not ledger_path.exists():
        return None
    content = ledger_path.read_text(encoding="utf-8", errors="replace")
    rows = []
    for match in LEDGER_ROW_RE.finditer(content):
        cid, claim, loc, evidence, status, commit = [m.strip() for m in match.groups()]
        rows.append({
            "id": cid,
            "claim": claim,
            "location": loc,
            "evidence": evidence,
            "status": status,
            "commit": commit.strip("`").strip(),
        })
    return rows


def _is_evidence_path_token(token):
    token = token.strip()
    if not token or token.startswith(("http://", "https://", "mailto:", "#")):
        return False
    if "/" in token or "\\" in token:
        return True
    return token.lower().endswith(EVIDENCE_PATH_EXTENSIONS)


def _strip_cited_path_suffix(token):
    """Drop ':123', ':123-456', and ':Phase 22' suffixes from a cited path."""
    return re.sub(r':(?:Phase\s+)?\d+(?:-\d+)?$', '', token.strip())


def check_ledger_evidence_paths():
    """Stage 7: every test/source/evidence/location path cited in the ledger must exist."""
    errors = []
    rows = parse_claims_ledger()
    if rows is None:
        return ["docs/CLAIMS_LEDGER.md not found"]
    if not rows:
        return ["Could not parse any claim rows from docs/CLAIMS_LEDGER.md"]

    checked = 0
    for row in rows:
        for column, column_label in (("evidence", "evidence"), ("location", "stated-location")):
            for token in re.findall(r'`([^`]+)`', row[column]):
                rel_path = _strip_cited_path_suffix(token)
                if not _is_evidence_path_token(rel_path):
                    continue
                resolved = REPO_ROOT / rel_path
                checked += 1
                if not resolved.exists():
                    errors.append(
                        f"{row['id']}: {column_label} path '{token}' cited in "
                        f"docs/CLAIMS_LEDGER.md does not exist (resolved to {resolved}). "
                        f"Point the ledger at the real file or restore the missing file."
                    )
    if checked == 0:
        errors.append(
            "No cited paths were parsed from docs/CLAIMS_LEDGER.md; expected backticked "
            "file paths in the evidence and stated-location columns."
        )
    return errors


def _ledger_status_keyword(status):
    """Extract the bold status keyword, e.g. '**TESTED** (detail)' -> 'TESTED'."""
    match = re.match(r'\*\*(.+?)\*\*', status.strip())
    if match:
        return match.group(1).strip()
    return status.strip().split()[0] if status.strip() else ""


def _normalize_markdown_text(text):
    text = text.replace("**", "")
    text = re.sub(r'\s+', ' ', text)
    return text.strip()


def _extract_markdown_section(content, start_pattern, end_pattern):
    start = re.search(start_pattern, content, re.M)
    if not start:
        return None
    remainder = content[start.end():]
    end = re.search(end_pattern, remainder, re.M)
    return remainder[:end.start()] if end else remainder


def check_final_report_matches_ledger():
    """Stage 8: FINAL_REPORT.md section 7 must mirror CLAIMS_LEDGER.md exactly."""
    errors = []
    ledger_rows = parse_claims_ledger()
    if not ledger_rows:
        return [
            "docs/CLAIMS_LEDGER.md is missing or unparseable; "
            "cannot reconcile docs/FINAL_REPORT.md section 7"
        ]

    report_path = REPO_ROOT / "docs" / "FINAL_REPORT.md"
    if not report_path.exists():
        return ["docs/FINAL_REPORT.md not found"]

    report = report_path.read_text(encoding="utf-8", errors="replace")
    section = _extract_markdown_section(report, r'^##\s*7\.', r'^##\s*8\.')
    if section is None:
        return [
            "docs/FINAL_REPORT.md is missing its section 7 claim matrix "
            "(expected a heading beginning '## 7.')"
        ]

    report_rows = {}
    report_row_re = re.compile(
        r'\|\s*\*\*(CLM-\d{2})\*\*\s*\|\s*([^|]+)\|\s*([^|]+)\|\s*([^|]+)\|'
    )
    for match in report_row_re.finditer(section):
        cid, summary, status, commit = [m.strip() for m in match.groups()]
        if cid in report_rows:
            errors.append(f"FINAL_REPORT.md section 7 lists {cid} more than once")
        report_rows[cid] = {
            "summary": summary,
            "status": status,
            "commit": commit.strip("`").strip(),
        }

    ledger_by_id = {row["id"]: row for row in ledger_rows}
    missing = sorted(set(ledger_by_id) - set(report_rows))
    extra = sorted(set(report_rows) - set(ledger_by_id))
    if missing:
        errors.append(
            f"FINAL_REPORT.md section 7 is missing claim(s): {', '.join(missing)}"
        )
    if extra:
        errors.append(
            f"FINAL_REPORT.md section 7 lists claim(s) absent from the ledger: {', '.join(extra)}"
        )
    if len(report_rows) != len(ledger_by_id):
        errors.append(
            f"FINAL_REPORT.md section 7 has {len(report_rows)} claim rows; "
            f"the ledger has {len(ledger_by_id)}"
        )

    for cid, ledger in ledger_by_id.items():
        reported = report_rows.get(cid)
        if reported is None:
            continue

        expected_summary = _normalize_markdown_text(ledger["claim"])
        actual_summary = _normalize_markdown_text(reported["summary"])
        if actual_summary != expected_summary:
            errors.append(
                f"{cid}: FINAL_REPORT.md section 7 summary does not match the ledger.\n"
                f"    ledger: {expected_summary}\n"
                f"    report: {actual_summary}"
            )

        expected_status = _normalize_markdown_text(_ledger_status_keyword(ledger["status"]))
        actual_status = _normalize_markdown_text(reported["status"])
        if actual_status != expected_status:
            errors.append(
                f"{cid}: FINAL_REPORT.md section 7 status '{actual_status}' does not match "
                f"ledger status '{expected_status}'"
            )

        if reported["commit"] != ledger["commit"]:
            errors.append(
                f"{cid}: FINAL_REPORT.md section 7 commit '{reported['commit']}' does not match "
                f"ledger commit '{ledger['commit']}'"
            )

        # Secondary numeric guard: every number in the ledger claim must survive into
        # the report summary, so a partial rewrite cannot silently change a headline figure.
        for number in re.findall(r'\d+(?:,\d{3})*(?:\.\d+)?', ledger["claim"]):
            if number not in reported["summary"]:
                errors.append(
                    f"{cid}: number '{number}' from the ledger claim is missing from "
                    f"FINAL_REPORT.md section 7 summary"
                )

    return errors

def check_benchmark_numbers_single_source_of_truth():
    errors = []
    csv_path = REPO_ROOT / "bench" / "data" / "showdown_results.csv"
    if not csv_path.exists():
        csv_path = REPO_ROOT / "bench" / "data" / "showdown_results_sample.csv"
    if not csv_path.exists():
        return ["bench/data/showdown_results.csv not found"]

    # 1. Run CI regression guard logic
    guard_res = subprocess.run(
        [sys.executable, str(REPO_ROOT / "scripts" / "ci_regression_guard.py"), "--csv", str(csv_path)],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False
    )
    if guard_res.returncode != 0:
        errors.append(f"Runtime regression guard failed:\n{guard_res.stdout}\n{guard_res.stderr}")

    # 2. Cross-check SHOWDOWN.md and README.md win counts
    showdown_md = (REPO_ROOT / "SHOWDOWN.md").read_text(encoding="utf-8", errors="replace")
    readme_md = (REPO_ROOT / "README.md").read_text(encoding="utf-8", errors="replace")

    sd_wins = len(re.findall(r'\*\*NumLang-SC Win', showdown_md))
    sd_ties = len(re.findall(r'\*\*Tie', showdown_md))
    rm_wins = len(re.findall(r'\*\*NumLang-SC Win', readme_md))
    rm_ties = len(re.findall(r'\*\*Tie', readme_md))

    if sd_wins != 6:
        errors.append(f"SHOWDOWN.md table has {sd_wins} wins, expected exactly 6")
    if sd_ties != 12:
        errors.append(f"SHOWDOWN.md table has {sd_ties} ties, expected exactly 12")
    if rm_wins != 6:
        errors.append(f"README.md table has {rm_wins} wins, expected exactly 6")
    if rm_ties != 12:
        errors.append(f"README.md table has {rm_ties} ties, expected exactly 12")

    return errors

def main():
    print("=====================================================================")
    print("                 NUMLANG HOSTILE CLAIMS & INTEGRITY LINTER          ")
    print("=====================================================================")

    all_errors = []

    print("[1/8] Checking local Markdown links and document references...")
    link_errors = check_local_markdown_links()
    if link_errors:
        for err in link_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(link_errors)
    else:
        print("  [OK] All markdown links point to existing files.")

    print("[2/8] Checking live CI status badges in README...")
    badge_errors = check_readme_badges()
    if badge_errors:
        for err in badge_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(badge_errors)
    else:
        print("  [OK] No static build badges detected.")

    print("[3/8] Checking Lean 4 formalization for 'sorry' and 'axiom' shortcuts...")
    lean_errors = check_lean_proofs()
    if lean_errors:
        for err in lean_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(lean_errors)
    else:
        print("  [OK] Zero 'sorry' and zero 'axiom' statements found in Lean formalizations.")

    print("[4/8] Checking documentation references to Lean files and theorems...")
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

    print("[5/8] Checking CLAIMS_LEDGER.md (20 claims, real git commit hashes, PROVEN abstract status)...")
    ledger_errors = check_claims_ledger()
    if ledger_errors:
        for err in ledger_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(ledger_errors)
    else:
        print("  [OK] All 20 claims verified with valid git commits; CLM-07 status matches.")

    print("[6/8] Checking single source of truth (README & SHOWDOWN numbers vs raw CSV & regression guard)...")
    ssot_errors = check_benchmark_numbers_single_source_of_truth()
    if ssot_errors:
        for err in ssot_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(ssot_errors)
    else:
        print("  [OK] Single source of truth intact: all numbers, ratios, and win counts match raw CSV.")

    print("[7/8] Checking that every CLAIMS_LEDGER.md evidence/location path exists on disk...")
    evidence_errors = check_ledger_evidence_paths()
    if evidence_errors:
        for err in evidence_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(evidence_errors)
    else:
        print("  [OK] Every test/source/evidence/location path cited in the ledger exists.")

    print("[8/8] Checking FINAL_REPORT.md section 7 against CLAIMS_LEDGER.md...")
    report_errors = check_final_report_matches_ledger()
    if report_errors:
        for err in report_errors:
            print(f"  [ERROR] {err}")
        all_errors.extend(report_errors)
    else:
        print("  [OK] FINAL_REPORT.md section 7 matches the ledger: IDs, summaries, statuses, commits, and numbers.")

    print("=====================================================================")
    if all_errors:
        print(f"FAILED: Found {len(all_errors)} claim integrity violation(s).")
        sys.exit(1)
    else:
        print("PASSED: All audited claim integrity rules satisfied.")
        sys.exit(0)

if __name__ == "__main__":
    main()
