//! Page templates (NS-10, PR-10): `templates/<type>.md` in the graph. A template's frontmatter is the property
//! skeleton and its blocks are the starting content. Pure functions; the file access is in `state.rs`.

use notatnik_core::{Block, BlockOp, parse_page};

/// What a template contributes to a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// The template's own `type` (raw YAML), set on a new page that does not have one.
    pub ty: Option<String>,
    /// `(key, raw YAML value)` in file order; an empty value means "show the field, empty".
    pub props: Vec<(String, String)>,
    /// `(parent path, index, text)` in insertion order (a block's parent comes before its children).
    pub blocks: Vec<(Vec<usize>, usize, String)>,
    /// Where `{{cursor}}` stood: the position in `blocks` and the offset in characters into that block's text. The
    /// first one counts.
    pub cursor: Option<(usize, usize)>,
}

/// The values the `{{...}}` variables stand for.
#[derive(Clone, Copy)]
pub struct Vars<'a> {
    pub title: &'a str,
    pub date: &'a str,
    /// The time of day, `HH:MM` (`{{time}}`); empty where there is none.
    pub time: &'a str,
    /// The page's own properties, `(lower-case key, text)`: `{{prop.<key>}}` and `{{type}}`. Empty for a page that
    /// does not exist yet.
    pub props: &'a [(String, String)],
    /// `{{selection}}` and `{{clipboard}}`: what `/template` is given; empty elsewhere.
    pub selection: &'a str,
    pub clipboard: &'a str,
    /// The answers to `{{ask:Label}}`, `(label, text)`; a question without one gets its default, else nothing.
    pub answers: &'a [(String, String)],
}

/// A question of a template: `{{ask:Label}}` or `{{ask:Label|default}}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub label: String,
    pub default: String,
    /// `{{askpage:Label}}`: the answer is a page, picked from the existing ones or a new name.
    pub page: bool,
}

/// `Label`, `default`, is-a-page of the placeholder `name` if it is a question (the text between the braces):
/// `ask:` or `askpage:`.
fn parse_ask(name: &str) -> Option<(&str, &str, bool)> {
    let rest = name.trim_start();
    let (rest, page) = if rest
        .get(..4)
        .is_some_and(|p| p.eq_ignore_ascii_case("ask:"))
    {
        (&rest[4..], false)
    } else if rest
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("askpage:"))
    {
        (&rest[8..], true)
    } else {
        return None;
    };
    let (label, default) = rest.split_once('|').unwrap_or((rest, ""));
    let label = label.trim();
    (!label.is_empty()).then_some((label, default.trim(), page))
}

/// The distinct questions of `text`, in the order they first appear (a label asked twice is asked once).
pub fn asks(text: &str) -> Vec<Ask> {
    let mut out: Vec<Ask> = Vec::new();
    notatnik_core::placeholders::expand_with(text, |name| {
        if let Some((label, default, page)) = parse_ask(name) {
            if !out.iter().any(|a| a.label == label) {
                out.push(Ask {
                    label: label.to_string(),
                    default: default.to_string(),
                    page,
                });
            }
        }
        None
    });
    out
}

/// A property value as text for a placeholder: links as `[[Name]]`, lists joined with `, `.
pub fn prop_text(v: &notatnik_core::PropertyValue) -> String {
    use notatnik_core::PropertyValue as P;
    match v {
        P::Text(s) => s.clone(),
        P::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{n:.0}"),
        P::Number(n) => n.to_string(),
        P::Bool(b) => b.to_string(),
        P::Date(d) => d.format("%Y-%m-%d").to_string(),
        P::DateTime(d) => d.format("%Y-%m-%d %H:%M").to_string(),
        P::Links(l) => l
            .iter()
            .map(|n| format!("[[{n}]]"))
            .collect::<Vec<_>>()
            .join(", "),
        P::List(items) => items.iter().map(prop_text).collect::<Vec<_>>().join(", "),
    }
}

/// The properties of a page file (frontmatter, valid or not: an invalid one has none), for [`Vars::props`].
pub fn page_props(text: &str) -> Vec<(String, String)> {
    parse_page(text)
        .frontmatter
        .filter(|f| f.valid)
        .map(|f| {
            f.entries
                .iter()
                .map(|e| (e.key.to_lowercase(), prop_text(&e.value)))
                .collect()
        })
        .unwrap_or_default()
}

/// `{{type}}` and `{{prop.<key>}}`: the value in `props`, empty when the page has none. Other names are not ours.
pub fn expand_props(text: &str, props: &[(String, String)]) -> String {
    notatnik_core::placeholders::expand_with(text, |name| {
        let name = name.trim().to_lowercase();
        let key = if name == "type" {
            "type"
        } else {
            name.strip_prefix("prop.")?.trim()
        };
        Some(
            props
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
        )
    })
}

/// The local time of day as `HH:MM`, for `{{time}}`.
pub fn now_time() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

/// The date and time placeholders (`{{date+7d}}`, `{{tomorrow}}`, `{{monday}}`, `{{time}}`...), relative to `date`.
fn dates(text: &str, date: &str, time: &str) -> String {
    match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(day) => notatnik_core::placeholders::expand_dates(
            text,
            day,
            Some(time).filter(|t| !t.is_empty()),
        ),
        Err(_) => text.to_string(),
    }
}

fn fill(text: &str, v: &Vars) -> String {
    let namespace = v.title.rsplit_once('/').map_or("", |(ns, _)| ns);
    let leaf = v.title.rsplit('/').next().unwrap_or(v.title);
    let filled = text
        .replace("{{title}}", leaf)
        .replace("{{namespace}}", namespace)
        .replace("{{cursor}}", "");
    // last, so that text taken from the page or the clipboard is never read as a placeholder itself
    let done = dates(&expand_props(&filled, v.props), v.date, v.time);
    notatnik_core::placeholders::expand_with(&done, |name| {
        match name.trim().to_lowercase().as_str() {
            "selection" => Some(v.selection.to_string()),
            "clipboard" => Some(v.clipboard.to_string()),
            _ => {
                let (label, default, _) = parse_ask(name)?;
                Some(
                    v.answers
                        .iter()
                        .find(|(l, _)| l == label)
                        .map_or(default, |(_, a)| a.as_str())
                        .to_string(),
                )
            }
        }
    })
}

fn collect(
    src: &str,
    blocks: &[Block],
    parent: &[usize],
    v: &Vars,
    out: &mut Vec<(Vec<usize>, usize, String)>,
    cursor: &mut Option<(usize, usize)>,
) {
    for (i, b) in blocks.iter().enumerate() {
        let raw = b.edit_text(src);
        if cursor.is_none() {
            if let Some((before, _)) = raw.split_once("{{cursor}}") {
                *cursor = Some((out.len(), fill(before, v).chars().count()));
            }
        }
        out.push((parent.to_vec(), i, fill(&raw, v)));
        let mut path = parent.to_vec();
        path.push(i);
        collect(src, &b.children, &path, v, out, cursor);
    }
}

/// Reads a template file. Properties with multi-line values are skipped (a property is set as one line).
pub fn parse_template(text: &str, v: &Vars) -> Template {
    let page = parse_page(text);
    // `{{type}}` is the page's type; a page that has none yet gets the template's own
    let own_type = page
        .frontmatter
        .as_ref()
        .filter(|f| f.valid)
        .and_then(|f| {
            f.entries
                .iter()
                .find(|e| e.key.eq_ignore_ascii_case("type"))
        })
        .map(|e| prop_text(&e.value))
        .filter(|t| !t.is_empty());
    let mut with_type = v.props.to_vec();
    if let Some(t) = own_type.filter(|_| !with_type.iter().any(|(k, _)| k == "type")) {
        with_type.push(("type".into(), t));
    }
    let v = &Vars {
        props: &with_type,
        ..*v
    };
    let props = page
        .frontmatter
        .as_ref()
        .filter(|f| f.valid)
        .map(|f| {
            f.entries
                .iter()
                .filter(|e| !e.key.eq_ignore_ascii_case("type") && !e.raw.contains('\n'))
                .map(|e| (e.key.clone(), fill(&e.raw, v)))
                .collect()
        })
        .unwrap_or_default();
    let mut blocks = Vec::new();
    let mut cursor = None;
    collect(text, &page.blocks, &[], v, &mut blocks, &mut cursor);
    let ty = page
        .frontmatter
        .as_ref()
        .filter(|f| f.valid)
        .and_then(|f| {
            f.entries
                .iter()
                .find(|e| e.key.eq_ignore_ascii_case("type"))
        })
        .map(|e| e.raw.clone())
        .filter(|r| !r.is_empty());
    Template {
        ty,
        props,
        blocks,
        cursor,
    }
}

/// A template named in the config: `person`, `person.md` or `templates/person.md` all mean `templates/person.md`.
pub fn template_name(configured: &str) -> Option<String> {
    let t = configured.trim();
    let t = t.strip_prefix("templates/").unwrap_or(t);
    let t = t.strip_suffix(".md").unwrap_or(t);
    (!t.is_empty() && !t.contains(['/', '\\']) && !t.starts_with('.')).then(|| t.to_string())
}

/// The first value of a `type` property as a template name: quotes, `[[ ]]` and a leading `#` removed.
pub fn type_name(raw: &str) -> Option<String> {
    let first = raw.trim().trim_start_matches('[').split(',').next()?;
    let t = first
        .trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '[' | ']'))
        .trim_start_matches('#')
        .trim();
    (!t.is_empty() && !t.contains(['/', '\\']) && t != "." && t != "..").then(|| t.to_string())
}

/// The operations that bring a page to the template: every template property the page lacks (`have`: its keys,
/// lowercase) and, for a page without blocks, the template's blocks. Returns the ops and the keys added.
pub fn ops_for(
    t: &Template,
    have: &[String],
    with_blocks: bool,
    with_type: bool,
) -> (Vec<BlockOp>, Vec<String>) {
    let mut ops = Vec::new();
    let mut added = Vec::new();
    if let (true, Some(ty)) = (with_type, &t.ty) {
        if !have.iter().any(|k| k == "type") {
            ops.push(BlockOp::SetPageProperty {
                key: "type".into(),
                value: ty.clone(),
            });
            added.push("type".to_string());
        }
    }
    for (k, v) in &t.props {
        if !have.contains(&k.to_lowercase()) {
            ops.push(BlockOp::SetPageProperty {
                key: k.clone(),
                value: v.clone(),
            });
            added.push(k.clone());
        }
    }
    if with_blocks {
        for (parent, index, text) in &t.blocks {
            ops.push(BlockOp::Insert {
                parent: parent.clone(),
                index: *index,
                text: text.clone(),
            });
        }
    }
    (ops, added)
}

/// Keys of a page that a template never carries: they belong to that one page.
const PAGE_ONLY: [&str; 4] = ["title", "aliases", "archived", "template"];

/// The text of a template made from a page: its `type`, its other single-line properties (the values only when
/// `keep_values`) and its blocks without block IDs. `None` when the page has no usable `type`.
pub fn render(src: &str, keep_values: bool) -> Option<(String, String)> {
    let page = parse_page(src);
    let fm = page.frontmatter.as_ref().filter(|f| f.valid)?;
    let ty_raw = &fm
        .entries
        .iter()
        .find(|e| e.key.eq_ignore_ascii_case("type"))?
        .raw;
    let name = type_name(ty_raw)?;
    let mut out = format!("---\ntype: {ty_raw}\n");
    for e in &fm.entries {
        let skip = e.key.eq_ignore_ascii_case("type")
            || PAGE_ONLY.iter().any(|k| e.key.eq_ignore_ascii_case(k))
            || e.raw.contains('\n');
        if skip {
            continue;
        }
        if keep_values && !e.raw.is_empty() {
            out.push_str(&format!("{}: {}\n", e.key, e.raw));
        } else {
            out.push_str(&format!("{}:\n", e.key));
        }
    }
    out.push_str("---\n");
    fn blocks(src: &str, bs: &[Block], depth: usize, out: &mut String) {
        for b in bs {
            let pad = "  ".repeat(depth);
            let text = b.edit_text(src);
            for (i, line) in text.split('\n').enumerate() {
                out.push_str(&pad);
                out.push_str(if i == 0 { "- " } else { "  " });
                out.push_str(line);
                out.push('\n');
            }
            blocks(src, &b.children, depth + 1, out);
        }
    }
    blocks(src, &page.blocks, 0, &mut out);
    Some((name, out))
}

/// A variable that needs a page, used in a query that is not on one: the position (1-based line and column) and
/// the message.
pub type QueryVarError = (usize, usize, String);

/// Fills the variables of a query (QV-12): `{{page}}` is a link to the page the query block sits on (`[[Page]]`; also
/// inside brackets, `[[{{page}}]]`), `{{title}}` its last name segment, `{{namespace}}` the part before it and
/// `{{date}}` today. The empty link `[[]]` still means the page. Without a page (agenda, saved queries) the first
/// three are an error, so a query never silently matches nothing.
pub fn expand_query(
    text: &str,
    page: Option<&str>,
    today: &str,
    props: &[(String, String)],
) -> Result<String, QueryVarError> {
    let Some(p) = page else {
        for var in [
            "{{page}}",
            "{{title}}",
            "{{namespace}}",
            "{{type}}",
            "{{prop.",
        ] {
            if let Some(at) = text.find(var) {
                let before = &text[..at];
                let line = before.matches('\n').count() + 1;
                let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
                return Err((
                    line,
                    col,
                    format!(
                        "`{}` needs a page: use it in a query block on a page",
                        if var == "{{prop." {
                            "{{prop.<key>}}"
                        } else {
                            var
                        }
                    ),
                ));
            }
        }
        return Ok(dates(text, today, ""));
    };
    let link = format!("[[{p}]]");
    let leaf = p.rsplit('/').next().unwrap_or(p);
    let ns = p.rsplit_once('/').map_or("", |(ns, _)| ns);
    let filled = text
        .replace("[[{{page}}]]", &link)
        .replace("{{page}}", &link)
        .replace("[[]]", &link)
        .replace("{{title}}", leaf)
        .replace("{{namespace}}", ns);
    Ok(dates(&expand_props(&filled, props), today, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: Vars = Vars {
        title: "people/Jan",
        date: "2026-10-11",
        time: "09:05",
        props: &[],
        selection: "",
        clipboard: "",
        answers: &[],
    };

    #[test]
    fn date_placeholders_work_in_templates_and_queries() {
        let t = parse_template(
            "---\ntype: x\ndue: {{date+7d}}\n---\n- At {{time}} on {{weekday}}, review {{next monday}}\n",
            &V,
        );
        assert_eq!(t.props, vec![("due".into(), "2026-10-18".into())]);
        assert_eq!(t.blocks[0].2, "At 09:05 on Sunday, review 2026-10-12");
        // in a query, relative to today; with or without a page
        for page in [Some("a"), None] {
            assert_eq!(
                expand_query(
                    "deadline: {{tomorrow}}\nscheduled: {{date-1w}}",
                    page,
                    "2026-10-11",
                    &[]
                )
                .unwrap(),
                "deadline: 2026-10-12\nscheduled: 2026-10-04"
            );
        }
        // no time of day in a query
        assert_eq!(
            expand_query("x: {{time}}", None, "2026-10-11", &[]).unwrap(),
            "x: {{time}}"
        );
    }

    #[test]
    fn type_and_page_properties_fill_in() {
        let props = page_props(
            "---\ntype: person\npartner: \"[[part. EY]]\"\nn: 3\ntags: [a, b]\n---\n- x\n",
        );
        let get = |k: &str| {
            props
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get("type").as_deref(), Some("person"));
        assert_eq!(get("n").as_deref(), Some("3"));
        assert_eq!(
            expand_props(
                "{{type}}|{{ prop.Partner }}|{{prop.n}}|{{prop.zz}}|{{title}}",
                &props
            ),
            "person|[[part. EY]]|3||{{title}}"
        );
        // in a template: the page's value wins, the template's own type is the fallback
        let tpl = "---\ntype: meeting\n---\n- {{type}} with {{prop.partner}}\n";
        let page = |props: &[(String, String)]| {
            parse_template(tpl, &Vars { props, ..V }).blocks[0]
                .2
                .clone()
        };
        assert_eq!(page(&props), "person with [[part. EY]]");
        assert_eq!(page(&[]), "meeting with ");
        // a query
        assert_eq!(
            expand_query(
                "prop.partner: {{prop.partner}}",
                Some("a"),
                "2026-10-11",
                &props
            )
            .unwrap(),
            "prop.partner: [[part. EY]]"
        );
        assert!(expand_query("x: {{type}}", None, "2026-10-11", &[]).is_err());
    }

    #[test]
    fn questions_are_listed_once_and_answered_last() {
        let tpl = "- Meeting with {{ask:Who}} about {{ Ask: Topic | status }}\n  - {{ask:Who}} said {{cursor}}\n";
        assert_eq!(
            asks(tpl),
            vec![
                Ask {
                    label: "Who".into(),
                    default: String::new(),
                    page: false
                },
                Ask {
                    label: "Topic".into(),
                    default: "status".into(),
                    page: false
                },
            ]
        );
        assert!(asks("{{ask:}} {{ask}} {{date}}").is_empty());
        // a page question; asked once when the same label is also a plain question
        assert_eq!(
            asks("{{askpage:Partner}} {{ask:Partner}}"),
            vec![Ask {
                label: "Partner".into(),
                default: String::new(),
                page: true
            }]
        );
        let answers = vec![("Who".to_string(), "Ann {{date}}".to_string())];
        let t = parse_template(
            tpl,
            &Vars {
                answers: &answers,
                ..V
            },
        );
        // an answer is text, never a placeholder; an unanswered question gets its default
        assert_eq!(t.blocks[0].2, "Meeting with Ann {{date}} about status");
        assert_eq!(t.blocks[1].2, "Ann {{date}} said ");
        assert_eq!(t.cursor, Some((1, 18)));
        // nothing answered: defaults, else empty
        assert_eq!(
            parse_template(tpl, &V).blocks[0].2,
            "Meeting with  about status"
        );
    }

    #[test]
    fn the_first_cursor_marker_is_remembered_by_block_and_offset() {
        let t = parse_template(
            "- Meeting {{title}}\n  - With: {{cursor}} and more\n  - Later {{cursor}}\n",
            &V,
        );
        assert_eq!(t.cursor, Some((1, 6)));
        assert_eq!(t.blocks[1].2, "With:  and more");
        assert_eq!(parse_template("- plain\n", &V).cursor, None);
    }

    #[test]
    fn reads_properties_and_nested_blocks_with_variables() {
        let t = parse_template(
            "---\ntype: person\nimportance: 1\npartner:\nadded: {{date}}\n---\n- Contact {{title}}\n  - Phone\n- Notes {{cursor}}\n",
            &V,
        );
        assert_eq!(
            t.props,
            vec![
                ("importance".into(), "1".into()),
                ("partner".into(), String::new()),
                ("added".into(), "2026-10-11".into()),
            ]
        );
        assert_eq!(
            t.blocks,
            vec![
                (vec![], 0, "Contact Jan".into()),
                (vec![0], 0, "Phone".into()),
                (vec![], 1, "Notes ".into()),
            ]
        );
    }

    #[test]
    fn missing_properties_only_and_blocks_only_for_an_empty_page() {
        let t = parse_template("---\na: 1\nb: 2\n---\n- x\n", &V);
        let (ops, added) = ops_for(&t, &["a".into()], false, false);
        assert_eq!(added, vec!["b"]);
        assert_eq!(ops.len(), 1);
        let (ops, _) = ops_for(&t, &[], true, false);
        assert_eq!(ops.len(), 3);
    }

    #[test]
    fn a_page_becomes_a_template() {
        let src = "---\ntype: person\ntitle: Jan\nimportance: 3\npartner: \"[[Acme]]\"\n---\n- Contact ^k3x9qa\n  - Phone\n- Notes\n  more\n";
        let (name, text) = render(src, true).unwrap();
        assert_eq!(name, "person");
        assert!(
            text.starts_with("---\ntype: person\nimportance: 3\npartner: \"[[Acme]]\"\n---\n"),
            "{text}"
        );
        assert!(text.contains("- Contact"), "{text}");
        assert!(
            !text.contains("^k3x9qa") && !text.contains("title:"),
            "{text}"
        );
        assert!(text.contains("  - Phone\n- Notes\n  more\n"), "{text}");
        let (_, empty) = render(src, false).unwrap();
        assert!(
            empty.starts_with("---\ntype: person\nimportance:\npartner:\n---\n"),
            "{empty}"
        );
        // round trip: applying what was saved gives the same properties and blocks
        let t = parse_template(&text, &V);
        assert_eq!(t.props.len(), 2);
        assert_eq!(t.blocks.len(), 3);
        assert_eq!(render("- no frontmatter\n", true), None);
    }

    #[test]
    fn configured_names_and_the_templates_own_type() {
        assert_eq!(
            template_name("templates/person.md").as_deref(),
            Some("person")
        );
        assert_eq!(template_name("person").as_deref(), Some("person"));
        assert_eq!(template_name("../x.md"), None);
        let t = parse_template("---\ntype: person\na: 1\n---\n", &V);
        let (ops, added) = ops_for(&t, &[], false, true);
        assert_eq!(added, vec!["type", "a"]);
        assert_eq!(ops.len(), 2);
        let (_, added) = ops_for(&t, &["type".into()], false, true);
        assert_eq!(added, vec!["a"]);
    }

    #[test]
    fn query_variables() {
        let q = "type: page\nprop.partner: {{page}}\nns: {{namespace}}\ntag: [[{{page}}]]\nprop.x: [[]]\n";
        assert_eq!(
            expand_query(q, Some("part/Acme"), "2026-10-11", &[]).unwrap(),
            "type: page\nprop.partner: [[part/Acme]]\nns: part\ntag: [[part/Acme]]\nprop.x: [[part/Acme]]\n"
        );
        assert_eq!(
            expand_query("prop.n: {{title}}", Some("a/b/Jan"), "", &[]).unwrap(),
            "prop.n: Jan"
        );
        // without a page: an error with the position; `{{date}}` still works
        let e = expand_query(
            "task: todo\nprop.partner: {{page}}",
            None,
            "2026-10-11",
            &[],
        )
        .unwrap_err();
        assert_eq!((e.0, e.1), (2, 15));
        assert_eq!(
            expand_query("deadline: {{date}}", None, "2026-10-11", &[]).unwrap(),
            "deadline: 2026-10-11"
        );
    }

    #[test]
    fn type_names() {
        assert_eq!(type_name("person").as_deref(), Some("person"));
        assert_eq!(type_name("\"Person\"").as_deref(), Some("Person"));
        assert_eq!(type_name("[[person]]").as_deref(), Some("person"));
        assert_eq!(type_name("[person, boss]").as_deref(), Some("person"));
        assert_eq!(type_name("../x"), None);
        assert_eq!(type_name(""), None);
    }
}
