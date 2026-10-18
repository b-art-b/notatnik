//! An in-memory CalDAV server for tests: ETags, sync tokens, conditional writes and a failure that can be
//! switched on. It answers the way a real server does for the cases the sync depends on.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::{CalDavClient, Calendar, Cond, Error, ItemRef, Result, ServerItem, SyncResult};

#[derive(Default)]
struct Cal {
    meta: Option<Calendar>,
    items: BTreeMap<String, (String, String)>,
    rev: u64,
    /// (revision, name, deleted)
    log: Vec<(u64, String, bool)>,
    /// Tokens older than this are no longer known.
    valid_from: u64,
}

#[derive(Default)]
struct State {
    cals: Vec<Cal>,
    etag: u64,
    requests: usize,
    fail: Option<&'static str>,
}

/// The server. Share it between "devices" by reference.
#[derive(Default)]
pub struct FakeServer {
    state: Mutex<State>,
}

impl FakeServer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a calendar and returns its `href`.
    pub fn add_calendar(&self, name: &str, components: &[&str]) -> String {
        let mut s = self.state.lock().unwrap();
        let href = format!("/calendars/me/{}/", name.to_lowercase().replace(' ', "-"));
        s.cals.push(Cal {
            meta: Some(Calendar {
                href: href.clone(),
                name: name.into(),
                components: components.iter().map(|c| (*c).into()).collect(),
            }),
            ..Cal::default()
        });
        href
    }

    /// The next call fails: `"offline"` or `"server"`.
    pub fn fail_next(&self, kind: &'static str) {
        self.state.lock().unwrap().fail = Some(kind);
    }

    /// The server forgets old sync tokens (as after a restore or a long time).
    pub fn forget_sync_tokens(&self, calendar: &str) {
        let mut s = self.state.lock().unwrap();
        if let Some(c) = find(&mut s, calendar) {
            c.rev += 1;
            c.valid_from = c.rev;
        }
    }

    /// How many calls the server has answered.
    pub fn requests(&self) -> usize {
        self.state.lock().unwrap().requests
    }

    /// The text of an item, if it exists (does not count as a request).
    pub fn text(&self, calendar: &str, name: &str) -> Option<String> {
        let mut s = self.state.lock().unwrap();
        find(&mut s, calendar)?
            .items
            .get(name)
            .map(|(_, t)| t.clone())
    }

    /// Item names of a calendar (does not count as a request).
    pub fn names(&self, calendar: &str) -> Vec<String> {
        let mut s = self.state.lock().unwrap();
        find(&mut s, calendar)
            .map(|c| c.items.keys().cloned().collect())
            .unwrap_or_default()
    }
}

fn find<'a>(s: &'a mut State, href: &str) -> Option<&'a mut Cal> {
    s.cals
        .iter_mut()
        .find(|c| c.meta.as_ref().is_some_and(|m| m.href == href))
}

fn begin(s: &mut State) -> Result<()> {
    s.requests += 1;
    match s.fail.take() {
        Some("offline") => Err(Error::Offline("no network (test)".into())),
        Some(_) => Err(Error::Server("500 (test)".into())),
        None => Ok(()),
    }
}

fn missing<T>(href: &str) -> Result<T> {
    Err(Error::NotFound(href.into()))
}

impl CalDavClient for FakeServer {
    fn calendars(&self) -> Result<Vec<Calendar>> {
        let mut s = self.state.lock().unwrap();
        begin(&mut s)?;
        Ok(s.cals.iter().filter_map(|c| c.meta.clone()).collect())
    }

    fn list(&self, calendar: &str) -> Result<Vec<ItemRef>> {
        let mut s = self.state.lock().unwrap();
        begin(&mut s)?;
        let Some(c) = find(&mut s, calendar) else {
            return missing(calendar);
        };
        Ok(c.items
            .iter()
            .map(|(n, (e, _))| ItemRef {
                name: n.clone(),
                etag: e.clone(),
            })
            .collect())
    }

    fn get(&self, calendar: &str, names: &[String]) -> Result<Vec<ServerItem>> {
        let mut s = self.state.lock().unwrap();
        begin(&mut s)?;
        let Some(c) = find(&mut s, calendar) else {
            return missing(calendar);
        };
        Ok(names
            .iter()
            .filter_map(|n| {
                c.items.get(n).map(|(e, t)| ServerItem {
                    name: n.clone(),
                    etag: e.clone(),
                    ics: t.clone(),
                })
            })
            .collect())
    }

    fn put(&self, calendar: &str, name: &str, ics: &str, cond: &Cond) -> Result<String> {
        let mut s = self.state.lock().unwrap();
        begin(&mut s)?;
        s.etag += 1;
        let etag = format!("\"{}\"", s.etag);
        let Some(c) = find(&mut s, calendar) else {
            return missing(calendar);
        };
        match (cond, c.items.get(name)) {
            (Cond::Create, Some(_)) => return Err(Error::Changed(format!("{name} exists"))),
            (Cond::IfMatch(want), Some((have, _))) if want != have => {
                return Err(Error::Changed(format!("{name} changed")));
            }
            (Cond::IfMatch(_), None) => return Err(Error::Changed(format!("{name} is gone"))),
            _ => {}
        }
        c.rev += 1;
        c.items.insert(name.into(), (etag.clone(), ics.into()));
        c.log.push((c.rev, name.into(), false));
        Ok(etag)
    }

    fn delete(&self, calendar: &str, name: &str, etag: Option<&str>) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        begin(&mut s)?;
        let Some(c) = find(&mut s, calendar) else {
            return missing(calendar);
        };
        match (c.items.get(name), etag) {
            (None, _) => return missing(name),
            (Some((have, _)), Some(want)) if have != want => {
                return Err(Error::Changed(format!("{name} changed")));
            }
            _ => {}
        }
        c.items.remove(name);
        c.rev += 1;
        c.log.push((c.rev, name.into(), true));
        Ok(())
    }

    fn sync(&self, calendar: &str, token: Option<&str>) -> Result<SyncResult> {
        let mut s = self.state.lock().unwrap();
        begin(&mut s)?;
        let Some(c) = find(&mut s, calendar) else {
            return missing(calendar);
        };
        let since = match token {
            None => 0,
            Some(t) => match t.strip_prefix("rev-").and_then(|n| n.parse::<u64>().ok()) {
                Some(n) if n >= c.valid_from && n <= c.rev => n,
                _ => return Err(Error::Changed("unknown sync token".into())),
            },
        };
        let mut changed = Vec::new();
        let mut deleted = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (rev, name, gone) in c.log.iter().rev() {
            if *rev <= since || !seen.insert(name.clone()) {
                continue;
            }
            match c.items.get(name) {
                Some((etag, _)) if !gone => changed.push(ItemRef {
                    name: name.clone(),
                    etag: etag.clone(),
                }),
                None if *gone && since > 0 => deleted.push(name.clone()),
                _ => {}
            }
        }
        changed.sort_by(|a, b| a.name.cmp(&b.name));
        deleted.sort();
        Ok(SyncResult {
            token: format!("rev-{}", c.rev),
            changed,
            deleted,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tasks() -> (FakeServer, String) {
        let s = FakeServer::new();
        let h = s.add_calendar("Tasks", &["VTODO"]);
        (s, h)
    }

    #[test]
    fn conditional_writes_refuse_a_lost_race() {
        let (s, h) = tasks();
        let e1 = s.put(&h, "a.ics", "one", &Cond::Create).unwrap();
        assert!(matches!(
            s.put(&h, "a.ics", "again", &Cond::Create),
            Err(Error::Changed(_))
        ));
        let e2 = s
            .put(&h, "a.ics", "two", &Cond::IfMatch(e1.clone()))
            .unwrap();
        assert_ne!(e1, e2);
        assert!(matches!(
            s.put(&h, "a.ics", "late", &Cond::IfMatch(e1)),
            Err(Error::Changed(_))
        ));
        assert_eq!(s.text(&h, "a.ics").as_deref(), Some("two"));
    }

    #[test]
    fn sync_reports_what_changed_since_the_token() {
        let (s, h) = tasks();
        s.put(&h, "a.ics", "1", &Cond::Create).unwrap();
        s.put(&h, "b.ics", "1", &Cond::Create).unwrap();
        let first = s.sync(&h, None).unwrap();
        assert_eq!(first.changed.len(), 2);

        let quiet = s.sync(&h, Some(&first.token)).unwrap();
        assert!(quiet.changed.is_empty() && quiet.deleted.is_empty());
        assert_eq!(quiet.token, first.token);

        s.put(&h, "c.ics", "1", &Cond::Create).unwrap();
        s.delete(&h, "a.ics", None).unwrap();
        let next = s.sync(&h, Some(&first.token)).unwrap();
        assert_eq!(
            next.changed
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>(),
            ["c.ics"]
        );
        assert_eq!(next.deleted, ["a.ics"]);
    }

    #[test]
    fn a_forgotten_token_is_an_error_and_failures_can_be_injected() {
        let (s, h) = tasks();
        s.put(&h, "a.ics", "1", &Cond::Create).unwrap();
        let t = s.sync(&h, None).unwrap().token;
        s.forget_sync_tokens(&h);
        assert!(matches!(s.sync(&h, Some(&t)), Err(Error::Changed(_))));
        assert!(s.sync(&h, None).is_ok());

        s.fail_next("offline");
        assert!(matches!(s.list(&h), Err(Error::Offline(_))));
        assert!(s.list(&h).is_ok()); // only the next call fails
        assert!(matches!(s.list("/nope/"), Err(Error::NotFound(_))));
    }

    #[test]
    fn calendars_tell_what_they_take() {
        let s = FakeServer::new();
        s.add_calendar("Personal", &["VEVENT", "VTODO"]);
        s.add_calendar("Birthdays", &["VEVENT"]);
        let cals = s.calendars().unwrap();
        assert!(cals[0].takes("vtodo") && !cals[1].takes("VTODO"));
    }
}
