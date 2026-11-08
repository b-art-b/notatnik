//! Android home-screen widgets (AN-5..AN-7, `docs/phases/android-widgets.md`).
//!
//! Two files of the device (never of the graph, ST-2) in `<data dir>/widgets/`: `capture-queue.jsonl`, which the
//! widget's dialog appends to and this module drains into the graph, and `today.json`, the snapshot of today's tasks
//! that the task widget reads. The widget never touches the graph's Markdown: the app is the only writer.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use super::*;

/// What the Kotlin side of the app does for the widgets. Nothing on a computer.
pub trait Widgets: Send + Sync {
    /// Tells the platform where the widget files are (the widget reads and writes them from another process).
    fn directory(&self, dir: &Path);
    /// The widgets should draw again from `today.json`.
    fn refresh(&self);
    /// The link (`notatnik://capture`, `notatnik://page/<name>`) the app was opened with, once.
    fn take_link(&self) -> Option<String>;
}

/// No widgets (a computer, the dev server, tests).
pub struct NoWidgets;

impl Widgets for NoWidgets {
    fn directory(&self, _dir: &Path) {}
    fn refresh(&self) {}
    fn take_link(&self) -> Option<String> {
        None
    }
}

const QUEUE: &str = "capture-queue.jsonl";
const DRAINING: &str = "capture-queue.draining";
const FAILED: &str = "capture-queue.failed";
const APPLIED: &str = "capture-applied.json";
const SNAPSHOT: &str = "today.json";
/// How many ids of drained lines are remembered, so that a crash between writing and deleting adds nothing twice.
const KEEP_IDS: usize = 200;
/// The longest list the task widget gets.
const MAX_TASKS: usize = 20;
/// The longest line a capture may add (a pasted page is not a note).
const MAX_CAPTURE: usize = 20_000;

/// One captured line, as the widget's dialog writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueLine {
    /// Random, to tell a line that was drained already.
    pub id: String,
    /// The graph the widget was showing (`GraphInfo.id`); empty = whichever graph is open.
    #[serde(default)]
    pub graph: String,
    /// The text.
    pub text: String,
    /// When it was typed, local time `YYYY-MM-DDTHH:MM[:SS]`; its day decides the journal.
    #[serde(default)]
    pub at: String,
}

/// What a drain did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Drained {
    /// Lines added to the graph.
    pub applied: u32,
    /// Lines for another graph, left in the queue.
    pub kept: u32,
    /// Lines that could not be used, moved to `capture-queue.failed`.
    pub failed: u32,
}

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    // one write of the whole line, so a reader never sees half of it
    f.write_all(format!("{}\n", line.trim_end()).as_bytes())
}

fn load_ids(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join(APPLIED))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

/// Applies the queued lines with `apply` and empties the queue. A line is dropped from the queue only after it was
/// applied; the file is renamed first so a widget appending meanwhile starts a new queue.
fn drain(
    dir: &Path,
    graph_id: &str,
    mut apply: impl FnMut(&QueueLine) -> Result<(), String>,
) -> io::Result<Drained> {
    let mut out = Drained::default();
    let (queue, draining) = (dir.join(QUEUE), dir.join(DRAINING));
    // a crash left its half-done drain: finish that one first
    if !draining.exists() {
        if !queue.exists() {
            return Ok(out);
        }
        fs::rename(&queue, &draining)?;
    }
    let text = fs::read_to_string(&draining)?;
    let mut ids = load_ids(dir);
    for raw in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Ok(line) = serde_json::from_str::<QueueLine>(raw) else {
            append_line(&dir.join(FAILED), raw)?;
            out.failed += 1;
            continue;
        };
        if ids.contains(&line.id) {
            continue;
        }
        if !line.graph.is_empty() && line.graph != graph_id {
            append_line(&queue, raw)?;
            out.kept += 1;
            continue;
        }
        match apply(&line) {
            Ok(()) => {
                ids.push(line.id);
                out.applied += 1;
            }
            Err(e) => {
                log::warn!("a captured line could not be added: {e}");
                append_line(&dir.join(FAILED), raw)?;
                out.failed += 1;
            }
        }
    }
    if ids.len() > KEEP_IDS {
        ids.drain(..ids.len() - KEEP_IDS);
    }
    write_atomic(
        &dir.join(APPLIED),
        serde_json::to_string(&ids).unwrap_or_default().as_bytes(),
    )?;
    fs::remove_file(&draining).or_else(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e)
        }
    })?;
    Ok(out)
}

/// The day (`YYYY-MM-DD`) of a queue line's `at`; `fallback` when it is not a date.
fn day_of(at: &str, fallback: &str) -> String {
    at.get(..10)
        .filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
        .unwrap_or(fallback)
        .to_string()
}

/// One task of the snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotTask {
    /// Page name.
    pub page: String,
    /// The task's first line without its marker.
    pub text: String,
    /// `YYYY-MM-DD`.
    pub due: Option<String>,
    /// Dated before today.
    pub overdue: bool,
    /// `TODO`, `DOING` ...
    pub marker: String,
}

/// `today.json`: what the task widget draws.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Format version.
    pub v: u32,
    /// `GraphInfo.id` of the graph the tasks are from.
    pub graph: String,
    /// Today when it was written.
    pub date: String,
    /// When it was written (not compared when deciding to write again).
    pub written_at: String,
    /// The tasks, today's first.
    pub tasks: Vec<SnapshotTask>,
    /// How many more there are than `tasks` holds.
    pub more: u32,
}

/// The text of a task block's first line: no marker, no checkbox, no link brackets.
fn task_text(raw: &str) -> String {
    let first = raw.lines().next().unwrap_or("").trim();
    let mut s = first;
    for m in ["TODO ", "LATER ", "DOING ", "NOW ", "WAITING ", "DONE "] {
        if let Some(r) = s.strip_prefix(m) {
            s = r.trim_start();
            break;
        }
    }
    for c in ["[ ] ", "[x] ", "[X] "] {
        if let Some(r) = s.strip_prefix(c) {
            s = r;
            break;
        }
    }
    s.replace("[[", "").replace("]]", "")
}

/// Today's tasks first, then the overdue ones, each group by priority; at most [`MAX_TASKS`].
fn snapshot_of(graph: &str, a: &AgendaView, keep: impl Fn(&str) -> bool, now: &str) -> Snapshot {
    let take = |items: &[AgendaItem], overdue: bool| -> Vec<SnapshotTask> {
        let mut v: Vec<&AgendaItem> = items.iter().filter(|i| keep(&i.page)).collect();
        v.sort_by_key(|i| {
            i.block
                .task
                .as_ref()
                .and_then(|t| t.priority_rank)
                .unwrap_or(u32::MAX)
        });
        v.into_iter()
            .map(|i| SnapshotTask {
                page: i.page.clone(),
                text: task_text(&i.block.text),
                due: i.date.clone(),
                overdue,
                marker: i
                    .block
                    .task
                    .as_ref()
                    .map(|t| format!("{:?}", t.state).to_uppercase())
                    .unwrap_or_default(),
            })
            .collect()
    };
    let mut all = take(&a.today_items, false);
    all.extend(take(&a.overdue, true));
    let more = all.len().saturating_sub(MAX_TASKS) as u32;
    all.truncate(MAX_TASKS);
    Snapshot {
        v: 1,
        graph: graph.to_string(),
        date: a.today.clone(),
        written_at: now.to_string(),
        tasks: all,
        more,
    }
}

/// Writes the snapshot unless the file already says the same; whether it was written.
fn write_snapshot(dir: &Path, s: &Snapshot) -> io::Result<bool> {
    fs::create_dir_all(dir)?;
    let path = dir.join(SNAPSHOT);
    let same = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str::<Snapshot>(&t).ok())
        .is_some_and(|old| {
            Snapshot {
                written_at: s.written_at.clone(),
                ..old
            } == *s
        });
    if same {
        return Ok(false);
    }
    write_atomic(
        &path,
        serde_json::to_string(s).unwrap_or_default().as_bytes(),
    )?;
    Ok(true)
}

impl AppState {
    /// Uses `widgets` for what only the platform can do (Android).
    #[must_use]
    pub fn with_widgets(mut self, widgets: Arc<dyn Widgets>) -> Self {
        self.widgets = widgets;
        self
    }

    fn widgets_dir(&self) -> std::path::PathBuf {
        self.dirs.data.join("widgets")
    }

    /// `capture`: adds a line as the last top-level block of a page (today's journal when none is named) (AN-5).
    pub fn capture(&self, a: CaptureArgs) -> ApiResult<StringValue> {
        let page = a
            .page
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map_or_else(
                || self.today().format("%Y-%m-%d").to_string(),
                str::to_string,
            );
        self.capture_to(&page, &a.text)
    }

    fn capture_to(&self, page: &str, text: &str) -> ApiResult<StringValue> {
        let text = text.replace("\r\n", "\n").trim().to_string();
        if text.is_empty() {
            return Err(ApiError::new("invalid", "there is nothing to add"));
        }
        if text.chars().count() > MAX_CAPTURE {
            return Err(ApiError::new("invalid", "that is too long for a note"));
        }
        let view = self.get_page(GetPageArgs {
            name: page.to_string(),
        })?;
        let r = self.apply_ops(ApplyOpsArgs {
            page: view.name.clone(),
            base_hash: view.hash.clone(),
            ops: vec![BlockOp::Insert {
                parent: vec![],
                index: view.blocks.len(),
                text,
            }],
        })?;
        Ok(StringValue { value: r.page.name })
    }

    /// `widgets_sync`: adds what the widget captured, writes the snapshot of today's tasks, asks the platform to redraw
    /// (AN-6, AN-7). Called when the app starts, comes back to the foreground, and after the notes changed.
    pub fn widgets_sync(&self, _a: NoArgs) -> ApiResult<WidgetsReport> {
        let dir = self.widgets_dir();
        self.widgets.directory(&dir);
        let (graph_id, rules) =
            self.with_open(|o| Ok((o.graph.id(), o.graph.config().encryption.rules())))?;
        let today = self.today().format("%Y-%m-%d").to_string();
        let drained = drain(&dir, &graph_id, |l| {
            self.capture_to(&day_of(&l.at, &today), &l.text)
                .map(|_| ())
                .map_err(|e| e.message)
        })
        .map_err(|e| ApiError::new("internal", format!("the capture queue: {e}")))?;
        let agenda = self.agenda(AgendaArgs {
            ns: None,
            tag: None,
            priority: None,
        })?;
        // tasks of an encrypted page are not copied to a place outside the graph's protection (EN-*)
        let files = self.with_open(|o| {
            let mut m = HashSet::new();
            for p in agenda
                .today_items
                .iter()
                .chain(&agenda.overdue)
                .map(|i| &i.page)
            {
                let rel = o
                    .graph
                    .with_index(|i| i.page_info(p))?
                    .and_then(|i| i.file_path)
                    .unwrap_or_else(|| {
                        page_to_path(
                            &PageName::parse(p).unwrap_or_else(|_| PageName::parse("x").unwrap()),
                        )
                    });
                if rules.is_encrypted(&rel) {
                    m.insert(p.to_lowercase());
                }
            }
            Ok(m)
        })?;
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let snap = snapshot_of(
            &graph_id,
            &agenda,
            |p| !files.contains(&p.to_lowercase()),
            &now,
        );
        let changed = write_snapshot(&dir, &snap)
            .map_err(|e| ApiError::new("internal", format!("the widget snapshot: {e}")))?;
        if changed || drained.applied > 0 {
            self.widgets.refresh();
        }
        Ok(WidgetsReport {
            drained: drained.applied,
            kept: drained.kept,
            failed: drained.failed,
            tasks: snap.tasks.len() as u32,
        })
    }

    /// `take_link`: the `notatnik://` link the app was opened with, once; empty when there is none.
    pub fn take_link(&self, _a: NoArgs) -> ApiResult<StringValue> {
        Ok(StringValue {
            value: self.widgets.take_link().unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, graph: &str, text: &str, at: &str) -> String {
        serde_json::to_string(&QueueLine {
            id: id.into(),
            graph: graph.into(),
            text: text.into(),
            at: at.into(),
        })
        .unwrap()
    }

    fn dir_with(lines: &[String]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(QUEUE), lines.join("\n") + "\n").unwrap();
        d
    }

    #[test]
    fn a_queue_is_applied_in_order_and_removed() {
        let d = dir_with(&[
            line("1", "", "one", "2026-11-08T10:00"),
            line("2", "", "two", "2026-11-08T10:01"),
            line("3", "", "three", "2026-11-08T10:02"),
        ]);
        let mut got = vec![];
        let r = drain(d.path(), "g", |l| {
            got.push(l.text.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(got, ["one", "two", "three"]);
        assert_eq!(r.applied, 3);
        assert!(!d.path().join(QUEUE).exists() && !d.path().join(DRAINING).exists());
    }

    #[test]
    fn a_line_applied_before_a_crash_is_not_added_twice() {
        let d = dir_with(&[line("1", "", "one", ""), line("2", "", "two", "")]);
        // a first drain that died after "one": the draining file is still there, "one" is remembered
        fs::rename(d.path().join(QUEUE), d.path().join(DRAINING)).unwrap();
        fs::write(d.path().join(APPLIED), "[\"1\"]").unwrap();
        let mut got = vec![];
        drain(d.path(), "g", |l| {
            got.push(l.text.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(got, ["two"]);
    }

    #[test]
    fn a_line_for_another_graph_stays_and_a_broken_one_goes_to_failed() {
        let d = dir_with(&[
            line("1", "other", "elsewhere", ""),
            "this is not json".into(),
            line("2", "g", "here", ""),
            line("3", "", "fails", ""),
        ]);
        let r = drain(d.path(), "g", |l| {
            if l.text == "fails" {
                Err("no".into())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!((r.applied, r.kept, r.failed), (1, 1, 2));
        let kept = fs::read_to_string(d.path().join(QUEUE)).unwrap();
        assert!(kept.contains("elsewhere") && !kept.contains("\"text\":\"here\""));
        let failed = fs::read_to_string(d.path().join(FAILED)).unwrap();
        assert!(failed.contains("not json") && failed.contains("fails"));
    }

    #[test]
    fn without_a_queue_nothing_happens() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            drain(d.path(), "g", |_| Ok(())).unwrap(),
            Drained::default()
        );
    }

    #[test]
    fn the_day_of_a_line_is_the_day_it_was_typed() {
        assert_eq!(day_of("2026-11-07T23:58", "2026-11-08"), "2026-11-07");
        assert_eq!(day_of("", "2026-11-08"), "2026-11-08");
        assert_eq!(day_of("yesterday", "2026-11-08"), "2026-11-08");
    }

    #[test]
    fn a_task_shows_its_first_line_without_marker_and_brackets() {
        assert_eq!(
            task_text("TODO call [[Anna]] about it\nscheduled:: x"),
            "call Anna about it"
        );
        assert_eq!(task_text("DOING [ ] plain"), "plain");
    }
}
