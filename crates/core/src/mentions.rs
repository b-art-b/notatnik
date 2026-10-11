//! Plain-text mentions of a page name (unlinked references, LN-4).
//!
//! A mention is a whole-word, case- and diacritic-insensitive occurrence of the name in a block's text that is
//! not already part of markup: inside `[[...]]`, `((...))`, a `#tag`, inline code, a fenced code block or a URL.

use crate::names::search_fold;
use std::ops::Range;

/// Marks (per char) the parts of `chars` that are markup and must not count as a mention.
fn protected(chars: &[(usize, char)]) -> Vec<bool> {
    let n = chars.len();
    let mut mask = vec![false; n];
    let at = |i: usize| chars.get(i).map(|(_, c)| *c);
    let mut fence = false;
    let mut line_start = true;
    let mut i = 0;
    while i < n {
        let c = chars[i].1;
        if line_start {
            // a fence line toggles the fenced state; the line itself is protected
            let mut j = i;
            while matches!(at(j), Some(' ' | '\t')) {
                j += 1;
            }
            if at(j) == Some('`') && at(j + 1) == Some('`') && at(j + 2) == Some('`') {
                fence = !fence;
                while i < n && chars[i].1 != '\n' {
                    mask[i] = true;
                    i += 1;
                }
                line_start = false;
                continue;
            }
        }
        // a property line (`key:: value`) is data, not prose
        if line_start && c != '\n' {
            let end = (i..n).find(|&k| chars[k].1 == '\n').unwrap_or(n);
            let line: String = chars[i..end].iter().map(|(_, c)| *c).collect();
            if crate::props::split_property_line(line.trim_start()).is_some() {
                mask[i..end].iter_mut().for_each(|m| *m = true);
                i = end;
                continue;
            }
        }
        if fence {
            mask[i] = true;
            line_start = c == '\n';
            i += 1;
            continue;
        }
        line_start = c == '\n';
        // `[[...]]`, `((...))`, inline code `...`
        let close: Option<&[char]> = match (c, at(i + 1)) {
            ('[', Some('[')) => Some(&[']', ']']),
            ('(', Some('(')) => Some(&[')', ')']),
            ('`', _) => Some(&['`']),
            _ => None,
        };
        if let Some(close) = close {
            let open_len = if c == '`' { 1 } else { 2 };
            let mut j = i + open_len;
            let mut end = None;
            while j + close.len() <= n {
                if (0..close.len()).all(|k| at(j + k) == Some(close[k])) {
                    end = Some(j + close.len());
                    break;
                }
                if at(j) == Some('\n') && c != '`' {
                    break;
                }
                j += 1;
            }
            if let Some(e) = end {
                mask[i..e].iter_mut().for_each(|m| *m = true);
                i = e;
                continue;
            }
        }
        // `#tag` and URLs run to the next whitespace
        let tag = c == '#' && (i == 0 || !at(i - 1).is_some_and(char::is_alphanumeric));
        let url = (c == 'h' && (i == 0 || at(i - 1).is_some_and(char::is_whitespace)))
            && (0..4).all(|k| at(i + k) == "http".chars().nth(k))
            && matches!(
                (at(i + 4), at(i + 5)),
                (Some(':'), Some('/')) | (Some('s'), Some(':'))
            );
        if tag || url {
            while i < n && !chars[i].1.is_whitespace() {
                mask[i] = true;
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    mask
}

/// The first mention of `term` in `text` (byte range in `text`), or `None`.
pub fn find_mention(text: &str, term: &str) -> Option<Range<usize>> {
    let needle: Vec<char> = search_fold(term.trim()).chars().collect();
    if needle.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let folded: Vec<char> = chars
        .iter()
        .map(|(_, c)| search_fold(&c.to_string()).chars().next().unwrap_or(*c))
        .collect();
    let mask = protected(&chars);
    let word = |i: usize| folded.get(i).is_some_and(|c| c.is_alphanumeric());
    let mut i = 0;
    while i + needle.len() <= folded.len() {
        if folded.get(i..i + needle.len()) == Some(&needle[..])
            && (i == 0 || !(word(i - 1) || folded[i - 1] == '@'))
            && !(word(i + needle.len()) || folded.get(i + needle.len()) == Some(&'@'))
            && mask
                .get(i..i + needle.len())
                .is_some_and(|m| m.iter().all(|p| !p))
        {
            let start = chars[i].0;
            let end = chars.get(i + needle.len()).map_or(text.len(), |(b, _)| *b);
            return Some(start..end);
        }
        i += 1;
    }
    None
}

/// `text` with the first mention of `term` turned into a link to `target`: `[[target]]` if the mention is
/// written exactly like the target, else `[[target|mention]]`. `None` if there is no mention.
pub fn link_first_mention(text: &str, term: &str, target: &str) -> Option<String> {
    let r = find_mention(text, term)?;
    let mention = text.get(r.clone())?;
    let link = if mention == target {
        format!("[[{target}]]")
    } else {
        format!("[[{target}|{mention}]]")
    };
    Some(format!(
        "{}{}{}",
        text.get(..r.start)?,
        link,
        text.get(r.end..)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str, term: &str) -> Option<String> {
        find_mention(text, term).map(|r| text[r].to_string())
    }

    #[test]
    fn whole_words_case_insensitive() {
        assert_eq!(found("met anna today", "Anna").as_deref(), Some("anna"));
        assert_eq!(found("Anna's idea", "Anna").as_deref(), Some("Anna"));
        assert_eq!(found("Annabelle came", "Anna"), None);
        assert_eq!(found("Joanna came", "Anna"), None);
        assert_eq!(found("Zażółć gęślą", "zazolc").as_deref(), Some("Zażółć"));
        assert_eq!(
            found("two words here", "words here").as_deref(),
            Some("words here")
        );
    }

    #[test]
    fn markup_is_not_a_mention() {
        assert_eq!(found("see [[Anna]] now", "Anna"), None);
        assert_eq!(found("see [[Anna|her]] now", "Anna"), None);
        assert_eq!(found("tag #Anna here", "Anna"), None);
        assert_eq!(found("code `Anna` here", "Anna"), None);
        assert_eq!(
            found("ref ((abc123)) Anna", "Anna").as_deref(),
            Some("Anna")
        );
        assert_eq!(found("https://example.com/Anna ok", "Anna"), None);
        assert_eq!(found("```\nAnna\n```\nplain", "Anna"), None);
        assert_eq!(found("Note\nemail:: anna@example.com", "Anna"), None);
        assert_eq!(found("mail anna@example.com now", "Anna"), None);
        assert_eq!(
            found("Note\nowner:: Anna\nAnna again", "Anna").map(|s| s.len()),
            Some(4)
        );
        assert_eq!(
            found("```\nAnna\n```\nAnna again", "Anna").as_deref(),
            Some("Anna")
        );
        // a mention after a link counts
        assert_eq!(found("[[Anna]] and Anna", "Anna").as_deref(), Some("Anna"));
    }

    #[test]
    fn links_only_the_first_mention() {
        assert_eq!(
            link_first_mention("met anna and Anna", "Anna", "Anna").unwrap(),
            "met [[Anna|anna]] and Anna"
        );
        assert_eq!(
            link_first_mention("Anna's idea", "Anna", "Anna").unwrap(),
            "[[Anna]]'s idea"
        );
        assert_eq!(link_first_mention("nothing", "Anna", "Anna"), None);
    }
}
