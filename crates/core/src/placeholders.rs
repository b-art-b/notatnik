//! Date and time placeholders of templates and queries (`{{date+7d}}`, `{{tomorrow}}`, `{{monday}}`, `{{time}}`,
//! ...). Pure: the day and the time of day are passed in. See `docs/placeholders.md`.
//!
//! The names are matched without regard to case and to spaces around `+`/`-`. A name that is not one of these is
//! left as it is, so other placeholders (`{{page}}`, `{{title}}`) can be filled in by their own code.

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday", Weekday::Mon),
    ("tuesday", Weekday::Tue),
    ("wednesday", Weekday::Wed),
    ("thursday", Weekday::Thu),
    ("friday", Weekday::Fri),
    ("saturday", Weekday::Sat),
    ("sunday", Weekday::Sun),
];

fn iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// `base` moved by `n` units (`d` days, `w` weeks, `m` months, `y` years); a month end is clamped
/// (`2026-01-31` + 1m is `2026-02-28`).
fn shift(base: NaiveDate, n: i64, unit: char) -> Option<NaiveDate> {
    let back = n < 0;
    let k = n.unsigned_abs();
    match unit {
        'd' | 'w' => {
            let days = Days::new(if unit == 'w' { k.checked_mul(7)? } else { k });
            if back {
                base.checked_sub_days(days)
            } else {
                base.checked_add_days(days)
            }
        }
        'm' | 'y' => {
            let months =
                Months::new(u32::try_from(if unit == 'y' { k.checked_mul(12)? } else { k }).ok()?);
            if back {
                base.checked_sub_months(months)
            } else {
                base.checked_add_months(months)
            }
        }
        _ => None,
    }
}

/// `date+7d`, `date - 2w`: the offset part, `+7d` → (7, 'd').
fn offset(rest: &str) -> Option<(i64, char)> {
    let rest: String = rest.chars().filter(|c| !c.is_whitespace()).collect();
    let sign = match rest.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let body = &rest[1..];
    let unit = body.chars().last()?;
    let digits = &body[..body.len() - unit.len_utf8()];
    if digits.is_empty() || digits.len() > 5 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((sign * digits.parse::<i64>().ok()?, unit))
}

/// What the placeholder `name` (the text between the braces) stands for, relative to `day`. `time` is the time of
/// day as `HH:MM`; without it `{{time}}` and `{{datetime}}` are not known.
pub fn date_var(name: &str, day: NaiveDate, time: Option<&str>) -> Option<String> {
    let n = name.trim().to_lowercase();
    match n.as_str() {
        "date" => return Some(iso(day)),
        "yesterday" => return shift(day, -1, 'd').map(iso),
        "tomorrow" => return shift(day, 1, 'd').map(iso),
        "weekday" => return Some(day.format("%A").to_string()),
        "week" => {
            let w = day.iso_week();
            return Some(format!("{}-W{:02}", w.year(), w.week()));
        }
        "month" => return Some(day.format("%Y-%m").to_string()),
        "year" => return Some(day.format("%Y").to_string()),
        "time" => return time.map(str::to_string),
        "datetime" => return time.map(|t| format!("{} {t}", iso(day))),
        _ => {}
    }
    if let Some(rest) = n.strip_prefix("date") {
        let (k, unit) = offset(rest)?;
        return shift(day, k, unit).map(iso);
    }
    // `monday`: that day of the week of `day` (Monday to Sunday); `next monday`: the first one after `day`
    let (next, word) = match n.strip_prefix("next ") {
        Some(w) => (true, w.trim()),
        None => (false, n.as_str()),
    };
    let target = WEEKDAYS.iter().find(|(w, _)| *w == word)?.1;
    if next {
        let ahead = (7 + target.num_days_from_monday() - day.weekday().num_days_from_monday()) % 7;
        shift(day, i64::from(if ahead == 0 { 7 } else { ahead }), 'd').map(iso)
    } else {
        let back = i64::from(day.weekday().num_days_from_monday());
        shift(day, i64::from(target.num_days_from_monday()) - back, 'd').map(iso)
    }
}

/// Replaces every `{{name}}` that [`date_var`] knows; everything else stays.
pub fn expand_dates(text: &str, day: NaiveDate, time: Option<&str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                match date_var(&after[..end], day, time) {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // a Wednesday
    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()
    }
    fn x(s: &str) -> String {
        expand_dates(s, d(), Some("09:05"))
    }

    #[test]
    fn plain_names() {
        assert_eq!(x("{{date}}"), "2026-10-14");
        assert_eq!(x("{{yesterday}} {{tomorrow}}"), "2026-10-13 2026-10-15");
        assert_eq!(
            x("{{weekday}}, {{week}}, {{month}}, {{year}}"),
            "Wednesday, 2026-W42, 2026-10, 2026"
        );
        assert_eq!(x("{{time}} / {{datetime}}"), "09:05 / 2026-10-14 09:05");
    }

    #[test]
    fn offsets_in_days_weeks_months_and_years() {
        assert_eq!(x("{{date+7d}}"), "2026-10-21");
        assert_eq!(x("{{date-1w}}"), "2026-10-07");
        assert_eq!(x("{{ Date + 2 W }}"), "2026-10-28");
        assert_eq!(x("{{date+1m}}"), "2026-11-14");
        assert_eq!(x("{{date+1y}}"), "2027-10-14");
        assert_eq!(
            expand_dates(
                "{{date+1m}}",
                NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
                None
            ),
            "2026-02-28"
        );
        // not an offset: left alone
        for s in [
            "{{date+}}",
            "{{date+d}}",
            "{{date+7x}}",
            "{{date7d}}",
            "{{date+999999d}}",
        ] {
            assert_eq!(x(s), s, "{s}");
        }
    }

    #[test]
    fn weekdays_of_this_week_and_the_next_one() {
        assert_eq!(
            x("{{monday}} {{wednesday}} {{sunday}}"),
            "2026-10-12 2026-10-14 2026-10-18"
        );
        assert_eq!(x("{{next monday}}"), "2026-10-19");
        // the same day is not "next": a week later
        assert_eq!(x("{{next wednesday}}"), "2026-10-21");
        assert_eq!(x("{{next friday}}"), "2026-10-16");
    }

    #[test]
    fn others_stay_and_time_needs_a_time() {
        assert_eq!(
            x("{{page}} {{title}} {{cursor}} {{nope}}"),
            "{{page}} {{title}} {{cursor}} {{nope}}"
        );
        assert_eq!(
            expand_dates("{{time}}|{{datetime}}", d(), None),
            "{{time}}|{{datetime}}"
        );
        assert_eq!(x("a {{date"), "a {{date");
        assert_eq!(x("}} {{date}} {{"), "}} 2026-10-14 {{");
    }
}
