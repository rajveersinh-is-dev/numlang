# Phase 3: Code Generation & Native Compilation Pipeline - Research

**Researched:** 2026-09-10
**Domain:** Machine Intermediate Representation (IR), Cranelift Codegen, Windows x86_64 Native Linking
**Confidence:** HIGH

<user_constraints>
## User Constraints (from REQUIREMENTS.md & ROADMAP.md)

- **CODEGEN-01:** AST is lowered to Intermediate Representation (LLVM IR / Cranelift) for arithmetic and variable assignments.
- **CODEGEN-02:** Control flow constructs (`if`/`else`, `while`, loops) and function calls are lowered to machine IR.
- **CODEGEN-03:** Compiler links and outputs native Windows x86_64 machine executables.
- **CLI-03:** CLI supports diagnostic flags `--emit-tokens`, `--emit-ast`, and `--emit-ir`.
</user_constraints>

<architectural_responsibility_map>
## Architectural Responsibility Map

- `src/ir/mod.rs`: Intermediate representation data structures and text formatter for `--emit-ir`.
- `src/codegen/mod.rs`: Cranelift backend lowering typed AST and IR into native machine instructions.
- `src/codegen/linker.rs`: Linker driver discovering Windows tools (`rust-lld.exe` from Rust sysroot or MSVC `link.exe`) and producing `.exe`.
</architectural_responsibility_map>

<research_summary>
## Summary

Phase 3 transitions `numlang` from an analyzed frontend into a true native compiler producing standalone Windows x86_64 executables.

### 1. Intermediate Representation (IR)
To satisfy `CLI-03` (`--emit-ir`) and `CODEGEN-01`:
A strongly typed, SSA-friendly intermediate representation is defined in `src/ir/mod.rs`. It models functions, basic blocks, typed values (`i32`, `i64`, `f32`, `f64`, `bool`), arithmetic instructions (`add`, `sub`, `mul`, `div`, `mod`, `pow`), comparisons (`icmp`, `fcmp`), and control transfer (`br`, `brif`, `ret`, `call`).
The IR provides a pretty-printer producing formatted, readable text for `--emit-ir`.

### 2. Cranelift Backend Architecture
We use the official Cranelift code generator:
- `cranelift-codegen` (v0.135): Native machine code generation for x86_64.
- `cranelift-frontend` (v0.135): FunctionBuilder for SSA construction and variable tracking.
- `cranelift-module` (v0.135): Global symbol and function module management.
- `cranelift-object` (v0.135): Emitting native COFF object files (`.obj`) on Windows.
- `cranelift-native` (v0.135): Auto-detecting host CPU architecture and instruction sets (AVX2, FMA, BMI).

Cranelift compiles at multi-megabyte-per-second speeds with zero C++ external dependency overhead, avoiding fragile LLVM C++ toolchain installations while generating native machine instructions.

### 3. Windows Native Linking Pipeline
Windows executables are PE32+ (x86_64) binaries. The compilation pipeline:
1. `numlang` lowers typed AST to Cranelift module.
2. `ObjectModule::finish` emits a standard COFF `.obj` file.
3. Linker Driver invokes `rust-lld.exe` (bundled in the Rust toolchain sysroot at `<sysroot>/lib/rustlib/x86_64-pc-windows-msvc/bin/rust-lld.exe`) or MSVC `link.exe`.
4. Linker flags `/subsystem:console /entry:main` link the `.obj` with `kernel32.lib` into a runnable Windows `.exe`.
5. When executed, the `.exe` runs natively on bare metal Windows x86_64!
</research_summary>

<standard_stack>
## Standard Stack

### Dependencies
| Crate | Version | Purpose |
|---|---|---|
| `cranelift-codegen` | 0.135 | x86_64 machine code generation |
| `cranelift-frontend` | 0.135 | SSA construction and variable translation |
| `cranelift-module` | 0.135 | Module management and symbol resolution |
| `cranelift-object` | 0.135 | COFF object file writer for Windows |
| `cranelift-native` | 0.135 | Host target ISA detection |
| `target-lexicon` | 0.13 | Target triple management |

</standard_stack>
