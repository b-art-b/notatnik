//! Assets (ST-8): pasted or dropped images are stored below `assets/YYYY/MM/` under a name that carries a
//! hash of the content, so the same picture is stored once, and are linked with a path relative to the page.

use chrono::{Datelike, NaiveDate};

use crate::{Error, Graph, Result, atomic_write, hash_bytes};

/// The largest asset the app accepts (a screenshot is a few MB).
pub const MAX_ASSET_BYTES: usize = 25 * 1024 * 1024;

/// The file extension of an image, from its first bytes; `None` for anything that is not a png, jpeg, gif or webp.
/// The extension never comes from a name or a MIME type the sender claims.
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("gif")
    } else if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// A file name stem made of the ASCII letters, digits, `-` and `_` of `hint` (a dropped file's name), at most 40
/// characters; `pasted` when nothing is left.
fn clean_stem(hint: Option<&str>) -> String {
    let base = hint.unwrap_or("");
    let base = base.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = base.rsplit_once('.').map_or(base, |(s, _)| s);
    let mut out = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(40).collect();
    if out.is_empty() { "pasted".into() } else { out }
}

/// The path of `asset` (graph-relative, e.g. `assets/2026/10/x.png`) as written in the page file `page_path`
/// (graph-relative, e.g. `journals/2026/2026-10-17.md`): `../../assets/2026/10/x.png`.
pub fn relative_asset_path(page_path: &str, asset: &str) -> String {
    let depth = page_path.matches('/').count();
    if depth == 0 {
        asset.to_string()
    } else {
        format!("{}{asset}", "../".repeat(depth))
    }
}

impl Graph {
    /// Stores an image below `assets/YYYY/MM/<name>-<hash8>.<ext>` and returns its graph-relative path. The
    /// bytes are written as they are; an identical file that is already there is kept and not rewritten.
    pub fn save_asset(
        &self,
        bytes: &[u8],
        name_hint: Option<&str>,
        today: NaiveDate,
    ) -> Result<String> {
        if bytes.is_empty() {
            return Err(Error::Asset("the image is empty".into()));
        }
        if bytes.len() > MAX_ASSET_BYTES {
            return Err(Error::Asset(format!(
                "the image is larger than {} MB",
                MAX_ASSET_BYTES / (1024 * 1024)
            )));
        }
        let Some(ext) = sniff_image(bytes) else {
            return Err(Error::Asset(
                "only png, jpeg, gif and webp images can be added".into(),
            ));
        };
        let hash = hash_bytes(bytes);
        let rel = format!(
            "assets/{:04}/{:02}/{}-{}.{ext}",
            today.year(),
            today.month(),
            clean_stem(name_hint),
            &hash[..8]
        );
        let abs = self.root().join(&rel);
        // the hash is in the name: a file that exists already holds this image (a rare clash of 8 hex digits
        // is caught by comparing the length too)
        if std::fs::metadata(&abs).is_ok_and(|m| m.len() == bytes.len() as u64) {
            return Ok(rel);
        }
        atomic_write(self.root(), &abs, bytes)?;
        Ok(rel)
    }
}
