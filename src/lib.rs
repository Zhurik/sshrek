//! sshrek — обёртка над ssh с историей подключений.
//!
//! Логика живёт в библиотеке, а `main.rs` только склеивает её команды:
//! так каждый модуль остаётся публичным API и тестируется в одиночку.

pub mod config;
pub mod history;
pub mod paths;
pub mod picker;
pub mod render;
pub mod runner;
pub mod sort;
