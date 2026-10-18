//! Round trip against a real server on one calendar: create, read, update, stale ETag, sync delta, delete.
//!
//! `NC_SERVER=... NC_USER=... NC_PASSWORD=... NC_CALENDAR=my-todos cargo run -p notatnik-caldav --example live`
//! Uses one item with a unique name and removes it at the end. The password comes from the environment only.

use notatnik_caldav::{CalDavClient, Cond, HttpCalDav};

fn ics(summary: &str, status: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//notatnik//live//EN\r\nBEGIN:VTODO\r\nUID:notatnik-live-test\r\nDTSTAMP:20260101T000000Z\r\nSUMMARY:{summary}\r\nSTATUS:{status}\r\nDUE:20260301T090000\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
    )
}

fn main() {
    let get = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let client = HttpCalDav::new(&get("NC_SERVER"), &get("NC_USER"), &get("NC_PASSWORD")).unwrap();
    let want = get("NC_CALENDAR");
    let cal = client
        .calendars()
        .unwrap()
        .into_iter()
        .find(|c| c.href.trim_end_matches('/').ends_with(&want))
        .expect("calendar not found")
        .href;
    let name = "notatnik-live-test.ics";
    let names = vec![name.to_string()];
    let start = client.sync(&cal, None).unwrap();
    println!("start token {}", start.token);

    let e1 = client
        .put(&cal, name, &ics("live one", "NEEDS-ACTION"), &Cond::Create)
        .unwrap();
    println!("created, etag {e1}");
    println!(
        "create again: {:?}",
        client
            .put(&cal, name, &ics("x", "NEEDS-ACTION"), &Cond::Create)
            .err()
    );

    let got = client.get(&cal, &names).unwrap();
    println!(
        "read back {} item(s); has our summary: {}",
        got.len(),
        got[0].ics.contains("SUMMARY:live one")
    );

    let delta = client.sync(&cal, Some(&start.token)).unwrap();
    println!(
        "delta after create: changed {:?}, token {}",
        delta.changed.iter().map(|i| &i.name).collect::<Vec<_>>(),
        delta.token
    );

    let e2 = client
        .put(
            &cal,
            name,
            &ics("live two", "COMPLETED"),
            &Cond::IfMatch(e1.clone()),
        )
        .unwrap();
    println!("updated, etag {e2}");
    println!(
        "update with stale etag: {:?}",
        client
            .put(&cal, name, &ics("x", "NEEDS-ACTION"), &Cond::IfMatch(e1))
            .err()
    );

    let quiet = client.sync(&cal, Some(&delta.token)).unwrap();
    println!(
        "changed since delta: {:?}",
        quiet.changed.iter().map(|i| &i.name).collect::<Vec<_>>()
    );

    client.delete(&cal, name, Some(&e2)).unwrap();
    let gone = client.sync(&cal, Some(&quiet.token)).unwrap();
    println!("deleted; sync reports removed {:?}", gone.deleted);
    println!(
        "still listed: {}",
        client.list(&cal).unwrap().iter().any(|i| i.name == name)
    );
}
