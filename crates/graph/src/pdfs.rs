//! PDFs in the graph (PA-1, PA-2, PA-9): a PDF is an ordinary asset that is never written; its annotations live in a
//! page of their own, the sidecar `hls/<name>`. This module knows which paths are PDFs and what their hash is; the
//! sidecar itself is made by the app layer with the usual page operations.

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use crate::{Error, Graph, Result};

/// Page names of the sidecars start with this namespace.
pub const SIDECAR_NAMESPACE: &str = "hls";

/// `assets/.../name.pdf` (graph-relative, the extension in any case), without `.`/`..` segments, backslashes or
/// empty segments. This is the only kind of file `annotation_page` works on.
pub fn is_pdf_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("assets/") else {
        return false;
    };
    if path.contains('\\') || path.contains('\0') {
        return false;
    }
    if rest
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return false;
    }
    path.len() > "assets/.pdf".len() && path.to_ascii_lowercase().ends_with(".pdf")
}

/// A path as a page link writes it (`../../assets/report.pdf`, `./assets/…`) made graph-relative.
pub fn graph_relative(path: &str) -> String {
    let mut p = path.trim();
    loop {
        if let Some(r) = p.strip_prefix("../") {
            p = r;
        } else if let Some(r) = p.strip_prefix("./") {
            p = r;
        } else {
            break;
        }
    }
    p.to_string()
}

/// The part of a PDF's file name that names its sidecar: `report.pdf` gives `report`.
pub fn pdf_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.get(..name.len().saturating_sub(4)).unwrap_or(name);
    let stem = stem.trim();
    if stem.is_empty() {
        "document".to_string()
    } else {
        stem.to_string()
    }
}

type Cache = HashMap<String, (u64, Option<SystemTime>, String)>;

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

impl Graph {
    /// The first 16 hex digits of the blake3 of a PDF's bytes (the `sha` of its sidecar). The file is read once for
    /// each size and modification time; a PDF is never read into memory as a whole.
    pub fn pdf_sha(&self, path: &str) -> Result<String> {
        if !is_pdf_path(path) {
            return Err(Error::Asset("not a PDF in the assets of this graph".into()));
        }
        let full = self.root().join(path);
        let meta = std::fs::metadata(&full)
            .map_err(|_| Error::NotFound(format!("the PDF {path} is not in the graph")))?;
        if !meta.is_file() {
            return Err(Error::Asset("that is not a file".into()));
        }
        let key = format!("{}\0{path}", self.root().display());
        let stamp = (meta.len(), meta.modified().ok());
        if let Some((len, mtime, sha)) = cache().lock().ok().and_then(|c| c.get(&key).cloned())
            && (len, mtime) == stamp
        {
            return Ok(sha);
        }
        let mut file = std::fs::File::open(&full)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let sha = hasher.finalize().to_hex()[..16].to_string();
        if let Ok(mut c) = cache().lock() {
            c.insert(key, (stamp.0, stamp.1, sha.clone()));
        }
        Ok(sha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_paths() {
        assert!(is_pdf_path("assets/report.pdf"));
        assert!(is_pdf_path("assets/2026/10/Report.PDF"));
        assert!(!is_pdf_path("assets/report.png"));
        assert!(!is_pdf_path("pages/report.pdf"));
        assert!(!is_pdf_path("assets/../pages/x.pdf"));
        assert!(!is_pdf_path("assets//x.pdf"));
        assert!(!is_pdf_path("assets/.pdf"));
        assert!(!is_pdf_path("assets/a\\b.pdf"));
    }

    #[test]
    fn link_paths_become_graph_relative() {
        assert_eq!(
            graph_relative("../../assets/a.pdf"),
            "assets/a.pdf".to_string()
        );
        assert_eq!(graph_relative("assets/a.pdf"), "assets/a.pdf".to_string());
    }

    #[test]
    fn stems() {
        assert_eq!(pdf_stem("assets/2026/Annual Report.pdf"), "Annual Report");
        assert_eq!(pdf_stem("assets/x.PDF"), "x");
    }
}
