//! Resolution of relative dates against an injected "today" (core never reads the clock).

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::Serialize;

use super::ast::{DateExpr, DateUnit, RelDate};

/// First day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WeekStart {
    /// Monday (default).
    #[default]
    Monday,
    /// Sunday.
    Sunday,
}

/// An inclusive range of days. Single dates have `start == end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DateRange {
    /// First day.
    pub start: NaiveDate,
    /// Last day (inclusive).
    pub end: NaiveDate,
}

impl DateRange {
    fn day(d: NaiveDate) -> Self {
        DateRange { start: d, end: d }
    }
}

fn add_days(d: NaiveDate, n: i64) -> NaiveDate {
    let days = Days::new(n.unsigned_abs());
    let r = if n >= 0 {
        d.checked_add_days(days)
    } else {
        d.checked_sub_days(days)
    };
    r.unwrap_or(d)
}

fn add_months(d: NaiveDate, n: i64) -> NaiveDate {
    let m = Months::new(u32::try_from(n.unsigned_abs()).unwrap_or(u32::MAX));
    let r = if n >= 0 {
        d.checked_add_months(m)
    } else {
        d.checked_sub_months(m)
    };
    r.unwrap_or(d)
}

fn week_start_of(d: NaiveDate, ws: WeekStart) -> NaiveDate {
    let back = match ws {
        WeekStart::Monday => d.weekday().num_days_from_monday(),
        WeekStart::Sunday => d.weekday().num_days_from_sunday(),
    };
    add_days(d, -i64::from(back))
}

impl DateExpr {
    /// Resolves to a range of days. Absolute dates and offsets are single days;
    /// `this-week`, `next-week` and `this-month` are ranges.
    pub fn resolve(&self, today: NaiveDate, week_start: WeekStart) -> DateRange {
        match self {
            DateExpr::Abs(d) => DateRange::day(*d),
            DateExpr::Rel(r) => match r {
                RelDate::Today => DateRange::day(today),
                RelDate::Yesterday => DateRange::day(add_days(today, -1)),
                RelDate::Tomorrow => DateRange::day(add_days(today, 1)),
                RelDate::Offset { n, unit } => {
                    let n = i64::from(*n);
                    DateRange::day(match unit {
                        DateUnit::Day => add_days(today, n),
                        DateUnit::Week => add_days(today, n * 7),
                        DateUnit::Month => add_months(today, n),
                        DateUnit::Year => add_months(today, n * 12),
                    })
                }
                RelDate::ThisWeek | RelDate::NextWeek => {
                    let mut start = week_start_of(today, week_start);
                    if *r == RelDate::NextWeek {
                        start = add_days(start, 7);
                    }
                    DateRange {
                        start,
                        end: add_days(start, 6),
                    }
                }
                RelDate::ThisMonth => {
                    let start = today.with_day(1).unwrap_or(today);
                    let end = add_days(add_months(start, 1), -1);
                    DateRange { start, end }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::parse::parse_date_expr;
    use rstest::rstest;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[rstest]
    #[case("today", d(2026, 10, 3), d(2026, 10, 3), d(2026, 10, 3))]
    #[case("yesterday", d(2026, 10, 3), d(2026, 10, 2), d(2026, 10, 2))]
    #[case("tomorrow", d(2026, 10, 3), d(2026, 10, 4), d(2026, 10, 4))]
    #[case("+7d", d(2026, 10, 3), d(2026, 10, 10), d(2026, 10, 10))]
    #[case("-2w", d(2026, 10, 3), d(2026, 9, 19), d(2026, 9, 19))]
    #[case("+1m", d(2026, 1, 31), d(2026, 2, 28), d(2026, 2, 28))]
    #[case("-1m", d(2026, 3, 31), d(2026, 2, 28), d(2026, 2, 28))]
    #[case("+1y", d(2028, 2, 29), d(2029, 2, 28), d(2029, 2, 28))]
    #[case("this-week", d(2026, 10, 3), d(2026, 9, 28), d(2026, 10, 4))]
    #[case("next-week", d(2026, 10, 3), d(2026, 10, 5), d(2026, 10, 11))]
    #[case("this-week", d(2026, 10, 5), d(2026, 10, 5), d(2026, 10, 11))]
    #[case("this-month", d(2026, 2, 10), d(2026, 2, 1), d(2026, 2, 28))]
    #[case("this-month", d(2028, 2, 10), d(2028, 2, 1), d(2028, 2, 29))]
    #[case("2026-12-24", d(2026, 10, 3), d(2026, 12, 24), d(2026, 12, 24))]
    fn resolves(
        #[case] expr: &str,
        #[case] today: NaiveDate,
        #[case] start: NaiveDate,
        #[case] end: NaiveDate,
    ) {
        let e = parse_date_expr(expr).unwrap();
        assert_eq!(
            e.resolve(today, WeekStart::Monday),
            DateRange { start, end },
            "{expr}"
        );
    }

    #[test]
    fn sunday_week_start() {
        let e = parse_date_expr("this-week").unwrap();
        let r = e.resolve(d(2026, 10, 3), WeekStart::Sunday);
        assert_eq!((r.start, r.end), (d(2026, 9, 27), d(2026, 10, 3)));
    }
}
