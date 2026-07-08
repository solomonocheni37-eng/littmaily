//! CardDAV client implementation for discovering address books and synchronizing contacts.
//!
//! Implements RFC 6352 (CardDAV), RFC 6578 (sync-collection for incremental syncing),
//! and RFC 6764 (service discovery).

pub mod client;
pub mod error;
pub mod models;
pub mod sync;

pub use client::CardDavClient;
pub use error::CardDavError;
pub use models::{AddressBook, VCardObject};
pub use sync::SyncEngine;
