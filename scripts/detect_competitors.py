#!/usr/bin/env python3
"""
Read-only host probe for NumLang showdown competitors.

Checks for rustc, cl/MSVC, clang, gcc, ghc, hosc, spsc, lean, and lake.
Reports detected status, exact version/path, and benchmark applicability.
Does not install, modify, or execute benchmark workloads.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

TOOLS = [
    "rustc",
    "cl",
    "clang",
    "gcc",
    "ghc",
    "hosc",
    "spsc",
    "lean",
    "lake",
]

VCVARS_CANDIDATES = [
    r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
    r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
    r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
    r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
    r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
    r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat",
    r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvars64.bat",
]

VERSION_ARGS = {
    "rustc": ["-Vv"],
    "cl": [],  # handled via vcvars; direct cl prints version to stderr
    "clang": ["--version"],
    "gcc": ["--version"],
    "ghc": ["--version"],
    "hosc": ["--version"],
    "spsc": ["--version"],
    "lean": ["--version"],
    "lake": ["--version"],
}

BENCHMARK_APPLICABILITY = {
    "rustc": "All 18 showdown benchmarks with bench/showdown/rust/*.rs (Rustc-O3: rustc -C opt-level=3)",
    "cl": "All 18 showdown benchmarks with bench/showdown/c/*.c (MSVC-O2: cl.exe /O2 /nologo via vcvars64)",
    "clang": "C showdown benchmarks (bench/showdown/c/*.c) if harness adds clang path; current Rust harness detects but does not compile clang cells",
    "gcc": "C showdown benchmarks (bench/showdown/c/*.c) if harness adds gcc path; current Rust harness detects but does not compile gcc cells",
    "ghc": "Haskell showdown benchmarks (bench/showdown/haskell/*.hs) where present; GHC-O2: ghc -O2",
    "hosc": "G1 HOSC sources only (bench/showdown/hosc/{nrev,append3,kmp,peano_mul,tree_flip}.hosc); no G2/G3 HOSC sources in registry",
    "spsc": "No SPSC sources registered in bench/showdown/benchmarks.toml; detection only in current harness",
    "lean": "Formal proof checks (lake build in proof/), not runtime showdown benchmarks",
    "lake": "Formal proof checks (lake build in proof/), not runtime showdown benchmarks",
}

INSTALL_HINTS = {
    "rustc": {
        "windows": "winget install --id Rustlang.Rustup -e --source winget; rustup default stable",
        "blockers": "None if winget/rustup available; may require PATH refresh/new shell.",
    },
    "cl": {
        "windows": "winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override \"--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive --norestart\"",
        "blockers": "Large install; may require admin/elevation; Build Tools license/EULA; requires vcvars/dev-shell for cl.exe on PATH.",
    },
    "clang": {
        "windows": "winget install --id LLVM.LLVM -e  OR  choco install llvm -y",
        "blockers": "choco may require admin; winget may require admin depending on machine policy; PATH refresh needed.",
    },
    "gcc": {
        "windows": "winget install --id MSYS2.MSYS2 -e; then msys2 shell: pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gcc; add msys64\\ucrt64\\bin to PATH",
        "blockers": "Multi-step; MSYS2 install/admin/policy may be required; PATH not auto-added for MinGW gcc; not non-interactive without careful shell setup.",
    },
    "ghc": {
        "windows": "winget install --id GHCup.GHCup -e; ghcup install ghc latest --set; ghcup install cabal latest --set",
        "blockers": "GHCup installer may prompt or require policy approval; network/download size; PATH refresh; cabal needed for some workflows.",
    },
    "hosc": {
        "windows": "Install GHC+cabal first, then: cabal update; cabal install hosc",
        "blockers": "HOSC is an academic Haskell package; Windows/cabal build may fail due to deps/toolchain; non-interactive reliability low; requires GHC/cabal already installed.",
    },
    "spsc": {
        "windows": "No standard Windows package; likely source build from academic repo (Haskell/Java variant dependent).",
        "blockers": "SPSC distribution is not packaged in winget/choco/scoop; source/location ambiguous; may require legacy Haskell/Java toolchain; not reliably non-interactive.",
    },
    "lean": {
        "windows": "winget install --id Lean.Elan -e; elan default leanprover/lean4:stable",
        "blockers": "PATH refresh; elan self-update/network access.",
    },
    "lake": {
        "windows": "Installed with Lean/elan toolchain (lake ships alongside lean).",
        "blockers": "Same as lean/elan.",
    },
}


def run(cmd, timeout=10, shell=False):
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            shell=shell,
        )
        out = (proc.stdout or "") + ("\n" + proc.stderr if proc.stderr else "")
        return proc.returncode, out.strip()
    except Exception as e:
        return None, str(e)


def find_vcvars():
    for c in VCVARS_CANDIDATES:
        p = Path(c)
        if p.exists():
            return p
    return None


def probe_cl():
    direct = shutil.which("cl")
    vcvars = find_vcvars()
    if direct:
        rc, out = run(["cl"], timeout=10)
        version = first_line(out) or "cl.exe found"
        return {
            "tool": "cl",
            "installed": True,
            "path": direct,
            "version": version,
            "invocation": "cl.exe /O2 /nologo /Fe:<out.exe> <in.c>",
            "detection": "PATH",
        }
    if vcvars:
        # Probe through vcvars to get version without changing host state.
        cmd = f'call "{vcvars}" >nul 2>&1 && cl 2>&1'
        rc, out = run(cmd, timeout=20, shell=True)
        version = first_line(out) or "MSVC cl via vcvars64.bat"
        return {
            "tool": "cl",
            "installed": True,
            "path": str(vcvars),
            "version": version,
            "invocation": f'call "{vcvars}" >nul 2>&1 && cl.exe /O2 /nologo /Fe:<out.exe> <in.c>',
            "detection": "vcvars64.bat",
        }
    return {
        "tool": "cl",
        "installed": False,
        "path": None,
        "version": None,
        "invocation": "cl.exe /O2 /nologo /Fe:<out.exe> <in.c>",
        "detection": "not found",
    }


def first_line(text):
    for line in (text or "").splitlines():
        s = line.strip()
        if s:
            return s
    return None


def probe_tool(name):
    if name == "cl":
        return probe_cl()
    path = shutil.which(name)
    if not path:
        return {
            "tool": name,
            "installed": False,
            "path": None,
            "version": None,
            "invocation": " ".join([name] + VERSION_ARGS.get(name, ["--version"])),
            "detection": "not found",
        }
    args = [path] + VERSION_ARGS.get(name, ["--version"])
    rc, out = run(args, timeout=10)
    return {
        "tool": name,
        "installed": True,
        "path": path,
        "version": out if name == "rustc" else (first_line(out) or "unknown"),
        "invocation": " ".join([name] + VERSION_ARGS.get(name, ["--version"])),
        "detection": "PATH",
    }


def main():
    parser = argparse.ArgumentParser(description="Read-only NumLang competitor detector")
    parser.add_argument("--json", action="store_true", help="Emit JSON")
    args = parser.parse_args()

    results = []
    for tool in TOOLS:
        row = probe_tool(tool)
        row["benchmarks"] = BENCHMARK_APPLICABILITY.get(tool, "")
        row["install_hint"] = INSTALL_HINTS.get(tool, {})
        results.append(row)

    payload = {
        "host": {
            "platform": platform.platform(),
            "system": platform.system(),
            "machine": platform.machine(),
            "python": sys.version.split()[0],
        },
        "tools": results,
    }

    if args.json:
        print(json.dumps(payload, indent=2))
        return 0

    print(f"Host: {payload['host']['platform']} ({payload['host']['machine']})")
    print()
    header = f"{'Tool':<8} {'Status':<13} {'Version/Path':<70} {'Benchmarks'}"
    print(header)
    print("-" * len(header))
    for r in results:
        status = "INSTALLED" if r["installed"] else "NOT_INSTALLED"
        detail = r["version"] or ""
        if r["path"]:
            detail = f"{detail} [{r['path']}]" if detail else r["path"]
        detail = detail.replace("\n", " | ")
        if len(detail) > 70:
            detail = detail[:67] + "..."
        bench = r["benchmarks"]
        if len(bench) > 60:
            bench = bench[:57] + "..."
        print(f"{r['tool']:<8} {status:<13} {detail:<70} {bench}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
