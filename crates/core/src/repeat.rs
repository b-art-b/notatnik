//! Next occurrence of a repeating task when it is completed (P3.15).
//!
//! Default semantics ("next future occurrence", like org-mode `++`): the next date of the series that
//! is after both the old date and the completion date. `from-completion` counts from the completion
//! date instead. `scheduled` and `deadline` keep their distance. Raw `RRULE:` values aren't shifted.

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

use crate::{Moment, Repeat, RepeatRule, RepeatUnit};

fn add_units(d: NaiveDate, n: u32, unit: RepeatUnit) -> NaiveDate {
    match unit {
        RepeatUnit::Day => d.checked_add_days(Days::new(u64::from(n))),
        RepeatUnit::Week => d.checked_add_days(Days::new(u64::from(n) * 7)),
        RepeatUnit::Month => d.checked_add_months(Months::new(n)),
        RepeatUnit::Year => d.checked_add_months(Months::new(n.saturating_mul(12))),
    }
    .unwrap_or(d)
}

fn is_weekday(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

/// (n, unit) of the rule's step, or `None` for `weekdays` and raw rules.
fn step(rule: &RepeatRule) -> Option<(u32, RepeatUnit)> {
    Some(match rule {
        RepeatRule::Daily => (1, RepeatUnit::Day),
        RepeatRule::Weekly => (1, RepeatUnit::Week),
        RepeatRule::Monthly => (1, RepeatUnit::Month),
        RepeatRule::Yearly => (1, RepeatUnit::Year),
        RepeatRule::Every { n, unit } => (*n, *unit),
        RepeatRule::Weekdays | RepeatRule::Rrule { .. } => return None,
    })
}

fn next_weekday_after(mut d: NaiveDate) -> NaiveDate {
    loop {
        d = d.checked_add_days(Days::new(1)).unwrap_or(d);
        if is_weekday(d) {
            return d;
        }
    }
}

/// The next date for a task whose current (earliest) date is `base`, completed on `completed`.
pub fn next_date(repeat: &Repeat, base: NaiveDate, completed: NaiveDate) -> Option<NaiveDate> {
    if repeat.from_completion {
        return Some(match step(&repeat.rule) {
            Some((n, unit)) => add_units(completed, n, unit),
            None if matches!(repeat.rule, RepeatRule::Weekdays) => next_weekday_after(completed),
            None => return None,
        });
    }
    match step(&repeat.rule) {
        Some((n, unit)) => {
            // k-th occurrence counted from the original date, so month-end clamping doesn't drift
            for k in 1..=100_000u32 {
                let d = add_units(base, n.saturating_mul(k), unit);
                if d > completed {
                    return Some(d);
                }
            }
            None
        }
        None if matches!(repeat.rule, RepeatRule::Weekdays) => {
            let mut d = next_weekday_after(base);
            while d <= completed {
                d = next_weekday_after(d);
            }
            Some(d)
        }
        None => None,
    }
}

/// New `scheduled` and `deadline` after completing on `completed`. `None` when the rule can't be shifted
/// or the task has no date.
pub fn advance(
    repeat: &Repeat,
    scheduled: Option<Moment>,
    deadline: Option<Moment>,
    completed: NaiveDate,
) -> Option<(Option<Moment>, Option<Moment>)> {
    let base = scheduled.or(deadline)?;
    let new_base = next_date(repeat, base.date, completed)?;
    let shifted = |m: Moment, date: NaiveDate| Moment { date, ..m };
    Some(match (scheduled, deadline) {
        (Some(s), Some(d)) => {
            let delta = d.date.signed_duration_since(s.date).num_days();
            let nd = if delta >= 0 {
                new_base.checked_add_days(Days::new(delta as u64))
            } else {
                new_base.checked_sub_days(Days::new((-delta) as u64))
            }
            .unwrap_or(new_base);
            (Some(shifted(s, new_base)), Some(shifted(d, nd)))
        }
        (Some(s), None) => (Some(shifted(s, new_base)), None),
        (None, Some(d)) => (None, Some(shifted(d, new_base))),
        (None, None) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[rstest]
    #[case("daily", "2026-10-01", "2026-10-03", "2026-10-04")]
    #[case("daily", "2026-10-03", "2026-10-03", "2026-10-04")]
    #[case("daily", "2026-10-09", "2026-10-03", "2026-10-10")] // completed early: next after the scheduled date
    #[case("weekly", "2026-09-28", "2026-10-03", "2026-10-05")]
    #[case("weekly", "2026-10-05", "2026-10-05", "2026-10-12")]
    #[case("monthly", "2026-01-31", "2026-02-02", "2026-02-28")]
    #[case("monthly", "2026-01-31", "2026-03-01", "2026-03-31")] // no drift after the clamp
    #[case("monthly", "2026-10-15", "2026-10-03", "2026-11-15")]
    #[case("yearly", "2024-02-29", "2024-03-01", "2025-02-28")]
    #[case("weekdays", "2026-10-02", "2026-10-02", "2026-10-05")]
    #[case("weekdays", "2026-10-05", "2026-10-05", "2026-10-06")]
    #[case("weekdays", "2026-09-28", "2026-10-03", "2026-10-05")]
    #[case("every 2w", "2026-09-21", "2026-10-03", "2026-10-05")]
    #[case("every 3d", "2026-10-01", "2026-10-01", "2026-10-04")]
    #[case("every 2m", "2026-01-31", "2026-02-01", "2026-03-31")]
    #[case("weekly from-completion", "2026-09-28", "2026-10-03", "2026-10-10")]
    #[case("daily from-completion", "2026-09-01", "2026-10-03", "2026-10-04")]
    #[case("every 2w from-completion", "2026-09-28", "2026-10-03", "2026-10-17")]
    #[case("weekdays from-completion", "2026-09-28", "2026-10-02", "2026-10-05")]
    fn next_dates(
        #[case] repeat: &str,
        #[case] base: &str,
        #[case] done: &str,
        #[case] want: &str,
    ) {
        let r = Repeat::parse(repeat).unwrap();
        assert_eq!(
            next_date(&r, d(base), d(done)),
            Some(d(want)),
            "{repeat} {base} done {done}"
        );
    }

    #[test]
    fn raw_rules_are_not_shifted() {
        let r = Repeat::parse("RRULE:FREQ=WEEKLY;BYDAY=MO").unwrap();
        assert_eq!(next_date(&r, d("2026-10-05"), d("2026-10-05")), None);
    }

    #[test]
    fn scheduled_and_deadline_keep_their_distance_and_times() {
        let r = Repeat::parse("weekly").unwrap();
        let s = Moment::parse("2026-09-28 09:30-10:15");
        let dl = Moment::parse("2026-10-01 17:00");
        let (ns, nd) = advance(&r, s, dl, d("2026-10-03")).unwrap();
        assert_eq!(ns.unwrap().to_string(), "2026-10-05 09:30-10:15");
        assert_eq!(nd.unwrap().to_string(), "2026-10-08 17:00");
        // only a deadline
        let (ns, nd) = advance(&r, None, Moment::parse("2026-09-30"), d("2026-10-03")).unwrap();
        assert!(ns.is_none());
        assert_eq!(nd.unwrap().to_string(), "2026-10-07");
        assert!(advance(&r, None, None, d("2026-10-03")).is_none());
    }
}
