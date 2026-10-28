//! The connection history: an append-only log of session events and the
//! per-host statistics derived from it.

pub mod event;
pub mod stats;
pub mod store;

pub use event::{new_session_id, Record};
pub use stats::{aggregate, last_host, HostStats};
pub use store::Store;
