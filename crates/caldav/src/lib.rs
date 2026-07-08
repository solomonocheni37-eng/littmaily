//! CalDAV client implementation for discovering calendars and synchronizing events.
//!
//! This crate provides a lightweight, async CalDAV client that handles RFC 6578
//! (sync-collection) for efficient incremental syncing, and RFC 6764 for service discovery.

pub mod client;
pub mod error;
pub mod models;
pub mod sync;

pub use client::CalDavClient;
pub use error::CalDavError;
pub use models::{Calendar, CalendarObject};
pub use sync::SyncEngine;
