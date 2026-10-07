//! Cranelift code generation backend for NumLang.
//!
//! Re-exports all functionality from `crate::codegen::cranelift`.

pub trait UnwrapOrCodegenError<T> {
    fn unwrap_or_err(self) -> Result<T, CodegenError>;
    fn expect_or_err(self, msg: &str) -> Result<T, CodegenError>;
}
impl<T, E: std::fmt::Debug> UnwrapOrCodegenError<T> for Result<T, E> {
    fn unwrap_or_err(self) -> Result<T, CodegenError> {
        self.map_err(|e| CodegenError::BackendError(format!("{:?}", e)))
    }
    fn expect_or_err(self, msg: &str) -> Result<T, CodegenError> {
        self.map_err(|e| CodegenError::BackendError(format!("{}: {:?}", msg, e)))
    }
}
impl<T> UnwrapOrCodegenError<T> for Option<T> {
    fn unwrap_or_err(self) -> Result<T, CodegenError> {
        self.ok_or_else(|| CodegenError::BackendError("unwrap failed".into()))
    }
    fn expect_or_err(self, msg: &str) -> Result<T, CodegenError> {
        self.ok_or_else(|| CodegenError::BackendError(msg.into()))
    }
}


pub use crate::codegen::cranelift::*;
