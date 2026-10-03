use std::fmt;

/// Index error.
#[derive(Debug)]
pub enum Error {
    /// SQLite error.
    Sqlite(rusqlite::Error),
    /// File system error.
    Io(std::io::Error),
    /// A query could not be compiled.
    Query(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "index database error: {e}"),
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::Query(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
