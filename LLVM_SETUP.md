# LLVM Toolchain Setup Guide for NumLang

This document provides instructions for installing LLVM and enabling the optional LLVM backend in NumLang (`--features llvm-backend`).

---

## 1. Overview

NumLang includes a dual-backend compilation architecture:
1. **Cranelift Backend (Default)**: Pure Rust, zero external dependencies, extremely fast compilation, optimized for development, interactive workflows, and closed-form supercompiler outputs.
2. **LLVM Backend (Optional, Release Mode)**: Utilizes LLVM 18 via the `inkwell` crate for aggressive release optimizations (`-O3`), auto-vectorization (AVX2/SIMD), loop unrolling, and Link-Time Optimization (LTO).

Because the LLVM backend requires native LLVM C/C++ libraries during compilation, it is gated behind the Cargo feature:
```toml
[features]
default = []
llvm-backend = ["dep:inkwell"]
```

---

## 2. Installing LLVM on Windows

### Option A: Via Chocolatey (Recommended)

Run PowerShell as Administrator:
```powershell
choco install llvm --version=18.1.8 -y
```

Verify installation:
```powershell
llvm-config --version
clang --version
```

### Option B: Via Winget

```powershell
winget install LLVM.LLVM
```

### Option C: Official LLVM Pre-built Binary Installer

1. Download the official installer from GitHub Releases:
   [LLVM Releases](https://github.com/llvm/llvm-project/releases/tag/llvmorg-18.1.8)
   Select `LLVM-18.1.8-win64.exe`.
2. Run the installer and ensure you check **"Add LLVM to the system PATH for all users"**.

---

## 3. Environment Variables Configuration

`llvm-sys` (the underlying binding used by `inkwell`) requires finding LLVM tools and libraries. If `llvm-config` is not found in your `PATH`, set the `LLVM_SYS_180_PREFIX` environment variable:

```powershell
# Set for current PowerShell session
$env:LLVM_SYS_180_PREFIX = "C:\Program Files\LLVM"

# Or set permanently
[Environment]::SetEnvironmentVariable("LLVM_SYS_180_PREFIX", "C:\Program Files\LLVM", "User")
```

Verify that `C:\Program Files\LLVM\bin` contains `llvm-config.exe` or `LLVM-C.dll` / `LLVM.lib`.

---

## 4. Building NumLang with the LLVM Backend

Once LLVM 18 is installed and configured:

```powershell
# Build numlang compiler with LLVM backend support
cargo build --release --features llvm-backend

# Run test suite with LLVM backend enabled
cargo test --features llvm-backend

# Run dual-backend comparative benchmarks
cargo test --test llvm_vs_cranelift_benchmarks --features llvm-backend -- --nocapture
```

---

## 5. Usage

Compile any NumLang program using the LLVM backend:
```powershell
# Build native standalone executable with LLVM -O3
numlang build --backend llvm --opt-level 3 src/main.nl -o main.exe

# Run directly using the LLVM backend
numlang run --backend llvm --opt-level 3 src/main.nl
```
