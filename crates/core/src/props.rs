//! Property value inference (file-format.md §3.3).

use crate::{
    PropType, PropertyValue,
    names::PageName,
    task::{parse_date, parse_datetime},
};

/// True for a valid property key: `[a-z0-9_-]+`.
pub fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Splits a property line (without line break) into `(key, value)` if it has the form `key:: value`.
/// The `::` must be followed by a space or the end of the line.
pub fn split_property_line(line: &str) -> Option<(&str, &str)> {
    let idx = line.find("::")?;
    let key = line.get(..idx)?;
    if !is_valid_key(key) {
        return None;
    }
    let rest = line.get(idx + 2..)?;
    if rest.is_empty() {
        return Some((key, ""));
    }
    if rest.starts_with([' ', '\t']) {
        return Some((key, rest));
    }
    None
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b.get(i) {
            Some(b'[') if b.get(i + 1) == Some(&b'[') => {
                depth += 1;
                i += 2;
                continue;
            }
            Some(b']') if b.get(i + 1) == Some(&b']') => {
                depth = depth.saturating_sub(1);
                i += 2;
                continue;
            }
            Some(b',') if depth == 0 => {
                parts.push(s.get(start..i).unwrap_or(""));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(s.get(start..).unwrap_or(""));
    parts
}

fn single_link(s: &str) -> Option<String> {
    let inner = s.strip_prefix("[[")?.strip_suffix("]]")?;
    if inner.contains("[[") || inner.contains("]]") {
        return None;
    }
    let target = inner.split_once('|').map_or(inner, |(t, _)| t);
    PageName::parse(target).ok().map(|n| n.as_str().to_string())
}

fn is_number(s: &str) -> bool {
    let t = s.strip_prefix('-').unwrap_or(s);
    let (int, frac) = match t.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (t, None),
    };
    if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if frac.is_none() && int.len() > 1 && int.starts_with('0') {
        return false;
    }
    match frac {
        Some(f) => !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()),
        None => true,
    }
}

/// Infers the type of one scalar value.
pub fn infer_scalar(raw: &str) -> PropertyValue {
    let s = raw.trim();
    if let Some(l) = single_link(s) {
        return PropertyValue::Links(vec![l]);
    }
    if let Some(d) = parse_date(s) {
        return PropertyValue::Date(d);
    }
    if let Some(dt) = parse_datetime(s) {
        return PropertyValue::DateTime(dt);
    }
    if is_number(s) {
        if let Ok(n) = s.parse::<f64>() {
            return PropertyValue::Number(n);
        }
    }
    match s {
        "true" => PropertyValue::Bool(true),
        "false" => PropertyValue::Bool(false),
        _ => PropertyValue::Text(s.to_string()),
    }
}

fn same_kind(a: &PropertyValue, b: &PropertyValue) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Infers the typed value of a raw property value (commas split into lists when all items agree).
pub fn infer_value(raw: &str) -> PropertyValue {
    let s = raw.trim();
    let parts = split_top_level_commas(s);
    if parts.len() > 1 {
        let items: Vec<PropertyValue> = parts.iter().map(|p| infer_scalar(p)).collect();
        if items.iter().all(|i| matches!(i, PropertyValue::Links(_))) {
            let links = items
                .into_iter()
                .filter_map(|i| {
                    if let PropertyValue::Links(l) = i {
                        Some(l)
                    } else {
                        None
                    }
                })
                .flatten()
                .collect();
            return PropertyValue::Links(links);
        }
        let first_ok = items
            .first()
            .is_some_and(|f| !matches!(f, PropertyValue::Text(_)));
        if first_ok
            && items
                .iter()
                .all(|i| items.first().is_some_and(|f| same_kind(f, i)))
        {
            return PropertyValue::List(items);
        }
        return PropertyValue::Text(s.to_string());
    }
    infer_scalar(s)
}

/// Applies a declared type. `Err` carries a message when the value doesn't fit the type.
pub fn coerce_value(raw: &str, ty: PropType) -> Result<PropertyValue, String> {
    let s = raw.trim();
    match ty {
        PropType::Text => Ok(PropertyValue::Text(s.to_string())),
        PropType::Number if is_number(s) => Ok(PropertyValue::Number(
            s.parse().map_err(|_| "invalid number")?,
        )),
        PropType::Number => Err(format!("expected a number, found `{s}`")),
        PropType::Date => parse_date(s)
            .map(PropertyValue::Date)
            .ok_or_else(|| format!("expected a date (YYYY-MM-DD), found `{s}`")),
        PropType::DateTime => parse_datetime(s)
            .map(PropertyValue::DateTime)
            .ok_or_else(|| format!("expected a date and time (YYYY-MM-DD HH:MM), found `{s}`")),
        PropType::Bool => match s {
            "true" => Ok(PropertyValue::Bool(true)),
            "false" => Ok(PropertyValue::Bool(false)),
            _ => Err(format!("expected true or false, found `{s}`")),
        },
        PropType::Page => {
            let mut links = Vec::new();
            for part in split_top_level_commas(s) {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let target = single_link(part)
                    .or_else(|| PageName::parse(part).ok().map(|n| n.as_str().to_string()));
                match target {
                    Some(t) => links.push(t),
                    None => return Err(format!("expected a page name, found `{part}`")),
                }
            }
            Ok(PropertyValue::Links(links))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rstest::rstest;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[rstest]
    #[case("2026-10-05", PropertyValue::Date(d(2026, 10, 5)))]
    #[case("2026-13-01", PropertyValue::Text("2026-13-01".into()))]
    #[case("2026-02-30", PropertyValue::Text("2026-02-30".into()))]
    #[case("2026-10-05 14:00", PropertyValue::DateTime(d(2026, 10, 5).and_hms_opt(14, 0, 0).unwrap()))]
    #[case("2026-10-05T14:00", PropertyValue::DateTime(d(2026, 10, 5).and_hms_opt(14, 0, 0).unwrap()))]
    #[case("2026-10-05 25:00", PropertyValue::Text("2026-10-05 25:00".into()))]
    #[case("42", PropertyValue::Number(42.0))]
    #[case("-3", PropertyValue::Number(-3.0))]
    #[case("1.5", PropertyValue::Number(1.5))]
    #[case("1.", PropertyValue::Text("1.".into()))]
    #[case("007", PropertyValue::Text("007".into()))]
    #[case("true", PropertyValue::Bool(true))]
    #[case("false", PropertyValue::Bool(false))]
    #[case("True", PropertyValue::Text("True".into()))]
    #[case("[[people/anna]]", PropertyValue::Links(vec!["people/anna".into()]))]
    #[case("[[a]], [[b]]", PropertyValue::Links(vec!["a".into(), "b".into()]))]
    #[case("[[ x / y ]]", PropertyValue::Links(vec!["x/y".into()]))]
    #[case("hello world", PropertyValue::Text("hello world".into()))]
    #[case("a, b", PropertyValue::Text("a, b".into()))]
    #[case("1, 2, 3", PropertyValue::List(vec![PropertyValue::Number(1.0), PropertyValue::Number(2.0), PropertyValue::Number(3.0)]))]
    #[case("2026-09-26, 2026-10-03", PropertyValue::List(vec![PropertyValue::Date(d(2026, 9, 26)), PropertyValue::Date(d(2026, 10, 3))]))]
    #[case("", PropertyValue::Text(String::new()))]
    fn inference(#[case] raw: &str, #[case] want: PropertyValue) {
        assert_eq!(infer_value(raw), want, "{raw}");
    }

    #[rstest]
    #[case("status:: active", Some(("status", " active")))]
    #[case("status::", Some(("status", "")))]
    #[case("std::fmt", None)]
    #[case("Status:: x", None)]
    #[case("a b:: x", None)]
    #[case("::x", None)]
    fn property_lines(#[case] line: &str, #[case] want: Option<(&str, &str)>) {
        assert_eq!(split_property_line(line), want);
    }

    #[test]
    fn declared_types() {
        assert!(coerce_value("x", PropType::Number).is_err());
        assert_eq!(
            coerce_value("2026-10-05", PropType::Text).unwrap(),
            PropertyValue::Text("2026-10-05".into())
        );
        assert_eq!(
            coerce_value("anna", PropType::Page).unwrap(),
            PropertyValue::Links(vec!["anna".into()])
        );
        assert_eq!(
            coerce_value("anna, [[bob]]", PropType::Page).unwrap(),
            PropertyValue::Links(vec!["anna".into(), "bob".into()])
        );
    }
}
