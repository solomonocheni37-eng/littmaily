//! Tauri IPC command handlers.
//!
//! All commands are re-exported here so `main.rs` can register them via
//! `tauri::generate_handler![commands::*]` without importing every module individually.

pub mod accounts;
pub mod calendar;
pub mod contacts;
pub mod mail;
pub mod misc;
pub mod oauth;
pub mod search;

pub use accounts::*;
pub use calendar::*;
pub use contacts::*;
pub use mail::*;
pub use misc::*;
pub use oauth::*;
pub use search::*;
