//! A cleaned HTML fragment written as XHTML. An EPUB chapter is an XML document, so `<br>` has to be `<br/>`, and
//! every `&`, `<` and quote in text and attributes has to be escaped. Only what the cleaning lets through can be here.

use dom_query::{Document, NodeRef};

const VOID: &[&str] = &["br", "hr", "img", "col", "wbr"];

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // a control character other than tab and line breaks cannot be in an XML document at all
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// The nodes of a fragment, written as XHTML.
pub fn serialize(fragment: &Document) -> String {
    let mut out = String::new();
    for n in fragment.root().children() {
        node(&n, &mut out);
    }
    out
}

fn node(n: &NodeRef, out: &mut String) {
    if n.is_text() {
        out.push_str(&escape(&n.text()));
    } else if n.is_element() {
        let name = n
            .node_name()
            .map(|t| t.to_string())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if name == "html" || name == "body" {
            // a wrapper the parser added around a fragment
            for c in n.children() {
                node(&c, out);
            }
            return;
        }
        out.push('<');
        out.push_str(&name);
        for a in n.attrs() {
            let key = a.name.local.to_string();
            if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                continue;
            }
            out.push(' ');
            out.push_str(&key);
            out.push_str("=\"");
            out.push_str(&escape(&a.value));
            out.push('"');
        }
        if VOID.contains(&name.as_str()) {
            out.push_str("/>");
        } else {
            out.push('>');
            for c in n.children() {
                node(&c, out);
            }
            out.push_str("</");
            out.push_str(&name);
            out.push('>');
        }
    }
    // comments and the rest are left out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xhtml_is_well_formed() {
        let d = Document::fragment(
            "<p>a &amp; b &lt; c<br>d<img src=\"x.png\" alt=\"q&quot;r\"></p><hr><!-- c --><ul><li>1<li>2</ul>",
        );
        let s = serialize(&d);
        assert_eq!(
            s,
            "<p>a &amp; b &lt; c<br/>d<img src=\"x.png\" alt=\"q&quot;r\"/></p><hr/><ul><li>1</li><li>2</li></ul>"
        );
    }

    #[test]
    fn control_characters_are_dropped() {
        assert_eq!(escape("a\u{0}b\u{1b}c\td"), "abc\td");
    }
}
