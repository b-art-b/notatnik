//! Task markers and task property types (file-format.md §5).

use std::fmt;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Serialize, Serializer};

/// Task state marker at the start of a block's first line.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, serde::Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    /// `TODO`
    Todo,
    /// `DOING`
    Doing,
    /// `WAITING`
    Waiting,
    /// `DONE`
    Done,
    /// `CANCELED`
    Canceled,
}

impl TaskState {
    /// All markers with their states, in canonical order.
    pub const ALL: [TaskState; 5] = [
        TaskState::Todo,
        TaskState::Doing,
        TaskState::Waiting,
        TaskState::Done,
        TaskState::Canceled,
    ];

    /// The marker text (`TODO`, ...).
    pub const fn marker(self) -> &'static str {
        match self {
            TaskState::Todo => "TODO",
            TaskState::Doing => "DOING",
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
            TaskState::Doing => "doing",
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

/// Task priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// `high`
    High,
    /// `medium`
    Medium,
    /// `low`
    Low,
}

impl Priority {
    /// Parses `high`, `medium` or `low`.
    pub fn parse(s: &str) -> Option<Priority> {
        match s {
            "high" => Some(Priority::High),
            "medium" => Some(Priority::Medium),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }

    /// The property text.
    pub const fn name(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    /// Sort rank: high first.
    pub const fn rank(self) -> u8 {
        match self {
            Priority::High => 0,
            Priority::Medium => 1,
            Priority::Low => 2,
        }
    }
}

/// A date with an optional time of day. Dates without time are "floating" (no timezone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Moment {
    /// The day.
    pub date: NaiveDate,
    /// Local time of day, if given.
    pub time: Option<NaiveTime>,
}

impl Moment {
    /// Parses `YYYY-MM-DD` or `YYYY-MM-DD HH:MM` (also with `T`, optional seconds).
    pub fn parse(s: &str) -> Option<Moment> {
        let s = s.trim();
        if let Some(d) = parse_date(s) {
            return Some(Moment {
                date: d,
                time: None,
            });
        }
        parse_datetime(s).map(|dt| Moment {
            date: dt.date(),
            time: Some(dt.time()),
        })
    }
}

impl fmt::Display for Moment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.time {
            Some(t) => write!(f, "{} {}", self.date.format("%Y-%m-%d"), t.format("%H:%M")),
            None => write!(f, "{}", self.date.format("%Y-%m-%d")),
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
