//! Clipping a web page into a one-chapter EPUB (docs/phases/web-annotation.md, W1).
//!
//! `clip` fetches the page under the limits of [`net`], cuts out the article ([`extract`]), cleans it ([`clean`]), stores
//! its pictures ([`images`]) and writes the book ([`epub`]). Nothing here knows about the app, the graph or the UI.

mod charset;
mod clean;
mod epub;
mod error;
mod extract;
mod images;
pub mod net;
mod xhtml;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dom_query::Document;
use url::Url;

pub use error::{ClipError, Note};
pub use net::ClipPolicy;

/// A clip that worked.
#[derive(Debug, Clone)]
pub struct Clip {
    pub title: String,
    pub byline: Option<String>,
    /// The address after the redirects.
    pub source: String,
    /// `YYYY-MM-DD`, the page's own date when it has one.
    pub date: Option<String>,
    pub language: String,
    /// `<slug>-<hash8>.epub`: the same address always gives the same name.
    pub file_name: String,
    pub epub: Vec<u8>,
    pub notes: Vec<Note>,
}

/// Where a clip is. The callback of [`clip`] gets these and says whether to go on (`false` cancels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Fetching,
    Cleaning,
    Picture { done: usize, total: usize },
    Writing,
}

/// Clips `url` as of `now`. `progress` is told how far it is and returns `false` to cancel.
pub fn clip(
    url: &str,
    policy: &ClipPolicy,
    now: DateTime<Utc>,
    progress: &mut dyn FnMut(Progress) -> bool,
) -> Result<Clip, ClipError> {
    let policy = Arc::new(policy.clone());
    let mut step = |p: Progress| {
        if progress(p) {
            Ok(())
        } else {
            Err(ClipError::Cancelled)
        }
    };

    let mut start = Url::parse(url.trim()).map_err(|_| ClipError::BadAddress)?;
    net::check_address(&start)?;
    start.set_fragment(None);

    step(Progress::Fetching)?;
    let page = net::get(
        &policy,
        &start,
        "text/html,application/xhtml+xml;q=0.9,*/*;q=0.1",
        net::MAX_PAGE_BYTES,
        policy.page_timeout.unwrap_or(net::PAGE_TIMEOUT),
    )?;
    let kind = page
        .content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let looks_html = {
        let head =
            String::from_utf8_lossy(&page.body[..page.body.len().min(512)]).to_ascii_lowercase();
        head.contains("<html") || head.contains("<!doctype html")
    };
    if !(kind.contains("html") || (kind.is_empty() && looks_html)) {
        return Err(ClipError::NotHtml(kind));
    }
    let html = charset::decode(&page.body, &page.content_type);

    step(Progress::Cleaning)?;
    let scripts = html.to_ascii_lowercase().matches("<script").count();
    let base = base_of(&html, &page.url);
    let found = extract::extract(&html, &base);
    if found.text_len < 200 && scripts > 0 {
        return Err(ClipError::NeedsScripts);
    }
    let mut notes = Vec::new();
    if found.whole_page {
        notes.push(Note::WholePage);
    } else if found.text_len < 500 {
        notes.push(Note::Short);
    }
    let cleaned = clean::clean(&found.html, &base);

    // the pictures
    let fragment = Document::fragment(cleaned);
    let imgs = fragment.select("img");
    let total = imgs.length().min(images::MAX_PICTURES);
    let mut pictures: Vec<images::Picture> = Vec::new();
    let mut by_address: HashMap<String, Option<usize>> = HashMap::new();
    let mut spent = 0u64;
    let mut left_out = 0usize;
    for (i, img) in imgs.nodes().iter().enumerate() {
        let alt = img.attr("alt").map(|a| a.to_string()).unwrap_or_default();
        let address = img.attr("src").map(|s| s.to_string()).unwrap_or_default();
        let slot = if i >= images::MAX_PICTURES {
            None
        } else if let Some(known) = by_address.get(&address) {
            *known
        } else {
            step(Progress::Picture {
                done: i.min(total),
                total,
            })?;
            let got = Url::parse(&address).ok().and_then(|u| {
                images::fetch(
                    &policy,
                    &u,
                    pictures.len(),
                    images::MAX_TOTAL_BYTES.saturating_sub(spent),
                )
            });
            let slot = got.map(|p| {
                spent += p.bytes.len() as u64;
                pictures.push(p);
                pictures.len() - 1
            });
            by_address.insert(address, slot);
            slot
        };
        match slot {
            Some(n) => {
                img.retain_attrs(&["src", "alt", "title"]);
                img.set_attr("src", &pictures[n].name);
                if !img.has_attr("alt") {
                    img.set_attr("alt", "");
                }
            }
            None => {
                left_out += 1;
                if alt.trim().is_empty() {
                    img.remove_from_parent();
                } else {
                    img.replace_with_html(xhtml::escape(&alt));
                }
            }
        }
    }
    if left_out > 0 {
        notes.push(Note::PicturesLeftOut(left_out));
    }
    let body = xhtml::serialize(&fragment);

    step(Progress::Writing)?;
    let host = page.url.host_str().unwrap_or("page").to_string();
    let title = if found.title.is_empty() {
        host.clone()
    } else {
        found.title.clone()
    };
    let language = language_of(found.language.as_deref());
    let key = blake3::hash(start.as_str().as_bytes());
    let hex = key.to_hex();
    let identifier = {
        let h = hex.as_str();
        format!(
            "urn:uuid:{}-{}-{}-{}-{}",
            &h[0..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
    };
    let modified = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let clipped_on = now.format("%Y-%m-%d").to_string();
    let source = page.url.to_string();
    let epub = epub::write(&epub::Book {
        title: &title,
        byline: found.byline.as_deref(),
        language: &language,
        source: &source,
        date: found.date.as_deref(),
        modified: &modified,
        clipped_on: &clipped_on,
        identifier: &identifier,
        body: &body,
        pictures: &pictures,
    })?;
    Ok(Clip {
        file_name: format!("{}-{}.epub", slug(&title, &host), &hex.as_str()[..8]),
        title,
        byline: found.byline,
        source,
        date: found.date,
        language,
        epub,
        notes,
    })
}

/// The address relative links are made absolute against: the page's `<base href>` if it has one.
fn base_of(html: &str, page: &Url) -> Url {
    let doc = Document::from(html);
    doc.select("base[href]")
        .attr("href")
        .and_then(|h| page.join(h.trim()).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or_else(|| page.clone())
}

/// A BCP 47 tag the package can carry; `und` (undetermined) when the page has none or something odd.
fn language_of(lang: Option<&str>) -> String {
    let l = lang.unwrap_or("").trim().replace('_', "-");
    let first = l.split([',', ' ']).next().unwrap_or("");
    let ok = (2..=12).contains(&first.len())
        && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && first
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic());
    if ok {
        first.to_string()
    } else {
        "und".to_string()
    }
}

/// A file name part from a title: lower-case ASCII letters and digits, single dashes, at most 60 characters. A title
/// with none (another script, only symbols) is named after the host.
pub fn slug(title: &str, host: &str) -> String {
    let make = |s: &str| {
        let mut out = String::new();
        for c in s.chars() {
            if c.is_ascii_alphanumeric() {
                out.push(c.to_ascii_lowercase());
            } else if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
            if out.len() >= 60 {
                break;
            }
        }
        out.trim_matches('-').to_string()
    };
    let t = make(title);
    if !t.is_empty() {
        return t;
    }
    let h = make(host);
    if h.is_empty() { "page".to_string() } else { h }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_safe_file_names() {
        assert_eq!(
            slug("How to Cook: Pasta & Sauce!", "x.org"),
            "how-to-cook-pasta-sauce"
        );
        assert_eq!(slug("../../etc/passwd", "x.org"), "etc-passwd");
        assert_eq!(slug("日本語のタイトル", "example.co.jp"), "example-co-jp");
        assert_eq!(slug("😀😀", ""), "page");
        assert!(slug(&"a".repeat(200), "h").len() <= 60);
        assert!(!slug("a/b\\c:d", "h").contains(['/', '\\', ':']));
    }

    #[test]
    fn languages() {
        assert_eq!(language_of(Some("pl")), "pl");
        assert_eq!(language_of(Some("en_US")), "en-US");
        assert_eq!(language_of(Some("")), "und");
        assert_eq!(language_of(Some("<script>")), "und");
        assert_eq!(language_of(None), "und");
    }
}
