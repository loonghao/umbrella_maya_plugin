//! Core Umbrella domain logic.
//!
//! This crate intentionally has no Maya, PyO3, C ABI, or CLI dependencies.
//! Those outer layers adapt to the small interfaces exposed here.

pub mod antivirus;
pub mod error;

pub use error::{Result, UmbrellaError};
