//! Local-first, encrypted storage layer for Littmaily.
//!
//! Uses SQLCipher for database encryption and FTS5 for full-text search,
//! alongside a content-addressed blob store for deduplicated attachment storage.

pub mod blob;
pub mod db;
pub mod models;
pub mod repository;
