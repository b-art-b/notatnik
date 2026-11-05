//! PDFs and EPUBs of this computer, read where they are (LV-1 to LV-8, LV1): a `file:///` link is checked, the file
//! is registered under a random token, and the viewers fetch `local/<token>` through the asset route. No URL can name
//! a path, so a note cannot make the app serve another file; a registered file is only ever read.
//!
//! Paths and names of local files are not logged above `debug`.

use std::io::Read as _;
use std::path::PathBuf;

use super::*;

/// A document larger than this is not opened in the viewer (LV-5).
const MAX_BYTES: u64 = 100 * 1024 * 1024;
/// The prefix of a registered file as the viewers and the asset route see it.
pub(super) const LOCAL_PREFIX: &str = "local/";
/// How many files stay registered; the oldest tokens stop working (a viewer asks again when it opens).
const MAX_TOKENS: usize = 256;

fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b':') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The path a `file:` address names, decoded: `file:///home/me/My%20Doc.pdf` gives `/home/me/My Doc.pdf` and
/// `file:///C:/Users/me/a.pdf` gives `C:/Users/me/a.pdf`. A fragment or a query is dropped (`#page=3`). `None` for
/// anything that is not a plain local address of a PDF or an EPUB: a host, a `..` segment (also encoded), a NUL, a
/// backslash, another extension.
pub(super) fn file_uri_path(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url.get(..7).filter(|p| p.eq_ignore_ascii_case("file://"))?;
    let rest = &url[rest.len()..];
    let rest = rest.split(['#', '?']).next().unwrap_or("");
    let (host, path) = rest.split_at(rest.find('/')?);
    if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
        return None;
    }
    let path = decode(path)?;
    if path.contains('\0') || path.contains('\\') {
        return None;
    }
    // `/C:/Users/me/a.pdf`: the drive letter of Windows
    let b = path.as_bytes();
    let path = if b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        path[1..].to_string()
    } else {
        path
    };
    let segments: Vec<&str> = path.split('/').collect();
    if segments.iter().skip(1).any(|s| s.is_empty()) && !path.starts_with("//") {
        // `//` inside the path: refused, nothing needs it
        return None;
    }
    if segments.iter().any(|s| *s == ".." || *s == ".") {
        return None;
    }
    let lower = path.to_ascii_lowercase();
    (lower.ends_with(".pdf") || lower.ends_with(".epub")).then_some(path)
}

/// The address of a file as it is stored in a sidecar: `file://` and the path with every segment encoded, so one file
/// always has one address however the link spelled it.
pub(super) fn canonical_address(path: &str) -> String {
    let (drive, rest) = match path.as_bytes() {
        [d, b':', ..] if d.is_ascii_alphabetic() => ("/", path),
        _ => ("", path),
    };
    let encoded: Vec<String> = rest.split('/').map(encode_segment).collect();
    format!("file://{drive}{}", encoded.join("/"))
}

fn kind_of(name: &str) -> &'static str {
    if name.to_ascii_lowercase().ends_with(".pdf") {
        "pdf"
    } else {
        "epub"
    }
}

impl AppState {
    /// Lets this state read files of this computer for the viewers. The desktop shell and the tests (`--allow-local-files`
    /// of the dev server) turn it on; a browser, a phone and a tablet cannot read a path of a computer.
    pub fn with_local_files(mut self) -> Self {
        self.allow_local_files = true;
        self
    }

    /// A registered file for a `local/<token>` path.
    pub(super) fn local_file(&self, rel: &str) -> ApiResult<PathBuf> {
        let bad = || ApiError::new("invalid", "that is not an open document");
        let clean = rel.trim_start_matches("../").trim_start_matches("./");
        let token = clean.strip_prefix(LOCAL_PREFIX).ok_or_else(bad)?;
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let path = self
            .local_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, p)| p.clone())
            .ok_or_else(bad)?;
        let meta = std::fs::metadata(&path).map_err(|_| {
            ApiError::new(
                "not_found",
                "The file is not there any more (it may have been moved or deleted)",
            )
        })?;
        if !meta.is_file() {
            return Err(bad());
        }
        if meta.len() > MAX_BYTES {
            return Err(ApiError::new(
                "too_large",
                "The file is too large to open here (over 100 MB)",
            ));
        }
        Ok(path)
    }

    /// The file behind a path the viewers got from `remote_open` or `local_open`, and its name.
    pub(super) fn document_file(&self, rel: &str) -> ApiResult<(PathBuf, String)> {
        let clean = rel.trim_start_matches("../").trim_start_matches("./");
        if clean.starts_with(LOCAL_PREFIX) {
            let p = self.local_file(clean)?;
            let name = p
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            return Ok((p, name));
        }
        let p = self.cache_file(rel)?;
        let name = p
            .parent()
            .and_then(|d| std::fs::read(d.join("meta.json")).ok())
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|m| m["name"].as_str().map(String::from))
            .or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        Ok((p, name))
    }

    /// `local_open`: checks a `file:///` link to a PDF or an EPUB of this computer and registers the file for the
    /// viewers. The file is read where it is; nothing is copied.
    pub fn local_open(&self, a: LocalOpenArgs) -> ApiResult<LocalOpened> {
        if !self.allow_local_files {
            return Err(ApiError::new(
                "unsupported",
                "Files of this computer open in the viewer in the desktop app",
            ));
        }
        let invalid = || {
            ApiError::new(
                "invalid",
                "That is not a link to a PDF or an EPUB on this computer",
            )
        };
        let path = file_uri_path(&a.url).ok_or_else(invalid)?;
        let full = PathBuf::from(&path);
        if !full.is_absolute() && !path.as_bytes().get(1).is_some_and(|c| *c == b':') {
            return Err(invalid());
        }
        let meta = std::fs::metadata(&full).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                ApiError::new("unreadable", "The file cannot be read (no permission)")
            }
            _ => ApiError::new(
                "not_found",
                "The file is not there (it may have been moved or deleted)",
            ),
        })?;
        if !meta.is_file() {
            return Err(invalid());
        }
        if meta.len() > MAX_BYTES {
            return Err(ApiError::new(
                "too_large",
                "The file is too large to open here (over 100 MB); open it in the system's program",
            ));
        }
        let name = full
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let kind = kind_of(&name);
        // a renamed file is not a document: look at the first bytes
        let mut head = vec![0u8; 1024];
        let n = std::fs::File::open(&full)
            .and_then(|mut f| f.read(&mut head))
            .map_err(|_| ApiError::new("unreadable", "The file cannot be read (no permission)"))?;
        head.truncate(n);
        let ok = if kind == "pdf" {
            head.windows(5).any(|w| w == b"%PDF-")
        } else {
            head.starts_with(b"PK\x03\x04")
        };
        if !ok {
            return Err(ApiError::new(
                "invalid",
                format!("That file is not a real {}", kind.to_uppercase()),
            ));
        }
        let token = {
            let mut files = self
                .local_files
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // the same file keeps its token while it is registered, so an open viewer's address keeps working
            if let Some((t, _)) = files.iter().find(|(_, p)| *p == full) {
                t.clone()
            } else {
                static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let t = blake3::hash(format!("{nanos}-{n}-{path}").as_bytes()).to_hex()[..24]
                    .to_string();
                files.push((t.clone(), full.clone()));
                if files.len() > MAX_TOKENS {
                    files.remove(0);
                }
                t
            }
        };
        log::info!("opened a file of this computer ({} bytes)", meta.len());
        Ok(LocalOpened {
            kind: kind.into(),
            name,
            size: meta.len(),
            address: canonical_address(&path),
            path: format!("{LOCAL_PREFIX}{token}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_of_files_are_read_and_the_dangerous_ones_refused() {
        let ok = |u: &str| file_uri_path(u);
        assert_eq!(
            ok("file:///home/me/a.pdf").as_deref(),
            Some("/home/me/a.pdf")
        );
        assert_eq!(
            ok("file://localhost/home/me/a.pdf").as_deref(),
            Some("/home/me/a.pdf")
        );
        assert_eq!(
            ok("FILE:///home/me/A.PDF").as_deref(),
            Some("/home/me/A.PDF")
        );
        assert_eq!(
            ok("file:///other/nc/Rozrachunki%20do%20luty%202024_0001.pdf").as_deref(),
            Some("/other/nc/Rozrachunki do luty 2024_0001.pdf")
        );
        assert_eq!(
            ok("file:///home/me/%C5%BC%C3%B3%C5%82w.epub").as_deref(),
            Some("/home/me/żółw.epub")
        );
        assert_eq!(
            ok("file:///C:/Users/me/a.pdf").as_deref(),
            Some("C:/Users/me/a.pdf")
        );
        // a page or a query is not part of the path
        assert_eq!(
            ok("file:///home/me/a.pdf#page=12").as_deref(),
            Some("/home/me/a.pdf")
        );
        assert_eq!(
            ok("file:///home/me/a.pdf?x=1").as_deref(),
            Some("/home/me/a.pdf")
        );
        // refused
        for bad in [
            "file://server/share/a.pdf",
            "file:///home/me/../you/a.pdf",
            "file:///home/me/%2e%2e/you/a.pdf",
            "file:///home/me/./a.pdf",
            "file:///home/me/a.pdf%00.txt",
            "file:///home/me/a%00.pdf",
            "file:///home/me/a.txt",
            "file:///home/me/",
            "file:///home/me",
            "file:///home//me/a.pdf",
            "file:///home/me/a\\b.pdf",
            "file:///home/me/%zz.pdf",
            "https://example.org/a.pdf",
            "/home/me/a.pdf",
            "file:",
            "file://",
        ] {
            assert_eq!(ok(bad), None, "{bad}");
        }
    }

    #[test]
    fn one_file_has_one_address_however_the_link_spelled_it() {
        let a = canonical_address("/other/nc/Rozrachunki do luty 2024_0001.pdf");
        assert_eq!(
            a,
            "file:///other/nc/Rozrachunki%20do%20luty%202024_0001.pdf"
        );
        assert_eq!(canonical_address(&file_uri_path(&a).unwrap()), a);
        assert_eq!(
            canonical_address(
                &file_uri_path("file://localhost/other/nc/Rozrachunki%20do%20luty%202024_0001.pdf")
                    .unwrap()
            ),
            a
        );
        assert_eq!(
            canonical_address("C:/Users/me/a b.pdf"),
            "file:///C:/Users/me/a%20b.pdf"
        );
    }
}
