//! Turning the bytes of a page into text: a byte order mark first, then the `charset` of the header, then the one the
//! page names itself in its first 2 KB, otherwise UTF-8.

use encoding_rs::{Encoding, UTF_8};

pub fn decode(bytes: &[u8], content_type: &str) -> String {
    let enc = Encoding::for_bom(bytes)
        .map(|(e, _)| e)
        .or_else(|| charset_in(content_type))
        .or_else(|| {
            charset_in(
                &String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).to_ascii_lowercase(),
            )
        })
        .unwrap_or(UTF_8);
    enc.decode(bytes).0.into_owned()
}

/// The encoding named after `charset=` in `text` (a header value, or the start of a page).
fn charset_in(text: &str) -> Option<&'static Encoding> {
    let lower = text.to_ascii_lowercase();
    let i = lower.find("charset")? + "charset".len();
    let rest = lower[i..].trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.trim_start_matches(['"', '\'']);
    let label: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
        .collect();
    Encoding::for_label(label.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_meta_and_bom_decide() {
        // "żółć" in ISO-8859-2
        let latin2 = [0xbf, 0xf3, 0xb3, 0xe6];
        assert_eq!(decode(&latin2, "text/html; charset=iso-8859-2"), "żółć");
        let mut page = b"<html><head><meta charset=\"iso-8859-2\"></head><body>".to_vec();
        page.extend_from_slice(&latin2);
        assert!(decode(&page, "text/html").ends_with("żółć"));
        let mut http_equiv =
            b"<meta http-equiv=\"Content-Type\" content=\"text/html; charset=ISO-8859-2\">"
                .to_vec();
        http_equiv.extend_from_slice(&latin2);
        assert!(decode(&http_equiv, "").ends_with("żółć"));
        // a byte order mark wins over a wrong header
        let mut bom = vec![0xef, 0xbb, 0xbf];
        bom.extend_from_slice("żółć".as_bytes());
        assert_eq!(decode(&bom, "text/html; charset=iso-8859-2"), "żółć");
        // nothing named: UTF-8
        assert_eq!(decode("żółć".as_bytes(), "text/html"), "żółć");
        assert_eq!(decode(b"abc", "text/html; charset=nonsense"), "abc");
    }
}
