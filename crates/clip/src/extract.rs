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

/// A byline that names somebody. Some themes mark the byline up as `<span class="byline"> by </span>` followed by the
/// author in another element, and Readability then takes the word "by" itself (found on a real page in W6).
fn real_byline(b: &str) -> Option<String> {
    let b = b.trim();
    let letters = b.chars().filter(|c| c.is_alphanumeric()).count();
    (letters >= 3 && !b.eq_ignore_ascii_case("by")).then(|| b.to_string())
}

/// Class names that mark a table as page furniture (navigation boxes, notices); such a table is dropped.
const FURNITURE_TABLES: [&str; 6] = ["navbox", "sidebar", "ambox", "mbox", "metadata", "toc"];

/// Readability throws away elements whose class or id *sounds* like page furniture (`header`, `footer`, `menu`,
/// `sidebar` ...). A Wikipedia list is `<table class="sticky-header-multi sortable wikitable">` with rows of class
/// `static-row-header`, so the whole table went (found on a real page in W6). The classes of a data table (one with
/// header cells and no table inside it) say nothing about the article, so they are removed before Readability looks;
/// the output never keeps classes anyway.
fn prepare_tables(html: &str) -> String {
    if !html.contains("<table") {
        return html.to_string();
    }
    let doc = Document::from(html);
    for t in doc.select("table").iter() {
        let class = t.attr("class").unwrap_or_default().to_lowercase();
        if FURNITURE_TABLES.iter().any(|f| class.contains(f)) {
            // navigation boxes and notices are page furniture: without their class nothing else would drop them
            t.remove();
            continue;
        }
        if t.select("table").exists() || !t.select("th").exists() {
            continue;
        }
        t.remove_attr("class");
        t.remove_attr("id");
        for i in t.select("*").iter() {
            i.remove_attr("class");
            i.remove_attr("id");
        }
    }
    doc.html().to_string()
}

pub fn extract(html: &str, url: &Url) -> Extracted {
    let cfg = Config {
        max_elements_to_parse: 30_000,
        ..Config::default()
    };
    let prepared = prepare_tables(html);
    if let Ok(mut r) = Readability::new(prepared.as_str(), Some(url.as_str()), Some(cfg))
        && let Ok(a) = r.parse()
        && a.length > 0
    {
        return Extracted {
            title: a.title.trim().to_string(),
            byline: a.byline.and_then(|b| real_byline(&b)),
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
    fn a_byline_is_a_name_not_the_word_by() {
        assert_eq!(real_byline(" by "), None);
        assert_eq!(real_byline("By"), None);
        assert_eq!(real_byline(""), None);
        assert_eq!(real_byline(" Ada Quay ").as_deref(), Some("Ada Quay"));
        assert_eq!(real_byline("By Ada Quay").as_deref(), Some("By Ada Quay"));
    }

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
