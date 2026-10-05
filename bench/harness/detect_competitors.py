#!/usr/bin/env python3
"""
Competitor Detection Script for NumLang Supercompiler Showdown
Probes the host system for installed compilers and supercompilers:
- HOSC (Higher-Order Supercompiler)
- SPSC (Simple Positive Supercompiler)
- GHC (Glasgow Haskell Compiler)
- rustc (Rust compiler)
- clang (LLVM C compiler)
- gcc (GNU C compiler)
- cl (Microsoft Visual C++ compiler via vcvars64)
"""

import sys
import shutil
import subprocess
import json
from pathlib import Path

def find_vcvars64():
    candidates = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Auxiliary\Build\vcvars64.bat",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvars64.bat",
    ]
    for c in candidates:
        p = Path(c)
        if p.exists():
            return str(p)
    return None

def probe_tool(name, version_args=("--version",)):
    path = shutil.which(name)
    if not path:
        return None
    try:
        res = subprocess.run([path] + list(version_args), capture_output=True, text=True, timeout=5)
        first_line = (res.stdout or res.stderr).splitlines()
        version = first_line[0].strip() if first_line else "unknown"
        return {"installed": True, "path": path, "version": version}
    except Exception as e:
        return {"installed": True, "path": path, "version": f"error: {e}"}

def detect_all():
    tools = {}

    # 1. Supercompilers
    hosc = probe_tool("hosc")
    tools["HOSC"] = hosc if hosc else {"installed": False, "reason": "NOT_INSTALLED: hosc binary not in PATH"}

    spsc = probe_tool("spsc")
    tools["SPSC"] = spsc if spsc else {"installed": False, "reason": "NOT_INSTALLED: spsc binary not in PATH"}

    # 2. Haskell (GHC)
    ghc = probe_tool("ghc")
    tools["GHC-O2"] = ghc if ghc else {"installed": False, "reason": "NOT_INSTALLED: ghc binary not in PATH"}

    # 3. Rust (rustc)
    rustc = probe_tool("rustc")
    tools["Rustc-O"] = rustc if rustc else {"installed": False, "reason": "NOT_INSTALLED: rustc binary not in PATH"}

    # 4. Clang / LLVM
    clang = probe_tool("clang")
    tools["Clang-O3"] = clang if clang else {"installed": False, "reason": "NOT_INSTALLED: clang binary not in PATH"}

    # 5. GCC
    gcc = probe_tool("gcc")
    tools["GCC-O3"] = gcc if gcc else {"installed": False, "reason": "NOT_INSTALLED: gcc binary not in PATH"}

    # 6. MSVC (cl.exe)
    cl_path = shutil.which("cl")
    vcvars = find_vcvars64()
    if cl_path:
        cl_probe = probe_tool("cl", version_args=())
        tools["MSVC-O2"] = cl_probe if cl_probe else {"installed": True, "path": cl_path, "version": "MSVC cl"}
    elif vcvars:
        tools["MSVC-O2"] = {"installed": True, "path": vcvars, "version": "MSVC cl via vcvars64.bat"}
    else:
        tools["MSVC-O2"] = {"installed": False, "reason": "NOT_INSTALLED: cl.exe / vcvars64.bat not found"}

    return tools

def main():
    as_json = "--json" in sys.argv
    tools = detect_all()

    if as_json:
        print(json.dumps(tools, indent=2))
        return

    print("=================================================================")
    print("NUM-LANG SUPERCOMPILER SHOWDOWN: COMPETITOR TOOLCHAIN DETECTION")
    print("=================================================================")
    for name, info in tools.items():
        if info.get("installed"):
            print(f"  [FOUND]        {name:<12} -> {info.get('version')} ({info.get('path')})")
        else:
            print(f"  [MISSING]      {name:<12} -> {info.get('reason')}")
    print("=================================================================")

if __name__ == "__main__":
    main()
