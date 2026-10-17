//! Task markers and task property types (file-format.md §5).

use std::fmt;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Serialize, Serializer};

/// Task state marker at the start of a block's first line.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    /// `TODO`
    Todo,
    /// `LATER`: to do, like `TODO` (Logseq's words for the same step)
    Later,
    /// `DOING`
    Doing,
    /// `NOW`: in progress, like `DOING`
    Now,
    /// `WAITING`
    Waiting,
    /// `DONE`
    Done,
    /// `CANCELED`
    Canceled,
}

impl TaskState {
    /// All markers with their states, in canonical order.
    pub const ALL: [TaskState; 7] = [
        TaskState::Todo,
        TaskState::Later,
        TaskState::Doing,
        TaskState::Now,
        TaskState::Waiting,
        TaskState::Done,
        TaskState::Canceled,
    ];

    /// The marker text (`TODO`, ...).
    pub const fn marker(self) -> &'static str {
        match self {
            TaskState::Todo => "TODO",
            TaskState::Later => "LATER",
            TaskState::Doing => "DOING",
            TaskState::Now => "NOW",
            TaskState::Waiting => "WAITING",
            TaskState::Done => "DONE",
            TaskState::Canceled => "CANCELED",
        }
    }

    /// Parses a marker word (case-sensitive).
    pub fn from_marker(s: &str) -> Option<TaskState> {
        TaskState::ALL.into_iter().find(|t| t.marker() == s)
    }

    /// Lower-case name used in queries and the index (`todo`, ...).
    pub const fn name(self) -> &'static str {
        match self {
            TaskState::Todo => "todo",
            TaskState::Later => "later",
            TaskState::Doing => "doing",
            TaskState::Now => "now",
            TaskState::Waiting => "waiting",
            TaskState::Done => "done",
            TaskState::Canceled => "canceled",
        }
    }

    /// Parses the lower-case name.
    pub fn from_name(s: &str) -> Option<TaskState> {
        TaskState::ALL.into_iter().find(|t| t.name() == s)
    }
}

/// A task priority: the rank (0 = most important) and the label the graph gives that rank (`A`, `B`, `C` by
/// default; `[tasks] priorities` in the config sets how many there are and what they are called).
///
/// In a file the property is the label (`priority:: A`); `high`, `medium` and `low` are still understood and mean
/// the first, the middle and the last label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Priority {
    rank: u8,
    label: String,
}

impl serde::Serialize for Priority {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.label)
    }
}

impl Priority {
    /// `A`, `B`, `C`.
    pub fn default_labels() -> Vec<String> {
        vec!["A".into(), "B".into(), "C".into()]
    }

    /// Parses a property value against the default labels.
    pub fn parse(s: &str) -> Option<Priority> {
        Priority::parse_with(s, &[])
    }

    /// Parses a property value against the graph's labels (most important first; none = `A`, `B`, `C`). The
    /// match ignores case. `high` is the first label, `low` the last, `medium` the middle one.
    pub fn parse_with(s: &str, labels: &[String]) -> Option<Priority> {
        let defaults;
        let labels = if labels.is_empty() {
            defaults = Priority::default_labels();
            &defaults[..]
        } else {
            labels
        };
        let t = s.trim().to_lowercase();
        let n = labels.len();
        let rank = match t.as_str() {
            _ if t.is_empty() => return None,
            "high" if labels.iter().all(|l| l.to_lowercase() != "high") => 0,
            "low" if labels.iter().all(|l| l.to_lowercase() != "low") => n - 1,
            "medium" if labels.iter().all(|l| l.to_lowercase() != "medium") => n / 2,
            _ => labels.iter().position(|l| l.trim().to_lowercase() == t)?,
        };
        let rank = rank.min(n - 1);
        Some(Priority {
            rank: u8::try_from(rank).ok()?,
            label: labels[rank].trim().to_string(),
        })
    }

    /// The label as it is written in the file.
    pub fn name(&self) -> &str {
        &self.label
    }

    /// Sort rank: the most important first.
    pub const fn rank(&self) -> u8 {
        self.rank
    }
}

/// A date with an optional time of day. Dates without time are "floating" (no timezone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Moment {
    /// The day.
    pub date: NaiveDate,
    /// Local time of day, if given (the start of a range).
    pub time: Option<NaiveTime>,
    /// End of the time range on the same day, if given (`2026-10-14 09:00-10:30`); always after `time`.
    pub end: Option<NaiveTime>,
}

impl Moment {
    /// Parses `YYYY-MM-DD`, `YYYY-MM-DD HH:MM` (also with `T`, optional seconds) or a range on one day,
    /// `YYYY-MM-DD HH:MM-HH:MM` (a hyphen or an en dash, spaces around it allowed). An end that is not after the
    /// start makes the value invalid.
    pub fn parse(s: &str) -> Option<Moment> {
        let s = s.trim();
        if let Some(d) = parse_date(s) {
            return Some(Moment {
                date: d,
                time: None,
                end: None,
            });
        }
        if let Some(dt) = parse_datetime(s) {
            return Some(Moment {
                date: dt.date(),
                time: Some(dt.time()),
                end: None,
            });
        }
        // `<date> <start>-<end>`: the date is the first ten characters, so the hyphens in it do not count
        let head = s.get(..10)?;
        let rest = s.get(10..)?;
        let (start, end) = rest.split_once(['-', '–'])?;
        let start = parse_datetime(&format!("{head} {}", start.trim().trim_start_matches('T')))?;
        let end = parse_clock(end.trim())?;
        (end > start.time()).then_some(Moment {
            date: start.date(),
            time: Some(start.time()),
            end: Some(end),
        })
    }
}

/// Strict `HH:MM[:SS]`.
fn parse_clock(t: &str) -> Option<NaiveTime> {
    let tb = t.as_bytes();
    let ok = match tb.len() {
        5 => tb.get(2) == Some(&b':'),
        8 => tb.get(2) == Some(&b':') && tb.get(5) == Some(&b':'),
        _ => false,
    };
    if !ok
        || !t
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 2 || i == 5 || c.is_ascii_digit())
    {
        return None;
    }
    NaiveTime::parse_from_str(t, if tb.len() == 5 { "%H:%M" } else { "%H:%M:%S" }).ok()
}

impl fmt::Display for Moment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.time, self.end) {
            (Some(t), Some(e)) => write!(
                f,
                "{} {}-{}",
                self.date.format("%Y-%m-%d"),
                t.format("%H:%M"),
                e.format("%H:%M")
            ),
            (Some(t), None) => write!(f, "{} {}", self.date.format("%Y-%m-%d"), t.format("%H:%M")),
            _ => write!(f, "{}", self.date.format("%Y-%m-%d")),
        }
    }
}

impl Serialize for Moment {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Strict `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let b = s.as_bytes();
    if b.len() != 10 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Strict `YYYY-MM-DD[ T]HH:MM[:SS]`.
pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let (d, t) = s.split_once([' ', 'T'])?;
    let date = parse_date(d)?;
    let tb = t.as_bytes();
    let ok = match tb.len() {
        5 => tb.get(2) == Some(&b':'),
        8 => tb.get(2) == Some(&b':') && tb.get(5) == Some(&b':'),
        _ => false,
    };
    if !ok
        || !t
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 2 || i == 5 || c.is_ascii_digit())
    {
        return None;
    }
    let time =
        NaiveTime::parse_from_str(t, if tb.len() == 5 { "%H:%M" } else { "%H:%M:%S" }).ok()?;
    Some(NaiveDateTime::new(date, time))
}

/// Unit of an `every Nx` repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatUnit {
    /// Days (`d`).
    Day,
    /// Weeks (`w`).
    Week,
    /// Months (`m`).
    Month,
    /// Years (`y`).
    Year,
}

/// The recurrence rule of a repeating task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepeatRule {
    /// `daily`
    Daily,
    /// `weekly`
    Weekly,
    /// `monthly`
    Monthly,
    /// `yearly`
    Yearly,
    /// `weekdays` (Monday to Friday)
    Weekdays,
    /// `every 2w`
    Every {
        /// Interval, at least 1.
        n: u32,
        /// Unit.
        unit: RepeatUnit,
    },
    /// Raw `RRULE:...` (everything after `RRULE:`).
    Rrule {
        /// The rule text.
        rule: String,
    },
}

/// `repeat::` value: a rule plus whether the next date counts from completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Repeat {
    /// The rule.
    pub rule: RepeatRule,
    /// `from-completion` suffix: next occurrence is relative to the completion date.
    pub from_completion: bool,
}

impl Repeat {
    /// Parses a `repeat::` value; `None` if it is not one of the documented forms.
    pub fn parse(raw: &str) -> Option<Repeat> {
        let s = raw.trim();
        let (body, from_completion) = match s.strip_suffix("from-completion") {
            Some(b) if b.ends_with(char::is_whitespace) => (b.trim(), true),
            _ => (s, false),
        };
        let rule = match body {
            "daily" => RepeatRule::Daily,
            "weekly" => RepeatRule::Weekly,
            "monthly" => RepeatRule::Monthly,
            "yearly" => RepeatRule::Yearly,
            "weekdays" => RepeatRule::Weekdays,
            _ => {
                if let Some(rest) = body.strip_prefix("RRULE:") {
                    if rest.trim().is_empty() {
                        return None;
                    }
                    RepeatRule::Rrule {
                        rule: rest.trim().to_string(),
                    }
                } else {
                    let rest = body.strip_prefix("every ")?.trim();
                    let unit = match rest.chars().last()? {
                        'd' => RepeatUnit::Day,
                        'w' => RepeatUnit::Week,
                        'm' => RepeatUnit::Month,
                        'y' => RepeatUnit::Year,
                        _ => return None,
                    };
                    let digits = rest.get(..rest.len() - 1)?;
                    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    let n: u32 = digits.parse().ok()?;
                    if n == 0 {
                        return None;
                    }
                    RepeatRule::Every { n, unit }
                }
            }
        };
        Some(Repeat {
            rule,
            from_completion,
        })
    }
}

/// A parsed task: marker plus the task properties of the block.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Task {
    /// Current state.
    pub state: TaskState,
    /// Span of the marker word in the source.
    pub marker_span: crate::Span,
    /// `scheduled::`
    pub scheduled: Option<Moment>,
    /// `deadline::`
    pub deadline: Option<Moment>,
    /// `priority::`
    pub priority: Option<Priority>,
    /// `repeat::`
    pub repeat: Option<Repeat>,
    /// `done::`
    pub done: Option<NaiveDateTime>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_moment_has_an_optional_start_and_end_time() {
        let m = Moment::parse("2026-10-14").unwrap();
        assert_eq!((m.time, m.end), (None, None));
        assert_eq!(m.to_string(), "2026-10-14");
        let m = Moment::parse("2026-10-14 09:00").unwrap();
        assert_eq!(m.end, None);
        assert_eq!(m.to_string(), "2026-10-14 09:00");
        let m = Moment::parse("2026-10-14 09:00-10:30").unwrap();
        assert_eq!(m.to_string(), "2026-10-14 09:00-10:30");
        // an en dash, spaces and a `T` are read, and written the canonical way
        for s in [
            "2026-10-14 09:00 – 10:30",
            "2026-10-14 09:00 - 10:30",
            "2026-10-14T09:00-10:30",
            "2026-10-14 09:00:00-10:30:00",
        ] {
            assert_eq!(
                Moment::parse(s).unwrap().to_string(),
                "2026-10-14 09:00-10:30",
                "{s}"
            );
        }
        // the end must be after the start
        for s in [
            "2026-10-14 10:30-09:00",
            "2026-10-14 09:00-09:00",
            "2026-10-14 09:00-",
            "2026-10-14 -10:30",
            "2026-10-14 9:00-10:30",
        ] {
            assert_eq!(Moment::parse(s), None, "{s}");
        }
        // ordering: the date first, then the start
        assert!(Moment::parse("2026-10-14 09:00-10:00") < Moment::parse("2026-10-14 10:00"));
    }

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("daily", true)]
    #[case("weekly", true)]
    #[case("monthly", true)]
    #[case("yearly", true)]
    #[case("weekdays", true)]
    #[case("every 2w", true)]
    #[case("every 10d", true)]
    #[case("weekly from-completion", true)]
    #[case("RRULE:FREQ=WEEKLY;BYDAY=MO", true)]
    #[case("every 0w", false)]
    #[case("every w", false)]
    #[case("every 2x", false)]
    #[case("sometimes", false)]
    #[case("RRULE:", false)]
    fn repeat_values(#[case] raw: &str, #[case] ok: bool) {
        assert_eq!(Repeat::parse(raw).is_some(), ok, "{raw}");
    }

    #[test]
    fn moments() {
        assert_eq!(
            Moment::parse("2026-10-05").unwrap().to_string(),
            "2026-10-05"
        );
        assert_eq!(
            Moment::parse("2026-10-05 14:00").unwrap().to_string(),
            "2026-10-05 14:00"
        );
        assert_eq!(
            Moment::parse("2026-10-05T14:00").unwrap().to_string(),
            "2026-10-05 14:00"
        );
        assert!(Moment::parse("2026-13-01").is_none());
        assert!(Moment::parse("2026-10-05 25:00").is_none());
        assert!(Moment::parse("tomorrow").is_none());
    }
}
