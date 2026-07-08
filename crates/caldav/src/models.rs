use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A remote CalDAV calendar collection.
///
/// `ctag` and `sync_token` are used to determine if the calendar's contents
/// have changed since the last sync, avoiding unnecessary full downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    /// The URL of the calendar collection. May be relative to the server root.
    pub url: String,
    pub display_name: String,
    /// Calendar tag used for quick change detection (non-standard but widely supported).
    pub ctag: Option<String>,
    /// RFC 6578 sync-token for incremental synchronization.
    pub sync_token: Option<String>,
}

/// A single iCalendar object (event, todo, journal) stored on the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarObject {
    /// The iCalendar UID, extracted from the `UID:` property.
    pub uid: String,
    /// The entity tag used for optimistic concurrency control during updates.
    pub etag: String,
    /// The URL of the specific `.ics` resource.
    pub url: String,
    /// The raw iCalendar text payload.
    pub ical_data: String,
    pub last_modified: Option<DateTime<Utc>>,
}
