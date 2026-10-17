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

    /// Files below `assets/` that no page mentions, sorted by path: `(graph-relative path, size in bytes)`.
    /// A page mentions an asset when its text holds the asset's path (`assets/...`) anywhere, in a link or in
    /// code, so a file is only listed when nothing could point at it.
    pub fn orphan_assets(&self) -> Result<Vec<(String, u64)>> {
        let root = self.root();
        let mut assets = Vec::new();
        collect_files(&root.join("assets"), "assets", &mut assets);
        if assets.is_empty() {
            return Ok(assets);
        }
        let mut pages = Vec::new();
        collect_pages(root, "", &mut pages);
        let mut used = std::collections::BTreeSet::new();
        for p in pages {
            if let Ok(text) = std::fs::read_to_string(root.join(&p)) {
                mentioned_assets(&text, &mut used);
            }
        }
        assets.retain(|(rel, _)| !used.contains(rel));
        assets.sort();
        Ok(assets)
    }

    /// Moves orphan assets to the trash (`<data dir>/trash/<id>/`, outside the graph, so not synced) and
    /// deletes them. A path that is not an orphan now (a page links it since the list was made) is left alone.
    /// Returns how many files were deleted.
    pub fn trash_orphan_assets(&self, rels: &[String]) -> Result<usize> {
        let orphans: std::collections::BTreeSet<String> =
            self.orphan_assets()?.into_iter().map(|(r, _)| r).collect();
        let data_dir = self.lock().data_dir.clone();
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
            .to_string();
        let mut n = 0;
        for rel in rels {
            if !orphans.contains(rel) {
                continue;
            }
            let abs = self.root().join(rel);
            let dest = data_dir.join("trash").join(&id).join(rel);
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::copy(&abs, &dest)?;
            std::fs::remove_file(&abs)?;
            n += 1;
        }
        Ok(n)
    }
}

fn collect_files(dir: &std::path::Path, rel: &str, out: &mut Vec<(String, u64)>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let (Some(name), Ok(ty)) = (e.file_name().to_str().map(str::to_string), e.file_type())
        else {
            continue;
        };
        let rel = format!("{rel}/{name}");
        if ty.is_dir() {
            collect_files(&e.path(), &rel, out);
        } else if ty.is_file() {
            out.push((rel, e.metadata().map_or(0, |m| m.len())));
        }
    }
}

/// The `.md` files of the graph, not looking into hidden folders or `assets/`.
fn collect_pages(dir: &std::path::Path, rel: &str, out: &mut Vec<String>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let (Some(name), Ok(ty)) = (e.file_name().to_str().map(str::to_string), e.file_type())
        else {
            continue;
        };
        let rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if ty.is_dir() {
            if !name.starts_with('.') && rel != "assets" {
                collect_pages(&e.path(), &rel, out);
            }
        } else if ty.is_file() && name.ends_with(".md") {
            out.push(rel);
        }
    }
}

/// Adds every `assets/...` path in `text` (up to a character that ends a link or a word) to `out`. A path
/// that is written percent-encoded is added decoded too.
fn mentioned_assets(text: &str, out: &mut std::collections::BTreeSet<String>) {
    let mut rest = text;
    while let Some(i) = rest.find("assets/") {
        let tail = &rest[i..];
        let end = tail
            .find(|c: char| {
                c.is_whitespace() || matches!(c, ')' | ']' | '<' | '>' | '"' | '\'' | '|')
            })
            .unwrap_or(tail.len());
        let path = &tail[..end];
        out.insert(path.to_string());
        if path.contains('%') {
            out.insert(percent_decode(path));
        }
        rest = &tail["assets/".len()..];
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
