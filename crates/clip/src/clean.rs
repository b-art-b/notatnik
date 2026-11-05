//! Cleaning: an allow-list of what a clipped article may contain. No script, style, frame, form or object, no `on…`
//! attribute, no `javascript:` or `data:` address. This is a second layer: the book is also shown under the app's
//! content policy, which blocks scripts and remote resources.

use std::collections::HashSet;

use url::Url;

/// The cleaned HTML of `html`, with relative addresses made absolute against `base`.
pub fn clean(html: &str, base: &Url) -> String {
    let dropped = [
        "script", "style", "title", "head", "noscript", "template", "svg", "math", "iframe",
        "frame", "frameset", "object", "embed", "applet", "form", "button", "select", "textarea",
        "audio", "video", "canvas", "map",
    ];
    let mut b = ammonia::Builder::default();
    b.rm_tags(dropped)
        .url_schemes(
            ["http", "https", "mailto"]
                .into_iter()
                .collect::<HashSet<_>>(),
        )
        .url_relative(ammonia::UrlRelative::RewriteWithBase(base.clone()))
        .link_rel(Some("noopener noreferrer"))
        .clean_content_tags(dropped.into_iter().collect());
    b.clean(html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html: &str) -> String {
        clean(html, &Url::parse("https://example.org/a/b.html").unwrap())
    }

    #[test]
    fn nothing_that_runs_or_embeds_is_left() {
        let out = run(
            r#"<p onclick="x()" onmouseover="y()">Hello <a href="javascript:alert(1)">bad</a>
            <a href="JaVaScRiPt:alert(2)">bad2</a> <a href="data:text/html,hi">bad3</a> <a href="vbscript:x">bad4</a>
            <a href="ftp://example.org/f">bad5</a><img src="data:image/png;base64,AAAA" alt="d" onerror="z()">
            <script>alert(3)</script><style>p{color:red}</style><iframe src="https://evil.invalid/"></iframe>
            <object data="x"></object><embed src="x"><form action="/x"><input name="a"><button>Send</button></form>
            <svg onload="q()"><script>1</script></svg><math><mi>x</mi></math><video src="v.mp4"></video></p>"#,
        );
        let lower = out.to_ascii_lowercase();
        for bad in [
            "<script",
            "<style",
            "<iframe",
            "<object",
            "<embed",
            "<form",
            "<input",
            "<button",
            "<svg",
            "<math",
            "<video",
            "onclick",
            "onmouseover",
            "onerror",
            "onload",
            "javascript:",
            "vbscript:",
            "data:",
            "ftp:",
            "alert(",
            "evil.invalid",
        ] {
            assert!(!lower.contains(bad), "{bad} left in {out}");
        }
        assert!(out.contains("Hello") && out.contains("bad"));
    }

    #[test]
    fn links_are_absolute_and_do_not_leak_the_referrer() {
        let out = run(
            r#"<a href="/x/y">one</a> <a href="../up">two</a> <a href="rel.html#frag">three</a>
            <a href="https://other.org/p" target="_blank">four</a><img src="pic.png" alt="p">"#,
        );
        assert!(out.contains(r#"href="https://example.org/x/y""#), "{out}");
        assert!(out.contains(r#"href="https://example.org/up""#), "{out}");
        assert!(
            out.contains(r#"href="https://example.org/a/rel.html#frag""#),
            "{out}"
        );
        assert!(
            out.contains(r#"src="https://example.org/a/pic.png""#),
            "{out}"
        );
        assert!(
            out.matches(r#"rel="noopener noreferrer""#).count() >= 4,
            "{out}"
        );
        assert!(!out.contains("target="), "{out}");
    }

    #[test]
    fn the_structure_of_an_article_survives() {
        let out = run(
            "<h2>Title</h2><p>A <em>b</em> <strong>c</strong></p><ul><li>x</li></ul><blockquote>q</blockquote><pre><code>let a = 1;</code></pre><table><tr><td colspan=\"2\">t</td></tr></table>",
        );
        for keep in [
            "<h2>",
            "<em>",
            "<strong>",
            "<ul>",
            "<li>",
            "<blockquote>",
            "<pre>",
            "<code>",
            "<table>",
            "colspan",
        ] {
            assert!(out.contains(keep), "{keep} lost in {out}");
        }
    }
}
