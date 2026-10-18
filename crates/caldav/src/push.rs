//! Sending the dated tasks of a graph to a calendar (CA-4, CA-6, CA-9).
//!
//! The caller says which tasks should be in the calendar ([`Todo`] values from [`crate::map`]); this module makes
//! the calendar match, with as few requests as possible. What was sent last time is kept in a [`PushState`] that
//! the caller stores; losing it only costs one comparison with the server.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::ical::{Status, Todo, parse_todo};
use crate::{CalDavClient, Cond, Error, Result};

/// What happens to the calendar item of a task that is gone from the notes (CA-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDelete {
    /// Mark it completed; it stays in the calendar.
    Complete,
    /// Delete it.
    Delete,
}

/// Settings of one push.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// The current time in UTC: `DTSTAMP` of what is written, and the completion time of a retired item.
    pub now: NaiveDateTime,
    pub on_delete: OnDelete,
    /// The graph's index is complete and nothing is being re-read. Only then does a missing task count towards
    /// its removal: a page that is briefly absent (a sync in progress) must not delete anything.
    pub settled: bool,
}

/// What was sent for one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pushed {
    pub etag: String,
    /// Hash of the item without its `DTSTAMP`: equal hash, nothing to send.
    pub hash: String,
    /// The text that was sent (used to retire the item).
    pub ics: String,
    /// In how many runs in a row the task was missing from the notes.
    #[serde(default)]
    pub missing: u8,
    /// Marked completed because the task is gone; a task that comes back is sent again.
    #[serde(default)]
    pub retired: bool,
}

/// Per device and calendar: what the last push left on the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushState {
    /// The calendar these entries belong to; another calendar starts again.
    pub calendar: String,
    /// By UID.
    pub items: BTreeMap<String, Pushed>,
}

/// What a push did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub retired: usize,
    pub deleted: usize,
    /// UIDs whose server copy changed since we wrote it: not overwritten (reading it back is the pull's job).
    pub conflicts: Vec<String>,
}

/// FNV-1a: the same on every device and run, which is all a change check needs.
fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn name_of(uid: &str) -> String {
    format!("{uid}.ics")
}

/// Makes the calendar hold `desired` (and nothing of ours that is not in it, once the removal rules allow).
/// Stops with the error on a network or login failure; what was done before it stays in `state`.
pub fn push(
    client: &dyn CalDavClient,
    calendar: &str,
    desired: &[Todo],
    state: &mut PushState,
    opts: &Options,
) -> Result<Report> {
    if state.calendar != calendar {
        *state = PushState {
            calendar: calendar.to_string(),
            items: BTreeMap::new(),
        };
    }
    let mut report = Report::default();
    let mut wanted = std::collections::BTreeSet::new();
    for todo in desired {
        wanted.insert(todo.uid.clone());
        let mut stamped = todo.clone();
        stamped.stamp = None;
        let hash = fnv(&stamped.to_ics());
        stamped.stamp = Some(opts.now);
        let ics = stamped.to_ics();
        let known = state.items.get(&todo.uid).cloned();
        match known {
            Some(k) if k.hash == hash && !k.retired => {
                if k.missing != 0 {
                    state.items.get_mut(&todo.uid).unwrap().missing = 0;
                }
                report.unchanged += 1;
            }
            Some(k) => {
                match client.put(calendar, &name_of(&todo.uid), &ics, &Cond::IfMatch(k.etag)) {
                    Ok(etag) => {
                        state
                            .items
                            .insert(todo.uid.clone(), pushed(etag, hash, ics));
                        report.updated += 1;
                    }
                    Err(Error::Changed(_)) => report.conflicts.push(todo.uid.clone()),
                    Err(e) => return Err(e),
                }
            }
            None => match client.put(calendar, &name_of(&todo.uid), &ics, &Cond::Create) {
                Ok(etag) => {
                    state
                        .items
                        .insert(todo.uid.clone(), pushed(etag, hash, ics));
                    report.created += 1;
                }
                Err(Error::Changed(_)) => {
                    adopt(client, calendar, todo, &ics, hash, state, &mut report)?
                }
                Err(e) => return Err(e),
            },
        }
    }
    let gone: Vec<String> = state
        .items
        .iter()
        .filter(|(uid, p)| !wanted.contains(*uid) && !p.retired)
        .map(|(uid, _)| uid.clone())
        .collect();
    for uid in gone {
        if !opts.settled {
            continue;
        }
        let p = state.items.get_mut(&uid).unwrap();
        p.missing = p.missing.saturating_add(1);
        if p.missing < 2 {
            continue;
        }
        let (name, etag, text) = (name_of(&uid), p.etag.clone(), p.ics.clone());
        match opts.on_delete {
            OnDelete::Delete => match client.delete(calendar, &name, Some(&etag)) {
                Ok(()) | Err(Error::NotFound(_)) => {
                    state.items.remove(&uid);
                    report.deleted += 1;
                }
                Err(Error::Changed(_)) => report.conflicts.push(uid),
                Err(e) => return Err(e),
            },
            OnDelete::Complete => {
                let mut todo = parse_todo(&text).unwrap_or_default();
                if matches!(todo.status, Some(Status::Completed | Status::Cancelled)) {
                    state.items.get_mut(&uid).unwrap().retired = true;
                    continue;
                }
                todo.status = Some(Status::Completed);
                todo.completed = Some(opts.now);
                todo.stamp = Some(opts.now);
                let ics = todo.to_ics();
                match client.put(calendar, &name, &ics, &Cond::IfMatch(etag)) {
                    Ok(new_etag) => {
                        let p = state.items.get_mut(&uid).unwrap();
                        p.etag = new_etag;
                        p.ics = ics;
                        p.retired = true;
                        report.retired += 1;
                    }
                    Err(Error::Changed(_)) => report.conflicts.push(uid),
                    Err(Error::NotFound(_)) => {
                        state.items.remove(&uid);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(report)
}

fn pushed(etag: String, hash: String, ics: String) -> Pushed {
    Pushed {
        etag,
        hash,
        ics,
        missing: 0,
        retired: false,
    }
}

/// The item exists on the server and we do not know it (another device sent it, or our state was lost): when it
/// already says the same, take its ETag; otherwise our version replaces it, since the notes are the source.
fn adopt(
    client: &dyn CalDavClient,
    calendar: &str,
    todo: &Todo,
    ics: &str,
    hash: String,
    state: &mut PushState,
    report: &mut Report,
) -> Result<()> {
    let name = name_of(&todo.uid);
    let Some(remote) = client
        .get(calendar, std::slice::from_ref(&name))?
        .into_iter()
        .next()
    else {
        report.conflicts.push(todo.uid.clone()); // gone again meanwhile: the next run creates it
        return Ok(());
    };
    let same = parse_todo(&remote.ics).is_some_and(|mut r| {
        r.stamp = None;
        r.modified = None;
        let mut mine = todo.clone();
        mine.stamp = None;
        r == mine
    });
    if same {
        state
            .items
            .insert(todo.uid.clone(), pushed(remote.etag, hash, ics.to_string()));
        report.unchanged += 1;
        return Ok(());
    }
    match client.put(calendar, &name, ics, &Cond::IfMatch(remote.etag)) {
        Ok(etag) => {
            state
                .items
                .insert(todo.uid.clone(), pushed(etag, hash, ics.to_string()));
            report.updated += 1;
        }
        Err(Error::Changed(_)) => report.conflicts.push(todo.uid.clone()),
        Err(e) => return Err(e),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeServer;
    use crate::ical::When;
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 18)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap()
    }

    fn opts(on_delete: OnDelete, settled: bool) -> Options {
        Options {
            now: now(),
            on_delete,
            settled,
        }
    }

    fn todo(n: u32, due_day: u32) -> Todo {
        Todo {
            uid: format!("notatnik-g1-t{n:05}"),
            summary: format!("Task {n}"),
            status: Some(Status::NeedsAction),
            due: Some(When::Date(
                NaiveDate::from_ymd_opt(2026, 10, due_day).unwrap(),
            )),
            ..Todo::default()
        }
    }

    fn ten() -> Vec<Todo> {
        (0..10).map(|n| todo(n, 20)).collect()
    }

    fn setup() -> (FakeServer, String) {
        let s = FakeServer::new();
        let h = s.add_calendar("Tasks", &["VTODO"]);
        (s, h)
    }

    #[test]
    fn ten_new_tasks_are_created_and_a_second_push_sends_nothing() {
        let (s, h) = setup();
        let mut st = PushState::default();
        let r = push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!((r.created, r.unchanged), (10, 0));
        assert_eq!(s.names(&h).len(), 10);
        assert!(
            s.text(&h, "notatnik-g1-t00003.ics")
                .unwrap()
                .contains("SUMMARY:Task 3")
        );

        let before = s.requests();
        let r = push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!((r.created, r.updated, r.unchanged), (0, 0, 10));
        assert_eq!(s.requests(), before, "an unchanged push makes no request");
    }

    #[test]
    fn one_changed_task_is_one_request() {
        let (s, h) = setup();
        let mut st = PushState::default();
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        let before = s.requests();
        let mut tasks = ten();
        tasks[4] = todo(4, 25);
        let r = push(&s, &h, &tasks, &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!((r.updated, r.unchanged), (1, 9));
        assert_eq!(s.requests(), before + 1);
        assert!(
            s.text(&h, "notatnik-g1-t00004.ics")
                .unwrap()
                .contains("DUE;VALUE=DATE:20261025")
        );
    }

    #[test]
    fn two_devices_with_their_own_state_do_not_duplicate_or_resend() {
        let (s, h) = setup();
        let (mut a, mut b) = (PushState::default(), PushState::default());
        push(&s, &h, &ten(), &mut a, &opts(OnDelete::Complete, true)).unwrap();
        // the second device knows nothing: it finds the same content and takes the server's ETags
        let r = push(&s, &h, &ten(), &mut b, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!((r.created, r.updated, r.unchanged), (0, 0, 10));
        assert_eq!(s.names(&h).len(), 10);
        // a lost state costs one comparison per item, not a rewrite
        let mut lost = PushState::default();
        let before = s.requests();
        push(&s, &h, &ten(), &mut lost, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(s.requests() - before, 20); // a refused create and a read for each
        // the first device changes one task; the second one sends the old text again only if it changed too
        let mut tasks = ten();
        tasks[0] = todo(0, 22);
        push(&s, &h, &tasks, &mut a, &opts(OnDelete::Complete, true)).unwrap();
        let r = push(&s, &h, &tasks, &mut b, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(
            r.conflicts,
            ["notatnik-g1-t00000"],
            "its ETag is stale; the pull sorts it out"
        );
        assert_eq!(r.updated, 0);
    }

    #[test]
    fn a_server_copy_changed_elsewhere_is_not_overwritten() {
        let (s, h) = setup();
        let mut st = PushState::default();
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        // someone completes it on the phone
        let name = "notatnik-g1-t00001.ics";
        let etag = s
            .list(&h)
            .unwrap()
            .into_iter()
            .find(|i| i.name == name)
            .unwrap()
            .etag;
        s.put(
            &h,
            name,
            &s.text(&h, name)
                .unwrap()
                .replace("NEEDS-ACTION", "COMPLETED"),
            &Cond::IfMatch(etag),
        )
        .unwrap();
        let mut tasks = ten();
        tasks[1] = todo(1, 23);
        let r = push(&s, &h, &tasks, &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(r.conflicts, ["notatnik-g1-t00001"]);
        assert!(s.text(&h, name).unwrap().contains("COMPLETED"));
    }

    #[test]
    fn a_missing_task_is_retired_only_after_two_settled_runs() {
        let (s, h) = setup();
        let mut st = PushState::default();
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        let fewer = &ten()[1..];
        // the index is not settled (a page may be missing for a moment): nothing happens, however often
        for _ in 0..3 {
            push(&s, &h, fewer, &mut st, &opts(OnDelete::Complete, false)).unwrap();
        }
        assert!(
            s.text(&h, "notatnik-g1-t00000.ics")
                .unwrap()
                .contains("NEEDS-ACTION")
        );
        // one settled run: still nothing
        let r = push(&s, &h, fewer, &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(r.retired, 0);
        assert!(
            s.text(&h, "notatnik-g1-t00000.ics")
                .unwrap()
                .contains("NEEDS-ACTION")
        );
        // the second: completed, and it stays
        let r = push(&s, &h, fewer, &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(r.retired, 1);
        let text = s.text(&h, "notatnik-g1-t00000.ics").unwrap();
        assert!(
            text.contains("STATUS:COMPLETED") && text.contains("COMPLETED:20261018T090000Z"),
            "{text}"
        );
        // and it is not touched again
        let before = s.requests();
        push(&s, &h, fewer, &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(s.requests(), before);
        // when the task comes back it is sent again, open
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert!(
            s.text(&h, "notatnik-g1-t00000.ics")
                .unwrap()
                .contains("NEEDS-ACTION")
        );
    }

    #[test]
    fn a_task_that_comes_back_in_time_is_not_removed() {
        let (s, h) = setup();
        let mut st = PushState::default();
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Delete, true)).unwrap();
        push(&s, &h, &ten()[1..], &mut st, &opts(OnDelete::Delete, true)).unwrap(); // missing once
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Delete, true)).unwrap(); // back: the count starts over
        push(&s, &h, &ten()[1..], &mut st, &opts(OnDelete::Delete, true)).unwrap();
        assert_eq!(s.names(&h).len(), 10);
    }

    #[test]
    fn delete_policy_removes_the_item() {
        let (s, h) = setup();
        let mut st = PushState::default();
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Delete, true)).unwrap();
        for _ in 0..2 {
            push(&s, &h, &ten()[1..], &mut st, &opts(OnDelete::Delete, true)).unwrap();
        }
        assert_eq!(s.names(&h).len(), 9);
        assert!(s.text(&h, "notatnik-g1-t00000.ics").is_none());
        assert!(!st.items.contains_key("notatnik-g1-t00000"));
    }

    #[test]
    fn a_network_failure_stops_the_push_and_keeps_what_was_done() {
        let (s, h) = setup();
        let mut st = PushState::default();
        push(
            &s,
            &h,
            &ten()[..3],
            &mut st,
            &opts(OnDelete::Complete, true),
        )
        .unwrap();
        s.fail_next("offline");
        let err = push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap_err();
        assert!(matches!(err, Error::Offline(_)));
        assert_eq!(st.items.len(), 3);
        // the next run finishes the job
        let r = push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!((r.created, r.unchanged), (7, 3));
    }

    #[test]
    fn another_calendar_starts_again() {
        let (s, h) = setup();
        let other = s.add_calendar("Work", &["VTODO"]);
        let mut st = PushState::default();
        push(&s, &h, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        let r = push(&s, &other, &ten(), &mut st, &opts(OnDelete::Complete, true)).unwrap();
        assert_eq!(r.created, 10);
        assert_eq!(s.names(&other).len(), 10);
    }
}
