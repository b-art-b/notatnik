//! Events of a calendar, read for display (CA-3, JR-5): a repeating event is expanded for a window of days,
//! changed or moved occurrences replace the ones they stand for, deleted occurrences (`EXDATE`) and canceled
//! events are left out, and every time is shown in the user's local time zone. Events are never written back.

use std::str::FromStr;

use std::collections::BTreeMap;

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::ical::{Prop, When, parse_when, split_prop, unescape, unfold};
use crate::{CalDavClient, Error, Result};

/// One event on one stretch of days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub uid: String,
    pub title: String,
    /// First day, local.
    pub date: NaiveDate,
    /// Last day (the same as `date` for most events).
    pub end_date: NaiveDate,
    /// `None` for an all-day event.
    pub start: Option<NaiveTime>,
    pub end: Option<NaiveTime>,
    pub location: Option<String>,
}

/// How a start is read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Day,
    /// A clock time with no zone: the same clock time wherever the user is.
    Floating,
    /// An instant (UTC, or a clock time in a named zone).
    Instant,
}

struct Raw {
    uid: String,
    title: String,
    location: Option<String>,
    cancelled: bool,
    start: When,
    end: Option<When>,
    duration: Option<chrono::Duration>,
    recurrence_id: Option<When>,
    /// DTSTART, RRULE, EXDATE and RDATE as written, for the rule engine.
    rule_lines: Vec<String>,
    has_rule: bool,
}

fn zone(tzid: &str) -> Option<chrono_tz::Tz> {
    chrono_tz::Tz::from_str(tzid).ok()
}

/// A `When` as an instant in UTC; `None` for a plain day or a clock time with no usable zone.
fn instant(w: &When) -> Option<NaiveDateTime> {
    match w {
        When::Date(_) => None,
        When::Utc(t) => Some(*t),
        When::Local { at, tzid: Some(z) } => zone(z)
            .and_then(|z| z.from_local_datetime(at).earliest())
            .map(|d| d.naive_utc()),
        When::Local { tzid: None, .. } => None,
    }
}

fn kind(w: &When) -> Kind {
    match w {
        When::Date(_) => Kind::Day,
        When::Utc(_) => Kind::Instant,
        When::Local { .. } => {
            if instant(w).is_some() {
                Kind::Instant
            } else {
                Kind::Floating
            }
        }
    }
}

fn to_local(utc: NaiveDateTime) -> NaiveDateTime {
    Local.from_utc_datetime(&utc).naive_local()
}

fn parse_duration(v: &str) -> Option<chrono::Duration> {
    let v = v.trim();
    let (neg, v) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v.strip_prefix('+').unwrap_or(v)),
    };
    let v = v.strip_prefix('P')?;
    let mut total = chrono::Duration::zero();
    let mut num = String::new();
    let mut in_time = false;
    for c in v.chars() {
        match c {
            'T' => in_time = true,
            '0'..='9' => num.push(c),
            u => {
                let n: i64 = num.parse().ok()?;
                num.clear();
                total += match (u, in_time) {
                    ('W', _) => chrono::Duration::weeks(n),
                    ('D', _) => chrono::Duration::days(n),
                    ('H', true) => chrono::Duration::hours(n),
                    ('M', true) => chrono::Duration::minutes(n),
                    ('S', true) => chrono::Duration::seconds(n),
                    _ => return None,
                };
            }
        }
    }
    Some(if neg { -total } else { total })
}

fn components(ics: &str) -> Vec<Raw> {
    let mut out = Vec::new();
    let mut cur: Option<Raw> = None;
    let mut depth = 0;
    for line in unfold(ics) {
        let Some(p) = split_prop(&line) else { continue };
        if p.name == "BEGIN" {
            if cur.is_some() {
                depth += 1;
            } else if p.value.eq_ignore_ascii_case("VEVENT") {
                cur = Some(Raw {
                    uid: String::new(),
                    title: String::new(),
                    location: None,
                    cancelled: false,
                    start: When::Date(NaiveDate::MIN),
                    end: None,
                    duration: None,
                    recurrence_id: None,
                    rule_lines: Vec::new(),
                    has_rule: false,
                });
                depth = 0;
            }
            continue;
        }
        if p.name == "END" {
            if depth > 0 {
                depth -= 1;
            } else if p.value.eq_ignore_ascii_case("VEVENT") {
                if let Some(r) = cur.take() {
                    out.push(r);
                }
            }
            continue;
        }
        let Some(r) = cur.as_mut() else { continue };
        if depth > 0 {
            continue;
        }
        read_prop(r, &p, &line);
    }
    out
}

fn read_prop(r: &mut Raw, p: &Prop, line: &str) {
    match p.name.as_str() {
        "UID" => r.uid = unescape(p.value),
        "SUMMARY" => r.title = unescape(p.value),
        "LOCATION" => r.location = Some(unescape(p.value)).filter(|l| !l.is_empty()),
        "STATUS" => r.cancelled = p.value.eq_ignore_ascii_case("CANCELLED"),
        "DTSTART" => {
            if let Some(w) = parse_when(p) {
                r.start = w;
                r.rule_lines.insert(0, line.to_string());
            }
        }
        "DTEND" => r.end = parse_when(p),
        "DURATION" => r.duration = parse_duration(p.value),
        "RECURRENCE-ID" => r.recurrence_id = parse_when(p),
        "RRULE" => {
            r.has_rule = true;
            r.rule_lines.push(line.to_string());
        }
        "EXDATE" | "RDATE" => {
            r.has_rule = true;
            r.rule_lines.push(line.to_string());
        }
        _ => {}
    }
}

/// How long the event lasts, as a span from its start.
fn length(r: &Raw) -> chrono::Duration {
    if let Some(d) = r.duration {
        return d;
    }
    match (&r.end, &r.start) {
        (Some(e), s) => match (instant(e), instant(s)) {
            (Some(e), Some(s)) => e - s,
            _ => match (e, s) {
                (When::Date(e), When::Date(s)) => chrono::Duration::days((*e - *s).num_days()),
                (
                    When::Local { at: e, .. } | When::Utc(e),
                    When::Local { at: s, .. } | When::Utc(s),
                ) => *e - *s,
                _ => chrono::Duration::zero(),
            },
        },
        (None, When::Date(_)) => chrono::Duration::days(1),
        (None, _) => chrono::Duration::zero(),
    }
}

/// The days and times of one occurrence that starts at `start` (a local clock time for a day or floating event, UTC
/// for an instant).
fn occurrence(r: &Raw, k: Kind, start: NaiveDateTime, len: chrono::Duration) -> Occurrence {
    let (date, s, end_date, e) = match k {
        Kind::Day => {
            let last = (start.date() + len - chrono::Duration::days(1)).max(start.date());
            (start.date(), None, last, None)
        }
        Kind::Floating => {
            let end = start + len;
            (
                start.date(),
                Some(start.time()),
                end.date(),
                Some(end.time()),
            )
        }
        Kind::Instant => {
            let (a, b) = (to_local(start), to_local(start + len));
            (a.date(), Some(a.time()), b.date(), Some(b.time()))
        }
    };
    Occurrence {
        uid: r.uid.clone(),
        title: r.title.clone(),
        date,
        end_date,
        start: s,
        end: e,
        location: r.location.clone(),
    }
}

/// What was read from one calendar of events: kept on this device (it can be read again from the server).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCal {
    /// RFC 6578 token: what changed since is all the next read needs.
    #[serde(default)]
    pub token: Option<String>,
    /// By file name: the ETag and the text of every event resource.
    #[serde(default)]
    pub items: BTreeMap<String, (String, String)>,
}

/// Brings `cal` up to date with the calendar `href`: one request when nothing changed. Returns how many
/// resources changed or went.
pub fn refresh(client: &dyn CalDavClient, href: &str, cal: &mut EventCal) -> Result<usize> {
    let (refs, deleted, token) = match cal.token.as_deref() {
        Some(t) => match client.sync(href, Some(t)) {
            Ok(r) => (r.changed, Some(r.deleted), Some(r.token)),
            Err(Error::Changed(_)) => everything(client, href)?,
            Err(e) => return Err(e),
        },
        None => everything(client, href)?,
    };
    let ask: Vec<String> = refs
        .iter()
        .filter(|i| cal.items.get(&i.name).is_none_or(|(e, _)| *e != i.etag))
        .map(|i| i.name.clone())
        .collect();
    let mut n = 0;
    if !ask.is_empty() {
        for item in client.get(href, &ask)? {
            cal.items.insert(item.name, (item.etag, item.ics));
            n += 1;
        }
    }
    match &deleted {
        Some(gone) => {
            for name in gone {
                n += usize::from(cal.items.remove(name).is_some());
            }
        }
        None => {
            let here: std::collections::BTreeSet<&str> =
                refs.iter().map(|i| i.name.as_str()).collect();
            let before = cal.items.len();
            cal.items.retain(|name, _| here.contains(name.as_str()));
            n += before - cal.items.len();
        }
    }
    cal.token = token;
    Ok(n)
}

/// Items to look at, the names the server says are gone (none when it cannot say), and a sync token.
type Everything = (Vec<crate::ItemRef>, Option<Vec<String>>, Option<String>);

fn everything(client: &dyn CalDavClient, href: &str) -> Result<Everything> {
    match client.sync(href, None) {
        Ok(r) => Ok((r.changed, None, Some(r.token))),
        Err(e @ (Error::Offline(_) | Error::Auth(_))) => Err(e),
        Err(_) => Ok((client.list(href)?, None, None)),
    }
}

/// Every occurrence of the events in `ics` that touches a day from `from` to `to` (inclusive), by start.
pub fn occurrences(ics: &str, from: NaiveDate, to: NaiveDate) -> Vec<Occurrence> {
    let raws = components(ics);
    let mut out = Vec::new();
    let overridden: Vec<NaiveDateTime> = raws
        .iter()
        .filter_map(|r| r.recurrence_id.as_ref())
        .filter_map(|w| instant(w).or_else(|| local_of_naive(w)))
        .collect();
    for r in &raws {
        if r.cancelled {
            continue;
        }
        let k = kind(&r.start);
        let len = length(r);
        let mut starts: Vec<NaiveDateTime> = Vec::new();
        if r.has_rule && r.recurrence_id.is_none() {
            starts = expand(r, k, from, to, len);
            // an occurrence that has an override of its own is replaced by it
            starts.retain(|s| !overridden.contains(s));
        } else if let Some(s) = naive_start(r, k) {
            starts.push(s);
        }
        for s in starts {
            let o = occurrence(r, k, s, len);
            if o.date <= to && o.end_date >= from {
                out.push(o);
            }
        }
    }
    out.sort_by(|a, b| (a.date, a.start, &a.title).cmp(&(b.date, b.start, &b.title)));
    out
}

/// The naive value `occurrence` expects for a start: UTC for an instant, the written clock time otherwise.
fn naive_start(r: &Raw, k: Kind) -> Option<NaiveDateTime> {
    match (&r.start, k) {
        (When::Date(d), _) => d.and_hms_opt(0, 0, 0),
        (w, Kind::Instant) => instant(w),
        (When::Local { at, .. }, _) => Some(*at),
        _ => None,
    }
}

fn local_of_naive(w: &When) -> Option<NaiveDateTime> {
    match w {
        When::Date(d) => d.and_hms_opt(0, 0, 0),
        When::Local { at, .. } => Some(*at),
        When::Utc(t) => Some(*t),
    }
}

/// The starts of a repeating event in the window (same convention as [`naive_start`]).
fn expand(
    r: &Raw,
    k: Kind,
    from: NaiveDate,
    to: NaiveDate,
    len: chrono::Duration,
) -> Vec<NaiveDateTime> {
    let Ok(set) = rrule::RRuleSet::from_str(&r.rule_lines.join("\n")) else {
        return naive_start(r, k).into_iter().collect();
    };
    // a window a little wider than asked, so that long events and zone shifts at the edges are not lost
    let lo = from.and_hms_opt(0, 0, 0).unwrap_or_default() - len - chrono::Duration::days(2);
    let hi = to.and_hms_opt(23, 59, 59).unwrap_or_default() + chrono::Duration::days(2);
    let utc = rrule::Tz::UTC;
    let set = set
        .after(utc.from_utc_datetime(&lo))
        .before(utc.from_utc_datetime(&hi));
    set.all(2000)
        .dates
        .into_iter()
        .map(|d| match k {
            Kind::Instant => d.with_timezone(&Utc).naive_utc(),
            _ => d.naive_local(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn ics(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{body}END:VCALENDAR\r\n")
    }

    fn event(props: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:e1\r\n{props}END:VEVENT\r\n")
    }

    #[test]
    fn a_calendar_is_read_once_then_by_change_and_what_is_deleted_goes() {
        use crate::Cond;
        use crate::fake::FakeServer;
        let s = FakeServer::new();
        let h = s.add_calendar("Events", &["VEVENT"]);
        let one = ics(&event("SUMMARY:One\r\nDTSTART:20261020T090000\r\n"));
        let two = one.replace("UID:e1", "UID:e2").replace("One", "Two");
        s.put(&h, "e1.ics", &one, &Cond::Create).unwrap();
        s.put(&h, "e2.ics", &two, &Cond::Create).unwrap();
        let mut cal = EventCal::default();
        assert_eq!(refresh(&s, &h, &mut cal).unwrap(), 2);
        let before = s.requests();
        assert_eq!(refresh(&s, &h, &mut cal).unwrap(), 0);
        assert_eq!(s.requests() - before, 1, "nothing changed: one request");
        s.delete(&h, "e1.ics", None).unwrap();
        assert_eq!(refresh(&s, &h, &mut cal).unwrap(), 1);
        assert_eq!(cal.items.keys().collect::<Vec<_>>(), ["e2.ics"]);
        // a token the server forgot: everything is compared again
        s.forget_sync_tokens(&h);
        s.delete(&h, "e2.ics", None).unwrap();
        assert_eq!(refresh(&s, &h, &mut cal).unwrap(), 1);
        assert!(cal.items.is_empty());
    }

    #[test]
    fn a_single_event_and_an_all_day_event() {
        let one = ics(&event(
            "SUMMARY:Dentist\r\nDTSTART:20261020T093000\r\nDTEND:20261020T103000\r\nLOCATION:Main St\r\n",
        ));
        let o = occurrences(&one, d(2026, 10, 1), d(2026, 10, 31));
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].title, "Dentist");
        assert_eq!(o[0].date, d(2026, 10, 20));
        assert_eq!(o[0].start, NaiveTime::from_hms_opt(9, 30, 0));
        assert_eq!(o[0].end, NaiveTime::from_hms_opt(10, 30, 0));
        assert_eq!(o[0].location.as_deref(), Some("Main St"));
        assert!(occurrences(&one, d(2026, 10, 21), d(2026, 10, 31)).is_empty());

        let all = ics(&event(
            "SUMMARY:Holiday\r\nDTSTART;VALUE=DATE:20261023\r\nDTEND;VALUE=DATE:20261026\r\n",
        ));
        let o = occurrences(&all, d(2026, 10, 24), d(2026, 10, 24));
        assert_eq!(o.len(), 1, "a multi-day event shows on its middle day");
        assert_eq!(
            (o[0].date, o[0].end_date),
            (d(2026, 10, 23), d(2026, 10, 25))
        );
        assert_eq!(o[0].start, None);
    }

    #[test]
    fn a_weekly_event_with_a_deleted_day_and_a_moved_one() {
        let text = ics(&format!(
            "{}{}",
            event(
                "SUMMARY:Standup\r\nDTSTART:20261005T090000\r\nDTEND:20261005T091500\r\nRRULE:FREQ=WEEKLY;COUNT=5\r\nEXDATE:20261012T090000\r\n"
            ),
            "BEGIN:VEVENT\r\nUID:e1\r\nRECURRENCE-ID:20261019T090000\r\nSUMMARY:Standup (moved)\r\nDTSTART:20261020T140000\r\nDTEND:20261020T141500\r\nEND:VEVENT\r\n"
        ));
        let o = occurrences(&text, d(2026, 10, 1), d(2026, 11, 30));
        let days: Vec<(NaiveDate, &str)> = o.iter().map(|o| (o.date, o.title.as_str())).collect();
        assert_eq!(
            days,
            [
                (d(2026, 10, 5), "Standup"),
                (d(2026, 10, 20), "Standup (moved)"),
                (d(2026, 10, 26), "Standup"),
                (d(2026, 11, 2), "Standup"),
            ],
            "the 12th is deleted, the 19th moved to the 20th"
        );
    }

    #[test]
    fn an_endless_rule_is_cut_at_the_window() {
        let text = ics(&event(
            "SUMMARY:Rent\r\nDTSTART;VALUE=DATE:20260101\r\nRRULE:FREQ=MONTHLY;BYMONTHDAY=1\r\n",
        ));
        let o = occurrences(&text, d(2026, 10, 1), d(2027, 1, 31));
        let days: Vec<NaiveDate> = o.iter().map(|o| o.date).collect();
        assert_eq!(
            days,
            [
                d(2026, 10, 1),
                d(2026, 11, 1),
                d(2026, 12, 1),
                d(2027, 1, 1)
            ]
        );
    }

    #[test]
    fn a_time_in_another_zone_is_shown_in_local_time() {
        let text = ics(&event(
            "SUMMARY:Call\r\nDTSTART;TZID=America/New_York:20261020T090000\r\nDTEND;TZID=America/New_York:20261020T100000\r\n",
        ));
        let o = occurrences(&text, d(2026, 10, 19), d(2026, 10, 21));
        assert_eq!(o.len(), 1);
        let want = chrono_tz::America::New_York
            .with_ymd_and_hms(2026, 10, 20, 9, 0, 0)
            .unwrap()
            .naive_utc();
        let local = to_local(want);
        assert_eq!((o[0].date, o[0].start), (local.date(), Some(local.time())));
    }

    #[test]
    fn a_canceled_event_is_left_out_and_alarms_are_ignored() {
        let text = ics(&format!(
            "{}{}",
            event("SUMMARY:Gone\r\nSTATUS:CANCELLED\r\nDTSTART:20261020T090000\r\n"),
            "BEGIN:VEVENT\r\nUID:e2\r\nSUMMARY:Kept\r\nDTSTART:20261021T090000\r\nBEGIN:VALARM\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\n"
        ));
        let o = occurrences(&text, d(2026, 10, 1), d(2026, 10, 31));
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].title, "Kept");
    }

    #[test]
    fn a_duration_instead_of_an_end() {
        let text = ics(&event(
            "SUMMARY:Workshop\r\nDTSTART:20261020T090000\r\nDURATION:PT2H30M\r\n",
        ));
        let o = occurrences(&text, d(2026, 10, 20), d(2026, 10, 20));
        assert_eq!(o[0].end, NaiveTime::from_hms_opt(11, 30, 0));
    }
}
