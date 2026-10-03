use std::fmt;

/// Graph service error.
#[derive(Debug)]
pub enum Error {
    /// File system error.
    Io(std::io::Error),
    /// Index error.
    Index(notatnik_index::Error),
    /// The file changed on disk since the caller read it (base-hash mismatch).
    Conflict {
        /// Graph-relative path.
        path: String,
    },
    /// Config problem.
    Config(String),
    /// An edit operation was rejected.
    Edit(notatnik_core::EditError),
    /// No such page or file.
    NotFound(String),
    /// Invalid path for a page file.
    InvalidPath(String),
    /// A rename was refused or failed.
    Rename(String),
    /// A rename was interrupted by a conflict after some files were already written.
    PartialRename {
        /// Files already written.
        done: Vec<String>,
        /// The file that conflicted.
        failed: String,
    },
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::Index(e) => write!(f, "{e}"),
            Error::Conflict { path } => write!(
                f,
                "{path} changed on disk since it was read; nothing was written"
            ),
            Error::Config(m) => write!(f, "config error: {m}"),
            Error::Edit(e) => write!(f, "edit rejected: {e}"),
            Error::NotFound(p) => write!(f, "not found: {p}"),
            Error::InvalidPath(p) => write!(f, "not a page file path: {p}"),
            Error::Rename(m) => write!(f, "{m}"),
            Error::PartialRename { done, failed } => {
                write!(
                    f,
                    "rename stopped: {failed} changed on disk. Already rewritten: {}",
                    done.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
impl From<notatnik_index::Error> for Error {
    fn from(e: notatnik_index::Error) -> Self {
        Error::Index(e)
    }
}
impl From<notatnik_core::EditError> for Error {
    fn from(e: notatnik_core::EditError) -> Self {
        Error::Edit(e)
    }
}
