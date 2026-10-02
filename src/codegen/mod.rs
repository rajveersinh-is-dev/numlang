pub mod backend_trait;
pub mod cranelift;
pub mod cranelift_backend;
pub mod linker;
pub mod llvm_backend;

pub use backend_trait::BackendCompiler;

pub use cranelift_backend::{
    compile_mir_to_obj, compile_supercompiled_to_obj, compile_supercompiled_to_obj_with_cache,
    compile_supercompiled_to_obj_with_mode, compile_supercompiled_to_obj_with_mode_options,
    compile_to_obj, compile_to_obj_with_opt, CodegenError, CraneliftCompiler,
};
pub use linker::{link_executable, LinkerError};
pub use llvm_backend::{LlvmCompiler, LlvmError, OptLevel};
