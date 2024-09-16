#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod api;
#[cfg(target_os = "linux")]
mod linux;

/// Quick access to all useful functions for getting started.
pub mod prelude {
    pub use crate::api::*;
    #[cfg(target_os = "linux")]
    pub use crate::linux::{TestByATSPI, TestByATSPIError};
}
