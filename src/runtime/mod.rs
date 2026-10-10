//! NumLang Runtime Support Modules.

// SAFETY: Low-level runtime memory management, arena bump allocation, and FFI symbols require raw pointer manipulation.
#![allow(unsafe_code)]

pub mod arena;
pub mod parallel;
pub mod tier;

pub use tier::{SupercompileWorker, TierConfig, TierManager, TierTask};
