use crate::Result;

/// A calendar of the account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calendar {
    /// Path or address of the calendar collection; the key for every other call.
    pub href: String,
    pub name: String,
    /// Component types it takes: `VEVENT`, `VTODO`.
    pub components: Vec<String>,
}

impl Calendar {
    pub fn takes(&self, component: &str) -> bool {
        self.components
            .iter()
            .any(|c| c.eq_ignore_ascii_case(component))
    }
}

/// An item in a calendar, without its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRef {
    /// File name inside the calendar (`<uid>.ics`).
    pub name: String,
    pub etag: String,
}

/// An item with its iCalendar text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerItem {
    pub name: String,
    pub etag: String,
    pub ics: String,
}

/// The condition of a write (`If-None-Match: *` / `If-Match`), so two devices cannot overwrite each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cond {
    /// Only if no item has that name.
    Create,
    /// Only if the item still has this ETag.
    IfMatch(String),
}

/// What changed in a calendar since a sync token (RFC 6578).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncResult {
    /// Hand this back next time.
    pub token: String,
    pub changed: Vec<ItemRef>,
    /// Names of items that are gone.
    pub deleted: Vec<String>,
}

/// What the sync needs from a CalDAV server. A write that loses against another device returns
/// `Error::Changed`; a sync token the server no longer knows also returns `Error::Changed` (read everything
/// again with [`CalDavClient::list`]).
pub trait CalDavClient {
    fn calendars(&self) -> Result<Vec<Calendar>>;
    /// Every item of a calendar with its ETag.
    fn list(&self, calendar: &str) -> Result<Vec<ItemRef>>;
    /// The given items with their text; names that do not exist are left out.
    fn get(&self, calendar: &str, names: &[String]) -> Result<Vec<ServerItem>>;
    /// Writes an item and returns its new ETag.
    fn put(&self, calendar: &str, name: &str, ics: &str, cond: &Cond) -> Result<String>;
    /// Deletes an item; with an ETag only if it is still that one.
    fn delete(&self, calendar: &str, name: &str, etag: Option<&str>) -> Result<()>;
    /// Changes since `token` (`None`: everything, with a first token).
    fn sync(&self, calendar: &str, token: Option<&str>) -> Result<SyncResult>;
}
