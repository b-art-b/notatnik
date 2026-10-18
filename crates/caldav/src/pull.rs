//! Reading changes made in the calendar back (CA-5, CA-8, CA-9).
//!
//! For every task that is in the notes and in the calendar the engine compares three versions per field: what
//! the last sync left on the server (`base`), what the notes say now (`local`) and what the server says now
//! (`remote`). A field only one side changed takes that side's value; a field both changed is a conflict, decided
//! by whoever changed the item last (CA-8). The result is a list of [`Change`]s the caller writes into the
//! blocks, and the [`PushState`] is updated so the following push does not send the same thing back.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::ical::{Status, Todo, When, parse_todo};
use crate::map::uid_prefix;
use crate::push::PushState;
use crate::{CalDavClient, Error, Result, ServerItem};

/// A field the calendar and the notes share and both can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Status,
    Start,
    Due,
    Priority,
    Summary,
    Repeat,
}

impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Field::Status => "status",
            Field::Start => "scheduled",
            Field::Due => "deadline",
            Field::Priority => "priority",
            Field::Summary => "title",
            Field::Repeat => "repeat",
        }
    }
}

impl Field {
    /// The field with this name (as the sync log keeps it).
    pub fn from_name(name: &str) -> Option<Field> {
        FIELDS.into_iter().find(|f| f.name() == name)
    }
}

/// The change that sets `field` to a value as the sync log shows it (`when_text`, `status_text`, a number, the
/// title, the rule). `None` when the text is not such a value.
pub fn change_from_text(field: Field, text: &str) -> Option<Change> {
    let text = text.trim();
    let when = || -> Option<Option<When>> {
        if text.is_empty() {
            return Some(None);
        }
        if let Ok(at) = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M") {
            return Some(Some(When::Local { at, tzid: None }));
        }
        let d = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()?;
        Some(Some(When::Date(d)))
    };
    Some(match field {
        Field::Status => Change::Status {
            status: match text {
                "to do" => Status::NeedsAction,
                "in process" => Status::InProcess,
                "done" => Status::Completed,
                "canceled" => Status::Cancelled,
                _ => return None,
            },
            word: None,
            completed: None,
        },
        Field::Start => Change::Start(when()?),
        Field::Due => Change::Due(when()?),
        Field::Priority => Change::Priority(if text.is_empty() {
            None
        } else {
            Some(text.parse().ok()?)
        }),
        Field::Summary => Change::Summary(text.to_string()),
        Field::Repeat => Change::Repeat((!text.is_empty()).then(|| text.to_string())),
    })
}

/// A new value for a field of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Status {
        status: Status,
        /// `X-NOTATNIK-STATE` of the remote item (`WAITING`, `NOW`, ...).
        word: Option<String>,
        /// When it was completed (UTC), if the calendar says so.
        completed: Option<NaiveDateTime>,
    },
    Start(Option<When>),
    Due(Option<When>),
    /// 1 to 9; `None` removes the priority.
    Priority(Option<u8>),
    Summary(String),
    /// The text after `RRULE:`; `None` removes the repeat.
    Repeat(Option<String>),
}

impl Change {
    pub fn field(&self) -> Field {
        match self {
            Change::Status { .. } => Field::Status,
            Change::Start(_) => Field::Start,
            Change::Due(_) => Field::Due,
            Change::Priority(_) => Field::Priority,
            Change::Summary(_) => Field::Summary,
            Change::Repeat(_) => Field::Repeat,
        }
    }
}

/// Which side a conflict went to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Local,
    Remote,
}

/// A field both sides changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub uid: String,
    pub field: Field,
    pub local: String,
    pub remote: String,
    pub winner: Side,
}

/// What the server holds that matters to a pull.
#[derive(Debug, Clone, Default)]
pub struct Fetched {
    /// Tasks of this graph whose ETag is not the one we know, with their text.
    pub changed: Vec<ServerItem>,
    /// Tasks that were not created by this graph (no `notatnik-<graph>-` UID, but a `VTODO`).
    pub foreign: Vec<ServerItem>,
    /// UIDs we sent that are no longer on the server.
    pub removed: Vec<String>,
    /// UIDs of other apps' tasks we copied that are no longer on the server.
    pub gone_foreign: Vec<String>,
    /// Hand this to the next fetch (store it only when what was read has been dealt with).
    pub token: Option<String>,
}

/// Reads what changed in the calendar since the state was written. With a sync token that is one request (RFC
/// 6578) and nothing is read when nothing changed; without one, or when the server no longer knows it, the whole
/// calendar is listed and every new ETag is read. Both give the same result.
pub fn fetch(
    client: &dyn CalDavClient,
    calendar: &str,
    graph_id: &str,
    state: &PushState,
) -> Result<Fetched> {
    let prefix = uid_prefix(graph_id);
    let known = state.calendar == calendar;
    // (items that may have changed, names that are gone when the server says so, a token for next time)
    let (refs, deleted, token) = match state.token.as_deref().filter(|_| known) {
        Some(t) => match client.sync(calendar, Some(t)) {
            Ok(r) => (r.changed, Some(r.deleted), Some(r.token)),
            Err(Error::Changed(_)) => full(client, calendar)?,
            Err(e) => return Err(e),
        },
        None => full(client, calendar)?,
    };
    let mut ask = Vec::new();
    let mut foreign_names = Vec::new();
    for item in &refs {
        let Some(uid) = item.name.strip_suffix(".ics") else {
            continue;
        };
        if uid.starts_with(&prefix) {
            let same = state
                .items
                .get(uid)
                .is_some_and(|k| k.etag == item.etag && known);
            if !same {
                ask.push(item.name.clone());
            }
        } else if !uid.starts_with("notatnik-") {
            foreign_names.push(item.name.clone());
        }
    }
    let mut out = Fetched {
        token,
        ..Fetched::default()
    };
    ask.extend(foreign_names.iter().cloned());
    if !ask.is_empty() {
        for item in client.get(calendar, &ask)? {
            if foreign_names.contains(&item.name) {
                if parse_todo(&item.ics).is_some() {
                    out.foreign.push(item);
                }
            } else {
                out.changed.push(item);
            }
        }
    }
    if known {
        let gone = |uid: &str| match &deleted {
            Some(d) => d.iter().any(|n| n == &format!("{uid}.ics")),
            None => !refs.iter().any(|i| i.name == format!("{uid}.ics")),
        };
        for (uid, p) in &state.items {
            if !p.retired && gone(uid) {
                out.removed.push(uid.clone());
            }
        }
        for uid in state.imported.keys() {
            if gone(uid) {
                out.gone_foreign.push(uid.clone());
            }
        }
    }
    Ok(out)
}

type Everything = (Vec<crate::ItemRef>, Option<Vec<String>>, Option<String>);

/// Everything in the calendar, with a token when the server can give one.
fn full(client: &dyn CalDavClient, calendar: &str) -> Result<Everything> {
    match client.sync(calendar, None) {
        Ok(r) => Ok((r.changed, None, Some(r.token))),
        Err(e @ (Error::Offline(_) | Error::Auth(_))) => Err(e),
        // a server without sync-collection: list it
        Err(_) => Ok((client.list(calendar)?, None, None)),
    }
}

/// One task that changed in the calendar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pulled {
    pub uid: String,
    /// What to write into the block.
    pub changes: Vec<Change>,
    pub conflicts: Vec<Conflict>,
}

/// How a pull sees a task of the notes.
pub struct Local<'a> {
    pub todo: &'a Todo,
    /// When the notes changed (UTC), for deciding a conflict.
    pub modified: Option<NaiveDateTime>,
}

/// What a pull found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub items: Vec<Pulled>,
    /// Tasks that were deleted in the calendar. Their blocks stay; the push does not send them again.
    pub removed: Vec<String>,
}

fn status_of(t: &Todo) -> Status {
    t.status.unwrap_or(Status::NeedsAction)
}

fn priority_of(t: &Todo) -> Option<u8> {
    t.priority.filter(|p| *p > 0)
}

fn when_text(w: &Option<When>) -> String {
    match w {
        None => String::new(),
        Some(w) => {
            let (d, t) = w.floating();
            match t {
                Some(t) => format!("{} {}", d.format("%Y-%m-%d"), t.format("%H:%M")),
                None => d.format("%Y-%m-%d").to_string(),
            }
        }
    }
}

fn status_text(s: Status) -> String {
    match s {
        Status::NeedsAction => "to do",
        Status::InProcess => "in process",
        Status::Completed => "done",
        Status::Cancelled => "canceled",
    }
    .into()
}

/// The fields in which `a` and `b` differ, with each side's value as text.
fn differ(a: &Todo, b: &Todo, field: Field) -> Option<(String, String)> {
    let (x, y) = match field {
        Field::Status => {
            if status_of(a) == status_of(b) {
                return None;
            }
            (status_text(status_of(a)), status_text(status_of(b)))
        }
        Field::Start => {
            if a.start.as_ref().map(When::floating) == b.start.as_ref().map(When::floating) {
                return None;
            }
            (when_text(&a.start), when_text(&b.start))
        }
        Field::Due => {
            if a.due.as_ref().map(When::floating) == b.due.as_ref().map(When::floating) {
                return None;
            }
            (when_text(&a.due), when_text(&b.due))
        }
        Field::Priority => {
            if priority_of(a) == priority_of(b) {
                return None;
            }
            (
                priority_of(a).map(|p| p.to_string()).unwrap_or_default(),
                priority_of(b).map(|p| p.to_string()).unwrap_or_default(),
            )
        }
        Field::Summary => {
            if a.summary.trim() == b.summary.trim() {
                return None;
            }
            (a.summary.trim().to_string(), b.summary.trim().to_string())
        }
        Field::Repeat => {
            let n = |t: &Todo| t.rrule.as_deref().map(|r| r.trim().to_string());
            if n(a) == n(b) {
                return None;
            }
            (n(a).unwrap_or_default(), n(b).unwrap_or_default())
        }
    };
    Some((x, y))
}

fn change_of(remote: &Todo, field: Field) -> Change {
    match field {
        Field::Status => Change::Status {
            status: status_of(remote),
            word: remote.state.clone(),
            completed: remote.completed,
        },
        Field::Start => Change::Start(remote.start.clone()),
        Field::Due => Change::Due(remote.due.clone()),
        Field::Priority => Change::Priority(priority_of(remote)),
        Field::Summary => Change::Summary(remote.summary.trim().to_string()),
        Field::Repeat => Change::Repeat(remote.rrule.clone()),
    }
}

const FIELDS: [Field; 6] = [
    Field::Status,
    Field::Start,
    Field::Due,
    Field::Priority,
    Field::Summary,
    Field::Repeat,
];

/// Decides what to take from the changed items. `local` holds the tasks of the notes by UID; an item for which
/// the notes have no task (the block is gone, undated or `cal:: no`) is left alone. The state of every item that
/// was read is brought up to date: its ETag and text are the server's now, and an item whose notes are about to
/// change is marked so the push can tell the notes caught up.
pub fn plan(
    fetched: &Fetched,
    local: &BTreeMap<String, Local>,
    state: &mut PushState,
    calendar: &str,
) -> Plan {
    if state.calendar != calendar {
        *state = PushState {
            calendar: calendar.to_string(),
            ..PushState::default()
        };
    }
    let mut out = Plan::default();
    for item in &fetched.changed {
        let Some(remote) = parse_todo(&item.ics) else {
            continue;
        };
        let Some(mine) = local.get(&remote.uid) else {
            continue;
        };
        let known = state.items.get(&remote.uid).cloned();
        let base = known.as_ref().and_then(|k| parse_todo(&k.ics));
        let remote_time = remote.modified.or(remote.stamp);
        let mut pulled = Pulled {
            uid: remote.uid.clone(),
            ..Pulled::default()
        };
        let mut local_wins = false;
        for field in FIELDS {
            let remote_changed = match &base {
                Some(b) => differ(b, &remote, field).is_some(),
                // nothing is known about the last sync: the server's status counts, the rest is the notes'
                None => field == Field::Status && differ(mine.todo, &remote, field).is_some(),
            };
            if !remote_changed {
                continue;
            }
            let Some((l, r)) = differ(mine.todo, &remote, field) else {
                continue; // the notes already say it
            };
            let local_changed = base
                .as_ref()
                .is_none_or(|b| differ(b, mine.todo, field).is_some());
            if base.is_some() && local_changed {
                let winner = match (mine.modified, remote_time) {
                    (Some(l), Some(r)) if r > l => Side::Remote,
                    _ => Side::Local,
                };
                pulled.conflicts.push(Conflict {
                    uid: remote.uid.clone(),
                    field,
                    local: l,
                    remote: r,
                    winner,
                });
                if winner == Side::Remote {
                    pulled.changes.push(change_of(&remote, field));
                } else {
                    local_wins = true;
                }
            } else {
                pulled.changes.push(change_of(&remote, field));
            }
        }
        if known.is_some() {
            let k = state.items.get_mut(&remote.uid).unwrap();
            k.etag = item.etag.clone();
            k.ics = item.ics.clone();
            k.gone = false;
            if !pulled.changes.is_empty() {
                k.pulled = true;
            }
            if local_wins {
                // the notes keep their value, so the server must get it again
                k.hash.clear();
            }
        }
        if !pulled.changes.is_empty() || !pulled.conflicts.is_empty() {
            out.items.push(pulled);
        }
    }
    for uid in &fetched.removed {
        if let Some(k) = state.items.get_mut(uid) {
            if local.contains_key(uid) {
                k.gone = true;
                out.removed.push(uid.clone());
            } else {
                state.items.remove(uid);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeServer;
    use crate::ical::When;
    use crate::push::{OnDelete, Options, push};
    use crate::{CalDavClient, Cond};
    use chrono::NaiveDate;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 18)
            .unwrap()
            .and_hms_opt(9, 0, 0)
            .unwrap()
    }

    fn opts() -> Options {
        Options {
            now: now(),
            on_delete: OnDelete::Complete,
            settled: true,
        }
    }

    fn day(d: u32) -> Option<When> {
        Some(When::Date(NaiveDate::from_ymd_opt(2026, 10, d).unwrap()))
    }

    fn todo(n: u32) -> Todo {
        Todo {
            uid: format!("notatnik-g1-t{n:05}"),
            summary: format!("Task {n}"),
            status: Some(Status::NeedsAction),
            due: day(20),
            ..Todo::default()
        }
    }

    fn name(n: u32) -> String {
        format!("notatnik-g1-t{n:05}.ics")
    }

    /// Edits the server's copy like another client would.
    fn edit_remote(s: &FakeServer, h: &str, n: u32, f: impl Fn(&str) -> String) {
        let item = s.get(h, &[name(n)]).unwrap().into_iter().next().unwrap();
        s.put(h, &name(n), &f(&item.ics), &Cond::IfMatch(item.etag))
            .unwrap();
    }

    fn setup() -> (FakeServer, String, PushState) {
        let s = FakeServer::new();
        let h = s.add_calendar("Tasks", &["VTODO"]);
        let mut st = PushState::default();
        push(&s, &h, &[todo(1), todo(2)], &mut st, &opts()).unwrap();
        (s, h, st)
    }

    fn run(
        s: &FakeServer,
        h: &str,
        st: &mut PushState,
        locals: &[(Todo, Option<NaiveDateTime>)],
    ) -> Plan {
        let fetched = fetch(s, h, "g1", st).unwrap();
        let map: BTreeMap<String, Local> = locals
            .iter()
            .map(|(t, m)| {
                (
                    t.uid.clone(),
                    Local {
                        todo: t,
                        modified: *m,
                    },
                )
            })
            .collect();
        let token = fetched.token.clone();
        let p = plan(&fetched, &map, st, h);
        st.token = token;
        p
    }

    #[test]
    fn nothing_changed_nothing_is_read() {
        let (s, h, mut st) = setup();
        let before = s.requests();
        let p = run(&s, &h, &mut st, &[(todo(1), None), (todo(2), None)]);
        assert_eq!(p, Plan::default());
        assert_eq!(s.requests() - before, 1, "one listing");
    }

    #[test]
    fn with_a_token_a_quiet_calendar_costs_one_request_and_a_change_is_found_without_listing() {
        let (s, h, mut st) = setup();
        let both = [(todo(1), None), (todo(2), None)];
        run(&s, &h, &mut st, &both); // the first run takes the token
        assert!(st.token.is_some());
        let before = s.requests();
        assert_eq!(run(&s, &h, &mut st, &both), Plan::default());
        assert_eq!(s.requests() - before, 1);

        edit_remote(&s, &h, 2, |t| t.replace("NEEDS-ACTION", "COMPLETED"));
        let before = s.requests();
        let p = run(&s, &h, &mut st, &both);
        assert_eq!(p.items.len(), 1);
        assert_eq!(s.requests() - before, 2, "the report and one read");

        s.delete(&h, &name(1), None).unwrap();
        let p = run(&s, &h, &mut st, &both);
        assert_eq!(p.removed, ["notatnik-g1-t00001"]);
    }

    #[test]
    fn a_token_the_server_forgot_gives_a_full_comparison_and_a_new_token() {
        let (s, h, mut st) = setup();
        let both = [(todo(1), None), (todo(2), None)];
        run(&s, &h, &mut st, &both);
        let old = st.token.clone();
        edit_remote(&s, &h, 1, |t| t.replace("NEEDS-ACTION", "COMPLETED"));
        s.forget_sync_tokens(&h);
        let p = run(&s, &h, &mut st, &both);
        assert_eq!(p.items.len(), 1, "found by comparing everything");
        assert_ne!(st.token, old);
        assert!(st.token.is_some());
    }

    #[test]
    fn a_completion_in_the_calendar_comes_back_and_the_next_push_sends_nothing() {
        let (s, h, mut st) = setup();
        edit_remote(&s, &h, 1, |t| t.replace("NEEDS-ACTION", "COMPLETED"));
        let p = run(&s, &h, &mut st, &[(todo(1), None), (todo(2), None)]);
        assert_eq!(p.items.len(), 1);
        assert!(matches!(
            p.items[0].changes[..],
            [Change::Status {
                status: Status::Completed,
                ..
            }]
        ));
        // the notes now say DONE
        let mut done = todo(1);
        done.status = Some(Status::Completed);
        done.completed = Some(now());
        let before = s.requests();
        let r = push(&s, &h, &[done, todo(2)], &mut st, &opts()).unwrap();
        assert_eq!((r.created, r.updated, r.conflicts.len()), (0, 0, 0));
        assert_eq!(s.requests(), before, "the notes caught up: nothing sent");
    }

    #[test]
    fn a_changed_due_date_and_priority_come_back_other_fields_stay() {
        let (s, h, mut st) = setup();
        edit_remote(&s, &h, 2, |t| {
            t.replace("DUE;VALUE=DATE:20261020", "DUE;VALUE=DATE:20261030")
                .replace("SUMMARY:Task 2", "SUMMARY:Task 2\r\nPRIORITY:1")
        });
        let p = run(&s, &h, &mut st, &[(todo(1), None), (todo(2), None)]);
        assert_eq!(
            p.items[0].changes,
            [Change::Due(day(30)), Change::Priority(Some(1))]
        );
    }

    #[test]
    fn different_fields_changed_on_both_sides_keep_both() {
        let (s, h, mut st) = setup();
        edit_remote(&s, &h, 1, |t| t.replace("NEEDS-ACTION", "COMPLETED"));
        let mut mine = todo(1);
        mine.due = day(25); // changed in the notes meanwhile
        let p = run(&s, &h, &mut st, &[(mine.clone(), None), (todo(2), None)]);
        assert_eq!(p.items[0].changes.len(), 1);
        assert!(p.items[0].conflicts.is_empty());
        // the push then sends the notes' date on top of the completed item, with the new ETag
        mine.status = Some(Status::Completed);
        mine.completed = Some(now());
        let r = push(&s, &h, &[mine, todo(2)], &mut st, &opts()).unwrap();
        assert_eq!((r.updated, r.conflicts.len()), (1, 0));
        let text = s.text(&h, &name(1)).unwrap();
        assert!(text.contains("COMPLETED") && text.contains("DUE;VALUE=DATE:20261025"));
    }

    #[test]
    fn the_same_field_changed_on_both_sides_goes_to_the_newer_change_and_is_reported() {
        let (s, h, mut st) = setup();
        edit_remote(&s, &h, 1, |t| {
            t.replace("DUE;VALUE=DATE:20261020", "DUE;VALUE=DATE:20261030")
                .replace(
                    "SUMMARY:Task 1",
                    "SUMMARY:Task 1\r\nLAST-MODIFIED:20261018T080000Z",
                )
        });
        let mut mine = todo(1);
        mine.due = day(25);
        // the notes changed earlier than the calendar: the calendar wins
        let earlier = now() - chrono::Duration::hours(3);
        let p = run(&s, &h, &mut st, &[(mine.clone(), Some(earlier))]);
        assert_eq!(p.items[0].changes, [Change::Due(day(30))]);
        assert_eq!(p.items[0].conflicts[0].winner, Side::Remote);
        assert_eq!(p.items[0].conflicts[0].local, "2026-10-25");
        assert_eq!(p.items[0].conflicts[0].remote, "2026-10-30");

        // and the other way round: the notes changed after the calendar, the notes win and are sent again
        let (s, h, mut st) = setup();
        edit_remote(&s, &h, 1, |t| {
            t.replace("DUE;VALUE=DATE:20261020", "DUE;VALUE=DATE:20261030")
                .replace(
                    "SUMMARY:Task 1",
                    "SUMMARY:Task 1\r\nLAST-MODIFIED:20261018T080000Z",
                )
        });
        let later = now();
        let p = run(&s, &h, &mut st, &[(mine.clone(), Some(later))]);
        assert!(p.items[0].changes.is_empty());
        assert_eq!(p.items[0].conflicts[0].winner, Side::Local);
        let r = push(&s, &h, &[mine, todo(2)], &mut st, &opts()).unwrap();
        assert_eq!(r.updated, 1);
        assert!(
            s.text(&h, &name(1))
                .unwrap()
                .contains("DUE;VALUE=DATE:20261025")
        );
    }

    #[test]
    fn a_tie_goes_to_the_notes() {
        let (s, h, mut st) = setup();
        edit_remote(&s, &h, 1, |t| {
            t.replace("DUE;VALUE=DATE:20261020", "DUE;VALUE=DATE:20261030")
                .replace(
                    "SUMMARY:Task 1",
                    "SUMMARY:Task 1\r\nLAST-MODIFIED:20261018T090000Z",
                )
        });
        let mut mine = todo(1);
        mine.due = day(25);
        let p = run(&s, &h, &mut st, &[(mine, Some(now()))]);
        assert_eq!(p.items[0].conflicts[0].winner, Side::Local);
    }

    #[test]
    fn without_a_known_state_the_calendars_status_counts_and_the_rest_is_the_notes() {
        let s = FakeServer::new();
        let h = s.add_calendar("Tasks", &["VTODO"]);
        let mut theirs = todo(1);
        theirs.status = Some(Status::Completed);
        theirs.due = day(30);
        s.put(&h, &name(1), &theirs.to_ics(), &Cond::Create)
            .unwrap();
        let mut st = PushState::default();
        let p = run(&s, &h, &mut st, &[(todo(1), None)]);
        assert_eq!(p.items[0].changes.len(), 1);
        assert!(matches!(p.items[0].changes[0], Change::Status { .. }));
    }

    #[test]
    fn a_task_deleted_in_the_calendar_is_reported_and_not_sent_again() {
        let (s, h, mut st) = setup();
        s.delete(&h, &name(2), None).unwrap();
        let p = run(&s, &h, &mut st, &[(todo(1), None), (todo(2), None)]);
        assert_eq!(p.removed, ["notatnik-g1-t00002"]);
        let before = s.requests();
        let r = push(&s, &h, &[todo(1), todo(2)], &mut st, &opts()).unwrap();
        assert_eq!((r.created, r.unchanged), (0, 2));
        assert_eq!(s.requests(), before);
        assert!(s.text(&h, &name(2)).is_none());
    }

    #[test]
    fn tasks_of_other_apps_are_listed_as_foreign_and_ours_of_another_graph_are_ignored() {
        let (s, h, mut st) = setup();
        let other = Todo {
            uid: "abc-123".into(),
            summary: "Buy milk".into(),
            ..Todo::default()
        };
        s.put(&h, "abc-123.ics", &other.to_ics(), &Cond::Create)
            .unwrap();
        let foreign_graph = Todo {
            uid: "notatnik-g2-x".into(),
            summary: "Other graph".into(),
            ..Todo::default()
        };
        s.put(
            &h,
            "notatnik-g2-x.ics",
            &foreign_graph.to_ics(),
            &Cond::Create,
        )
        .unwrap();
        let f = fetch(&s, &h, "g1", &st).unwrap();
        assert_eq!(f.foreign.len(), 1);
        assert_eq!(f.foreign[0].name, "abc-123.ics");
        assert!(f.changed.is_empty());
        let _ = &mut st;
    }

    #[test]
    fn values_shown_in_the_log_can_be_read_back() {
        let day = |d| {
            Some(When::Date(
                chrono::NaiveDate::from_ymd_opt(2026, 10, d).unwrap(),
            ))
        };
        for (field, text) in [
            (Field::Due, "2026-10-25"),
            (Field::Start, "2026-10-25 09:30"),
            (Field::Due, ""),
            (Field::Priority, "5"),
            (Field::Priority, ""),
            (Field::Status, "done"),
            (Field::Status, "in process"),
            (Field::Summary, "Pay the rent"),
            (Field::Repeat, "FREQ=WEEKLY"),
        ] {
            let c = change_from_text(field, text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(c.field(), field);
        }
        assert_eq!(
            change_from_text(Field::Due, "2026-10-25"),
            Some(Change::Due(day(25)))
        );
        assert_eq!(change_from_text(Field::Due, "soon"), None);
        assert_eq!(change_from_text(Field::Status, "maybe"), None);
        assert_eq!(Field::from_name("deadline"), Some(Field::Due));
        assert_eq!(Field::from_name("x"), None);
    }
}
