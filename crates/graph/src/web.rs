//! Clipped web pages (WA-1, WA-2): a clip is a one-chapter EPUB below `assets/web/`, named `<slug>-<hash8>.epub`. The
//! hash is of the address, so the same address always finds the same file, whatever the page's title is today.

use crate::{Error, Graph, Result, atomic_write};

/// The folder of the clips, graph-relative.
pub const WEB_DIR: &str = "assets/web";
const MAX_CLIP_BYTES: usize = 64 * 1024 * 1024;

/// `<slug>-<8 hex>.epub`: lower-case ASCII letters, digits and dashes, and nothing that could name another place.
pub fn is_clip_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".epub") else {
        return false;
    };
    let Some((slug, hash)) = stem.rsplit_once('-') else {
        return false;
    };
    hash.len() == 8
        && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        && !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.contains("--")
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Graph {
    /// The clip made for the address with this hash (`hash8`), if there is one: graph-relative path.
    pub fn find_clip(&self, hash8: &str) -> Result<Option<String>> {
        let suffix = format!("-{hash8}.epub");
        let Ok(dir) = std::fs::read_dir(self.root().join(WEB_DIR)) else {
            return Ok(None);
        };
        let mut found: Vec<String> = dir
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(&suffix) && is_clip_name(n))
            .collect();
        found.sort();
        Ok(found.into_iter().next().map(|n| format!("{WEB_DIR}/{n}")))
    }

    /// Writes a clip (atomically, replacing a file of the same name) and returns its graph-relative path. Only a name
    /// that [`is_clip_name`] accepts is written, and only below `assets/web/`.
    pub fn save_clip(&self, file_name: &str, bytes: &[u8]) -> Result<String> {
        if !is_clip_name(file_name) {
            return Err(Error::Asset("not a name for a clipped page".into()));
        }
        if bytes.len() > MAX_CLIP_BYTES {
            return Err(Error::Asset("the clipped page is larger than 64 MB".into()));
        }
        let rel = format!("{WEB_DIR}/{file_name}");
        std::fs::create_dir_all(self.root().join(WEB_DIR))?;
        atomic_write(self.root(), &self.root().join(&rel), bytes)?;
        Ok(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_names() {
        assert!(is_clip_name("the-lighthouse-keeper-ead3cd35.epub"));
        assert!(is_clip_name("a-00000000.epub"));
        for bad in [
            "x.epub",
            "-ead3cd35.epub",
            "a--b-ead3cd35.epub",
            "A-ead3cd35.epub",
            "a/b-ead3cd35.epub",
            "../a-ead3cd35.epub",
            "a-ead3cd3.epub",
            "a-ead3cd35.pdf",
            "a-ead3cd3g.epub",
            "a b-ead3cd35.epub",
            "a\\b-ead3cd35.epub",
            "",
        ] {
            assert!(!is_clip_name(bad), "{bad:?}");
        }
    }
}
