use std::fmt;

/// Sync error. The variants tell the caller what to do: ask for the password, retry later, or show the message.
#[derive(Debug)]
pub enum Error {
    /// The server refused the login (401, 403).
    Auth(String),
    /// No such file or folder on the server.
    NotFound(String),
    /// A condition failed: the file changed on the server since it was read, or it exists already (412, 409).
    Changed(String),
    /// The server cannot be reached (no network, refused, timeout).
    Offline(String),
    /// The server answered with an error, or something it should not have said.
    Server(String),
    /// A file of the replica could not be read or written.
    Local(std::io::Error),
    /// The sync stopped before deleting that many files; see [`crate::Options::allow_mass_delete`].
    MassDelete(usize),
    /// The address is not allowed (plain http outside the local network).
    Url(String),
    /// A file could not be encrypted or decrypted (EN-12): the key is wrong, the file is cut short or was moved.
    Crypt(String),
    /// A file is larger than the limit of a download (the size in bytes, as far as it is known).
    TooLarge(u64),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Short machine-readable code for the UI: `auth`, `offline`, `changed`, `server`, `local`, `mass_delete`,
    /// `not_found`, `url`.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Auth(_) => "auth",
            Error::NotFound(_) => "not_found",
            Error::Changed(_) => "changed",
            Error::Offline(_) => "offline",
            Error::Server(_) => "server",
            Error::Local(_) => "local",
            Error::MassDelete(_) => "mass_delete",
            Error::Url(_) => "url",
            Error::Crypt(_) => "crypt",
            Error::TooLarge(_) => "too_large",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Auth(m) => write!(f, "the server refused the login: {m}"),
            Error::NotFound(p) => write!(f, "not found on the server: {p}"),
            Error::Changed(p) => write!(f, "changed on the server meanwhile: {p}"),
            Error::Offline(m) => write!(f, "the server cannot be reached: {m}"),
            Error::Server(m) => write!(f, "server error: {m}"),
            Error::Local(e) => write!(f, "local file error: {e}"),
            Error::MassDelete(n) => write!(
                f,
                "the sync would delete {n} files at once and was stopped; check the folder and the server, then sync again with \"Allow\""
            ),
            Error::Url(m) => write!(f, "{m}"),
            Error::Crypt(m) => write!(f, "encryption: {m}"),
            Error::TooLarge(n) => write!(
                f,
                "the file is too large to open here ({} MB)",
                n / 1_000_000
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Local(e)
    }
}
