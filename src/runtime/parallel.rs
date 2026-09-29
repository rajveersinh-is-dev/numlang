//! Runtime support for parallel execution.

/// Runtime shim used by Cranelift-generated code for Fork/Join parallelism.
/// Called via an extern "C" ABI function pointer.
#[no_mangle]
pub extern "C" fn __numlang_fork_join(
    left_fn: extern "C" fn() -> i64,
    right_fn: extern "C" fn() -> i64,
) -> i64 {
    let handle = std::thread::spawn(move || left_fn());
    let right_result = right_fn();
    let left_result = handle.join().unwrap();
    // Results are only meaningful if the two arms produce independent outputs;
    // for side-effect-free arms the join result is right_result (or left, if merged).
    let _ = left_result;
    right_result
}
