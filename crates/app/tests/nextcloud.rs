//! Full sync against a real Nextcloud (CA-4, CA-5, CA-8). Ignored by default:
//!
//! `NC_SERVER=http://localhost:8080 NC_USER=me NC_PASSWORD=... NC_CALENDAR=my-todos \
//!  cargo test -p notatnik-app --test nextcloud -- --ignored --nocapture`
//!
//! The password comes from the environment only. Every task made here has "NCLIVE" in its title and is removed at the end.

use std::sync::Arc;

use chrono::NaiveDate;
use notatnik_app::{AppOptions, AppState, dispatch};
use notatnik_caldav::{CalDavClient, Cond, HttpCalDav};
use notatnik_graph::Dirs;
use serde_json::{Value, json};

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| panic!("set {k}"))
}

/// Removes the test tasks from the server, also when the test fails.
struct Cleanup(HttpCalDav, String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let items = self.0.list(&self.1).unwrap_or_default();
        let names: Vec<String> = items.iter().map(|i| i.name.clone()).collect();
        for item in self.0.get(&self.1, &names).unwrap_or_default() {
            if item.ics.contains("NCLIVE") {
                let _ = self.0.delete(&self.1, &item.name, None);
            }
        }
    }
}

#[test]
#[ignore = "needs a real server, see the header"]
fn full_sync_against_a_real_nextcloud() {
    let (server, user, password) = (env("NC_SERVER"), env("NC_USER"), env("NC_PASSWORD"));
    let tmp = tempfile::tempdir().unwrap();
    let graph = tmp.path().join("graph");
    let app = AppState::new(AppOptions {
        dirs: Dirs::under(&tmp.path().join("device")),
        emit: Arc::new(|_, _| {}),
        today: Arc::new(|| NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()),
        watch: false,
    });
    let call = |name: &str, args: Value| {
        dispatch(&app, name, args).unwrap_or_else(|e| panic!("{name}: {e}"))
    };

    let page = "- TODO NCLIVE call Anna\n  deadline:: 2026-10-21\n- TODO NCLIVE pay rent\n  scheduled:: 2026-10-23\n  priority:: B\n- TODO NCLIVE water plants\n  deadline:: 2026-10-24\n";
    std::fs::create_dir_all(graph.join("pages")).unwrap();
    std::fs::write(graph.join("pages/live.md"), page).unwrap();
    call(
        "open_graph",
        json!({"path": graph.to_str().unwrap(), "create": true}),
    );
    let list = call(
        "calendar_connect",
        json!({"server": server, "user": user, "password": password}),
    );
    let cal = list["calendars"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["href"]
                .as_str()
                .unwrap()
                .trim_end_matches('/')
                .ends_with(&env("NC_CALENDAR"))
        })
        .expect("calendar not found")["href"]
        .as_str()
        .unwrap()
        .to_string();
    let dav = HttpCalDav::new(&server, &user, &password).unwrap();
    let _cleanup = Cleanup(
        HttpCalDav::new(&server, &user, &password).unwrap(),
        cal.clone(),
    );
    call(
        "calendar_save",
        json!({"enabled": true, "read": [], "tasks": cal, "import_tasks": false}),
    );

    let ours = || -> Vec<notatnik_caldav::ServerItem> {
        let names: Vec<String> = dav
            .list(&cal)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect();
        dav.get(&cal, &names)
            .unwrap()
            .into_iter()
            .filter(|i| i.ics.contains("NCLIVE"))
            .collect()
    };

    // notes to calendar
    let r = call("calendar_sync", json!({}));
    println!("first sync: {r}");
    assert_eq!(r["created"], 3, "{r}");
    assert_eq!(ours().len(), 3);
    let r = call("calendar_sync", json!({}));
    println!("quiet sync: {r}");
    assert_eq!(
        (
            r["created"].clone(),
            r["updated"].clone(),
            r["pulled"].clone()
        ),
        (json!(0), json!(0), json!(0))
    );

    // another client changes things
    let edit = |summary: &str, f: &dyn Fn(String) -> String| -> String {
        let item = ours()
            .into_iter()
            .find(|i| i.ics.contains(&format!("SUMMARY:{summary}")))
            .unwrap();
        dav.put(&cal, &item.name, &f(item.ics), &Cond::IfMatch(item.etag))
            .unwrap();
        item.name
    };
    edit("NCLIVE call Anna", &|t| {
        t.replace("NEEDS-ACTION", "COMPLETED")
    });
    edit("NCLIVE pay rent", &|t| {
        t.replace("DTSTART;VALUE=DATE:20261023", "DTSTART;VALUE=DATE:20261030")
            .replace("SUMMARY:NCLIVE pay rent", "SUMMARY:NCLIVE pay the rent")
    });
    let gone = edit("NCLIVE water plants", &|t| t);
    dav.delete(&cal, &gone, None).unwrap();

    let r = call("calendar_sync", json!({}));
    println!("pull sync: {r}");
    assert_eq!(r["pulled"], 2, "{r}");
    assert_eq!(r["removed_in_calendar"], 1, "{r}");
    let now = std::fs::read_to_string(graph.join("pages/live.md")).unwrap();
    println!("{now}");
    assert!(now.contains("- DONE NCLIVE call Anna"), "{now}");
    assert!(now.contains("- TODO NCLIVE pay the rent"), "{now}");
    assert!(now.contains("scheduled:: 2026-10-30"), "{now}");
    assert!(
        now.contains("deadline:: 2026-10-24"),
        "the block stays: {now}"
    );

    // nothing bounces back
    let r = call("calendar_sync", json!({}));
    println!("settled sync: {r}");
    assert_eq!(
        (
            r["created"].clone(),
            r["updated"].clone(),
            r["pulled"].clone()
        ),
        (json!(0), json!(0), json!(0))
    );
    println!("log: {}", call("calendar_log", json!({})));
}
