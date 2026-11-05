//! The article of a page: Mozilla's Readability as ported by `dom_smoothie`, and the page's own title and language when
//! it has no article.

use dom_query::Document;
use dom_smoothie::{Config, Readability};
use url::Url;

pub struct Extracted {
    pub title: String,
    pub byline: Option<String>,
    /// The article's HTML (not yet cleaned), or the whole page when `whole_page`.
    pub html: String,
    pub text_len: usize,
    pub language: Option<String>,
    /// `YYYY-MM-DD`
    pub date: Option<String>,
    pub whole_page: bool,
}

/// `2026-01-31T10:00:00Z` and the like give `2026-01-31`; anything else gives nothing.
pub fn date_of(s: &str) -> Option<String> {
    let d = s.trim().get(..10)?;
    let b = d.as_bytes();
    let ok = b.iter().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 {
            *c == b'-'
        } else {
            c.is_ascii_digit()
        }
    });
    ok.then(|| d.to_string())
}

pub fn extract(html: &str, url: &Url) -> Extracted {
    let cfg = Config {
        max_elements_to_parse: 30_000,
        ..Config::default()
    };
    if let Ok(mut r) = Readability::new(html, Some(url.as_str()), Some(cfg))
        && let Ok(a) = r.parse()
        && a.length > 0
    {
        return Extracted {
            title: a.title.trim().to_string(),
            byline: a
                .byline
                .map(|b| b.trim().to_string())
                .filter(|b| !b.is_empty()),
            html: a.content.to_string(),
            text_len: a.length,
            language: a.lang.filter(|l| !l.trim().is_empty()),
            date: a.published_time.as_deref().and_then(date_of),
            whole_page: false,
        };
    }
    // no article: the page itself
    let doc = Document::from(html);
    let meta = |sel: &str| {
        doc.select(sel)
            .attr("content")
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
    };
    let title = meta("meta[property=\"og:title\"]")
        .or_else(|| Some(doc.select("title").text().trim().to_string()).filter(|t| !t.is_empty()))
        .unwrap_or_default();
    let body = doc.select("body");
    let text_len = body
        .text()
        .split_whitespace()
        .map(|w| w.len() + 1)
        .sum::<usize>();
    Extracted {
        title,
        byline: meta("meta[name=\"author\"]"),
        html: if body.exists() {
            body.html().to_string()
        } else {
            html.to_string()
        },
        text_len,
        language: doc
            .select("html")
            .attr("lang")
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()),
        date: meta("meta[property=\"article:published_time\"]")
            .as_deref()
            .and_then(date_of),
        whole_page: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(
            date_of("2026-01-31T10:00:00Z").as_deref(),
            Some("2026-01-31")
        );
        assert_eq!(date_of(" 2026-01-31 ").as_deref(), Some("2026-01-31"));
        assert_eq!(date_of("31 January 2026"), None);
        assert_eq!(date_of("2026"), None);
    }
}
