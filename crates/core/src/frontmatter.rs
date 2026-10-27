//! YAML frontmatter (file-format.md §3.1).

use yaml_rust2::{Yaml, YamlLoader};

use crate::{
    Diagnostic, Frontmatter, Property, PropertyValue, Severity, Span,
    props::{infer_scalar, infer_value, split_top_level_commas},
};

fn line_end(src: &str, pos: usize) -> (usize, usize) {
    let rest = src.get(pos..).unwrap_or("");
    match rest.find('\n') {
        Some(i) => {
            let end = pos + i + 1;
            let ce = if i > 0 && rest.as_bytes().get(i - 1) == Some(&b'\r') {
                pos + i - 1
            } else {
                pos + i
            };
            (ce, end)
        }
        None => (src.len(), src.len()),
    }
}

fn yaml_to_value(y: &Yaml, raw: &str) -> PropertyValue {
    match y {
        Yaml::String(s) => infer_scalar(s),
        Yaml::Integer(i) => PropertyValue::Number(*i as f64),
        Yaml::Real(r) => r
            .parse()
            .map(PropertyValue::Number)
            .unwrap_or_else(|_| PropertyValue::Text(r.clone())),
        Yaml::Boolean(b) => PropertyValue::Bool(*b),
        Yaml::Array(a) => {
            // `partner: [[Page]]` written without quotes is YAML for a list holding a list: it was meant as the
            // link (as in a block property), so read it as one
            if let [Yaml::Array(inner)] = a.as_slice() {
                if let [Yaml::String(s)] = inner.as_slice() {
                    if let l @ PropertyValue::Links(_) = infer_scalar(&format!("[[{s}]]")) {
                        return l;
                    }
                }
            }
            let items: Vec<PropertyValue> = a.iter().map(|i| yaml_to_value(i, "")).collect();
            // `["[[A]]", "[[B]]"]`: a list of page links is one link value
            if items.len() > 1 && items.iter().all(|i| matches!(i, PropertyValue::Links(_))) {
                let links = items
                    .into_iter()
                    .flat_map(|i| match i {
                        PropertyValue::Links(l) => l,
                        _ => Vec::new(),
                    })
                    .collect();
                return PropertyValue::Links(links);
            }
            PropertyValue::List(items)
        }
        Yaml::Null => PropertyValue::Text(String::new()),
        _ => infer_value(raw),
    }
}

fn as_strings(v: &PropertyValue) -> Vec<String> {
    match v {
        // an item of a list is one value, even with a comma in it (`aliases: ["a, b"]`); only a bare scalar is split
        PropertyValue::List(items) => items
            .iter()
            .flat_map(|i| match i {
                PropertyValue::Text(s) => vec![s.trim().to_string()],
                other => as_strings(other),
            })
            .filter(|s| !s.is_empty())
            .collect(),
        PropertyValue::Text(s) => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        PropertyValue::Links(l) => l.clone(),
        PropertyValue::Number(n) => vec![n.to_string()],
        PropertyValue::Bool(b) => vec![b.to_string()],
        PropertyValue::Date(d) => vec![d.format("%Y-%m-%d").to_string()],
        PropertyValue::DateTime(d) => vec![d.format("%Y-%m-%d %H:%M").to_string()],
    }
}

fn first_string(v: &PropertyValue) -> Option<String> {
    as_strings(v).into_iter().next()
}

fn key_of_line(line: &str) -> Option<(&str, usize)> {
    let first = line.chars().next()?;
    if first.is_whitespace() || first == '#' || first == '-' {
        return None;
    }
    let colon = line.find(':')?;
    let key = line.get(..colon)?;
    let after = line.get(colon + 1..)?;
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    if key.is_empty() || key.contains(char::is_whitespace) || key.starts_with(['"', '\'']) {
        return None;
    }
    Some((key, colon))
}

/// The YAML with every top-level `key: [[A]], [[B]]` line turned into `key: ["[[A]]", "[[B]]"]`; None when no
/// line has that shape.
fn quote_link_lists(yaml: &str) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(yaml.len() + 16);
    for line in yaml.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let eol = &line[body.len()..];
        if let Some((_, colon)) = key_of_line(body) {
            let value = body[colon + 1..].trim();
            let parts: Vec<&str> = split_top_level_commas(value)
                .into_iter()
                .map(str::trim)
                .collect();
            let all_links = !parts.is_empty()
                && parts.iter().all(|p| {
                    p.len() > 4
                        && p.starts_with("[[")
                        && p.ends_with("]]")
                        && !p[2..p.len() - 2].contains(['[', ']', '"'])
                });
            if all_links && value.starts_with("[[") {
                let items: Vec<String> = parts.iter().map(|p| format!("\"{p}\"")).collect();
                out.push_str(&body[..=colon]);
                out.push_str(&format!(" [{}]{eol}", items.join(", ")));
                changed = true;
                continue;
            }
        }
        out.push_str(line);
    }
    changed.then_some(out)
}

/// Detects and parses frontmatter at `start` (after an optional BOM).
/// Returns the frontmatter (if any) and the offset where the body starts.
pub fn parse(src: &str, start: usize, diags: &mut Vec<Diagnostic>) -> (Option<Frontmatter>, usize) {
    let (ce, mut pos) = line_end(src, start);
    if src.get(start..ce).map(str::trim_end) != Some("---")
        || ce == src.len() && pos == ce && start == ce
    {
        return (None, start);
    }
    let yaml_start = pos;
    let mut closing: Option<(usize, usize)> = None;
    while pos < src.len() {
        let (lce, lend) = line_end(src, pos);
        let t = src.get(pos..lce).unwrap_or("").trim_end();
        if t == "---" || t == "..." {
            closing = Some((pos, lend));
            break;
        }
        pos = lend;
    }
    let Some((yaml_end, fm_end)) = closing else {
        return (None, start);
    };
    let span = Span::new(start, fm_end);
    let yaml = src.get(yaml_start..yaml_end).unwrap_or("");
    let mut fm = Frontmatter {
        span,
        valid: true,
        ..Frontmatter::default()
    };
    if yaml.trim().is_empty() {
        return (Some(fm), fm_end);
    }
    // `partner: [[A]], [[B]]` is how links are written in a block property but is not YAML: read it as the list of
    // links it means (the text is only changed for parsing, never in the file)
    let loaded = YamlLoader::load_from_str(yaml).or_else(|e| match quote_link_lists(yaml) {
        Some(fixed) => YamlLoader::load_from_str(&fixed).map_err(|_| e),
        None => Err(e),
    });
    let doc = match loaded {
        Ok(d) => d,
        Err(e) => {
            let line = e.marker().line().saturating_sub(1);
            let mut off = yaml_start;
            for (i, l) in yaml.split_inclusive('\n').enumerate() {
                if i == line {
                    break;
                }
                off += l.len();
            }
            let (lce, _) = line_end(src, off.min(yaml_end));
            diags.push(Diagnostic::new(
                Span::new(off.min(yaml_end), lce.max(off.min(yaml_end))),
                Severity::Error,
                "invalid-frontmatter",
                format!("invalid YAML in frontmatter: {e}"),
            ));
            fm.valid = false;
            return (Some(fm), fm_end);
        }
    };
    let Some(Yaml::Hash(map)) = doc.first() else {
        diags.push(Diagnostic::new(
            span,
            Severity::Error,
            "invalid-frontmatter",
            "frontmatter must be a mapping of keys to values",
        ));
        fm.valid = false;
        return (Some(fm), fm_end);
    };

    // Top-level key spans by line scanning.
    let mut keys: Vec<(String, usize, usize, usize)> = Vec::new(); // key, line start, colon abs, value start abs
    let mut pos = yaml_start;
    for line in yaml.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        if let Some((k, colon)) = key_of_line(body) {
            keys.push((k.to_string(), pos, pos + colon, pos + colon + 1));
        }
        pos += line.len();
    }
    for (idx, (key, lstart, colon, vstart)) in keys.iter().enumerate() {
        let end = keys.get(idx + 1).map_or(yaml_end, |n| n.1);
        let (lce, _) = line_end(src, *lstart);
        let raw = src.get(*vstart..lce).unwrap_or("").trim().to_string();
        let value = match map.get(&Yaml::String(key.clone())) {
            Some(y) => yaml_to_value(y, &raw),
            None => PropertyValue::Text(raw.clone()),
        };
        let ws = src.get(*vstart..lce).unwrap_or("").len()
            - src.get(*vstart..lce).unwrap_or("").trim_start().len();
        let prop = Property {
            key: key.clone(),
            raw,
            value: value.clone(),
            span: Span::new(*lstart, end),
            key_span: Span::new(*lstart, *colon),
            value_span: Span::new(*vstart + ws, lce),
        };
        match key.as_str() {
            "title" => fm.title = first_string(&value),
            "aliases" => fm.aliases = as_strings(&value),
            "tags" => fm.tags = as_strings(&value),
            "template" => fm.template = first_string(&value),
            "icon" => fm.icon = first_string(&value),
            _ => fm.properties.push(prop.clone()),
        }
        fm.entries.push(prop);
    }
    (Some(fm), fm_end)
}

#[cfg(test)]
mod alias_tests {
    use crate::parse_page;

    #[test]
    fn a_quoted_list_item_with_a_comma_is_one_alias() {
        let src = "---\naliases: [tgt, \"Quoted, [odd]: name\", plain two]\n---\n- x\n";
        let page = parse_page(src);
        assert_eq!(
            page.frontmatter.unwrap().aliases,
            ["tgt", "Quoted, [odd]: name", "plain two"]
        );
        // a bare value is still split at commas
        let page = parse_page("---\naliases: a, b\n---\n- x\n");
        assert_eq!(page.frontmatter.unwrap().aliases, ["a", "b"]);
    }
}
