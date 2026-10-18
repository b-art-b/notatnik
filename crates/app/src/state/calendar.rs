//! Calendar sync settings (CA-1, CA-2): which login and which calendars. The login is the one of the graph's
//! sync unless the graph is given one of its own. The sync engine itself is in `notatnik-caldav`.

use notatnik_caldav::ical::{Status, Todo, When};
use notatnik_caldav::map::{Context, todo_from_block};
use notatnik_caldav::map::{block_id_of, repeat_from_rrule};
use notatnik_caldav::pull::{self, Change, Local, Side};
use notatnik_caldav::push::{OnDelete, Options, PushState, push};
use notatnik_caldav::{CalDavClient, HttpCalDav};
use notatnik_core::{Block, BlockOp, BlockRef, Moment, TaskState};
use notatnik_graph::{CalendarAccount, CalendarConfig};

use super::syncing::{api_err, secret_key};
use super::*;

fn password_key(graph: &str) -> String {
    format!("calendar {graph}")
}

/// What it takes to talk to the calendar server.
struct Account {
    server: String,
    user: String,
    password: String,
    own: bool,
}

impl Account {
    fn view(&self) -> CalendarAccountView {
        CalendarAccountView {
            server: self.server.clone(),
            user: self.user.clone(),
            own: self.own,
        }
    }
}

impl AppState {
    fn calendar_settings(&self, graph: &str) -> CalendarConfig {
        notatnik_graph::DeviceSettings::load(&self.dirs)
            .ok()
            .and_then(|d| d.calendar_of(graph))
            .unwrap_or_default()
    }

    fn store_calendar_settings(&self, graph: &str, cfg: CalendarConfig) -> ApiResult<()> {
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        dev.set_calendar(graph, Some(cfg));
        dev.save(&self.dirs)?;
        Ok(())
    }

    /// The login for the calendar server of a graph: its own, else the one of its sync.
    fn calendar_account(&self, graph: &str, cfg: &CalendarConfig) -> ApiResult<Option<Account>> {
        let no_password = || {
            ApiError::new(
                "auth",
                "no password is stored for the calendar login; connect again and enter it",
            )
        };
        if let Some(a) = &cfg.account {
            let password = self
                .secrets
                .get(&password_key(graph))
                .ok_or_else(no_password)?;
            return Ok(Some(Account {
                server: a.server.clone(),
                user: a.user.clone(),
                password,
                own: true,
            }));
        }
        let Some(remote) = self.remote_of(graph) else {
            return Ok(None);
        };
        let password = self
            .secrets
            .get(&secret_key(&remote))
            .ok_or_else(no_password)?;
        Ok(Some(Account {
            server: remote.url,
            user: remote.user,
            password,
            own: false,
        }))
    }

    fn calendar_client_for(&self, a: &Account) -> ApiResult<Arc<dyn CalDavClient + Send + Sync>> {
        if let Some(c) = &self.calendar_client {
            return Ok(c.clone());
        }
        let client = HttpCalDav::new(&a.server, &a.user, &a.password).map_err(|e| api_err(&e))?;
        Ok(Arc::new(client))
    }

    fn calendar_list(client: &dyn CalDavClient) -> ApiResult<Vec<CalendarView>> {
        Ok(client
            .calendars()
            .map_err(|e| api_err(&e))?
            .into_iter()
            .map(|c| CalendarView {
                tasks: c.takes("VTODO"),
                events: c.takes("VEVENT"),
                href: c.href,
                name: c.name,
            })
            .collect())
    }

    fn calendar_status_of(&self, graph: &str) -> ApiResult<CalendarStatus> {
        let cfg = self.calendar_settings(graph);
        let account = match &cfg.account {
            Some(a) => Some(CalendarAccountView {
                server: a.server.clone(),
                user: a.user.clone(),
                own: true,
            }),
            None => self.remote_of(graph).map(|r| CalendarAccountView {
                server: r.url,
                user: r.user,
                own: false,
            }),
        };
        let section = self.with_open(|o| Ok(o.graph.config().calendar.clone()))?;
        Ok(CalendarStatus {
            account,
            enabled: cfg.enabled,
            read: cfg.read,
            tasks: cfg.tasks,
            tasks_as: section.tasks_as,
            on_delete: section.on_delete,
            import_tasks: section.import == "inbox",
            inbox: section.inbox,
        })
    }

    /// `calendar_status`: the calendar settings of the open graph.
    pub fn calendar_status(&self, _: NoArgs) -> ApiResult<CalendarStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        self.calendar_status_of(&id)
    }

    /// `calendar_connect`: logs in and lists the calendars. With no arguments the login of the graph's sync is
    /// used; with a server, user and password the graph gets a login of its own (kept only if it works).
    pub fn calendar_connect(&self, a: CalendarConnectArgs) -> ApiResult<CalendarList> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut cfg = self.calendar_settings(&id);
        let given = [&a.server, &a.user, &a.password]
            .iter()
            .any(|v| v.as_deref().is_some_and(|s| !s.trim().is_empty()));
        if !given {
            let account = self.calendar_account(&id, &cfg)?.ok_or_else(|| {
                ApiError::new(
                    "invalid",
                    "this graph is not synced with a server; enter the server and your login",
                )
            })?;
            let calendars = Self::calendar_list(&*self.calendar_client_for(&account)?)?;
            return Ok(CalendarList {
                account: account.view(),
                calendars,
            });
        }
        let field = |v: &Option<String>, what: &str| -> ApiResult<String> {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| ApiError::new("invalid", format!("enter the {what}")))
        };
        let account = Account {
            server: field(&a.server, "server address")?,
            user: field(&a.user, "user name")?,
            password: a
                .password
                .clone()
                .filter(|p| !p.is_empty())
                .ok_or_else(|| ApiError::new("invalid", "enter the password"))?,
            own: true,
        };
        let calendars = Self::calendar_list(&*self.calendar_client_for(&account)?)?;
        self.secrets
            .set(&password_key(&id), &account.password)
            .map_err(|e| {
                ApiError::new("server", format!("the password could not be stored: {e}"))
            })?;
        cfg.account = Some(CalendarAccount {
            server: account.server.clone(),
            user: account.user.clone(),
        });
        self.store_calendar_settings(&id, cfg)?;
        Ok(CalendarList {
            account: account.view(),
            calendars,
        })
    }

    /// `calendar_save`: turns the sync on or off and chooses the calendars. The task calendar must take tasks.
    pub fn calendar_save(&self, a: CalendarSaveArgs) -> ApiResult<CalendarStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut cfg = self.calendar_settings(&id);
        let status = self.calendar_status_of(&id)?;
        if a.enabled && a.tasks.is_none() && a.read.is_empty() {
            return Err(ApiError::new(
                "invalid",
                "choose a calendar for tasks or calendars to read events from",
            ));
        }
        if let Some(href) = &a.tasks {
            let account = self
                .calendar_account(&id, &cfg)?
                .ok_or_else(|| ApiError::new("invalid", "connect to a calendar server first"))?;
            let list = Self::calendar_list(&*self.calendar_client_for(&account)?)?;
            let cal = list
                .iter()
                .find(|c| &c.href == href)
                .ok_or_else(|| ApiError::new("not_found", "that calendar is not on the server"))?;
            let takes = if status.tasks_as == "vevent" {
                cal.events
            } else {
                cal.tasks
            };
            if !takes {
                return Err(ApiError::new(
                    "invalid",
                    format!(
                        "the calendar `{}` does not take {}; choose another",
                        cal.name,
                        if status.tasks_as == "vevent" {
                            "events"
                        } else {
                            "tasks"
                        }
                    ),
                ));
            }
        }
        cfg.enabled = a.enabled;
        cfg.read = a.read;
        cfg.tasks = a.tasks;
        self.store_calendar_settings(&id, cfg)?;
        let import = if a.import_tasks { "inbox" } else { "off" };
        if status.import_tasks != a.import_tasks {
            self.with_open(|o| {
                o.graph
                    .update_config(|c| c.calendar.import = import.into())?;
                Ok(())
            })?;
        }
        self.calendar_status_of(&id)
    }

    /// `calendar_forget`: removes the graph's own calendar login (the sync login stays) and turns the sync off.
    pub fn calendar_forget(&self, _: NoArgs) -> ApiResult<CalendarStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut cfg = self.calendar_settings(&id);
        cfg.account = None;
        cfg.enabled = false;
        self.secrets.delete(&password_key(&id));
        self.store_calendar_settings(&id, cfg)?;
        self.calendar_status_of(&id)
    }

    /// Where what the last push left on the server is kept: with the graph's cache, never in the graph.
    fn calendar_state_file(&self, graph: &str) -> PathBuf {
        self.dirs
            .data
            .join("graphs")
            .join(graph)
            .join("calendar-push.json")
    }

    /// `calendar_sync`: sends the dated tasks of the open graph to the task calendar. A dated task with no block
    /// ID is given one first (the calendar item is named after it).
    pub fn calendar_sync(&self, _: NoArgs) -> ApiResult<CalendarSyncReport> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let cfg = self.calendar_settings(&id);
        let section = self.with_open(|o| Ok(o.graph.config().calendar.clone()))?;
        let Some(calendar) = cfg.tasks.clone().filter(|_| cfg.enabled) else {
            return Err(ApiError::new(
                "invalid",
                "calendar sync is off or has no calendar for tasks (Settings → Calendar)",
            ));
        };
        if section.tasks_as == "vevent" {
            return Err(ApiError::new(
                "invalid",
                "tasks as calendar events (`[calendar] tasks_as = \"vevent\"`) are not supported yet",
            ));
        }
        let account = self
            .calendar_account(&id, &cfg)?
            .ok_or_else(|| ApiError::new("invalid", "connect to a calendar server first"))?;
        let client = self.calendar_client_for(&account)?;

        let collected = self.with_open(|o| collect_todos(o, &id))?;
        let mut ids_added = collected.added;
        let file = self.calendar_state_file(&id);
        let mut state: PushState = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let opts = Options {
            now: chrono::Utc::now().naive_utc(),
            on_delete: if section.on_delete == "delete" {
                OnDelete::Delete
            } else {
                OnDelete::Complete
            },
            // a graph whose index lists no page at all is not read yet: nothing may be removed because of it
            settled: collected.pages > 0,
        };
        let save = |state: &PushState| {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&file, serde_json::to_string(state).unwrap_or_default());
        };

        // 1. what changed in the calendar since last time goes into the notes
        let before = state.clone();
        let fetched = match pull::fetch(&*client, &calendar, &id, &state) {
            Ok(f) => f,
            Err(e) => return Err(api_err(&e)),
        };
        let local: BTreeMap<String, Local> = collected
            .todos
            .iter()
            .map(|t| {
                (
                    t.uid.clone(),
                    Local {
                        todo: t,
                        modified: collected.places.get(&t.uid).and_then(|p| p.modified),
                    },
                )
            })
            .collect();
        let plan = pull::plan(&fetched, &local, &mut state, &calendar);
        let priorities = self.with_open(|o| Ok(o.graph.config().tasks.priorities.clone()))?;
        let applied =
            self.with_open(|o| Ok(apply_pulled(o, &id, &collected, &plan, &priorities)))?;
        // a change that could not be written must not count as read: the next run reads it again
        for uid in &applied.failed {
            match before.items.get(uid) {
                Some(k) => {
                    state.items.insert(uid.clone(), k.clone());
                }
                None => {
                    state.items.remove(uid);
                }
            }
        }
        let removed_in_calendar = plan.removed.len() as u32;
        let mut log = applied.log;
        for uid in &plan.removed {
            if let Some(p) = collected.places.get(uid) {
                log.push(CalendarLogEntry {
                    at: now_local(),
                    kind: "removed_in_calendar".into(),
                    uid: uid.clone(),
                    title: p.title.clone(),
                    page: p.page.clone(),
                    field: None,
                    local: None,
                    remote: None,
                    winner: None,
                });
            }
        }
        let logged = log.len() as u32;
        if logged > 0 {
            self.append_calendar_log(&id, log);
        }
        save(&state);

        // tasks made in other apps are copied into the inbox page
        let mut imported_now = 0u32;
        if section.import == "inbox" {
            let n = self.with_open(|o| {
                Ok(import_foreign(
                    o,
                    &fetched.foreign,
                    &mut state,
                    &section.inbox,
                ))
            })?;
            imported_now = n.len() as u32;
            if !n.is_empty() {
                self.append_calendar_log(&id, n);
                save(&state);
            }
        }

        // 2. the notes as they are now go to the calendar
        let todos = if applied.pulled > 0 || imported_now > 0 {
            let again = self.with_open(|o| collect_todos(o, &id))?;
            ids_added += again.added;
            again.todos
        } else {
            collected.todos
        };
        let result = push(&*client, &calendar, &todos, &mut state, &opts);
        // the copy of an imported task is in the calendar under our UID: the original can go
        if result.is_ok() {
            remove_imported_originals(&*client, &calendar, &id, &fetched.foreign, &mut state);
        }
        // what was done is kept even when the push stopped half way
        save(&state);
        let r = result.map_err(|e| api_err(&e))?;
        Ok(CalendarSyncReport {
            created: r.created as u32,
            updated: r.updated as u32,
            unchanged: r.unchanged as u32,
            retired: r.retired as u32,
            deleted: r.deleted as u32,
            conflicts: r.conflicts,
            ids_added,
            imported: imported_now,
            pulled: applied.pulled,
            removed_in_calendar,
            logged,
        })
    }

    fn calendar_log_file(&self, graph: &str) -> PathBuf {
        self.dirs
            .data
            .join("graphs")
            .join(graph)
            .join("calendar-log.json")
    }

    fn read_calendar_log(&self, graph: &str) -> Vec<CalendarLogEntry> {
        std::fs::read_to_string(self.calendar_log_file(graph))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Adds entries to the sync log, which keeps the last 500.
    fn append_calendar_log(&self, graph: &str, entries: Vec<CalendarLogEntry>) {
        let mut all = self.read_calendar_log(graph);
        all.extend(entries);
        let keep = all.len().saturating_sub(500);
        all.drain(..keep);
        let file = self.calendar_log_file(graph);
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&file, serde_json::to_string(&all).unwrap_or_default());
    }

    /// `calendar_log`: what the sync did to the notes and what it could not decide, newest first.
    pub fn calendar_log(&self, _: NoArgs) -> ApiResult<Vec<CalendarLogEntry>> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut all = self.read_calendar_log(&id);
        all.reverse();
        Ok(all)
    }
}

/// Copies the open tasks other apps made into the inbox page (one block each, with a block ID and the task's
/// dates, priority and repeat) and remembers them, so each is copied once.
fn import_foreign(
    o: &Open,
    foreign: &[notatnik_caldav::ServerItem],
    state: &mut PushState,
    inbox: &str,
) -> Vec<CalendarLogEntry> {
    let priorities = o.graph.config().tasks.priorities.clone();
    let Ok(used) = o.graph.with_index(|i| i.all_block_uids()) else {
        return Vec::new();
    };
    let mut used = used;
    let mut rng_state = now_seed();
    let mut rng = move || {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        rng_state
    };
    let mut blocks: Vec<(String, String, String)> = Vec::new(); // foreign uid, block id, text
    for item in foreign {
        let Some(t) = notatnik_caldav::ical::parse_todo(&item.ics) else {
            continue;
        };
        if state.imported.contains_key(&t.uid)
            || t.summary.trim().is_empty()
            || matches!(t.status, Some(Status::Completed | Status::Cancelled))
        {
            continue;
        }
        let id = notatnik_core::ids::generate_block_id(&used, &mut rng);
        used.insert(id.clone());
        let marker = match (t.status, t.state.as_deref()) {
            (Some(Status::InProcess), _) => "DOING",
            (_, Some("WAITING")) => "WAITING",
            _ => "TODO",
        };
        let title = t.summary.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut text = format!("{marker} {title} ^{id}");
        if let Some(w) = &t.start {
            text.push_str(&format!("\nscheduled:: {}", moment_text(w)));
        }
        if let Some(w) = &t.due {
            text.push_str(&format!("\ndeadline:: {}", moment_text(w)));
        }
        if let Some(p) = t.priority.filter(|p| *p > 0) {
            let defaults = ["A".to_string(), "B".to_string(), "C".to_string()];
            let labels = if priorities.is_empty() {
                &defaults[..]
            } else {
                &priorities[..]
            };
            let n = labels.len();
            let rank = ((n - 1) * (usize::from(p).clamp(1, 9) - 1) + 4) / 8;
            text.push_str(&format!("\npriority:: {}", labels[rank.min(n - 1)]));
        }
        if let Some(r) = &t.rrule {
            text.push_str(&format!("\nrepeat:: {}", repeat_from_rrule(r)));
        }
        if let Some(d) = t.description.as_deref().filter(|d| !d.trim().is_empty()) {
            for line in d.lines() {
                text.push('\n');
                text.push_str(line.trim_end());
            }
        }
        blocks.push((t.uid.clone(), id, text));
    }
    if blocks.is_empty() {
        return Vec::new();
    }
    let Ok(rel) = AppState::page_rel(o, inbox) else {
        return Vec::new();
    };
    let written = match o.graph.read_page(&rel) {
        Ok(file) => {
            let opts = o.graph.config().parse_options();
            let parsed = notatnik_core::parse_page_with(&file.text, &opts);
            let ops: Vec<BlockOp> = blocks
                .iter()
                .enumerate()
                .map(|(i, (_, _, text))| BlockOp::Insert {
                    parent: Vec::new(),
                    index: parsed.blocks.len() + i,
                    text: text.clone(),
                })
                .collect();
            o.graph.edit_page(&rel, Some(&file.hash), &ops).is_ok()
        }
        Err(notatnik_graph::Error::NotFound(_)) => {
            let mut text = String::new();
            for (_, _, b) in &blocks {
                let mut lines = b.split('\n');
                text.push_str(&format!("- {}\n", lines.next().unwrap_or("")));
                for l in lines {
                    text.push_str(&format!("  {l}\n"));
                }
            }
            o.graph.write_page(&rel, &text, None).is_ok()
        }
        Err(_) => false,
    };
    if !written {
        return Vec::new(); // read again next time
    }
    let at = now_local();
    blocks
        .into_iter()
        .map(|(uid, id, text)| {
            state.imported.insert(uid.clone(), id);
            CalendarLogEntry {
                at: at.clone(),
                kind: "imported".into(),
                uid,
                title: text.lines().next().unwrap_or("").to_string(),
                page: inbox.to_string(),
                field: None,
                local: None,
                remote: None,
                winner: None,
            }
        })
        .collect()
}

/// Deletes the original of every imported task whose copy has been sent; forgets originals that are gone.
fn remove_imported_originals(
    client: &dyn CalDavClient,
    calendar: &str,
    graph_id: &str,
    foreign: &[notatnik_caldav::ServerItem],
    state: &mut PushState,
) {
    let entries: Vec<(String, String)> = state
        .imported
        .iter()
        .map(|(u, b)| (u.clone(), b.clone()))
        .collect();
    for (uid, block) in entries {
        let Some(orig) = foreign.iter().find(|i| i.name == format!("{uid}.ics")) else {
            state.imported.remove(&uid); // the original is gone
            continue;
        };
        let ours = notatnik_caldav::map::uid(graph_id, &block);
        if state.items.contains_key(&ours)
            && matches!(
                client.delete(calendar, &orig.name, Some(&orig.etag)),
                Ok(()) | Err(notatnik_caldav::Error::NotFound(_))
            )
        {
            state.imported.remove(&uid);
        }
    }
}

fn now_local() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M").to_string()
}

/// Where a shared task lives.
struct Place {
    page: String,
    rel: String,
    title: String,
    /// When the page file last changed (UTC).
    modified: Option<chrono::NaiveDateTime>,
}

/// The tasks of the notes that go to the calendar.
struct Collected {
    todos: Vec<Todo>,
    /// By UID.
    places: BTreeMap<String, Place>,
    added: u32,
    pages: usize,
}

/// What writing the pulled changes did.
#[derive(Default)]
struct Applied {
    pulled: u32,
    /// UIDs whose changes could not be written.
    failed: Vec<String>,
    log: Vec<CalendarLogEntry>,
}

fn find_block<'a>(blocks: &'a [Block], id: &str, path: &mut Vec<usize>) -> Option<&'a Block> {
    for (i, b) in blocks.iter().enumerate() {
        path.push(i);
        if b.block_id.as_ref().is_some_and(|x| x.id == id) {
            return Some(b);
        }
        if let Some(found) = find_block(&b.children, id, path) {
            return Some(found);
        }
        path.pop();
    }
    None
}

/// A title with no markup: what a plain calendar title can replace without loss.
fn plain_title(block: &Block, source: &str) -> bool {
    let first = block.first_line;
    if block.refs.iter().any(|r| first.contains_span(&r.span)) {
        return false;
    }
    let start = block
        .task
        .as_ref()
        .map_or(first.start, |t| t.marker_span.end.max(first.start));
    let end = block
        .block_id
        .as_ref()
        .map_or(first.end, |id| id.span.start.min(first.end));
    !source.get(start..end).unwrap_or("").contains([
        '*', '_', '`', '~', '[', ']', '<', '>', '#', '$', '\\', '|', '{', '}',
    ])
}

fn moment_text(w: &When) -> String {
    let (d, t) = w.floating();
    match t {
        Some(t) => format!("{} {}", d.format("%Y-%m-%d"), t.format("%H:%M")),
        None => d.format("%Y-%m-%d").to_string(),
    }
}

fn local_time(t: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    use chrono::TimeZone;
    chrono::Local.from_utc_datetime(&t).naive_local()
}

/// The block operations for the changes of one task; `None` for a change that cannot be written.
fn ops_for(
    block: &Block,
    source: &str,
    path: &[usize],
    changes: &[Change],
    priorities: &[String],
    log: &mut Vec<(Change, bool)>,
) -> Vec<BlockOp> {
    let at = BlockRef::at(path);
    let Some(task) = block.task.as_ref() else {
        return Vec::new();
    };
    let completes_repeat = task.repeat.is_some()
        && changes.iter().any(|c| {
            matches!(
                c,
                Change::Status {
                    status: Status::Completed,
                    ..
                }
            )
        });
    let mut ops = Vec::new();
    // a new title replaces the block's text from its original properties, so it goes first
    let mut ordered: Vec<&Change> = changes.iter().collect();
    ordered.sort_by_key(|c| !matches!(c, Change::Summary(_)));
    for c in ordered {
        let mut done = true;
        match c {
            Change::Status {
                status,
                word,
                completed,
            } => {
                let word = word.as_deref().unwrap_or("");
                let state = match status {
                    Status::NeedsAction => match task.state {
                        TaskState::Todo | TaskState::Later | TaskState::Waiting => task.state,
                        _ => match word {
                            "WAITING" => TaskState::Waiting,
                            "LATER" => TaskState::Later,
                            _ => TaskState::Todo,
                        },
                    },
                    Status::InProcess => {
                        if word == "NOW" {
                            TaskState::Now
                        } else {
                            TaskState::Doing
                        }
                    }
                    Status::Completed => TaskState::Done,
                    Status::Cancelled => TaskState::Canceled,
                };
                if state != task.state {
                    let when = completed
                        .map(local_time)
                        .unwrap_or_else(|| chrono::Local::now().naive_local());
                    ops.push(BlockOp::SetTaskState {
                        block: at.clone(),
                        state: Some(state),
                        done_at: Some(when.format("%Y-%m-%dT%H:%M").to_string()),
                        completed_limit: None,
                    });
                }
            }
            // the app moves a repeating task on when it is completed
            Change::Start(_) | Change::Due(_) if completes_repeat => {}
            Change::Start(w) => match w {
                None => ops.push(BlockOp::RemoveProperty {
                    block: at.clone(),
                    key: "scheduled".into(),
                }),
                Some(w) => {
                    let (date, time) = w.floating();
                    // a planned range keeps its end when it still fits
                    let mut text = moment_text(w);
                    if let (Some(t), Some(Moment { end: Some(e), .. })) = (time, &task.scheduled) {
                        if *e > t {
                            text = format!(
                                "{} {}-{}",
                                date.format("%Y-%m-%d"),
                                t.format("%H:%M"),
                                e.format("%H:%M")
                            );
                        }
                    }
                    ops.push(BlockOp::SetProperty {
                        block: at.clone(),
                        key: "scheduled".into(),
                        value: text,
                    });
                }
            },
            Change::Due(w) => match w {
                None => {
                    if task.deadline.is_some() {
                        ops.push(BlockOp::RemoveProperty {
                            block: at.clone(),
                            key: "deadline".into(),
                        });
                    }
                }
                Some(w) => {
                    let (date, time) = w.floating();
                    let range_end = match (&task.deadline, &task.scheduled) {
                        (
                            None,
                            Some(Moment {
                                date: d,
                                time: Some(st),
                                end: Some(_),
                            }),
                        ) if *d == date => time.filter(|t| t > st).map(|t| (*st, t)),
                        _ => None,
                    };
                    if let Some((st, end)) = range_end {
                        ops.push(BlockOp::SetProperty {
                            block: at.clone(),
                            key: "scheduled".into(),
                            value: format!(
                                "{} {}-{}",
                                date.format("%Y-%m-%d"),
                                st.format("%H:%M"),
                                end.format("%H:%M")
                            ),
                        });
                    } else {
                        ops.push(BlockOp::SetProperty {
                            block: at.clone(),
                            key: "deadline".into(),
                            value: moment_text(w),
                        });
                    }
                }
            },
            Change::Priority(p) => match p {
                None => ops.push(BlockOp::RemoveProperty {
                    block: at.clone(),
                    key: "priority".into(),
                }),
                Some(p) => {
                    let defaults = ["A".to_string(), "B".to_string(), "C".to_string()];
                    let labels = if priorities.is_empty() {
                        &defaults[..]
                    } else {
                        priorities
                    };
                    let n = labels.len();
                    let rank = ((n - 1) * (usize::from(*p).clamp(1, 9) - 1) + 4) / 8;
                    ops.push(BlockOp::SetProperty {
                        block: at.clone(),
                        key: "priority".into(),
                        value: labels[rank.min(n - 1)].clone(),
                    });
                }
            },
            Change::Summary(text) => {
                if plain_title(block, source) && !text.is_empty() {
                    let edit = block.edit_text(source);
                    let marker = task.marker_span.slice(source);
                    let first = format!("{marker} {text}");
                    let new = match edit.split_once('\n') {
                        Some((_, rest)) => format!("{first}\n{rest}"),
                        None => first,
                    };
                    ops.push(BlockOp::UpdateText {
                        block: at.clone(),
                        text: new,
                    });
                } else {
                    done = false;
                }
            }
            Change::Repeat(r) => match r {
                None => ops.push(BlockOp::RemoveProperty {
                    block: at.clone(),
                    key: "repeat".into(),
                }),
                Some(r) => {
                    let mut text = repeat_from_rrule(r);
                    if task.repeat.as_ref().is_some_and(|x| x.from_completion) {
                        text.push_str(" from-completion");
                    }
                    ops.push(BlockOp::SetProperty {
                        block: at.clone(),
                        key: "repeat".into(),
                        value: text,
                    });
                }
            },
        }
        log.push((c.clone(), done));
    }
    ops
}

/// Writes what was read from the calendar into the pages, one edit per page, guarded by the page's hash.
fn apply_pulled(
    o: &Open,
    graph_id: &str,
    collected: &Collected,
    plan: &pull::Plan,
    priorities: &[String],
) -> Applied {
    let mut out = Applied::default();
    let mut by_page: BTreeMap<&str, Vec<&pull::Pulled>> = BTreeMap::new();
    for item in &plan.items {
        if let Some(p) = collected.places.get(&item.uid) {
            by_page.entry(p.rel.as_str()).or_default().push(item);
        }
    }
    let opts = o.graph.config().parse_options();
    for (rel, items) in by_page {
        let Ok(file) = o.graph.read_page(rel) else {
            out.failed.extend(items.iter().map(|i| i.uid.clone()));
            continue;
        };
        let parsed = notatnik_core::parse_page_with(&file.text, &opts);
        let mut ops = Vec::new();
        let mut entries = Vec::new();
        let mut uids = Vec::new();
        for item in items {
            let Some(bid) = block_id_of(graph_id, &item.uid) else {
                continue;
            };
            let mut path = Vec::new();
            let Some(block) = find_block(&parsed.blocks, &bid, &mut path) else {
                out.failed.push(item.uid.clone());
                continue;
            };
            let place = &collected.places[&item.uid];
            let mut results = Vec::new();
            let more = ops_for(
                block,
                &file.text,
                &path,
                &item.changes,
                priorities,
                &mut results,
            );
            let entry = |kind: &str,
                         field: &str,
                         local: Option<String>,
                         remote: Option<String>,
                         winner: Option<&str>| {
                CalendarLogEntry {
                    at: now_local(),
                    kind: kind.into(),
                    uid: item.uid.clone(),
                    title: place.title.clone(),
                    page: place.page.clone(),
                    field: Some(field.into()),
                    local,
                    remote,
                    winner: winner.map(str::to_string),
                }
            };
            for c in &item.conflicts {
                let kept = match c.winner {
                    Side::Local => "local",
                    Side::Remote => "remote",
                };
                entries.push(entry(
                    "conflict",
                    c.field.name(),
                    Some(c.local.clone()),
                    Some(c.remote.clone()),
                    Some(kept),
                ));
            }
            for (change, written) in results {
                if !written {
                    entries.push(entry(
                        "title_kept",
                        change.field().name(),
                        Some(place.title.clone()),
                        match &change {
                            Change::Summary(t) => Some(t.clone()),
                            _ => None,
                        },
                        Some("local"),
                    ));
                } else if !item.conflicts.iter().any(|c| c.field == change.field()) {
                    entries.push(entry("pulled", change.field().name(), None, None, None));
                }
            }
            if !more.is_empty() {
                uids.push(item.uid.clone());
                ops.extend(more);
            }
        }
        if ops.is_empty() {
            out.log.extend(entries);
            continue;
        }
        match o.graph.edit_page(rel, Some(&file.hash), &ops) {
            Ok(_) => {
                out.pulled += uids.len() as u32;
                out.log.extend(entries);
            }
            Err(_) => out.failed.extend(uids),
        }
    }
    out
}

/// The calendar tasks of every page, after giving the dated tasks that lack one a block ID.
fn collect_todos(o: &Open, graph_id: &str) -> ApiResult<Collected> {
    let files = o.graph.with_index(|i| i.page_files())?;
    let opts = o.graph.config().parse_options();
    let priorities = o.graph.config().tasks.priorities.clone();
    let mut used = o.graph.with_index(|i| i.all_block_uids())?;
    let mut rng_state = now_seed();
    let mut rng = move || {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        rng_state
    };
    let now = chrono::Utc::now().naive_utc();
    let (mut todos, mut added) = (Vec::new(), 0u32);
    let mut places = BTreeMap::new();
    for (page, rel) in &files {
        let Ok(mut file) = o.graph.read_page(rel) else {
            continue;
        };
        let mut parsed = notatnik_core::parse_page_with(&file.text, &opts);
        // dated tasks without an ID get one, all of a page in one edit
        let mut ops = Vec::new();
        ids_for_dated_tasks(
            &parsed.blocks,
            &mut Vec::new(),
            &mut ops,
            &mut used,
            &mut rng,
        );
        if !ops.is_empty() {
            let n = ops.len() as u32;
            // a page that changed meanwhile is simply skipped this time; the next run adds the IDs
            if o.graph.edit_page(rel, Some(&file.hash), &ops).is_ok() {
                added += n;
                if let Ok(f) = o.graph.read_page(rel) {
                    file = f;
                    parsed = notatnik_core::parse_page_with(&file.text, &opts);
                }
            }
        }
        let ctx = Context {
            graph_id,
            page,
            source: &file.text,
            stamp: now,
            priorities: &priorities,
        };
        fn gather(ctx: &Context, blocks: &[Block], out: &mut Vec<Todo>) {
            for b in blocks {
                if let Some(t) = todo_from_block(ctx, b) {
                    out.push(t);
                }
                gather(ctx, &b.children, out);
            }
        }
        let mut found = Vec::new();
        gather(&ctx, &parsed.blocks, &mut found);
        let modified = std::fs::metadata(o.graph.root().join(rel))
            .and_then(|m| m.modified())
            .ok()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).naive_utc());
        for t in found {
            places.insert(
                t.uid.clone(),
                Place {
                    page: page.clone(),
                    rel: rel.clone(),
                    title: t.summary.clone(),
                    modified,
                },
            );
            todos.push(t);
        }
    }
    Ok(Collected {
        todos,
        places,
        added,
        pages: files.len(),
    })
}

/// An `EnsureBlockId` op for every dated task (not marked `cal:: no`) that has no block ID.
fn ids_for_dated_tasks<R: FnMut() -> u64>(
    blocks: &[Block],
    path: &mut Vec<usize>,
    ops: &mut Vec<BlockOp>,
    used: &mut std::collections::HashSet<String>,
    rng: &mut R,
) {
    for (i, b) in blocks.iter().enumerate() {
        path.push(i);
        let dated = b
            .task
            .as_ref()
            .is_some_and(|t| t.scheduled.is_some() || t.deadline.is_some());
        let private = b
            .properties
            .iter()
            .any(|p| p.key.eq_ignore_ascii_case("cal") && p.raw.trim().eq_ignore_ascii_case("no"));
        if dated && !private && b.block_id.is_none() {
            let id = notatnik_core::ids::generate_block_id(used, &mut *rng);
            used.insert(id.clone());
            ops.push(BlockOp::EnsureBlockId {
                block: BlockRef::at(path),
                id,
            });
        }
        ids_for_dated_tasks(&b.children, path, ops, used, rng);
        path.pop();
    }
}
