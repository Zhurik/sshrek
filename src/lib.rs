//! sshrek — an ssh wrapper that keeps a connection history.
//!
//! The logic lives in the library and `main.rs` only wires its commands
//! together: that way every module stays a public API and is testable alone.

pub mod cli;
pub mod config;
pub mod history;
pub mod paths;
pub mod picker;
pub mod render;
pub mod runner;
pub mod sort;
