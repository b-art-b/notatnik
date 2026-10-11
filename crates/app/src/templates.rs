//! Page templates (NS-10, PR-10): `templates/<type>.md` in the graph. A template's frontmatter is the property
//! skeleton and its blocks are the starting content. Pure functions; the file access is in `state.rs`.

use notatnik_core::{Block, BlockOp, parse_page};

/// What a template contributes to a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// `(key, raw YAML value)` in file order; an empty value means "show the field, empty".
    pub props: Vec<(String, String)>,
    /// `(parent path, index, text)` in insertion order (a block's parent comes before its children).
    pub blocks: Vec<(Vec<usize>, usize, String)>,
}

/// The values the `{{...}}` variables stand for.
pub struct Vars<'a> {
    pub title: &'a str,
    pub date: &'a str,
}

fn fill(text: &str, v: &Vars) -> String {
    let namespace = v.title.rsplit_once('/').map_or("", |(ns, _)| ns);
    let leaf = v.title.rsplit('/').next().unwrap_or(v.title);
    text.replace("{{title}}", leaf)
        .replace("{{namespace}}", namespace)
        .replace("{{date}}", v.date)
        .replace("{{cursor}}", "")
}

fn collect(
    src: &str,
    blocks: &[Block],
    parent: &[usize],
    v: &Vars,
    out: &mut Vec<(Vec<usize>, usize, String)>,
) {
    for (i, b) in blocks.iter().enumerate() {
        out.push((parent.to_vec(), i, fill(&b.edit_text(src), v)));
        let mut path = parent.to_vec();
        path.push(i);
        collect(src, &b.children, &path, v, out);
    }
}

/// Reads a template file. Properties with multi-line values are skipped (a property is set as one line).
pub fn parse_template(text: &str, v: &Vars) -> Template {
    let page = parse_page(text);
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
    collect(text, &page.blocks, &[], v, &mut blocks);
    Template { props, blocks }
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
pub fn ops_for(t: &Template, have: &[String], with_blocks: bool) -> (Vec<BlockOp>, Vec<String>) {
    let mut ops = Vec::new();
    let mut added = Vec::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    const V: Vars = Vars {
        title: "people/Jan",
        date: "2026-10-11",
    };

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
        let (ops, added) = ops_for(&t, &["a".into()], false);
        assert_eq!(added, vec!["b"]);
        assert_eq!(ops.len(), 1);
        let (ops, _) = ops_for(&t, &[], true);
        assert_eq!(ops.len(), 3);
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
