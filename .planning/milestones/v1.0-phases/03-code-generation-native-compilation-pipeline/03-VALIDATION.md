# Phase 3: Code Generation & Native Compilation Pipeline - Validation Plan

**Domain:** IR Lowering, Native Code Generation, Windows x86_64 Executable Linking
**Confidence:** HIGH

## Verification Commands

```bash
cargo test --test codegen_tests
cargo test
```

## Validation Scenarios

1. **IR Lowering & Emission:**
   - Verify `numlang --emit-ir sample.nl` produces formatted textual IR showing functions, basic blocks, and typed instructions.
   - Verify arithmetic (`+`, `-`, `*`, `/`) and assignment constructs lower correctly into IR instructions.
2. **Control Flow in IR:**
   - Verify `if`/`else` lowers to conditional branches (`brif`) and join blocks.
   - Verify `while` loops lower to loop headers, condition evaluations, and backward jumps.
3. **Object File Emission:**
   - Verify Cranelift compiles functions into a valid Windows COFF `.obj` file.
4. **Native Executable Linking & Execution:**
   - Verify linker driver successfully links the generated `.obj` into a standalone `.exe`.
   - Execute the compiled `.exe` on Windows x86_64 and verify exit code or output matches mathematical computation.
