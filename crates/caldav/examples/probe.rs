//! Tries the CalDAV client against a real server: lists the calendars and how many items each has.
//!
//! `NC_SERVER=http://localhost:8080 NC_USER=me NC_PASSWORD=... cargo run -p notatnik-caldav --example probe`
//! The password comes from the environment only; nothing is written anywhere.

use notatnik_caldav::{CalDavClient, HttpCalDav};

fn main() {
    let get = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let client = HttpCalDav::new(&get("NC_SERVER"), &get("NC_USER"), &get("NC_PASSWORD")).unwrap();
    for cal in client.calendars().unwrap() {
        let items = client.list(&cal.href).unwrap();
        let sync = client.sync(&cal.href, None).unwrap();
        println!(
            "{} {} [{}]: {} items, sync token {}",
            cal.name,
            cal.href,
            cal.components.join(","),
            items.len(),
            sync.token
        );
    }
}
