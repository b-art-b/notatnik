//! Task logbook: dated progress notes kept in a drawer inside a block (file-format.md §3.6).
//!
//! ```text
//! - DOING Write the spec
//!   :LOGBOOK:
//!   2026-10-04 14:30 Outline done
//!   2026-10-05 Reviewed with Anna
//!   :END:
//! ```
//!
//! The drawer is made of ordinary continuation lines, so the outline parser needs no special case and
//! round-trips it byte for byte. Only the renderer hides it and the UI lists it.

use std::ops::Range;

/// The line that opens a drawer.
pub const DRAWER_START: &str = ":LOGBOOK:";
/// The line that closes it.
pub const DRAWER_END: &str = ":END:";

/// One logbook line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM`; empty if the line doesn't start with a date.
    pub when: String,
    /// The note (the rest of the line).
    pub text: String,
}

/// Finds the drawer in the continuation lines of a block: the index range from the `:LOGBOOK:` line to the
/// `:END:` line, both included. A drawer without `:END:` is not a drawer (it stays plain text).
pub fn find<S: AsRef<str>>(lines: &[S]) -> Option<Range<usize>> {
    let start = lines
        .iter()
        .position(|l| l.as_ref().trim() == DRAWER_START)?;
    let end = lines
        .get(start + 1..)?
        .iter()
        .position(|l| l.as_ref().trim() == DRAWER_END)?;
    Some(start..start + 1 + end + 1)
}

/// Parses one entry line.
pub fn parse_entry(line: &str) -> LogEntry {
    let line = line.trim();
    let b = line.as_bytes();
    let digits = |r: Range<usize>| {
        b.get(r)
            .is_some_and(|s| !s.is_empty() && s.iter().all(u8::is_ascii_digit))
    };
    let date_ok = line.len() >= 10
        && digits(0..4)
        && b.get(4) == Some(&b'-')
        && digits(5..7)
        && b.get(7) == Some(&b'-')
        && digits(8..10);
    if !date_ok {
        return LogEntry {
            when: String::new(),
            text: line.to_string(),
        };
    }
    let time_ok = line.len() >= 16
        && b.get(10) == Some(&b' ')
        && digits(11..13)
        && b.get(13) == Some(&b':')
        && digits(14..16);
    let end = if time_ok { 16 } else { 10 };
    // the date must end the token: `2026-10-045` is text
    if line
        .as_bytes()
        .get(end)
        .is_some_and(|c| !c.is_ascii_whitespace())
    {
        return LogEntry {
            when: String::new(),
            text: line.to_string(),
        };
    }
    LogEntry {
        when: line.get(..end).unwrap_or("").to_string(),
        text: line.get(end..).unwrap_or("").trim().to_string(),
    }
}

/// The entries of the drawer in `lines` (blank lines skipped), oldest first as written.
pub fn entries<S: AsRef<str>>(lines: &[S]) -> Vec<LogEntry> {
    let Some(r) = find(lines) else {
        return Vec::new();
    };
    lines
        .get(r.start + 1..r.end - 1)
        .unwrap_or(&[])
        .iter()
        .filter(|l| !l.as_ref().trim().is_empty())
        .map(|l| parse_entry(l.as_ref()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_drawer() {
        let lines = ["a", ":LOGBOOK:", "2026-10-04 x", ":END:", "b"];
        assert_eq!(find(&lines), Some(1..4));
        assert_eq!(find(&["a", "b"]), None);
        // unterminated: plain text
        assert_eq!(find(&[":LOGBOOK:", "2026-10-04 x"]), None);
        // indentation does not matter
        assert_eq!(find(&["  :LOGBOOK:", "  :END:"]), Some(0..2));
    }

    #[test]
    fn parses_entries() {
        let e = parse_entry("2026-10-04 14:30 Outline done");
        assert_eq!(
            (e.when.as_str(), e.text.as_str()),
            ("2026-10-04 14:30", "Outline done")
        );
        let e = parse_entry("  2026-10-05 Reviewed ");
        assert_eq!(
            (e.when.as_str(), e.text.as_str()),
            ("2026-10-05", "Reviewed")
        );
        let e = parse_entry("2026-10-06");
        assert_eq!((e.when.as_str(), e.text.as_str()), ("2026-10-06", ""));
        let e = parse_entry("no date here");
        assert_eq!((e.when.as_str(), e.text.as_str()), ("", "no date here"));
        let e = parse_entry("2026-10-045 odd");
        assert_eq!(e.when, "");
    }

    #[test]
    fn lists_entries_skipping_blanks() {
        let lines = [
            ":LOGBOOK:",
            "2026-10-04 a",
            "",
            "2026-10-05 08:00 b",
            ":END:",
        ];
        let e = entries(&lines);
        assert_eq!(e.len(), 2);
        assert_eq!(e[1].when, "2026-10-05 08:00");
    }
}
