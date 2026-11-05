//! The real-page check of W6 (WA-1): clips the addresses in `NOTATNIK_REAL_PAGES` (one per line, `#` starts a comment)
//! over the real network and writes the books and a summary to `NOTATNIK_REAL_OUT` (default: a temp folder).
//! Ignored by default because it needs the network and its result is judged by reading it:
//!
//! ```text
//! NOTATNIK_REAL_PAGES=pages.txt cargo test -p notatnik-clip --test real -- --ignored --nocapture
//! ```

use notatnik_clip::{ClipPolicy, clip};

#[test]
#[ignore = "needs the network; run by hand (see the module doc)"]
fn clip_real_pages() {
    let Ok(list) = std::env::var("NOTATNIK_REAL_PAGES") else {
        eprintln!("NOTATNIK_REAL_PAGES is not set: nothing to do");
        return;
    };
    let out = std::env::var("NOTATNIK_REAL_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("notatnik-real-pages"));
    std::fs::create_dir_all(&out).unwrap();
    let urls = std::fs::read_to_string(list).unwrap();
    for url in urls
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let started = std::time::Instant::now();
        match clip(url, &ClipPolicy::default(), chrono::Utc::now(), &mut |_| {
            true
        }) {
            Ok(c) => {
                std::fs::write(out.join(&c.file_name), &c.epub).unwrap();
                let notes: Vec<String> = c.notes.iter().map(ToString::to_string).collect();
                println!(
                    "OK   {url}\n     title={:?} byline={:?} date={:?} lang={} size={} KB time={:?}\n     notes={notes:?}\n     file={}",
                    c.title,
                    c.byline,
                    c.date,
                    c.language,
                    c.epub.len() / 1024,
                    started.elapsed(),
                    c.file_name
                );
            }
            Err(e) => println!("FAIL {url}\n     {e} (after {:?})", started.elapsed()),
        }
    }
}
