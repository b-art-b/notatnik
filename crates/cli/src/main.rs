//! `notatnik`: command-line access to a graph (phase 1). Read commands accept `--json`.
//! Exit codes: 0 ok, 1 error, 2 `check` found issues.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use chrono::NaiveDate;
use clap::{Args, Parser, Subcommand};
use notatnik_core::{Block, parse_query};
use notatnik_graph::{Dirs, Graph, synth};
use notatnik_index::QueryContext;

#[derive(Parser)]
#[command(
    name = "notatnik",
    version,
    about = "Local-first block outliner: index, search and query a graph of Markdown files"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct Common {
    /// Print JSON instead of text
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new graph folder (pages/, journals/, .notatnik/config.toml)
    NewGraph {
        /// Folder to create
        path: PathBuf,
    },
    /// Bring the index up to date (or rebuild it from scratch)
    Index {
        /// Graph folder
        graph: PathBuf,
        /// Delete everything and index all files again
        #[arg(long)]
        rebuild: bool,
        #[command(flatten)]
        common: Common,
    },
    /// Full-text search (diacritic- and case-insensitive; `proj*` = prefix)
    Search {
        /// Graph folder
        graph: PathBuf,
        /// Search text
        text: String,
        /// Maximum number of results
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[command(flatten)]
        common: Common,
    },
    /// Run a query (text, or @file to read it from a file)
    Query {
        /// Graph folder
        graph: PathBuf,
        /// Query text, or @path
        query: String,
        /// Treat this date as today (YYYY-MM-DD); default: the system date
        #[arg(long)]
        today: Option<String>,
        #[command(flatten)]
        common: Common,
    },
    /// List everything that links to a page
    Backlinks {
        /// Graph folder
        graph: PathBuf,
        /// Page name
        page: String,
        #[command(flatten)]
        common: Common,
    },
    /// Print a page's parsed block tree
    Page {
        /// Graph folder
        graph: PathBuf,
        /// Page name
        page: String,
        #[command(flatten)]
        common: Common,
    },
    /// Health check: parse problems, conflict copies, dangling block refs, re-IDs (exit 2 if any problem)
    Check {
        /// Graph folder
        graph: PathBuf,
        #[command(flatten)]
        common: Common,
    },
    /// Rename a page and rewrite every reference to it
    Rename {
        /// Graph folder
        graph: PathBuf,
        /// Current page name
        old: String,
        /// New page name
        new: String,
        /// Only show what would change
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        common: Common,
    },
    /// Watch the graph folder and print an update for every external change
    Watch {
        /// Graph folder
        graph: PathBuf,
    },
    /// Decrypt an encrypted backup (`*.zip.enc`) without the app
    ///
    /// The password or the recovery key comes from the environment variable NOTATNIK_PASSWORD, or is asked for.
    Decrypt {
        /// The encrypted file
        file: PathBuf,
        /// Where to write the zip (default: the file name without `.enc`)
        #[arg(long, short)]
        out: Option<PathBuf>,
        /// Replace the output file if it exists
        #[arg(long)]
        force: bool,
    },
    /// Developer tools
    #[command(subcommand, hide = true)]
    Dev(Dev),
}

#[derive(Subcommand)]
enum Dev {
    /// Generate a deterministic synthetic graph for benchmarks
    GenGraph {
        /// PRNG seed
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Number of pages
        #[arg(long, default_value_t = 10_000)]
        pages: usize,
        /// Number of journal days
        #[arg(long, default_value_t = 730)]
        journal_days: usize,
        /// Output folder
        #[arg(long)]
        out: PathBuf,
    },
}

fn open(path: &Path) -> Result<Graph, String> {
    Graph::open(path, &Dirs::from_env())
        .map(|(g, _)| g)
        .map_err(|e| e.to_string())
}

fn print_json<T: serde::Serialize>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn outline(b: &Block, src: &str, out: &mut String) {
    use std::fmt::Write;
    let _ = write!(
        out,
        "{}- {}",
        "  ".repeat(b.level),
        b.edit_text(src).lines().next().unwrap_or("")
    );
    if let Some(id) = &b.block_id {
        let _ = write!(out, "   ^{}", id.id);
    }
    let _ = writeln!(out);
    for p in &b.properties {
        let _ = writeln!(out, "{}  {}:: {}", "  ".repeat(b.level), p.key, p.raw);
    }
    for c in &b.children {
        outline(c, src, out);
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::NewGraph { path } => {
            let (g, _) = Graph::create(&path, &Dirs::from_env()).map_err(|e| e.to_string())?;
            println!("created graph {} at {}", g.id(), g.root().display());
        }
        Command::Index {
            graph,
            rebuild,
            common,
        } => {
            let g = open(&graph)?;
            let root = g.root().to_path_buf();
            if rebuild {
                g.with_index_mut(|i| i.index_all(&root))
                    .map_err(|e| e.to_string())?;
            }
            let counts = g.with_index(|i| i.counts()).map_err(|e| e.to_string())?;
            if common.json {
                print_json(&counts);
            } else {
                println!(
                    "{} pages ({} virtual), {} blocks",
                    counts.pages, counts.virtual_pages, counts.blocks
                );
            }
        }
        Command::Search {
            graph,
            text,
            limit,
            common,
        } => {
            let g = open(&graph)?;
            let hits = g
                .with_index(|i| i.search(&text, limit))
                .map_err(|e| e.to_string())?;
            if common.json {
                print_json(&hits);
            } else {
                for h in &hits {
                    match h.path.as_deref() {
                        None => println!("[page] {}", h.page),
                        Some(p) => {
                            println!("{}#{}: {}", h.page, p, h.text.lines().next().unwrap_or(""))
                        }
                    }
                }
                if hits.is_empty() {
                    eprintln!("no results");
                }
            }
        }
        Command::Query {
            graph,
            query,
            today,
            common,
        } => {
            let text = match query.strip_prefix('@') {
                Some(f) => std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?,
                None => query,
            };
            let q = parse_query(&text).map_err(|errs| {
                errs.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            let g = open(&graph)?;
            let today = match today {
                Some(t) => NaiveDate::parse_from_str(&t, "%Y-%m-%d")
                    .map_err(|_| format!("invalid --today `{t}` (use YYYY-MM-DD)"))?,
                None => chrono::Local::now().date_naive(),
            };
            let ctx = QueryContext {
                today,
                week_start: g.config().week_start(),
            };
            let res = g
                .with_index(|i| i.run_query(&q, &ctx))
                .map_err(|e| e.to_string())?;
            if common.json {
                print_json(&res);
            } else {
                for r in &res.rows {
                    match &r.text {
                        Some(t) => println!("{}: {}", r.id, t.lines().next().unwrap_or("")),
                        None => println!("{}", r.id),
                    }
                }
                eprintln!("{} result(s)", res.rows.len());
            }
        }
        Command::Backlinks {
            graph,
            page,
            common,
        } => {
            let g = open(&graph)?;
            let links = g
                .with_index(|i| i.backlinks(&page))
                .map_err(|e| e.to_string())?;
            if common.json {
                print_json(&links);
            } else {
                for l in &links {
                    let loc = l
                        .block_path
                        .as_deref()
                        .map_or(String::from("(page)"), |p| format!("#{p}"));
                    let crumbs = if l.breadcrumbs.is_empty() {
                        String::new()
                    } else {
                        format!("{} > ", l.breadcrumbs.join(" > "))
                    };
                    println!(
                        "{}{loc}: {crumbs}{}",
                        l.source_page,
                        l.text.lines().next().unwrap_or("")
                    );
                }
                eprintln!("{} backlink(s)", links.len());
            }
        }
        Command::Page {
            graph,
            page,
            common,
        } => {
            let g = open(&graph)?;
            let info = g
                .with_index(|i| i.page_info(&page))
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("no page `{page}`"))?;
            let rel = info
                .file_path
                .ok_or_else(|| format!("`{}` has no file (it is only referenced)", info.name))?;
            let file = g.read_page(&rel).map_err(|e| e.to_string())?;
            let parsed = notatnik_core::parse_page_with(&file.text, &g.config().parse_options());
            if common.json {
                print_json(&parsed);
            } else {
                let mut out = String::new();
                if let Some(fm) = &parsed.frontmatter {
                    out.push_str(&format!(
                        "(frontmatter{})\n",
                        if fm.valid { "" } else { ", invalid" }
                    ));
                }
                if let Some(p) = &parsed.preamble {
                    out.push_str(&format!(
                        "{}\n",
                        p.edit_text(&file.text).lines().next().unwrap_or("")
                    ));
                }
                for b in &parsed.blocks {
                    outline(b, &file.text, &mut out);
                }
                print!("{out}");
                for d in &parsed.diagnostics {
                    eprintln!("warning: {} (at byte {})", d.message, d.span.start);
                }
            }
        }
        Command::Check { graph, common } => {
            let g = open(&graph)?;
            let rep = g.check().map_err(|e| e.to_string())?;
            if common.json {
                print_json(&rep);
            } else {
                for i in &rep.issues {
                    println!("{}: {}: {} [{}]", i.path, i.severity, i.message, i.code);
                }
                for c in &rep.conflicts {
                    println!("{c}: conflict copy (not indexed); merge it by hand");
                }
                for u in &rep.unmapped {
                    println!("{u}: markdown file that doesn't map to a page name (ignored)");
                }
                for (p, b, id) in &rep.dangling_block_refs {
                    println!("{p}#{b}: reference to unknown block id (({id}))");
                }
                for r in &rep.reids {
                    println!(
                        "{}: duplicate block id {} was replaced by {}",
                        r.path, r.old, r.new
                    );
                }
                if !rep.unresolved_links.is_empty() {
                    eprintln!(
                        "{} link(s) point to pages that don't exist yet",
                        rep.unresolved_links.len()
                    );
                }
                if !rep.has_problems() {
                    println!("0 issues");
                }
            }
            if rep.has_problems() {
                return Ok(ExitCode::from(2));
            }
        }
        Command::Rename {
            graph,
            old,
            new,
            dry_run,
            common,
        } => {
            let g = open(&graph)?;
            let plan = g.plan_rename(&old, &new).map_err(|e| e.to_string())?;
            if common.json || dry_run {
                if common.json {
                    print_json(&serde_json::json!({
                        "old": plan.old, "new": plan.new, "file_moves": plan.file_moves,
                        "edits": plan.edits.iter().map(|e| serde_json::json!({"path": e.path, "replacements": e.patches.len()})).collect::<Vec<_>>(),
                        "warnings": plan.warnings, "dry_run": dry_run,
                    }));
                } else {
                    println!("rename `{}` -> `{}`", plan.old, plan.new);
                    for (a, b) in &plan.file_moves {
                        println!("  move {a} -> {b}");
                    }
                    for e in &plan.edits {
                        println!("  rewrite {} ({} reference(s))", e.path, e.patches.len());
                        for p in &e.patches {
                            println!("      {:?} -> {:?}", p.old_text, p.new_text);
                        }
                    }
                    for w in &plan.warnings {
                        println!("  warning: {w}");
                    }
                    println!("(dry run: nothing was changed)");
                }
                if dry_run {
                    return Ok(ExitCode::SUCCESS);
                }
            }
            let report = g.apply_rename(&plan).map_err(|e| e.to_string())?;
            if !common.json {
                println!(
                    "renamed `{}` -> `{}`: {} file(s) rewritten, {} moved",
                    plan.old,
                    plan.new,
                    report.rewritten.len(),
                    report.moved.len()
                );
                for w in &plan.warnings {
                    eprintln!("warning: {w}");
                }
            }
        }
        Command::Watch { graph } => {
            let g = open(&graph)?;
            let w = g
                .watch(Duration::from_millis(200))
                .map_err(|e| e.to_string())?;
            eprintln!("watching {} (Ctrl+C to stop)", g.root().display());
            while let Ok(ev) = w.events().recv() {
                println!("changed: {}", ev.pages.join(", "));
            }
        }
        Command::Decrypt { file, out, force } => decrypt(&file, out, force)?,
        Command::Dev(Dev::GenGraph {
            seed,
            pages,
            journal_days,
            out,
        }) => {
            let rep = synth::generate(
                &synth::SynthOptions {
                    seed,
                    pages,
                    journal_days,
                },
                &out,
            )
            .map_err(|e| e.to_string())?;
            print_json(&rep);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `notatnik decrypt`: opens a sealed file with the password or recovery key and writes what is inside.
fn decrypt(file: &Path, out: Option<PathBuf>, force: bool) -> Result<(), String> {
    use std::io::Write;
    let out = match out {
        Some(o) => o,
        None => match file.to_str().and_then(|f| f.strip_suffix(".enc")) {
            Some(p) => PathBuf::from(p),
            None => {
                return Err(
                    "give the output file with --out (the name does not end in .enc)".into(),
                );
            }
        },
    };
    if out.exists() && !force {
        return Err(format!(
            "{} exists (use --force to replace it)",
            out.display()
        ));
    }
    let open_file = || std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()));
    // the descriptor sits in the file's header: the file and the password are all that is needed
    let header = notatnik_crypt::read_header(&mut std::io::BufReader::new(open_file()?))
        .map_err(|e| e.to_string())?;
    let secret = match std::env::var("NOTATNIK_PASSWORD") {
        Ok(s) if !s.is_empty() => s,
        _ => rpassword::prompt_password("Password or recovery key: ").map_err(|e| e.to_string())?,
    };
    let keys = header.vault.unlock(&secret).map_err(|e| e.to_string())?;
    let partial = out.with_extension("partial");
    let result = (|| -> Result<(), String> {
        let mut w = std::io::BufWriter::new(
            std::fs::File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?,
        );
        notatnik_crypt::open(&keys, &mut std::io::BufReader::new(open_file()?), &mut w)
            .map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        w.get_ref().sync_all().map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    std::fs::rename(&partial, &out).map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    Ok(())
}

fn main() -> ExitCode {
    // the commands print their own results; `RUST_LOG=notatnik_graph=debug` shows what the library does
    notatnik_log::init("warn");
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(1)
        }
    }
}
