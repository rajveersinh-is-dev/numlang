pub mod cranelift_backend;
pub mod linker;

pub use cranelift_backend::{
    compile_mir_to_obj, compile_supercompiled_to_obj, compile_to_obj, compile_to_obj_with_opt,
    CodegenError, CraneliftCompiler,
};
pub use linker::{link_executable, LinkerError};
