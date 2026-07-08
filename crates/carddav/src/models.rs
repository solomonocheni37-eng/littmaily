use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A remote CardDAV address book collection.
///
/// `ctag` and `sync_token` track collection state to avoid redundant full downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBook {
    /// The URL of the address book collection. May be relative to the server root.
    pub url: String,
    pub display_name: String,
    /// Collection tag used for quick change detection (non-standard but widely supported).
    pub ctag: Option<String>,
    /// RFC 6578 sync-token for incremental synchronization.
    pub sync_token: Option<String>,
}

/// A single vCard object stored on the server.
///
/// `etag` is required for optimistic concurrency control when updating the contact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VCardObject {
    /// The vCard UID, extracted from the `UID:` property.
    pub uid: String,
    /// The entity tag used for optimistic concurrency control during updates.
    pub etag: String,
    /// The URL of the specific `.vcf` resource.
    pub url: String,
    /// The raw vCard text payload.
    pub vcard_data: String,
    pub last_modified: Option<DateTime<Utc>>,
}
