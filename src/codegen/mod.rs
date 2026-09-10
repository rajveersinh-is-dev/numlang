pub mod cranelift_backend;
pub mod linker;

pub use cranelift_backend::{compile_to_obj, CodegenError, CraneliftCompiler};
pub use linker::{link_executable, LinkerError};
