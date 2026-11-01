//! Drawings (DR-1, DR-5, DR-7): a drawing is one SVG file, `assets/drawings/draw-<8 hex>.svg`, linked from a note as
//! an image. Editing it again overwrites the same file (written atomically), so the link never changes and no orphan
//! is left. Only files of exactly that name can be written or read through these two functions; other assets keep
//! the rules of `assets.rs`.

use crate::{Error, Graph, MAX_ASSET_BYTES, Result, atomic_write, hash_bytes};

/// The folder of the drawings, graph-relative.
pub const DRAWING_DIR: &str = "assets/drawings";

/// `assets/drawings/draw-<8 hex>.svg` (graph-relative, lower-case hex), and nothing else.
pub fn is_drawing_path(path: &str) -> bool {
    let Some(name) = path
        .strip_prefix(DRAWING_DIR)
        .and_then(|r| r.strip_prefix('/'))
    else {
        return false;
    };
    let Some(hex) = name
        .strip_prefix("draw-")
        .and_then(|r| r.strip_suffix(".svg"))
    else {
        return false;
    };
    hex.len() == 8 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The SVG text is an SVG document and nothing that runs: the editor writes plain paths, so a `<script>` can only
/// come from somewhere else.
fn check_svg(svg: &str) -> Result<()> {
    if svg.len() > MAX_ASSET_BYTES {
        return Err(Error::Asset(format!(
            "the drawing is larger than {} MB",
            MAX_ASSET_BYTES / (1024 * 1024)
        )));
    }
    let lower = svg.to_ascii_lowercase();
    let head = lower.trim_start();
    if !(head.starts_with("<svg") || head.starts_with("<?xml") || head.starts_with("<!doctype svg"))
        || !lower.contains("<svg")
    {
        return Err(Error::Asset("a drawing has to be an SVG".into()));
    }
    if lower.contains("<script") {
        return Err(Error::Asset("a drawing cannot contain a script".into()));
    }
    Ok(())
}

impl Graph {
    /// Stores a drawing. With `existing` (a drawing path) that file is overwritten; without it a new name is made.
    /// Returns the graph-relative path.
    pub fn save_drawing(&self, existing: Option<&str>, svg: &str) -> Result<String> {
        check_svg(svg)?;
        let rel = match existing {
            Some(p) if is_drawing_path(p) => p.to_string(),
            Some(_) => return Err(Error::Asset("not a drawing of this graph".into())),
            None => {
                let mut salt = 0u128;
                loop {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_nanos());
                    let hash = hash_bytes(format!("{now}-{salt}-{svg}").as_bytes());
                    let rel = format!("{DRAWING_DIR}/draw-{}.svg", &hash[..8]);
                    if !self.root().join(&rel).exists() {
                        break rel;
                    }
                    salt += 1;
                }
            }
        };
        atomic_write(self.root(), &self.root().join(&rel), svg.as_bytes())?;
        Ok(rel)
    }

    /// The SVG text of a drawing.
    pub fn read_drawing(&self, path: &str) -> Result<String> {
        if !is_drawing_path(path) {
            return Err(Error::Asset("not a drawing of this graph".into()));
        }
        std::fs::read_to_string(self.root().join(path))
            .map_err(|_| Error::Asset("the drawing was not found".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_name_of_a_drawing_is_one() {
        assert!(is_drawing_path("assets/drawings/draw-0123abcd.svg"));
        for bad in [
            "assets/drawings/draw-0123ABCD.svg",
            "assets/drawings/draw-0123abc.svg",
            "assets/drawings/draw-0123abcde.svg",
            "assets/drawings/x.svg",
            "assets/drawings/draw-0123abcd.png",
            "assets/drawings/../draw-0123abcd.svg",
            "assets/other/draw-0123abcd.svg",
            "../assets/drawings/draw-0123abcd.svg",
            "assets/drawings/sub/draw-0123abcd.svg",
        ] {
            assert!(!is_drawing_path(bad), "{bad}");
        }
    }

    #[test]
    fn text_that_is_not_an_svg_or_runs_is_refused() {
        assert!(check_svg("<svg xmlns='http://www.w3.org/2000/svg'></svg>").is_ok());
        assert!(check_svg("<?xml version='1.0'?><svg></svg>").is_ok());
        assert!(check_svg("hello").is_err());
        assert!(check_svg("<html><svg></svg></html>").is_err());
        assert!(check_svg("<svg><script>alert(1)</script></svg>").is_err());
        assert!(check_svg(&format!("<svg>{}</svg>", "x".repeat(MAX_ASSET_BYTES))).is_err());
    }
}
