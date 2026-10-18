//! What every CalDAV server must do for the sync: the same checks run against the in-memory server (always) and
//! against Radicale (`--features integration`), so the fake cannot drift from the real thing.
#![allow(dead_code)]

use std::collections::BTreeMap;

use chrono::NaiveDate;
use notatnik_caldav::ical::{Status, Todo, When};
use notatnik_caldav::pull::{self, Change, Local};
use notatnik_caldav::push::{OnDelete, Options, PushState, push};
use notatnik_caldav::{CalDavClient, Cond, Error};

fn ics(uid: &str, summary: &str, status: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//notatnik//contract//EN\r\nBEGIN:VTODO\r\nUID:{uid}\r\nDTSTAMP:20261018T080000Z\r\nSUMMARY:{summary}\r\nSTATUS:{status}\r\nDUE;VALUE=DATE:20261021\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
    )
}

fn names(items: &[notatnik_caldav::ItemRef]) -> Vec<String> {
    let mut n: Vec<String> = items.iter().map(|i| i.name.clone()).collect();
    n.sort();
    n
}

/// Create, read, conditional update, stale writes, sync tokens, deletion.
pub fn contract(client: &dyn CalDavClient, calendar: &str) {
    let cal = client
        .calendars()
        .unwrap()
        .into_iter()
        .find(|c| c.href == calendar)
        .expect("the calendar is listed");
    assert!(cal.takes("VTODO"), "{cal:?}");

    let start = client.sync(calendar, None).unwrap();
    assert!(start.changed.is_empty());

    let name = "contract-one.ics";
    let e1 = client
        .put(
            calendar,
            name,
            &ics("contract-one", "First", "NEEDS-ACTION"),
            &Cond::Create,
        )
        .unwrap();
    assert!(
        matches!(
            client.put(
                calendar,
                name,
                &ics("contract-one", "Again", "NEEDS-ACTION"),
                &Cond::Create
            ),
            Err(Error::Changed(_))
        ),
        "creating over an existing item is refused"
    );

    let got = client
        .get(calendar, &[name.into(), "nothing-here.ics".into()])
        .unwrap();
    assert_eq!(got.len(), 1, "names that do not exist are left out");
    assert!(got[0].ics.contains("UID:contract-one") && got[0].ics.contains("SUMMARY:First"));
    assert_eq!(got[0].etag, e1);
    let listed = client.list(calendar).unwrap();
    assert_eq!(names(&listed), [name]);
    assert_eq!(listed[0].etag, e1);

    let first = client.sync(calendar, Some(&start.token)).unwrap();
    assert_eq!(names(&first.changed), [name]);
    let quiet = client.sync(calendar, Some(&first.token)).unwrap();
    assert!(
        quiet.changed.is_empty() && quiet.deleted.is_empty(),
        "{quiet:?}"
    );

    let e2 = client
        .put(
            calendar,
            name,
            &ics("contract-one", "Second", "COMPLETED"),
            &Cond::IfMatch(e1.clone()),
        )
        .unwrap();
    assert_ne!(e1, e2);
    assert!(
        matches!(
            client.put(
                calendar,
                name,
                &ics("contract-one", "Stale", "NEEDS-ACTION"),
                &Cond::IfMatch(e1)
            ),
            Err(Error::Changed(_))
        ),
        "a write with an old ETag is refused"
    );
    let moved = client.sync(calendar, Some(&quiet.token)).unwrap();
    assert_eq!(names(&moved.changed), [name]);
    assert!(
        client.get(calendar, &[name.into()]).unwrap()[0]
            .ics
            .contains("SUMMARY:Second")
    );

    assert!(
        matches!(
            client.sync(calendar, Some("http://example.org/ns/sync/does-not-exist")),
            Err(Error::Changed(_))
        ),
        "a token the server does not know means: read everything again"
    );

    assert!(
        matches!(
            client.delete(calendar, name, Some("\"wrong\"")),
            Err(Error::Changed(_))
        ),
        "a delete with an old ETag is refused"
    );
    client.delete(calendar, name, Some(&e2)).unwrap();
    let gone = client.sync(calendar, Some(&moved.token)).unwrap();
    assert_eq!(gone.deleted, [name]);
    assert!(client.list(calendar).unwrap().is_empty());
    assert!(matches!(
        client.delete(calendar, name, None),
        Err(Error::NotFound(_))
    ));
}

fn todo(n: u32) -> Todo {
    Todo {
        uid: format!("notatnik-g1-t{n:05}"),
        summary: format!("Task {n}"),
        status: Some(Status::NeedsAction),
        due: Some(When::Date(NaiveDate::from_ymd_opt(2026, 10, 20).unwrap())),
        ..Todo::default()
    }
}

/// The engine on top of the server: send, send nothing twice, read a completion back.
pub fn engine(client: &dyn CalDavClient, calendar: &str) {
    let now = NaiveDate::from_ymd_opt(2026, 10, 18)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap();
    let opts = Options {
        now,
        on_delete: OnDelete::Complete,
        settled: true,
    };
    let todos: Vec<Todo> = (0..5).map(todo).collect();
    let mut state = PushState::default();

    let r = push(client, calendar, &todos, &mut state, &opts).unwrap();
    assert_eq!((r.created, r.updated), (5, 0));
    let r = push(client, calendar, &todos, &mut state, &opts).unwrap();
    assert_eq!(
        (r.created, r.updated),
        (0, 0),
        "a second push sends nothing"
    );

    // another client completes one
    let item = client
        .get(calendar, &["notatnik-g1-t00002.ics".to_string()])
        .unwrap()
        .remove(0);
    client
        .put(
            calendar,
            &item.name,
            &item.ics.replace("STATUS:NEEDS-ACTION", "STATUS:COMPLETED"),
            &Cond::IfMatch(item.etag),
        )
        .unwrap();
    let fetched = pull::fetch(client, calendar, "g1", &state).unwrap();
    let local: BTreeMap<String, Local> = todos
        .iter()
        .map(|t| {
            (
                t.uid.clone(),
                Local {
                    todo: t,
                    modified: None,
                },
            )
        })
        .collect();
    let plan = pull::plan(&fetched, &local, &mut state, calendar);
    assert_eq!(plan.items.len(), 1, "{plan:?}");
    assert!(matches!(
        plan.items[0].changes[..],
        [Change::Status {
            status: Status::Completed,
            ..
        }]
    ));
    // the notes caught up: nothing bounces back
    let mut done = todos.clone();
    done[2].status = Some(Status::Completed);
    let r = push(client, calendar, &done, &mut state, &opts).unwrap();
    assert_eq!((r.created, r.updated), (0, 0), "{r:?}");
}
