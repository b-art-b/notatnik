//! Why a clip failed, in words a person can read. No message contains the address of the page (it is the person's
//! reading, `docs/logging.md`), only its host.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipError {
    /// Not an `http` or `https` address, or one with a login in it.
    BadAddress,
    /// The address (or a redirect, or a picture) points to this computer or a private network.
    Refused,
    /// The site could not be reached (`host`).
    Unreachable(String),
    /// The site answered with this status (`host`, status).
    Status(String, u16),
    /// The answer is not a web page (the content type).
    NotHtml(String),
    /// The page is larger than the limit.
    TooBig,
    TooManyRedirects,
    /// The page builds its content with scripts: a plain fetch got an empty shell.
    NeedsScripts,
    /// The person cancelled.
    Cancelled,
    /// The book could not be written.
    Epub(String),
}

impl fmt::Display for ClipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipError::BadAddress => f.write_str(
                "That is not an address the app can fetch (only http and https, without a login in it).",
            ),
            ClipError::Refused => f.write_str(
                "That address points to this computer or to a private network, so the app does not fetch it.",
            ),
            ClipError::Unreachable(host) => write!(f, "Could not reach {host}."),
            ClipError::Status(host, code) => write!(f, "{host} answered {code}."),
            ClipError::NotHtml(t) => {
                if t.is_empty() {
                    f.write_str("That address is not a web page.")
                } else {
                    write!(f, "That address is not a web page (it is {t}).")
                }
            }
            ClipError::TooBig => f.write_str("That page is larger than 5 MB."),
            ClipError::TooManyRedirects => f.write_str("That address redirects too many times."),
            ClipError::NeedsScripts => f.write_str(
                "This page builds its content with scripts, so a plain fetch gets nothing to read.",
            ),
            ClipError::Cancelled => f.write_str("Cancelled."),
            ClipError::Epub(e) => write!(f, "The book could not be written: {e}"),
        }
    }
}

impl std::error::Error for ClipError {}

/// Things the person should know about a clip that worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// No article was found: the cleaned page body was used.
    WholePage,
    /// The article is very short.
    Short,
    /// This many pictures were left out (too many, too big, or they could not be fetched).
    PicturesLeftOut(usize),
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Note::WholePage => f.write_str("No article was found, so the whole page was kept."),
            Note::Short => f.write_str("The page has very little text."),
            Note::PicturesLeftOut(1) => f.write_str("1 picture was left out."),
            Note::PicturesLeftOut(n) => write!(f, "{n} pictures were left out."),
        }
    }
}
