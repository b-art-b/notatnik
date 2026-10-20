//! Full sync against a real Nextcloud (CA-3, CA-4, CA-5, CA-8, CA-9). Ignored by default:
//!
//! `NC_SERVER=http://localhost:8080 NC_USER=me NC_PASSWORD=... NC_CALENDAR=my-todos \
//!  cargo test -p notatnik-app --test nextcloud -- --ignored --nocapture --test-threads=1`
//!
//! One test at a time: the tests that make their own calendar can meet a Nextcloud that has not listed a calendar
//! yet when several are made at once.
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

/// An app with its own graph and a calendar made for the test (`tasks` or `events`); the items are removed at the
/// end, the empty calendar stays.
struct Env {
    app: AppState,
    _tmp: tempfile::TempDir,
    graph: std::path::PathBuf,
    dav: HttpCalDav,
    cal: String,
    _cleanup: Cleanup,
}

impl Env {
    fn new(kind: &str, page: Option<&str>) -> Env {
        let (server, user, password) = (env("NC_SERVER"), env("NC_USER"), env("NC_PASSWORD"));
        let dav = HttpCalDav::new(&server, &user, &password).unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let comp = if kind == "tasks" { "VTODO" } else { "VEVENT" };
        let cal = dav
            .make_calendar(&format!("nclive-{kind}-{stamp}"), &[comp])
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let graph = tmp.path().join("graph");
        let app = AppState::new(AppOptions {
            dirs: Dirs::under(&tmp.path().join("device")),
            emit: Arc::new(|_, _| {}),
            today: Arc::new(|| NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()),
            watch: false,
        });
        if let Some(page) = page {
            std::fs::create_dir_all(graph.join("pages")).unwrap();
            std::fs::write(graph.join("pages/live.md"), page).unwrap();
        }
        let env = Env {
            app,
            _tmp: tmp,
            graph,
            dav,
            _cleanup: Cleanup(
                HttpCalDav::new(&server, &user, &password).unwrap(),
                cal.clone(),
            ),
            cal,
        };
        env.call(
            "open_graph",
            json!({"path": env.graph.to_str().unwrap(), "create": true}),
        );
        env.call(
            "calendar_connect",
            json!({"server": server, "user": user, "password": password}),
        );
        env
    }

    fn call(&self, name: &str, args: Value) -> Value {
        dispatch(&self.app, name, args).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    fn save(&self, tasks: bool, read: bool, import: bool) {
        self.call(
            "calendar_save",
            json!({"enabled": true, "read": if read { vec![self.cal.clone()] } else { vec![] },
                   "tasks": if tasks { Some(self.cal.clone()) } else { None }, "import_tasks": import}),
        );
    }

    fn page(&self) -> String {
        std::fs::read_to_string(self.graph.join("pages/live.md")).unwrap()
    }

    fn items(&self) -> Vec<notatnik_caldav::ServerItem> {
        let names: Vec<String> = self
            .dav
            .list(&self.cal)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect();
        self.dav.get(&self.cal, &names).unwrap()
    }

    fn item(&self, summary: &str) -> Option<notatnik_caldav::ServerItem> {
        self.items()
            .into_iter()
            .find(|i| i.ics.contains(&format!("SUMMARY:{summary}")))
    }

    /// Changes an item on the server like another client would.
    fn edit(&self, summary: &str, f: &dyn Fn(String) -> String) -> String {
        let item = self
            .item(summary)
            .unwrap_or_else(|| panic!("no item {summary}"));
        self.dav
            .put(
                &self.cal,
                &item.name,
                &f(item.ics),
                &Cond::IfMatch(item.etag),
            )
            .unwrap();
        item.name
    }
}

/// The item as another client left it, with a last-modified time.
fn stamped(when: &str, ics: String) -> String {
    ics.replace(
        "STATUS:NEEDS-ACTION",
        &format!("STATUS:NEEDS-ACTION\r\nLAST-MODIFIED:{when}"),
    )
}

fn counts(r: &Value) -> (Value, Value, Value) {
    (
        r["created"].clone(),
        r["updated"].clone(),
        r["pulled"].clone(),
    )
}

/// Tasks made in other apps are copied into the inbox and the original is removed (CA-5; P4.10).
#[test]
#[ignore = "needs a real server, see the header"]
fn import_of_tasks_made_in_other_apps() {
    let e = Env::new("tasks", None);
    let theirs = |uid: &str, extra: &str| {
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//other//EN\r\nBEGIN:VTODO\r\nUID:{uid}\r\nDTSTAMP:20261018T080000Z\r\nSUMMARY:NCLIVE milk {uid}\r\nSTATUS:NEEDS-ACTION\r\n{extra}END:VTODO\r\nEND:VCALENDAR\r\n"
        );
        e.dav
            .put(&e.cal, &format!("F{uid}-random.ics"), &ics, &Cond::Create)
            .unwrap();
    };
    theirs(
        "aaa-1",
        "DUE;VALUE=DATE:20261025\r\nPRIORITY:1\r\nDESCRIPTION:two litres\r\n",
    );
    theirs("bbb-2", "");

    e.save(true, false, false);
    let r = e.call("calendar_sync", json!({}));
    println!("import off: {r}");
    assert_eq!(r["imported"], 0, "{r}");
    assert_eq!(e.items().len(), 2, "nothing is touched while import is off");

    e.save(true, false, true);
    let r = e.call("calendar_sync", json!({}));
    println!("import on: {r}");
    assert_eq!(r["imported"], 2, "{r}");
    let inbox = std::fs::read_to_string(e.graph.join("pages/inbox.md")).unwrap();
    println!("{inbox}");
    assert!(inbox.contains("TODO NCLIVE milk aaa-1 ^"), "{inbox}");
    assert!(
        inbox.contains("deadline:: 2026-10-25") && inbox.contains("priority:: A"),
        "{inbox}"
    );
    assert!(inbox.contains("two litres"), "{inbox}");
    assert!(inbox.contains("TODO NCLIVE milk bbb-2 ^"), "{inbox}");
    let names: Vec<String> = e.items().into_iter().map(|i| i.name).collect();
    assert!(
        !names.contains(&"Faaa-1-random.ics".to_string()),
        "dated original removed: {names:?}"
    );
    assert!(
        names.contains(&"Fbbb-2-random.ics".to_string()),
        "undated original stays: {names:?}"
    );
    assert_eq!(
        names.iter().filter(|n| n.starts_with("notatnik-")).count(),
        1,
        "{names:?}"
    );

    let r = e.call("calendar_sync", json!({}));
    assert_eq!(r["imported"], 0, "{r}");
    assert_eq!(
        std::fs::read_to_string(e.graph.join("pages/inbox.md")).unwrap(),
        inbox
    );
}

/// Events are read with repeats, exceptions, all-day and other time zones, and never written into the notes (CA-3).
#[test]
#[ignore = "needs a real server, see the header"]
fn events_are_read_and_not_written_into_the_notes() {
    let e = Env::new("events", Some("- a note\n"));
    let ev = |uid: &str, body: &str| {
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//other//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261018T080000Z\r\n{body}END:VEVENT\r\nEND:VCALENDAR\r\n"
        );
        e.dav
            .put(&e.cal, &format!("{uid}.ics"), &ics, &Cond::Create)
            .unwrap();
    };
    ev(
        "dentist",
        "SUMMARY:NCLIVE Dentist\r\nDTSTART:20261020T093000\r\nDTEND:20261020T103000\r\nLOCATION:Main street\r\n",
    );
    ev(
        "standup",
        "SUMMARY:NCLIVE Standup\r\nDTSTART:20261019T090000\r\nDTEND:20261019T091500\r\nRRULE:FREQ=DAILY;COUNT=4\r\nEXDATE:20261021T090000\r\n",
    );
    ev(
        "holiday",
        "SUMMARY:NCLIVE Holiday\r\nDTSTART;VALUE=DATE:20261022\r\nDTEND;VALUE=DATE:20261025\r\n",
    );
    ev(
        "utc",
        "SUMMARY:NCLIVE Call\r\nDTSTART:20261020T120000Z\r\nDTEND:20261020T130000Z\r\n",
    );

    e.save(false, true, false);
    let before = e.page();
    let r = e.call("calendar_sync", json!({}));
    println!("events: {r}");
    assert!(r["events_changed"].as_u64().unwrap() >= 4, "{r}");
    assert_eq!(e.page(), before, "events are not written into the notes");
    assert!(!e.graph.join("pages/inbox.md").exists());

    let titles = |from: &str, to: &str| -> Vec<String> {
        e.call("calendar_events", json!({"from": from, "to": to}))
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                format!(
                    "{} {}",
                    v["date"].as_str().unwrap(),
                    v["title"].as_str().unwrap()
                )
            })
            .collect()
    };
    let all = titles("2026-10-19", "2026-10-26");
    println!("{all:#?}");
    let count = |t: &str| all.iter().filter(|x| x.ends_with(t)).count();
    assert_eq!(
        count("NCLIVE Standup"),
        3,
        "four days, one excluded: {all:?}"
    );
    assert!(
        !all.contains(&"2026-10-21 NCLIVE Standup".to_string()),
        "{all:?}"
    );
    assert_eq!(
        count("NCLIVE Holiday"),
        1,
        "one entry for the whole stay: {all:?}"
    );
    assert_eq!(count("NCLIVE Dentist"), 1);
    assert_eq!(count("NCLIVE Call"), 1);
    let day = e.call(
        "calendar_events",
        json!({"from": "2026-10-20", "to": "2026-10-20"}),
    );
    let dentist = day
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["title"] == "NCLIVE Dentist")
        .unwrap();
    assert_eq!(dentist["start"], "09:30");
    let stay = e.call(
        "calendar_events",
        json!({"from": "2026-10-23", "to": "2026-10-23"}),
    );
    let holiday = stay
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["title"] == "NCLIVE Holiday");
    assert_eq!(
        holiday.expect("shown on the middle day")["end_date"],
        "2026-10-24"
    );
    assert_eq!(dentist["location"], "Main street");

    // a changed event shows up at the next sync
    e.edit("NCLIVE Dentist", &|t| {
        t.replace("NCLIVE Dentist", "NCLIVE Dentist moved")
            .replace("20261020T093000", "20261020T143000")
            .replace("20261020T103000", "20261020T153000")
    });
    let r = e.call("calendar_sync", json!({}));
    assert!(r["events_changed"].as_u64().unwrap() >= 1, "{r}");
    let day = e.call(
        "calendar_events",
        json!({"from": "2026-10-20", "to": "2026-10-20"}),
    );
    let moved = day
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["title"] == "NCLIVE Dentist moved")
        .expect("moved event");
    assert_eq!(moved["start"], "14:30");
    assert_eq!(e.page(), before);
}

/// Conflicts: the later change wins in both directions and is logged; "use the other value" turns it around;
/// a change in the notes goes out after the first send (CA-5, CA-8; P4.12).
#[test]
#[ignore = "needs a real server, see the header"]
fn conflicts_are_decided_by_the_later_change_and_can_be_turned_around() {
    let page = "- TODO NCLIVE remote wins\n  deadline:: 2026-10-21\n- TODO NCLIVE notes win\n  deadline:: 2026-10-22\n- TODO NCLIVE notes only\n  deadline:: 2026-10-23\n  priority:: C\n";
    let e = Env::new("tasks", Some(page));
    e.save(true, false, false);
    let r = e.call("calendar_sync", json!({}));
    assert_eq!(r["created"], 3, "{r}");

    // a change in the notes goes out
    let hash = e.call("get_page", json!({"name": "live"}))["hash"].clone();
    e.call("apply_ops", json!({"page": "live", "base_hash": hash, "ops": [
        {"op": "set_property", "block": {"path": [0]}, "key": "deadline", "value": "2026-10-25"},
        {"op": "set_property", "block": {"path": [1]}, "key": "deadline", "value": "2026-10-26"},
        {"op": "set_property", "block": {"path": [2]}, "key": "priority", "value": "A"},
    ]}));
    // the calendar moves the first date later than the notes did, the second one earlier
    e.edit("NCLIVE remote wins", &|t| {
        stamped(
            "20991231T000000Z",
            t.replace("DUE;VALUE=DATE:20261021", "DUE;VALUE=DATE:20261030"),
        )
    });
    e.edit("NCLIVE notes win", &|t| {
        stamped(
            "20000101T000000Z",
            t.replace("DUE;VALUE=DATE:20261022", "DUE;VALUE=DATE:20261031"),
        )
    });
    let r = e.call("calendar_sync", json!({}));
    println!("conflict sync: {r}");
    let page = e.page();
    println!("{page}");
    assert!(
        page.contains("NCLIVE remote wins") && page.contains("deadline:: 2026-10-30"),
        "{page}"
    );
    assert!(
        page.contains("deadline:: 2026-10-26"),
        "the notes' later change stays: {page}"
    );
    let due = |s: &str| {
        e.item(s)
            .unwrap()
            .ics
            .lines()
            .find(|l| l.starts_with("DUE"))
            .unwrap()
            .trim()
            .to_string()
    };
    assert!(
        due("NCLIVE notes win").ends_with("20261026"),
        "the calendar got the notes' value: {}",
        due("NCLIVE notes win")
    );
    assert!(
        e.item("NCLIVE notes only")
            .unwrap()
            .ics
            .contains("PRIORITY:1"),
        "a change in the notes goes out"
    );

    let log = e.call("calendar_log", json!({}));
    let winners: Vec<String> = log
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["kind"] == "conflict")
        .map(|x| {
            format!(
                "{}:{}",
                x["title"].as_str().unwrap(),
                x["winner"].as_str().unwrap()
            )
        })
        .collect();
    println!("{winners:?}");
    assert!(
        winners.contains(&"NCLIVE remote wins:remote".to_string()),
        "{winners:?}"
    );
    assert!(
        winners.contains(&"NCLIVE notes win:local".to_string()),
        "{winners:?}"
    );

    // turn the first one around: the notes' value goes back to the calendar
    let uid = log
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["title"] == "NCLIVE remote wins" && x["kind"] == "conflict")
        .unwrap()["uid"]
        .clone();
    let r = e.call(
        "calendar_use_other_value",
        json!({"uid": uid, "field": "deadline"}),
    );
    println!("turned around: {r}");
    assert!(e.page().contains("deadline:: 2026-10-25"), "{}", e.page());
    assert!(
        due("NCLIVE remote wins").ends_with("20261025"),
        "{}",
        due("NCLIVE remote wins")
    );

    // and everything settles
    let r = e.call("calendar_sync", json!({}));
    assert_eq!(counts(&r), (json!(0), json!(0), json!(0)), "{r}");
}

/// Encrypting chosen paths on a real server (EN-11 to EN-17, E6): the server holds `.enc` files and no text of a
/// marked page, a second device opens it with the password, a locked key stops only the marked files, and what the
/// server's trash bin keeps of a file that was synced in plain before it was marked. Uses and removes the folder
/// `Notes/notatnik-enc-<process id>`.
#[test]
#[ignore = "needs a real server, see the header"]
fn encrypted_paths_on_a_real_nextcloud() {
    use notatnik_sync::{Remote, WebDavRemote};
    let (server, user, password) = (env("NC_SERVER"), env("NC_USER"), env("NC_PASSWORD"));
    let folder = format!("Notes/notatnik-enc-{}", std::process::id());
    let marker = "SECRETMARKER-4711";
    let device = |name: &str| {
        let tmp = tempfile::tempdir().unwrap();
        let app = AppState::new(AppOptions {
            dirs: Dirs::under(&tmp.path().join(name)),
            emit: Arc::new(|_, _| {}),
            today: Arc::new(|| NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()),
            watch: false,
        });
        (tmp, app)
    };
    let (_t1, laptop) = device("laptop");
    let (_t2, phone) = device("phone");
    let call = |app: &AppState, name: &str, args: Value| {
        dispatch(app, name, args).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let seed = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(seed.path().join("pages")).unwrap();
    // a graph made on the laptop
    let args = |attach: bool| json!({"server": server, "user": user, "password": password, "folder": folder, "attach": attach});
    let info = call(&laptop, "create_remote_graph", args(false));
    let root = std::path::PathBuf::from(info["root"].as_str().unwrap());
    std::fs::create_dir_all(root.join("pages/private")).unwrap();
    std::fs::write(
        root.join("pages/private/diary.md"),
        format!("- {marker} one\n"),
    )
    .unwrap();
    std::fs::write(root.join("pages/open.md"), "- open text\n").unwrap();
    let url = notatnik_sync::folder_url(&server, &user, &folder).unwrap();
    let remote = WebDavRemote::new(&url, &user, &password).unwrap();
    let cleanup = Folder(url.clone(), user.clone(), password.clone());

    // synced in plain first, as a person who marks a page later would have it
    call(&laptop, "sync_now", json!({"allow_mass_delete": false}));
    assert!(
        String::from_utf8_lossy(&remote.get("pages/private/diary.md").unwrap().0).contains(marker)
    );

    call(
        &laptop,
        "encryption_setup",
        json!({"password": "correct horse"}),
    );
    call(&laptop, "encryption_confirm", json!({}));
    // this test looks at the visible `.enc` names; hidden names have their own test below
    call(&laptop, "encryption_names", json!({"hidden": false}));
    let p = call(
        &laptop,
        "encryption_preview",
        json!({"paths": ["pages/private/"], "encrypted": true}),
    );
    println!("preview: {p}");
    call(
        &laptop,
        "encryption_mark",
        json!({"paths": ["pages/private/"], "encrypted": true}),
    );
    let st = call(&laptop, "sync_now", json!({"allow_mass_delete": false}));
    assert_eq!(st["state"], "idle", "{st}");
    let sealed = remote.get("pages/private/diary.md.enc").unwrap().0;
    assert!(sealed.starts_with(b"NTNKFILE"));
    assert!(!sealed.windows(marker.len()).any(|w| w == marker.as_bytes()));
    assert!(
        remote.get("pages/private/diary.md").is_err(),
        "the plain copy is gone"
    );
    assert!(String::from_utf8_lossy(&remote.get("pages/open.md").unwrap().0).contains("open text"));
    assert!(remote.get(".notatnik/vault.json").is_ok());

    // what Nextcloud's trash bin kept of the plain copy
    let trash = trash_entries(&server, &user, &password, "diary.md");
    println!("trash bin entries for diary.md: {trash:?}");
    for t in &trash {
        let text = trash_text(&server, &user, &password, t);
        println!("  {t}: plain text kept = {}", text.contains(marker));
    }

    // a second device: vault and plain page first, the encrypted page after the password
    call(&phone, "create_remote_graph", args(true));
    let st = call(&phone, "encryption_status", json!({}));
    assert_eq!(
        (st["configured"].clone(), st["unlocked"].clone()),
        (json!(true), json!(false))
    );
    let proot = std::path::PathBuf::from(
        call(&phone, "graph_info", json!({}))["root"]
            .as_str()
            .unwrap(),
    );
    assert!(proot.join("pages/open.md").exists());
    assert!(!proot.join("pages/private/diary.md").exists());
    call(
        &phone,
        "encryption_unlock",
        json!({"secret": "correct horse"}),
    );
    call(&phone, "sync_now", json!({"allow_mass_delete": false}));
    assert!(
        std::fs::read_to_string(proot.join("pages/private/diary.md"))
            .unwrap()
            .contains(marker)
    );

    // an edit on the phone, read by the laptop; then a locked laptop leaves the encrypted file alone
    std::fs::write(
        proot.join("pages/private/diary.md"),
        format!("- {marker} one\n- phone\n"),
    )
    .unwrap();
    call(&phone, "sync_now", json!({"allow_mass_delete": false}));
    call(&laptop, "encryption_lock", json!({}));
    let st = call(&laptop, "sync_now", json!({"allow_mass_delete": false}));
    assert_eq!(st["left_alone"], 1, "{st}");
    assert!(
        !std::fs::read_to_string(root.join("pages/private/diary.md"))
            .unwrap()
            .contains("phone")
    );
    call(
        &laptop,
        "encryption_unlock",
        json!({"secret": "correct horse"}),
    );
    call(&laptop, "sync_now", json!({"allow_mass_delete": false}));
    assert!(
        std::fs::read_to_string(root.join("pages/private/diary.md"))
            .unwrap()
            .contains("phone")
    );

    // unmarked: stored as it is again
    call(
        &laptop,
        "encryption_mark",
        json!({"paths": ["pages/private/"], "encrypted": false}),
    );
    call(&laptop, "sync_now", json!({"allow_mass_delete": false}));
    assert!(remote.get("pages/private/diary.md.enc").is_err());
    assert!(
        String::from_utf8_lossy(&remote.get("pages/private/diary.md").unwrap().0).contains("phone")
    );
    drop(cleanup);
}

/// Every file below the synced folder, by its path on the server.
fn all_files(remote: &notatnik_sync::WebDavRemote, dir: &str) -> Vec<String> {
    use notatnik_sync::Remote;
    let mut out = Vec::new();
    if let Some(l) = remote.list(dir).unwrap() {
        for e in l.entries {
            let p = if dir.is_empty() {
                e.name.clone()
            } else {
                format!("{dir}/{}", e.name)
            };
            if e.dir {
                out.extend(all_files(remote, &p));
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// EN-25, EN-26 against a real server: no name of an encrypted file or folder on it, a second device learns the
/// paths from the objects, a rename arrives, switching the setting and re-encrypting leave no old object.
#[test]
#[ignore = "needs a real server, see the header"]
fn hidden_names_and_re_encrypting_on_a_real_nextcloud() {
    use notatnik_sync::{Remote, WebDavRemote};
    let (server, user, password) = (env("NC_SERVER"), env("NC_USER"), env("NC_PASSWORD"));
    let folder = format!("Notes/notatnik-hid-{}", std::process::id());
    let marker = "SECRETMARKER-4711";
    let device = |name: &str| {
        let tmp = tempfile::tempdir().unwrap();
        let app = AppState::new(AppOptions {
            dirs: Dirs::under(&tmp.path().join(name)),
            emit: Arc::new(|_, _| {}),
            today: Arc::new(|| NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()),
            watch: false,
        });
        (tmp, app)
    };
    let (_t1, laptop) = device("laptop");
    let (_t2, phone) = device("phone");
    let call = |app: &AppState, name: &str, args: Value| {
        dispatch(app, name, args).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let sync = |app: &AppState| {
        let st = call(app, "sync_now", json!({"allow_mass_delete": false}));
        assert_eq!(st["state"], "idle", "{st}");
        st
    };
    let args = |attach: bool| json!({"server": server, "user": user, "password": password, "folder": folder, "attach": attach});
    let info = call(&laptop, "create_remote_graph", args(false));
    let root = std::path::PathBuf::from(info["root"].as_str().unwrap());
    std::fs::create_dir_all(root.join("pages/private/deep")).unwrap();
    std::fs::write(
        root.join("pages/private/diary.md"),
        format!("- {marker} one\n"),
    )
    .unwrap();
    std::fs::write(
        root.join("pages/private/deep/Zażółć.md"),
        format!("- {marker} two\n"),
    )
    .unwrap();
    std::fs::write(root.join("pages/open.md"), "- open text\n").unwrap();
    let url = notatnik_sync::folder_url(&server, &user, &folder).unwrap();
    let remote = WebDavRemote::new(&url, &user, &password).unwrap();
    let cleanup = Folder(url.clone(), user.clone(), password.clone());

    call(
        &laptop,
        "encryption_setup",
        json!({"password": "correct horse"}),
    );
    call(&laptop, "encryption_confirm", json!({}));
    let st = call(
        &laptop,
        "encryption_mark",
        json!({"paths": ["pages/private/"], "encrypted": true}),
    );
    assert_eq!(st["names"], "hidden", "{st}");
    sync(&laptop);
    let files = all_files(&remote, "");
    println!("on the server: {files:?}");
    for f in &files {
        assert!(
            !f.contains("private")
                && !f.contains("diary")
                && !f.contains("deep")
                && !f.contains("Zażółć"),
            "{f}"
        );
    }
    let objects = |files: &[String]| -> Vec<String> {
        files
            .iter()
            .filter(|f| f.starts_with(".notatnik-enc/"))
            .cloned()
            .collect()
    };
    let first = objects(&files);
    assert_eq!(first.len(), 2, "{files:?}");
    for o in &first {
        let bytes = remote.get(o).unwrap().0;
        assert!(!bytes.windows(marker.len()).any(|w| w == marker.as_bytes()));
    }
    assert!(files.contains(&"pages/open.md".to_string()));
    // the folder of a marked path was never made
    assert!(
        !remote
            .list("pages")
            .unwrap()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.name == "private")
    );
    let v = call(&laptop, "sync_verify", json!({}));
    assert_eq!(v["clean"], true, "{v}");

    // a second device unlocks with the password and learns every path from the objects
    call(&phone, "create_remote_graph", args(true));
    let proot = std::path::PathBuf::from(
        call(&phone, "graph_info", json!({}))["root"]
            .as_str()
            .unwrap(),
    );
    let st = sync(&phone);
    assert_eq!(st["left_alone"], 2, "{st}");
    call(
        &phone,
        "encryption_unlock",
        json!({"secret": "correct horse"}),
    );
    sync(&phone);
    assert!(
        std::fs::read_to_string(proot.join("pages/private/deep/Zażółć.md"))
            .unwrap()
            .contains(marker)
    );

    // a rename on the phone arrives on the laptop; the objects are replaced, not added to
    std::fs::rename(
        proot.join("pages/private/diary.md"),
        proot.join("pages/private/journal.md"),
    )
    .unwrap();
    sync(&phone);
    sync(&laptop);
    assert!(
        root.join("pages/private/journal.md").exists()
            && !root.join("pages/private/diary.md").exists()
    );
    let second = objects(&all_files(&remote, ""));
    assert_eq!(second.len(), 2, "{second:?}");
    assert_ne!(second, first);

    // visible names and back: nothing of the other kind is left
    call(&laptop, "encryption_names", json!({"hidden": false}));
    sync(&laptop);
    let files = all_files(&remote, "");
    assert!(objects(&files).is_empty(), "{files:?}");
    assert!(
        files.contains(&"pages/private/journal.md.enc".to_string()),
        "{files:?}"
    );
    call(&laptop, "encryption_names", json!({"hidden": true}));
    sync(&laptop);
    let before = objects(&all_files(&remote, ""));
    assert_eq!(before.len(), 2);
    assert!(
        !all_files(&remote, "")
            .iter()
            .any(|f| f.ends_with(".md.enc"))
    );

    // re-encrypting with a new key: new names, no old object, the other device asks for the new password
    call(
        &laptop,
        "encryption_rotate",
        json!({"password": "brand new pass"}),
    );
    call(&laptop, "encryption_confirm", json!({}));
    sync(&laptop);
    let after = objects(&all_files(&remote, ""));
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(after.iter().all(|o| !before.contains(o)), "{after:?}");
    let v = call(&laptop, "sync_verify", json!({}));
    assert_eq!(v["clean"], true, "{v}");
    // the phone still has the old key: the new objects are reported, and nothing of its own is deleted
    let st = call(&phone, "sync_now", json!({"allow_mass_delete": false}));
    assert_eq!(st["error_code"], "files", "{st}");
    assert_eq!(st["deleted"], 0, "{st}");
    assert!(proot.join("pages/private/journal.md").exists());
    // the phone has the new vault now: the old password does not open it
    call(&phone, "encryption_lock", json!({}));
    assert!(
        dispatch(
            &phone,
            "encryption_unlock",
            json!({"secret": "correct horse"})
        )
        .is_err()
    );
    call(
        &phone,
        "encryption_unlock",
        json!({"secret": "brand new pass"}),
    );
    sync(&phone);
    assert!(
        std::fs::read_to_string(proot.join("pages/private/journal.md"))
            .unwrap()
            .contains(marker)
    );
    drop(cleanup);
}

struct Folder(String, String, String);

impl Drop for Folder {
    fn drop(&mut self) {
        if let Ok(r) = notatnik_sync::WebDavRemote::new(&self.0, &self.1, &self.2) {
            use notatnik_sync::Remote;
            let _ = r.delete("", None);
        }
    }
}

fn dav_request(
    server: &str,
    user: &str,
    password: &str,
    method: &str,
    path: &str,
    body: &str,
) -> String {
    let auth = notatnik_sync::basic_auth(user, password);
    let url = format!("{}/remote.php/dav/{path}", server.trim_end_matches('/'));
    let r = ureq::request(method, &url)
        .set("Authorization", &auth)
        .set("Depth", "1")
        .send_string(body);
    match r {
        Ok(r) => r.into_string().unwrap_or_default(),
        Err(ureq::Error::Status(_, r)) => r.into_string().unwrap_or_default(),
        Err(e) => panic!("{method} {url}: {e}"),
    }
}

/// The hrefs in the user's trash bin whose name starts like `name`.
fn trash_entries(server: &str, user: &str, password: &str, name: &str) -> Vec<String> {
    let xml = dav_request(
        server,
        user,
        password,
        "PROPFIND",
        &format!("trashbin/{user}/trash"),
        r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#,
    );
    xml.split("<d:href>")
        .skip(1)
        .filter_map(|s| s.split("</d:href>").next())
        .filter(|h| h.contains(&format!("/{name}.d")))
        .map(str::to_string)
        .collect()
}

fn trash_text(server: &str, user: &str, password: &str, href: &str) -> String {
    let auth = notatnik_sync::basic_auth(user, password);
    let url = format!("{}{href}", server.trim_end_matches('/'));
    ureq::get(&url)
        .set("Authorization", &auth)
        .call()
        .ok()
        .and_then(|r| r.into_string().ok())
        .unwrap_or_default()
}
