//! Clipping a web page into the graph (WA-1 to WA-4, W2): `notatnik-clip` does the fetching and the book; this module
//! runs it **without holding the graph** (a slow site must not stop every other command), reports where it is, can be
//! cancelled, and writes the book below `assets/web/`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use notatnik_clip::{ClipError, Progress};

use super::{ApiError, ApiResult, AppState};
use crate::dto::{
    ClipExistingArgs, ClipPageArgs, ClipProgressView, ClippedPage, NoArgs, StringValue,
};

/// A clip that is running.
pub(super) struct ClipRun {
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<ClipProgressView>>,
}

/// Ends the run when it goes out of scope, whichever way `clip_page` returns.
struct Ended<'a>(&'a Mutex<Option<ClipRun>>);

impl Drop for Ended<'_> {
    fn drop(&mut self) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

fn failure(e: ClipError) -> ApiError {
    let code = match e {
        ClipError::BadAddress | ClipError::Refused => "invalid",
        ClipError::Cancelled => "cancelled",
        _ => "clip_failed",
    };
    ApiError::new(code, e.to_string())
}

impl AppState {
    /// Lets this state fetch addresses on this computer and private networks. For tests only (the dev server's
    /// `--allow-local-fetch`); the app never calls it.
    pub fn with_local_fetch(mut self) -> Self {
        self.clip_policy.allow_private = true;
        self
    }

    fn run_lock(&self) -> std::sync::MutexGuard<'_, Option<ClipRun>> {
        self.clip_run
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `clip_page`: fetches the page, makes the book, stores it as `assets/web/<slug>-<hash8>.epub` (the same address
    /// always gets the same file, a title that changed does not rename it) and returns the link for `page`.
    pub fn clip_page(&self, a: ClipPageArgs) -> ApiResult<ClippedPage> {
        let page = Self::parse_name(&a.page)?;
        self.with_open(|_| Ok(()))?; // no graph, no clip
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(ClipProgressView {
            running: true,
            step: "fetching".into(),
            done: 0,
            total: 0,
        }));
        {
            let mut run = self.run_lock();
            if run.is_some() {
                return Err(ApiError::new("conflict", "a page is being clipped already"));
            }
            *run = Some(ClipRun {
                cancel: cancel.clone(),
                progress: progress.clone(),
            });
        }
        let _ended = Ended(&self.clip_run);

        // The fetch runs on a thread of its own and this one waits for it, looking at the cancel flag every 50 ms: a
        // site that holds the connection open must not make Cancel wait for the network timeout. A clip that was
        // cancelled leaves its thread to end by itself (the progress closure stops it at the next step, the
        // timeouts bound the rest); it stores nothing.
        let (tx, rx) = std::sync::mpsc::channel();
        let (url, policy, seen, stop) = (
            a.url.clone(),
            self.clip_policy.clone(),
            progress.clone(),
            cancel.clone(),
        );
        std::thread::spawn(move || {
            let clip = notatnik_clip::clip(&url, &policy, chrono::Utc::now(), &mut |p| {
                let mut v = seen
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match p {
                    Progress::Fetching => v.step = "fetching".into(),
                    Progress::Cleaning => v.step = "cleaning".into(),
                    Progress::Picture { done, total } => {
                        v.step = "pictures".into();
                        v.done = done as u32;
                        v.total = total as u32;
                    }
                    Progress::Writing => v.step = "writing".into(),
                }
                !stop.load(Ordering::SeqCst)
            });
            // nobody listens any more after a cancel
            let _ = tx.send(clip);
        });
        let clip = loop {
            match rx.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok(done) => break done.map_err(failure)?,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(failure(ClipError::Cancelled));
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(ApiError::new(
                        "clip_failed",
                        "the clip stopped unexpectedly",
                    ));
                }
            }
        };
        // a cancel that came while the book was being written: nothing is stored
        if cancel.load(Ordering::SeqCst) {
            return Err(failure(ClipError::Cancelled));
        }
        log::debug!(
            "clipped a page ({} bytes, {} notes)",
            clip.epub.len(),
            clip.notes.len()
        );
        let hash8 = notatnik_clip::address_key(&a.url).unwrap_or_default();
        self.with_open(|o| {
            let existing = o.graph.find_clip(&hash8)?;
            let file_name = existing
                .as_deref()
                .and_then(|p| p.rsplit('/').next())
                .map_or_else(|| clip.file_name.clone(), str::to_string);
            let path = o.graph.save_clip(&file_name, &clip.epub)?;
            let page_rel = Self::page_file(o, &page)?;
            let link = notatnik_graph::relative_asset_path(&page_rel, &path);
            let label: String = clip
                .title
                .chars()
                .flat_map(|c| match c {
                    '[' | ']' | '\\' => vec!['\\', c],
                    '\n' | '\r' => vec![' '],
                    c => vec![c],
                })
                .collect();
            Ok(ClippedPage {
                path,
                title: clip.title.clone(),
                markdown: format!("[{label}]({link})"),
                source: clip.source.clone(),
                notes: clip.notes.iter().map(ToString::to_string).collect(),
                replaced: existing.is_some(),
            })
        })
    }

    /// `clip_progress`: where the running clip is (`running: false` when none is).
    pub fn clip_progress(&self, _a: NoArgs) -> ApiResult<ClipProgressView> {
        Ok(match &*self.run_lock() {
            Some(run) => run
                .progress
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            None => ClipProgressView {
                running: false,
                step: String::new(),
                done: 0,
                total: 0,
            },
        })
    }

    /// `clip_cancel`: stops the running clip at its next step; nothing is stored.
    pub fn clip_cancel(&self, _a: NoArgs) -> ApiResult<NoArgs> {
        if let Some(run) = &*self.run_lock() {
            run.cancel.store(true, Ordering::SeqCst);
        }
        Ok(NoArgs {})
    }

    /// `clip_existing`: the book an address was clipped into before, if any (graph-relative path).
    pub fn clip_existing(&self, a: ClipExistingArgs) -> ApiResult<Option<StringValue>> {
        let Some(hash8) = notatnik_clip::address_key(&a.url) else {
            return Err(failure(ClipError::BadAddress));
        };
        self.with_open(|o| {
            Ok(o.graph
                .find_clip(&hash8)?
                .map(|value| StringValue { value }))
        })
    }
}
