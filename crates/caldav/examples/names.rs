//! Lists the items of one calendar: file name, UID and summary. `NC_SERVER NC_USER NC_PASSWORD NC_CALENDAR`.
use notatnik_caldav::{CalDavClient, HttpCalDav};

fn main() {
    let get = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let c = HttpCalDav::new(&get("NC_SERVER"), &get("NC_USER"), &get("NC_PASSWORD")).unwrap();
    let want = get("NC_CALENDAR");
    let cal = c
        .calendars()
        .unwrap()
        .into_iter()
        .find(|x| x.href.trim_end_matches('/').ends_with(&want))
        .expect("calendar");
    let names: Vec<String> = c
        .list(&cal.href)
        .unwrap()
        .into_iter()
        .map(|i| i.name)
        .collect();
    for i in c.get(&cal.href, &names).unwrap() {
        let f = |k: &str| {
            i.ics
                .lines()
                .find_map(|l| l.trim_end().strip_prefix(k))
                .unwrap_or("")
                .to_string()
        };
        println!(
            "{} | UID {} | {} | {}",
            i.name,
            f("UID:"),
            f("SUMMARY:"),
            i.ics
                .lines()
                .filter(|l| l.starts_with("DUE") || l.starts_with("DTSTART"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}
