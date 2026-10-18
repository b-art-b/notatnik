//! A task block as a calendar task (the table in `docs/phases/phase-4.md`, "Field mapping").
//!
//! Only blocks that can be shared are mapped: a task with a date and a block ID, not marked `cal:: no`. The
//! result depends on nothing but the block, the page name and the graph ID, so the same block always gives the
//! same item.

use chrono::{Local, NaiveDateTime, TimeZone};
use notatnik_core::{Block, Moment, RefKind, RepeatRule, RepeatUnit, TaskState};

use crate::ical::{Status, Todo, When};

/// What the mapping needs besides the block.
pub struct Context<'a> {
    /// The graph's ID (`[graph] id`); part of every UID.
    pub graph_id: &'a str,
    /// The page the block is on.
    pub page: &'a str,
    /// The page's text; the block's spans point into it.
    pub source: &'a str,
    /// `DTSTAMP` of the item.
    pub stamp: NaiveDateTime,
    /// The graph's priority labels, most important first (empty: `A`, `B`, `C`).
    pub priorities: &'a [String],
}

/// `notatnik-<graph>-<block id>`.
pub fn uid(graph_id: &str, block_id: &str) -> String {
    format!("notatnik-{graph_id}-{block_id}")
}

/// The block ID in a UID of this graph.
pub fn block_id_of(graph_id: &str, uid: &str) -> Option<String> {
    uid.strip_prefix(&format!("notatnik-{graph_id}-"))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// The calendar task for a block, or `None` when it is not shared: not a task, no date, no block ID yet, or
/// `cal:: no`.
pub fn todo_from_block(ctx: &Context, block: &Block) -> Option<Todo> {
    let task = block.task.as_ref()?;
    let id = &block.block_id.as_ref()?.id;
    if task.scheduled.is_none() && task.deadline.is_none() {
        return None;
    }
    if block
        .properties
        .iter()
        .any(|p| p.key.eq_ignore_ascii_case("cal") && p.raw.trim().eq_ignore_ascii_case("no"))
    {
        return None;
    }
    let (status, state) = match task.state {
        TaskState::Todo => (Status::NeedsAction, None),
        TaskState::Later => (Status::NeedsAction, Some("LATER")),
        TaskState::Waiting => (Status::NeedsAction, Some("WAITING")),
        TaskState::Doing => (Status::InProcess, None),
        TaskState::Now => (Status::InProcess, Some("NOW")),
        TaskState::Done => (Status::Completed, None),
        TaskState::Canceled => (Status::Cancelled, None),
    };
    let start = task.scheduled.as_ref().map(when);
    let mut due = task.deadline.as_ref().map(when);
    // a planned time range with no deadline: its end is the due time
    if due.is_none() {
        if let Some(Moment {
            date,
            time: Some(_),
            end: Some(end),
        }) = &task.scheduled
        {
            due = Some(When::Local {
                at: date.and_time(*end),
                tzid: None,
            });
        }
    }
    let first = block.first_line;
    let mut tags = Vec::new();
    for r in &block.refs {
        if r.kind == RefKind::Tag && first.contains_span(&r.span) && !tags.contains(&r.target) {
            tags.push(r.target.clone());
        }
    }
    let description = block
        .children
        .iter()
        .filter(|c| c.task.is_none())
        .map(|c| summary(ctx.source, c))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Some(Todo {
        uid: uid(ctx.graph_id, id),
        stamp: Some(ctx.stamp),
        summary: summary(ctx.source, block),
        status: Some(status),
        state: state.map(str::to_string),
        start,
        due,
        completed: task.done.map(to_utc),
        priority: task
            .priority
            .as_ref()
            .map(|p| priority_number(p.rank(), ctx.priorities.len().max(3))),
        rrule: task.repeat.as_ref().map(|r| rrule(&r.rule)),
        categories: tags,
        description: (!description.is_empty()).then_some(description),
        url: Some(format!("notatnik://page/{}#{id}", percent_encode(ctx.page))),
        page: Some(ctx.page.to_string()),
        ..Todo::default()
    })
}

fn when(m: &Moment) -> When {
    match m.time {
        Some(t) => When::Local {
            at: m.date.and_time(t),
            tzid: None,
        },
        None => When::Date(m.date),
    }
}

/// A completion time written in local time, as UTC.
fn to_utc(t: NaiveDateTime) -> NaiveDateTime {
    Local
        .from_local_datetime(&t)
        .earliest()
        .map(|l| l.naive_utc())
        .unwrap_or(t)
}

/// Rank (0 = most important) of `n` labels as an iCalendar priority: the first is 1, the last 9, the middle 5.
pub fn priority_number(rank: u8, n: usize) -> u8 {
    if n <= 1 {
        return 1;
    }
    let rank = usize::from(rank).min(n - 1);
    u8::try_from(1 + (rank * 8 + (n - 1) / 2) / (n - 1)).unwrap_or(9)
}

fn rrule(rule: &RepeatRule) -> String {
    match rule {
        RepeatRule::Daily => "FREQ=DAILY".into(),
        RepeatRule::Weekly => "FREQ=WEEKLY".into(),
        RepeatRule::Monthly => "FREQ=MONTHLY".into(),
        RepeatRule::Yearly => "FREQ=YEARLY".into(),
        RepeatRule::Weekdays => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".into(),
        RepeatRule::Every { n, unit } => {
            let freq = match unit {
                RepeatUnit::Day => "DAILY",
                RepeatUnit::Week => "WEEKLY",
                RepeatUnit::Month => "MONTHLY",
                RepeatUnit::Year => "YEARLY",
            };
            if *n == 1 {
                format!("FREQ={freq}")
            } else {
                format!("FREQ={freq};INTERVAL={n}")
            }
        }
        RepeatRule::Rrule { rule } => rule.clone(),
    }
}

/// The first line of a block as a calendar title: no task marker, no `^id`, links reduced to their label.
fn summary(source: &str, block: &Block) -> String {
    let first = block.first_line;
    let mut start = first.start;
    if let Some(t) = &block.task {
        if t.marker_span.end > start && t.marker_span.end <= first.end {
            start = t.marker_span.end;
        }
    }
    let end = block
        .block_id
        .as_ref()
        .map_or(first.end, |id| id.span.start.min(first.end));
    let mut out = String::new();
    let mut at = start;
    for r in &block.refs {
        if r.span.start < at || r.span.end > end {
            continue;
        }
        out.push_str(source.get(at..r.span.start).unwrap_or(""));
        match r.kind {
            RefKind::Page => {
                let text = r.label_span.unwrap_or(r.target_span).slice(source);
                out.push_str(text);
            }
            RefKind::Tag => out.push_str(r.span.slice(source)),
            RefKind::Block | RefKind::BlockEmbed | RefKind::PageEmbed => {}
        }
        at = r.span.end;
    }
    out.push_str(source.get(at..end).unwrap_or(""));
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use notatnik_core::parse_page;

    fn stamp() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 18)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap()
    }

    /// The calendar text of every shared task of a page, in order.
    fn ics_of(src: &str) -> Vec<String> {
        let page = parse_page(src);
        let ctx = Context {
            graph_id: "g1",
            page: "Work/Plan",
            source: src,
            stamp: stamp(),
            priorities: &[],
        };
        let mut out = Vec::new();
        fn walk(ctx: &Context, blocks: &[Block], out: &mut Vec<String>) {
            for b in blocks {
                if let Some(t) = todo_from_block(ctx, b) {
                    out.push(t.to_ics());
                }
                walk(ctx, &b.children, out);
            }
        }
        walk(&ctx, &page.blocks, &mut out);
        out
    }

    #[test]
    fn a_task_with_a_deadline_and_a_block_id_is_mapped() {
        let src = "- TODO Call [[Anna|Ann]] about #plan ^abc123\n  priority:: A\n  deadline:: 2026-10-21 17:30\n  scheduled:: 2026-10-20\n";
        let ics = ics_of(src);
        assert_eq!(ics.len(), 1);
        insta::assert_snapshot!(ics[0].replace("\r\n", "\n"));
    }

    #[test]
    fn not_shared_without_a_date_an_id_or_with_cal_no() {
        assert!(ics_of("- TODO no date ^abc123\n").is_empty());
        assert!(ics_of("- TODO no id\n  deadline:: 2026-10-21\n").is_empty());
        assert!(ics_of("- TODO private ^abc123\n  deadline:: 2026-10-21\n  cal:: no\n").is_empty());
        assert!(ics_of("- plain note\n").is_empty());
    }

    #[test]
    fn states_priorities_and_repeats() {
        let table = [
            ("TODO", "STATUS:NEEDS-ACTION", None),
            (
                "LATER",
                "STATUS:NEEDS-ACTION",
                Some("X-NOTATNIK-STATE:LATER"),
            ),
            ("DOING", "STATUS:IN-PROCESS", None),
            ("NOW", "STATUS:IN-PROCESS", Some("X-NOTATNIK-STATE:NOW")),
            (
                "WAITING",
                "STATUS:NEEDS-ACTION",
                Some("X-NOTATNIK-STATE:WAITING"),
            ),
            ("DONE", "STATUS:COMPLETED", None),
            ("CANCELED", "STATUS:CANCELLED", None),
        ];
        for (marker, status, extra) in table {
            let ics = &ics_of(&format!(
                "- {marker} t ^abc123\n  deadline:: 2026-10-21\n"
            ))[0];
            assert!(ics.contains(status), "{marker}: {ics}");
            assert_eq!(
                ics.contains("X-NOTATNIK-STATE"),
                extra.is_some(),
                "{marker}"
            );
            if let Some(e) = extra {
                assert!(ics.contains(e), "{marker}");
            }
        }
        let prio = |p: &str| {
            let ics = &ics_of(&format!(
                "- TODO t ^abc123\n  priority:: {p}\n  deadline:: 2026-10-21\n"
            ))[0];
            ics.lines()
                .find_map(|l| l.strip_prefix("PRIORITY:"))
                .map(str::to_string)
        };
        assert_eq!(
            (prio("A"), prio("B"), prio("C")),
            (Some("1".into()), Some("5".into()), Some("9".into()))
        );
        let rep = |r: &str| {
            let ics = &ics_of(&format!(
                "- TODO t ^abc123\n  deadline:: 2026-10-21\n  repeat:: {r}\n"
            ))[0];
            ics.lines()
                .find_map(|l| l.strip_prefix("RRULE:"))
                .map(str::to_string)
        };
        assert_eq!(rep("daily").as_deref(), Some("FREQ=DAILY"));
        assert_eq!(rep("every 2w").as_deref(), Some("FREQ=WEEKLY;INTERVAL=2"));
        assert_eq!(
            rep("weekdays").as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR")
        );
        assert_eq!(rep("every 1m").as_deref(), Some("FREQ=MONTHLY"));
    }

    #[test]
    fn a_time_range_without_a_deadline_gives_the_end_as_due() {
        let ics = &ics_of("- TODO meet ^abc123\n  scheduled:: 2026-10-14 09:00-10:30\n")[0];
        assert!(ics.contains("DTSTART:20261014T090000\r\n"), "{ics}");
        assert!(ics.contains("DUE:20261014T103000\r\n"), "{ics}");
    }

    #[test]
    fn uids_round_trip_and_belong_to_one_graph() {
        let u = uid("g1", "abc123");
        assert_eq!(u, "notatnik-g1-abc123");
        assert_eq!(block_id_of("g1", &u).as_deref(), Some("abc123"));
        assert_eq!(block_id_of("g2", &u), None);
        assert_eq!(block_id_of("g1", "someone-else"), None);
    }

    #[test]
    fn priority_labels_spread_over_one_to_nine() {
        assert_eq!(
            (0..3).map(|r| priority_number(r, 3)).collect::<Vec<_>>(),
            [1, 5, 9]
        );
        assert_eq!(
            (0..5).map(|r| priority_number(r, 5)).collect::<Vec<_>>(),
            [1, 3, 5, 7, 9]
        );
        assert_eq!(priority_number(0, 1), 1);
    }
}
