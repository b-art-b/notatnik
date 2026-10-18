//! The in-memory server passes the same checks as a real one (see `common`).
mod common;

use notatnik_caldav::fake::FakeServer;

#[test]
fn contract() {
    let s = FakeServer::new();
    let h = s.add_calendar("Tasks", &["VTODO"]);
    common::contract(&s, &h);
}

#[test]
fn engine() {
    let s = FakeServer::new();
    let h = s.add_calendar("Tasks", &["VTODO"]);
    common::engine(&s, &h);
}
