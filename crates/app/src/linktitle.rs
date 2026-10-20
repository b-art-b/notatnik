//! The title of a web page, for a link that was pasted (ED-27). Only used when the device setting `paste_titles`
//! is on: the app then asks the site once per paste. Pure parts (`title_from_html`) are tested without a network.

use std::io::Read;
use std::time::Duration;

/// How much of the page is read: the title is in the head.
const MAX_BYTES: u64 = 256 * 1024;
/// A title longer than this is cut.
const MAX_CHARS: usize = 200;

/// `https://host/path` or `http://…` without a login in the address; nothing else is ever requested.
pub fn fetchable(url: &str) -> bool {
    let u = url.trim();
    let rest = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"));
    let Some(rest) = rest else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty()
        && !host.contains('@')
        && !u.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Fetches `url` and returns its title. `None` when the page has none or is not HTML.
pub fn fetch_title(url: &str) -> Result<Option<String>, String> {
    if !fetchable(url) {
        return Err("only http and https addresses can be looked up".into());
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(8))
        .redirects(3)
        .user_agent("Mozilla/5.0 (compatible; Notatnik link title)")
        .build();
    let resp = agent
        .get(url)
        .set("Accept", "text/html,application/xhtml+xml")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("the site answered {code}"),
            ureq::Error::Transport(t) => format!("could not reach the site: {t}"),
        })?;
    let ct = resp.content_type().to_ascii_lowercase();
    if !ct.contains("html") {
        return Ok(None);
    }
    let mut buf = Vec::new();
    resp.into_reader()
        .take(MAX_BYTES)
        .read_to_end(&mut buf)
        .map_err(|e| format!("could not read the page: {e}"))?;
    Ok(title_from_html(&String::from_utf8_lossy(&buf)))
}

/// The page's title: `<title>`, else `og:title`. Entities decoded, white space collapsed, cut to a sensible length.
pub fn title_from_html(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let from_tag = lower.find("<title").and_then(|i| {
        let open_end = lower[i..].find('>')? + i + 1;
        let close = lower[open_end..].find("</title")? + open_end;
        Some(&html[open_end..close])
    });
    let raw = from_tag
        .map(str::to_string)
        .filter(|t| !t.trim().is_empty())
        .or_else(|| og_title(html, &lower))?;
    let text = decode_entities(&raw);
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    Some(one_line.chars().take(MAX_CHARS).collect())
}

fn og_title(html: &str, lower: &str) -> Option<String> {
    let mut at = 0;
    while let Some(i) = lower[at..].find("<meta") {
        let start = at + i;
        let end = lower[start..].find('>')? + start;
        let tag = &lower[start..end];
        if tag.contains("og:title") {
            let c = tag.find("content=")? + "content=".len();
            let q = html[start + c..].chars().next()?;
            if q == '"' || q == '\'' {
                let from = start + c + 1;
                let to = html[from..].find(q)? + from;
                return Some(html[from..to].to_string());
            }
        }
        at = end;
    }
    None
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let decoded = rest.find(';').filter(|&e| e <= 10).and_then(|e| {
            let name = &rest[1..e];
            let ch = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix('#')
                    .and_then(|n| {
                        n.strip_prefix(['x', 'X'])
                            .map_or_else(|| n.parse().ok(), |h| u32::from_str_radix(h, 16).ok())
                    })
                    .and_then(char::from_u32),
            }?;
            Some((ch, e + 1))
        });
        match decoded {
            Some((ch, n)) => {
                out.push(ch);
                rest = &rest[n..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_is_read_cleaned_and_decoded() {
        let html = "<html><head>\n<TITLE lang=en>\n  Tom &amp; Jerry&#39;s  &quot;best&quot;\n of &#x41;ll </TITLE></head>";
        assert_eq!(
            title_from_html(html).as_deref(),
            Some("Tom & Jerry's \"best\" of All")
        );
    }

    #[test]
    fn og_title_is_the_fallback() {
        let html =
            r#"<head><title> </title><meta property="og:title" content="A &lt;b&gt; page"></head>"#;
        assert_eq!(title_from_html(html).as_deref(), Some("A <b> page"));
        assert_eq!(title_from_html("<p>no head</p>"), None);
    }

    #[test]
    fn a_long_title_is_cut() {
        let html = format!("<title>{}</title>", "x".repeat(500));
        assert_eq!(title_from_html(&html).unwrap().chars().count(), 200);
    }

    #[test]
    fn only_plain_web_addresses_are_fetched() {
        assert!(fetchable("https://example.org/a?b=1"));
        assert!(fetchable("http://localhost:8080/"));
        for bad in [
            "file:///etc/passwd",
            "ftp://x.org",
            "https://user:pw@x.org/",
            "https://",
            "https://x.org/a b",
            "mailto:a@b.c",
        ] {
            assert!(!fetchable(bad), "{bad}");
        }
    }
}
