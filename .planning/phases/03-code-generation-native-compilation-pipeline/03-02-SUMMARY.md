# Plan Summary: 03-02 Cranelift Backend and Windows Native Linking

## Accomplishments
1. **Cranelift Integration**:
   - Added Cranelift 0.135 (`cranelift-codegen`, `cranelift-frontend`, `cranelift-module`, `cranelift-object`, `cranelift-native`, `target-lexicon`) to `Cargo.toml`.
2. **Cranelift SSA Translation**:
   - Implemented `CraneliftCompiler` and `FunctionTranslationState` in `src/codegen/cranelift_backend.rs`.
   - Translated functions, basic blocks, variables, arithmetic (`+`, `-`, `*`, `/`, `%`, `^`), unary (`-`, `!`), and comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) across both integer and floating-point types.
   - Handled loops (`while`) and conditionals (`if`/`else`) with Cranelift branch/jump instructions.
   - Handled `mainCRTStartup` entry point synthesis on Windows, calling `ExitProcess(exit_code)` to cleanly exit with the return value of `fn main() -> i64`.
3. **Windows Native Linker Driver**:
   - Implemented `src/codegen/linker.rs` which automatically finds `rust-lld.exe` inside the active `rustc --print sysroot` (or falls back to `link.exe`), resolves Windows SDK / MSVC libraries (`kernel32.lib`, etc.), and produces PE32+ standalone `.exe` executables.
4. **CLI Flags**:
   - Updated `src/main.rs` to support `--emit-obj <file.obj>` and `-o / --output <file.exe>`.
5. **Execution Verification**:
   - Added native binary execution tests in `tests/codegen_tests.rs`:
     - `test_compile_to_obj`: verifies COFF `.obj` emission.
     - `test_compile_and_execute_native_binary`: compiles a calculation returning 42 to `.exe` and runs it on Windows x86_64, asserting exit code 42.
     - `test_compile_and_execute_loop_binary`: compiles a 1..10 loop returning 55 to `.exe` and runs it on Windows x86_64, asserting exit code 55.

## Verification
- `cargo test --test codegen_tests` passed (6/6).
- `cargo test` passed (32/32 tests across all suites).
