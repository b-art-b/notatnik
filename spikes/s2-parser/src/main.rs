//! Spike S2: byte spans from pulldown-cmark / comrak vs a hand-written line-based outline parser.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::time::Instant;

const CRLF: &str = "- Parent ^abc123\r\n  key:: value\r\n  - Child with [[Link]] and #tag\r\n  - Second `[[not]]`\r\n";
const TABS: &str = "- Parent\n\t- Child tab\n\t\t- Grand\n\t- Child 2\n";
const FENCE: &str = "- Code block:\n  ```md\n  - not a block\n  [[x]] #nope\n  ```\n- Next [[real]]\n";

#[derive(Debug)]
struct Blk { start: usize, end: usize, depth: usize }

/// Hand-written: a block starts at a line whose indent+`- ` matches; fences are tracked so `- ` inside is text.
fn outline(src: &str) -> Vec<Blk> {
    let mut out: Vec<Blk> = Vec::new();
    let mut pos = 0;
    let mut fence: Option<(char, usize)> = None;
    let mut unit: Option<usize> = None;
    for line in src.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let trimmed = body.trim_start_matches([' ', '\t']);
        let indent = &body[..body.len() - trimmed.len()];
        if let Some((c, n)) = fence {
            let f = trimmed.chars().take_while(|&x| x == c).count();
            if f >= n && trimmed[f..].trim().is_empty() { fence = None; }
        } else {
            let f = trimmed.chars().take_while(|&x| x == '`' || x == '~').count();
            if f >= 3 { fence = Some((trimmed.chars().next().unwrap(), f)); }
        }
        let is_item = fence_start_ok(&fence, trimmed) && trimmed.starts_with("- ");
        if is_item {
            let w = if indent.contains('\t') { indent.matches('\t').count() } else { indent.len() };
            if w > 0 && unit.is_none() { unit = Some(w); }
            let depth = if w == 0 { 0 } else { w / unit.unwrap_or(1) };
            if let Some(l) = out.last_mut() { l.end = pos; }
            out.push(Blk { start: pos, end: src.len(), depth });
        }
        pos += line.len();
    }
    out
}
fn fence_start_ok(f: &Option<(char, usize)>, t: &str) -> bool {
    // an opening fence line itself is never an item; lines inside a fence are not items
    f.is_none() && !(t.starts_with("```") || t.starts_with("~~~"))
}

fn show(name: &str, src: &str) {
    println!("== {name}");
    println!("-- hand-written outline");
    for b in outline(src) { println!("  d{} {:?}", b.depth, &src[b.start..b.end]); }
    println!("-- pulldown-cmark list items");
    for (ev, r) in Parser::new_ext(src, Options::empty()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Item) => println!("  item  {:?}", &src[r.clone()]),
            Event::Code(_) => println!("  code  {:?}", &src[r]),
            Event::Text(t) if t.contains("[[") => println!("  text  {:?} @{:?}", &*t, r),
            _ => {}
        }
        let _ = TagEnd::Item;
    }
    println!("-- comrak list items");
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, src, &comrak::Options::default());
    for n in root.descendants() {
        let d = n.data.borrow();
        if matches!(d.value, comrak::nodes::NodeValue::Item(_)) {
            println!("  item  line {}:{}-{}:{}", d.sourcepos.start.line, d.sourcepos.start.column, d.sourcepos.end.line, d.sourcepos.end.column);
        }
    }
}

fn main() {
    show("crlf", CRLF); show("tabs", TABS); show("fence", FENCE);
    let mut big = String::new();
    for i in 0..2000 { big.push_str(&format!("{}- Block {i} with [[Link {i}]] and #tag `code` **bold**\n", "  ".repeat(i % 4))); }
    let t = Instant::now(); let n = outline(&big).len(); println!("hand outline 2000: {n} blocks {:?}", t.elapsed());
    let t = Instant::now(); let n = Parser::new_ext(&big, Options::empty()).into_offset_iter().count(); println!("pulldown 2000: {n} events {:?}", t.elapsed());
    let t = Instant::now(); let arena = comrak::Arena::new(); let r = comrak::parse_document(&arena, &big, &comrak::Options::default()); println!("comrak 2000: {} nodes {:?}", r.descendants().count(), t.elapsed());
}
