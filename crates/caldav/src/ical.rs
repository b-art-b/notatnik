//! The iCalendar text of a task (RFC 5545, VTODO), written the same way every time and read leniently.
//!
//! Writing is deterministic: the same [`Todo`] always gives the same bytes, so a push that changed nothing is
//! recognised as such. Reading keeps what the app maps (see phase 4) and ignores the rest, including alarms.
//! Times are written as floating local time (no time zone): the server and the phone show them as the same
//! clock time wherever the user is.

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};

/// `STATUS` of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NeedsAction,
    InProcess,
    Completed,
    Cancelled,
}

impl Status {
    fn text(self) -> &'static str {
        match self {
            Status::NeedsAction => "NEEDS-ACTION",
            Status::InProcess => "IN-PROCESS",
            Status::Completed => "COMPLETED",
            Status::Cancelled => "CANCELLED",
        }
    }

    fn parse(s: &str) -> Option<Status> {
        match s.to_ascii_uppercase().as_str() {
            "NEEDS-ACTION" => Some(Status::NeedsAction),
            "IN-PROCESS" => Some(Status::InProcess),
            "COMPLETED" => Some(Status::Completed),
            "CANCELLED" | "CANCELED" => Some(Status::Cancelled),
            _ => None,
        }
    }
}

/// A date or a time of day, as `DTSTART` and `DUE` hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    /// `VALUE=DATE`
    Date(NaiveDate),
    /// A clock time. `tzid` is kept when the server's value had one; we write none.
    Local {
        at: NaiveDateTime,
        tzid: Option<String>,
    },
    /// A time ending in `Z`.
    Utc(NaiveDateTime),
}

impl When {
    /// The day and clock time as the user sees them: a time in UTC is shown in local time; a time with another
    /// `TZID` is taken as the clock time it states (there is no time zone database here).
    pub fn floating(&self) -> (NaiveDate, Option<NaiveTime>) {
        match self {
            When::Date(d) => (*d, None),
            When::Local { at, .. } => (at.date(), Some(at.time())),
            When::Utc(at) => {
                let l = Local.from_utc_datetime(at).naive_local();
                (l.date(), Some(l.time()))
            }
        }
    }

    pub fn date(&self) -> NaiveDate {
        match self {
            When::Date(d) => *d,
            When::Local { at, .. } | When::Utc(at) => at.date(),
        }
    }
}

/// A task as the calendar holds it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Todo {
    pub uid: String,
    /// `DTSTAMP` (UTC). The caller picks it, so equal input gives equal bytes.
    pub stamp: Option<NaiveDateTime>,
    pub summary: String,
    pub status: Option<Status>,
    /// `X-NOTATNIK-STATE`: the app's own state word when STATUS cannot say it (`WAITING`).
    pub state: Option<String>,
    pub start: Option<When>,
    pub due: Option<When>,
    /// `COMPLETED` (UTC).
    pub completed: Option<NaiveDateTime>,
    /// 1 (highest) to 9; 0 or none = no priority.
    pub priority: Option<u8>,
    /// What follows `RRULE:`.
    pub rrule: Option<String>,
    pub categories: Vec<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    /// `X-NOTATNIK-PAGE`
    pub page: Option<String>,
    /// `LAST-MODIFIED` (UTC); only read.
    pub modified: Option<NaiveDateTime>,
}

impl Todo {
    /// Both say the same about everything the app maps: times compared as clock times, `DTSTAMP`, `LAST-MODIFIED`
    /// and `COMPLETED` ignored (the status says it).
    pub fn same_as(&self, other: &Todo) -> bool {
        let key = |t: &Todo| {
            (
                t.uid.clone(),
                t.summary.trim().to_string(),
                t.status.unwrap_or(Status::NeedsAction),
                t.start.as_ref().map(When::floating),
                t.due.as_ref().map(When::floating),
                t.priority.filter(|p| *p > 0),
                t.rrule.as_deref().map(|r| r.trim().to_string()),
                t.categories.clone(),
                t.description.clone(),
                t.url.clone(),
            )
        };
        key(self) == key(other)
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// Pushes a line folded at 75 octets (a continuation line starts with a space).
fn push_folded(out: &mut String, line: &str) {
    let mut width = 0;
    for c in line.chars() {
        let n = c.len_utf8();
        if width + n > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += n;
    }
    out.push_str("\r\n");
}

fn stamp(t: &NaiveDateTime) -> String {
    t.format("%Y%m%dT%H%M%S").to_string()
}

fn when_line(name: &str, w: &When) -> String {
    match w {
        When::Date(d) => format!("{name};VALUE=DATE:{}", d.format("%Y%m%d")),
        When::Local { at, tzid: None } => format!("{name}:{}", stamp(at)),
        When::Local { at, tzid: Some(z) } => format!("{name};TZID={z}:{}", stamp(at)),
        When::Utc(at) => format!("{name}:{}Z", stamp(at)),
    }
}

impl Todo {
    /// The whole `.ics` file: one calendar with this task.
    pub fn to_ics(&self) -> String {
        let mut o = String::new();
        let mut line = |l: String| push_folded(&mut o, &l);
        line("BEGIN:VCALENDAR".into());
        line("VERSION:2.0".into());
        line("PRODID:-//Notatnik//CalDAV//EN".into());
        line("BEGIN:VTODO".into());
        line(format!("UID:{}", escape(&self.uid)));
        if let Some(s) = &self.stamp {
            line(format!("DTSTAMP:{}Z", stamp(s)));
        }
        line(format!("SUMMARY:{}", escape(&self.summary)));
        if let Some(s) = self.status {
            line(format!("STATUS:{}", s.text()));
        }
        if let Some(s) = &self.state {
            line(format!("X-NOTATNIK-STATE:{}", escape(s)));
        }
        if let Some(w) = &self.start {
            line(when_line("DTSTART", w));
        }
        if let Some(w) = &self.due {
            line(when_line("DUE", w));
        }
        if let Some(p) = self.priority.filter(|p| (1..=9).contains(p)) {
            line(format!("PRIORITY:{p}"));
        }
        if let Some(r) = &self.rrule {
            line(format!("RRULE:{r}"));
        }
        if let Some(c) = &self.completed {
            line(format!("COMPLETED:{}Z", stamp(c)));
        }
        if !self.categories.is_empty() {
            let list: Vec<String> = self.categories.iter().map(|c| escape(c)).collect();
            line(format!("CATEGORIES:{}", list.join(",")));
        }
        if let Some(d) = &self.description {
            line(format!("DESCRIPTION:{}", escape(d)));
        }
        if let Some(u) = &self.url {
            line(format!("URL:{u}"));
        }
        if let Some(p) = &self.page {
            line(format!("X-NOTATNIK-PAGE:{}", escape(p)));
        }
        line("END:VTODO".into());
        line("END:VCALENDAR".into());
        o
    }
}

/// One unfolded content line.
struct Prop<'a> {
    name: String,
    params: Vec<(String, &'a str)>,
    value: &'a str,
}

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if let (Some(rest), Some(last)) = (raw.strip_prefix([' ', '\t']), lines.last_mut()) {
            last.push_str(rest);
        } else if !raw.is_empty() {
            lines.push(raw.to_string());
        }
    }
    lines
}

fn split_prop(line: &str) -> Option<Prop<'_>> {
    // the first ':' outside double quotes ends the name and parameters
    let mut quoted = false;
    let mut colon = None;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"')))
        .collect();
    Some(Prop {
        name,
        params,
        value,
    })
}

fn parse_stamp(v: &str) -> Option<(NaiveDateTime, bool)> {
    let utc = v.ends_with('Z') || v.ends_with('z');
    let v = v.trim_end_matches(['Z', 'z']);
    NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S")
        .ok()
        .map(|t| (t, utc))
}

fn parse_when(p: &Prop) -> Option<When> {
    let date_only = p
        .params
        .iter()
        .any(|(k, v)| k == "VALUE" && v.eq_ignore_ascii_case("DATE"));
    if date_only || (p.value.len() == 8 && !p.value.contains('T')) {
        return NaiveDate::parse_from_str(p.value, "%Y%m%d")
            .ok()
            .map(When::Date);
    }
    let (at, utc) = parse_stamp(p.value)?;
    if utc {
        return Some(When::Utc(at));
    }
    let tzid = p
        .params
        .iter()
        .find(|(k, _)| k == "TZID")
        .map(|(_, v)| (*v).to_string());
    Some(When::Local { at, tzid })
}

/// Reads the first VTODO of an `.ics` text. `None` when there is none (an event, a journal, garbage).
pub fn parse_todo(ics: &str) -> Option<Todo> {
    let lines = unfold(ics);
    let mut todo = Todo::default();
    let (mut inside, mut depth, mut found) = (false, 0, false);
    for l in &lines {
        let Some(p) = split_prop(l) else { continue };
        if p.name == "BEGIN" {
            if inside {
                depth += 1; // VALARM and the like
            } else if p.value.eq_ignore_ascii_case("VTODO") && !found {
                inside = true;
            }
            continue;
        }
        if p.name == "END" {
            if inside && depth > 0 {
                depth -= 1;
            } else if inside && p.value.eq_ignore_ascii_case("VTODO") {
                inside = false;
                found = true;
            }
            continue;
        }
        if !inside || depth > 0 {
            continue;
        }
        match p.name.as_str() {
            "UID" => todo.uid = unescape(p.value),
            "DTSTAMP" => todo.stamp = parse_stamp(p.value).map(|(t, _)| t),
            "SUMMARY" => todo.summary = unescape(p.value),
            "STATUS" => todo.status = Status::parse(p.value),
            "X-NOTATNIK-STATE" => todo.state = Some(unescape(p.value)),
            "DTSTART" => todo.start = parse_when(&p),
            "DUE" => todo.due = parse_when(&p),
            "COMPLETED" => todo.completed = parse_stamp(p.value).map(|(t, _)| t),
            "LAST-MODIFIED" => todo.modified = parse_stamp(p.value).map(|(t, _)| t),
            "PRIORITY" => todo.priority = p.value.trim().parse().ok().filter(|n| *n > 0),
            "RRULE" => todo.rrule = Some(p.value.to_string()),
            "CATEGORIES" => todo.categories.extend(split_list(p.value)),
            "DESCRIPTION" => todo.description = Some(unescape(p.value)),
            "URL" => todo.url = Some(p.value.to_string()),
            "X-NOTATNIK-PAGE" => todo.page = Some(unescape(p.value)),
            _ => {}
        }
    }
    (found && !todo.uid.is_empty()).then_some(todo)
}

/// A comma separated text list; `\,` is a comma inside an entry.
fn split_list(v: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for c in v.chars() {
        match (esc, c) {
            (true, c) => {
                cur.push('\\');
                cur.push(c);
                esc = false;
            }
            (false, '\\') => esc = true,
            (false, ',') => out.push(unescape(&std::mem::take(&mut cur))),
            (false, c) => cur.push(c),
        }
    }
    out.push(unescape(&cur));
    out.retain(|s| !s.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    fn t(y: i32, m: u32, day: u32, h: u32, mi: u32) -> NaiveDateTime {
        d(y, m, day).and_hms_opt(h, mi, 0).unwrap()
    }

    fn sample() -> Todo {
        Todo {
            uid: "notatnik-g1-abc123".into(),
            stamp: Some(t(2026, 10, 18, 8, 0)),
            summary: "Call Anna, about the (new) plan; bring notes".into(),
            status: Some(Status::NeedsAction),
            start: Some(When::Date(d(2026, 10, 20))),
            due: Some(When::Local {
                at: t(2026, 10, 21, 17, 30),
                tzid: None,
            }),
            priority: Some(1),
            categories: vec!["work".into(), "a,b".into()],
            description: Some("line one\nline two".into()),
            url: Some("notatnik://page/Plan#abc123".into()),
            page: Some("Plan".into()),
            ..Todo::default()
        }
    }

    #[test]
    fn the_same_task_always_gives_the_same_bytes() {
        let a = sample().to_ics();
        assert_eq!(a, sample().to_ics());
        assert_eq!(
            a,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Notatnik//CalDAV//EN\r\nBEGIN:VTODO\r\n\
UID:notatnik-g1-abc123\r\nDTSTAMP:20261018T080000Z\r\n\
SUMMARY:Call Anna\\, about the (new) plan\\; bring notes\r\nSTATUS:NEEDS-ACTION\r\n\
DTSTART;VALUE=DATE:20261020\r\nDUE:20261021T173000\r\nPRIORITY:1\r\nCATEGORIES:work,a\\,b\r\n\
DESCRIPTION:line one\\nline two\r\nURL:notatnik://page/Plan#abc123\r\nX-NOTATNIK-PAGE:Plan\r\n\
END:VTODO\r\nEND:VCALENDAR\r\n"
        );
    }

    #[test]
    fn what_is_written_is_read_back() {
        let todo = sample();
        assert_eq!(parse_todo(&todo.to_ics()), Some(todo));
    }

    #[test]
    fn done_tasks_repeats_and_states_survive() {
        let todo = Todo {
            uid: "u".into(),
            summary: "Water plants".into(),
            status: Some(Status::Completed),
            state: Some("WAITING".into()),
            start: Some(When::Utc(t(2026, 1, 2, 3, 4))),
            completed: Some(t(2026, 10, 18, 9, 15)),
            rrule: Some("FREQ=WEEKLY;INTERVAL=2".into()),
            ..Todo::default()
        };
        assert_eq!(parse_todo(&todo.to_ics()), Some(todo));
    }

    #[test]
    fn long_lines_fold_at_75_octets_and_unfold_again() {
        let todo = Todo {
            uid: "u".into(),
            summary: "żółć ".repeat(40),
            ..Todo::default()
        };
        let ics = todo.to_ics();
        for l in ics.split("\r\n") {
            assert!(l.len() <= 75, "{} octets: {l}", l.len());
        }
        assert_eq!(parse_todo(&ics).unwrap().summary, todo.summary);
    }

    #[test]
    fn a_task_from_nextcloud_tasks_is_read_and_its_alarm_ignored() {
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:x-1\r\n\
DTSTAMP:20261017T230444Z\r\nSUMMARY:Buy milk\r\nSTATUS:IN-PROCESS\r\n\
DUE;TZID=Europe/Warsaw:20261019T120000\r\nPRIORITY:5\r\nCATEGORIES:home,errands\r\n\
BEGIN:VALARM\r\nACTION:DISPLAY\r\nSUMMARY:alarm text\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\n\
LAST-MODIFIED:20261018T100000Z\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let todo = parse_todo(ics).unwrap();
        assert_eq!(todo.summary, "Buy milk");
        assert_eq!(todo.status, Some(Status::InProcess));
        assert_eq!(
            todo.due,
            Some(When::Local {
                at: t(2026, 10, 19, 12, 0),
                tzid: Some("Europe/Warsaw".into())
            })
        );
        assert_eq!(todo.priority, Some(5));
        assert_eq!(todo.categories, ["home", "errands"]);
        assert_eq!(todo.modified, Some(t(2026, 10, 18, 10, 0)));
    }

    #[test]
    fn other_components_and_garbage_are_not_tasks() {
        let event = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nSUMMARY:Meeting\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(parse_todo(event), None);
        assert_eq!(parse_todo("not a calendar"), None);
        assert_eq!(parse_todo(""), None);
    }

    #[test]
    fn priority_zero_is_none_and_bare_dates_need_no_value_parameter() {
        let ics = "BEGIN:VTODO\nUID:u\nPRIORITY:0\nDTSTART:20261020\nEND:VTODO\n";
        let todo = parse_todo(ics).unwrap();
        assert_eq!(todo.priority, None);
        assert_eq!(todo.start, Some(When::Date(d(2026, 10, 20))));
    }
}
