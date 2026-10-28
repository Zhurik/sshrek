//! sshrek — an ssh wrapper that keeps a connection history.
//!
//! The logic lives in the library and `main.rs` only wires its commands
//! together: that way every module stays a public API and is testable alone.
//!
//! One run threads through most of them. [`paths`] says where the ssh config
//! and the log file are; [`config`] reads the config into [`config::HostEntry`]
//! values; [`history`] folds the log into per-host [`history::HostStats`];
//! [`sort`] brings the two together in the requested order; [`render`] turns
//! them into the table, the picker rows or a preview card; [`picker`] asks fzf
//! to choose one of those rows; and [`runner`] hands the choice to ssh,
//! bracketing the session with the events that go back into the log.
//!
//! The two modules outside that path are [`cli`], which describes the command
//! line, and [`alias`], which emits the shell snippet binding the tool to
//! another command name.

pub mod alias;
pub mod cli;
pub mod config;
pub mod history;
pub mod paths;
pub mod picker;
pub mod render;
pub mod runner;
pub mod sort;
