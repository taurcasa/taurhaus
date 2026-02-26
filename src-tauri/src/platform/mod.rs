//! Platform-specific process inspection.
//!
//! Compile-time dispatch based on `target_os`. Each platform implements the
//! same function signatures — the compiler enforces the API contract.

mod types;
pub use types::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::*;
