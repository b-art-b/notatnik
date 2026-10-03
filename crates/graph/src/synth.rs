//! Deterministic synthetic graph generator for performance tests (docs/testing.md §3, P1.38).
//!
//! Same seed, same options → byte-identical output. Shape: namespaces up to depth 5, ~20 blocks per
//! page with a long tail (some pages with 2,000 blocks), power-law links, ~15% tasks, properties,
//! tags, block refs, Polish diacritics, two years of journals.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{Days, NaiveDate};

/// Generator options.
#[derive(Debug, Clone)]
pub struct SynthOptions {
    /// PRNG seed.
    pub seed: u64,
    /// Number of non-journal pages.
    pub pages: usize,
    /// Number of journal days (counting back from 2026-10-03).
    pub journal_days: usize,
}

impl Default for SynthOptions {
    fn default() -> Self {
        SynthOptions {
            seed: 1,
            pages: 10_000,
            journal_days: 730,
        }
    }
}

/// What was generated.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct SynthReport {
    /// Page files.
    pub pages: usize,
    /// Journal files.
    pub journals: usize,
    /// Total blocks.
    pub blocks: usize,
    /// Links to pages that intentionally don't exist.
    pub intentional_unresolved: usize,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items.get(self.below(items.len())).copied().unwrap_or("")
    }
}

const TOP: &[&str] = &[
    "work",
    "personal",
    "reading",
    "projects",
    "people",
    "meetings",
    "health",
    "finance",
    "travel",
    "home",
    "garden",
    "cooking",
    "learning",
    "ideas",
    "areas",
    "archive",
    "clients",
    "research",
    "writing",
    "tools",
    "music",
    "films",
    "sport",
    "family",
    "zakupy",
    "praca",
    "dom",
    "podróże",
    "książki",
    "pomysły",
    "notatki",
    "szkoła",
    "samochód",
    "ogród",
    "zdrowie",
    "finanse",
    "hobby",
    "events",
    "inbox",
    "wiki",
    "recipes",
    "papers",
    "talks",
    "courses",
    "devices",
    "software",
    "legal",
    "taxes",
    "plans",
    "goals",
    "reviews",
    "retro",
    "design",
    "ops",
    "sales",
    "support",
    "hiring",
    "books",
    "games",
    "photos",
];
const MID: &[&str] = &[
    "active", "someday", "2025", "2026", "notes", "tasks", "docs", "drafts", "żółw", "łódź",
    "misc", "q1", "q2", "q3", "q4", "ref",
];
const LEAF: &[&str] = &[
    "alpha",
    "beta",
    "gamma",
    "delta",
    "kickoff",
    "retro",
    "roadmap",
    "budget",
    "plan",
    "spec",
    "design",
    "review",
    "notes",
    "zażółć",
    "gęślą",
    "jaźń",
    "projekt",
    "spotkanie",
    "lista",
    "pomysł",
    "cel",
    "raport",
    "checklist",
    "summary",
    "draft",
];
const WORDS: &[&str] = &[
    "the",
    "project",
    "meeting",
    "decision",
    "follow",
    "up",
    "with",
    "team",
    "about",
    "budget",
    "design",
    "review",
    "notes",
    "important",
    "later",
    "tomorrow",
    "idea",
    "question",
    "answer",
    "update",
    "status",
    "risk",
    "plan",
    "release",
    "bug",
    "feature",
    "customer",
    "feedback",
    "docs",
    "test",
    "deploy",
    "monitor",
    "owner",
    "deadline",
    "scope",
    "quality",
    "projekt",
    "spotkanie",
    "decyzja",
    "zadanie",
    "żółw",
    "łódź",
    "gęś",
    "źródło",
    "ćwiczenie",
    "świat",
    "pięć",
    "Łukasz",
    "sprawdzić",
    "zrobić",
    "przygotować",
    "zażółć",
    "jaźń",
    "mama",
    "tata",
    "zakupy",
    "obiad",
    "książka",
    "podróż",
    "się",
];
const TAGS: &[&str] = &[
    "urgent",
    "idea",
    "someday",
    "waiting",
    "home",
    "work",
    "health",
    "finance",
    "book",
    "movie",
    "recipe",
    "project",
    "client",
    "review",
    "meeting",
    "learning",
    "pilne",
    "dom",
    "praca",
    "zdrowie",
    "książka",
    "pomysł",
    "q1",
    "q2",
    "q3",
    "q4",
    "tool",
    "bug",
    "feature",
    "docs",
];

fn base36_id(n: usize) -> String {
    const A: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut v = n;
    let mut s = [b'0'; 6];
    for i in (0..6).rev() {
        s[i] = A[v % 36];
        v /= 36;
    }
    String::from_utf8_lossy(&s).into_owned()
}

struct Gen<'a> {
    rng: Rng,
    names: &'a [String],
    ids: Vec<String>,
    next_id: usize,
    report: SynthReport,
    today: NaiveDate,
}

impl Gen<'_> {
    fn link_target(&mut self) -> String {
        if self.rng.chance(0.004) {
            self.report.intentional_unresolved += 1;
            return format!("missing/{}-{}", self.rng.pick(LEAF), self.rng.below(50));
        }
        let u = self.rng.unit();
        let idx = ((u * u * u) * self.names.len() as f64) as usize;
        self.names
            .get(idx.min(self.names.len().saturating_sub(1)))
            .cloned()
            .unwrap_or_default()
    }

    fn sentence(&mut self) -> String {
        let n = 4 + self.rng.below(9);
        let mut s = String::new();
        for i in 0..n {
            if i > 0 {
                s.push(' ');
            }
            if self.rng.chance(0.06) && !self.names.is_empty() {
                let t = self.link_target();
                let _ = write!(s, "[[{t}]]");
            } else if self.rng.chance(0.03) {
                let _ = write!(s, "#{}", self.rng.pick(TAGS));
            } else {
                s.push_str(self.rng.pick(WORDS));
            }
        }
        if self.rng.chance(0.012) && !self.ids.is_empty() {
            let id = self.ids[self.rng.below(self.ids.len())].clone();
            let _ = write!(s, " (({id}))");
        }
        s
    }

    fn date(&mut self, spread: usize) -> String {
        let off = self.rng.below(spread * 2 + 1) as i64 - spread as i64;
        let d = if off >= 0 {
            self.today.checked_add_days(Days::new(off as u64))
        } else {
            self.today.checked_sub_days(Days::new((-off) as u64))
        };
        d.unwrap_or(self.today).format("%Y-%m-%d").to_string()
    }

    /// One block (first line, properties, optional code fence) at `depth`, as text with a trailing newline.
    fn block(&mut self, depth: usize) -> String {
        let ind = "  ".repeat(depth);
        let cont = format!("{ind}  ");
        let mut out = String::new();
        let mut text = self.sentence();
        if self.rng.chance(0.15) {
            let r = self.rng.below(100);
            let marker = match r {
                0..=59 => "TODO",
                60..=64 => "DOING",
                65..=69 => "WAITING",
                70..=94 => "DONE",
                _ => "CANCELED",
            };
            text = format!("{marker} {text}");
        }
        let is_task = text.starts_with(|c: char| c.is_ascii_uppercase())
            && ["TODO", "DOING", "WAITING", "DONE", "CANCELED"]
                .iter()
                .any(|m| text.starts_with(m));
        if self.rng.chance(0.01) {
            let id = base36_id(self.next_id);
            self.next_id += 1;
            let _ = write!(text, " ^{id}");
            self.ids.push(id);
        }
        let _ = writeln!(out, "{ind}- {text}");
        if is_task && self.rng.chance(0.4) {
            if self.rng.chance(0.5) {
                let d = self.date(60);
                let _ = writeln!(out, "{cont}scheduled:: {d}");
            }
            if self.rng.chance(0.6) {
                let d = self.date(60);
                let _ = writeln!(out, "{cont}deadline:: {d}");
            }
            if self.rng.chance(0.2) {
                let p = self.rng.pick(&["high", "medium", "low"]).to_string();
                let _ = writeln!(out, "{cont}priority:: {p}");
            }
        } else if self.rng.chance(0.08) {
            let s = self
                .rng
                .pick(&["active", "paused", "done", "draft"])
                .to_string();
            let _ = writeln!(out, "{cont}status:: {s}");
            if self.rng.chance(0.5) {
                let r = 1 + self.rng.below(5);
                let _ = writeln!(out, "{cont}rating:: {r}");
            }
        }
        if self.rng.chance(0.01) {
            let _ = writeln!(
                out,
                "{cont}```rust\n{cont}fn main() {{ println!(\"- not a block [[x]]\"); }}\n{cont}```"
            );
        }
        self.report.blocks += 1;
        out
    }

    fn body(&mut self, blocks: usize) -> String {
        let mut out = String::new();
        let mut depth = 0usize;
        for i in 0..blocks {
            if i > 0 {
                depth = match self.rng.below(10) {
                    0..=3 => depth,
                    4..=5 => (depth + 1).min(4),
                    6..=7 => depth.saturating_sub(1),
                    8 => depth.saturating_sub(2),
                    _ => 0,
                };
            }
            if i == 0 {
                depth = 0;
            }
            out.push_str(&self.block(depth));
        }
        out
    }
}

fn page_names(opts: &SynthOptions, rng: &mut Rng) -> Vec<String> {
    let mut names = Vec::with_capacity(opts.pages);
    for i in 0..opts.pages {
        let depth = match rng.below(100) {
            0..=39 => 1,
            40..=69 => 2,
            70..=84 => 3,
            85..=94 => 4,
            _ => 5,
        };
        let mut segs: Vec<String> = Vec::new();
        if depth > 1 {
            segs.push(rng.pick(TOP).to_string());
            for _ in 0..depth - 2 {
                segs.push(rng.pick(MID).to_string());
            }
        }
        segs.push(format!("{}-{i}", rng.pick(LEAF)));
        names.push(segs.join("/"));
    }
    names
}

/// Generates a graph into `out`. Returns what was written.
pub fn generate(opts: &SynthOptions, out: &Path) -> std::io::Result<SynthReport> {
    let mut name_rng = Rng(opts.seed ^ 0xa5a5_a5a5);
    let names = page_names(opts, &mut name_rng);
    let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap_or_default();
    let mut g = Gen {
        rng: Rng(opts.seed),
        names: &names,
        ids: Vec::new(),
        next_id: 0,
        report: SynthReport::default(),
        today,
    };

    let write = |rel: PathBuf, text: String| -> std::io::Result<()> {
        let p = out.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(p, text)
    };

    for (i, name) in names.iter().enumerate() {
        let blocks = if i % 2000 == 1999 {
            2000
        } else if g.rng.chance(0.01) {
            300 + g.rng.below(700)
        } else {
            2 + (g.rng.unit().powi(2) * 60.0) as usize
        };
        let mut text = String::new();
        if g.rng.chance(0.2) {
            let _ = writeln!(text, "---");
            let _ = writeln!(text, "title: {}", name.rsplit('/').next().unwrap_or(name));
            if g.rng.chance(0.5) {
                let _ = writeln!(text, "aliases: [alias-{i}]");
            }
            let _ = writeln!(text, "tags: [{}, {}]", g.rng.pick(TAGS), g.rng.pick(TAGS));
            let _ = writeln!(
                text,
                "status: {}",
                g.rng.pick(&["active", "paused", "done"])
            );
            let _ = writeln!(text, "---\n");
        }
        text.push_str(&g.body(blocks));
        if g.rng.chance(0.002) {
            let top = g.rng.pick(TOP).to_string();
            let _ = writeln!(
                text,
                "- ```query\n  task: todo\n  ns: {top}\n  sort: deadline asc\n  ```"
            );
            g.report.blocks += 1;
        }
        let segs: Vec<String> = name.split('/').map(String::from).collect();
        write(PathBuf::from(format!("pages/{}.md", segs.join("/"))), text)?;
        g.report.pages += 1;
    }

    for d in 0..opts.journal_days {
        let date = today.checked_sub_days(Days::new(d as u64)).unwrap_or(today);
        let n = 5 + g.rng.below(11);
        let text = g.body(n);
        let s = date.format("%Y-%m-%d").to_string();
        write(
            PathBuf::from(format!("journals/{}/{s}.md", date.format("%Y"))),
            text,
        )?;
        g.report.journals += 1;
    }
    Ok(g.report)
}
