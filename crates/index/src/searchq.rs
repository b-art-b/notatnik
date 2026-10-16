//! The syntax of the search box (SR-4): words, `AND`, `OR`, `-word` / `NOT`, parentheses, "phrases", `prefix*`.
//!
//! ```text
//! budget report            both words (a space is AND)
//! budget AND report        the same, spelled out (also `&`)
//! budget OR forecast       either (also `|`)
//! budget -draft            `budget` but not `draft` (also `NOT draft`, `!draft`)
//! (budget OR forecast) -draft "annual plan" rep*
//! ```
//!
//! `AND`, `OR` and `NOT` are operators only in capitals (`and` and `or` are words in notes); NOT binds tightest, then
//! AND, then OR. FTS5 has no unary NOT, so a group is compiled as `(positives) NOT (negatives)`: every AND group
//! needs a word that must match, and `-(a OR b)` may only contain positive words.

use notatnik_core::search_fold;

/// What the search needs from a parsed query.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parsed {
    /// The FTS5 MATCH expression.
    pub fts: String,
    /// The words that must match, folded, for highlighting: `(word, is a prefix)`. Excluded words are left out.
    pub positive: Vec<(String, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String, bool),
    Phrase(String),
    And,
    Or,
    Not,
    Open,
    Close,
}

#[derive(Debug)]
enum Atom {
    /// A word (folded, prefix?) or a phrase of words (folded).
    Term(String, bool),
    Phrase(String),
    Group(Vec<Conj>),
}

/// An AND group: what must match and what must not.
#[derive(Debug, Default)]
struct Conj {
    pos: Vec<Atom>,
    neg: Vec<Atom>,
}

fn folded(s: &str) -> String {
    search_fold(s).replace('"', "")
}

fn has_word(s: &str) -> bool {
    s.chars().any(char::is_alphanumeric)
}

fn tokenize(q: &str) -> Vec<Tok> {
    let chars: Vec<char> = q.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                out.push(Tok::Open);
                i += 1;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
            }
            '|' => {
                out.push(Tok::Or);
                i += 1;
            }
            '&' => {
                out.push(Tok::And);
                i += 1;
            }
            '!' => {
                out.push(Tok::Not);
                i += 1;
            }
            // `-` negates what follows when it starts a token (`well-known` keeps its hyphen)
            '-' if chars.get(i + 1).is_some_and(|n| !n.is_whitespace()) => {
                out.push(Tok::Not);
                i += 1;
            }
            '"' => {
                let end = chars[i + 1..]
                    .iter()
                    .position(|&c| c == '"')
                    .map(|p| i + 1 + p);
                let stop = end.unwrap_or(chars.len());
                out.push(Tok::Phrase(chars[i + 1..stop].iter().collect()));
                i = end.map_or(chars.len(), |e| e + 1);
            }
            _ => {
                let start = i;
                while i < chars.len() && !chars[i].is_whitespace() && !"()|&\"".contains(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match word.as_str() {
                    "AND" => out.push(Tok::And),
                    "OR" => out.push(Tok::Or),
                    "NOT" => out.push(Tok::Not),
                    "-" => {} // a lone dash
                    _ => {
                        let prefix = word.ends_with('*');
                        out.push(Tok::Word(word.trim_end_matches('*').to_string(), prefix));
                    }
                }
            }
        }
    }
    out
}

struct P {
    toks: Vec<Tok>,
    at: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    /// `or := conj (OR conj)*`, until a `)` or the end.
    fn or(&mut self, depth: usize) -> Result<Vec<Conj>, String> {
        let mut out = vec![self.conj(depth)?];
        while self.peek() == Some(&Tok::Or) {
            self.at += 1;
            out.push(self.conj(depth)?);
        }
        Ok(out)
    }

    /// `conj := ['NOT'] atom (['AND'] ['NOT'] atom)*`
    fn conj(&mut self, depth: usize) -> Result<Conj, String> {
        let mut c = Conj::default();
        loop {
            match self.peek() {
                None | Some(Tok::Or | Tok::Close) => break,
                Some(Tok::And) => {
                    self.at += 1;
                    continue;
                }
                _ => {}
            }
            let mut negated = false;
            while self.peek() == Some(&Tok::Not) {
                self.at += 1;
                negated = !negated;
            }
            let Some(atom) = self.atom(depth)? else {
                continue;
            };
            if negated {
                c.neg.push(atom);
            } else {
                c.pos.push(atom);
            }
        }
        Ok(c)
    }

    /// A word, a phrase or a parenthesised group; `None` for something that holds no word (`(`, `)` empty, `...`).
    fn atom(&mut self, depth: usize) -> Result<Option<Atom>, String> {
        let Some(t) = self.toks.get(self.at).cloned() else {
            return Ok(None);
        };
        self.at += 1;
        match t {
            Tok::Word(w, prefix) => {
                let f = folded(&w);
                Ok(has_word(&f).then_some(Atom::Term(f, prefix)))
            }
            Tok::Phrase(p) => {
                let f = folded(&p);
                Ok(has_word(&f).then_some(Atom::Phrase(f)))
            }
            Tok::Open => {
                if depth > 20 {
                    return Err("too many nested parentheses".into());
                }
                let inner = self.or(depth + 1)?;
                if self.peek() != Some(&Tok::Close) {
                    return Err("a ( is not closed: add a )".into());
                }
                self.at += 1;
                Ok(inner
                    .iter()
                    .any(|c| !c.pos.is_empty() || !c.neg.is_empty())
                    .then_some(Atom::Group(inner)))
            }
            Tok::Close => Err("a ) has no matching (".into()),
            Tok::And | Tok::Or | Tok::Not => Ok(None),
        }
    }
}

fn atom_fts(a: &Atom, positive: &mut Vec<(String, bool)>, collect: bool) -> Result<String, String> {
    Ok(match a {
        Atom::Term(t, prefix) => {
            if collect {
                positive.push((t.clone(), *prefix));
            }
            format!("\"{t}\"{}", if *prefix { "*" } else { "" })
        }
        Atom::Phrase(p) => {
            if collect {
                for w in p.split_whitespace() {
                    positive.push((w.to_string(), false));
                }
            }
            format!("\"{p}\"")
        }
        Atom::Group(g) => format!("({})", or_fts(g, positive, collect)?),
    })
}

fn conj_fts(c: &Conj, positive: &mut Vec<(String, bool)>, collect: bool) -> Result<String, String> {
    if c.pos.is_empty() {
        return Err(
            "there must be a word that has to match: `-word` only takes results away (try `word -other`)".into(),
        );
    }
    let pos = c
        .pos
        .iter()
        .map(|a| atom_fts(a, positive, collect))
        .collect::<Result<Vec<_>, _>>()?
        .join(" AND ");
    if c.neg.is_empty() {
        return Ok(format!("({pos})"));
    }
    // what is excluded may not contain a NOT of its own, and its words are not highlighted
    let mut ignore = Vec::new();
    let neg = c
        .neg
        .iter()
        .map(|a| atom_fts(a, &mut ignore, false))
        .collect::<Result<Vec<_>, _>>()?
        .join(" OR ");
    Ok(format!("(({pos}) NOT ({neg}))"))
}

fn or_fts(g: &[Conj], positive: &mut Vec<(String, bool)>, collect: bool) -> Result<String, String> {
    let parts = g
        .iter()
        .filter(|c| !c.pos.is_empty() || !c.neg.is_empty())
        .map(|c| conj_fts(c, positive, collect))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(parts.join(" OR "))
}

/// Parses a search. `Ok(None)`: nothing to search for. `Err`: what is wrong with the syntax, in words.
pub(crate) fn parse(q: &str) -> Result<Option<Parsed>, String> {
    let mut p = P {
        toks: tokenize(q),
        at: 0,
    };
    let tree = p.or(0)?;
    if p.at < p.toks.len() {
        return Err("a ) has no matching (".into());
    }
    if tree.iter().all(|c| c.pos.is_empty() && c.neg.is_empty()) {
        return Ok(None);
    }
    let mut positive = Vec::new();
    let fts = or_fts(&tree, &mut positive, true)?;
    // a negated group inside a negation would need a NOT of its own
    if fts.is_empty() {
        return Ok(None);
    }
    positive.sort();
    positive.dedup();
    Ok(Some(Parsed { fts, positive }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fts(q: &str) -> String {
        parse(q).unwrap().unwrap().fts
    }

    #[test]
    fn words_are_anded_and_prefixes_phrases_and_folding_work() {
        assert_eq!(fts("budget report"), "(\"budget\" AND \"report\")");
        assert_eq!(fts("budget AND report"), fts("budget report"));
        assert_eq!(fts("budget & report"), fts("budget report"));
        assert_eq!(fts("proj*"), "(\"proj\"*)");
        assert_eq!(
            fts("\"annual plan\" Żółć"),
            "(\"annual plan\" AND \"zolc\")"
        );
        // a word with punctuation stays one phrase for the tokenizer
        assert_eq!(fts("well-known"), "(\"well-known\")");
    }

    #[test]
    fn or_binds_looser_than_and_and_parentheses_group() {
        assert_eq!(fts("a b OR c"), "(\"a\" AND \"b\") OR (\"c\")");
        assert_eq!(fts("a | b"), fts("a OR b"));
        assert_eq!(fts("(a OR b) c"), "(((\"a\") OR (\"b\")) AND \"c\")");
    }

    #[test]
    fn negation_excludes_from_a_positive_part() {
        assert_eq!(fts("a -b"), "((\"a\") NOT (\"b\"))");
        assert_eq!(fts("a NOT b"), fts("a -b"));
        assert_eq!(fts("a !b"), fts("a -b"));
        assert_eq!(fts("a -(b OR c)"), "((\"a\") NOT (((\"b\") OR (\"c\"))))");
        assert_eq!(fts("a -b -c"), "((\"a\") NOT (\"b\" OR \"c\"))");
        // the excluded words are not highlighted
        let p = parse("a -b c*").unwrap().unwrap();
        assert_eq!(
            p.positive,
            vec![("a".to_string(), false), ("c".to_string(), true)]
        );
    }

    #[test]
    fn lowercase_and_or_not_are_words() {
        assert_eq!(fts("this and that"), "(\"this\" AND \"and\" AND \"that\")");
        assert_eq!(fts("this or that"), "(\"this\" AND \"or\" AND \"that\")");
    }

    #[test]
    fn mistakes_are_explained() {
        assert!(
            parse("-only")
                .unwrap_err()
                .contains("only takes results away")
        );
        assert!(
            parse("a OR -b")
                .unwrap_err()
                .contains("word that has to match")
        );
        assert!(parse("(a b").unwrap_err().contains("not closed"));
        assert!(parse("a b)").unwrap_err().contains("no matching"));
        assert_eq!(parse("   ").unwrap(), None);
        assert_eq!(parse("... ---").unwrap(), None);
        assert_eq!(parse("()").unwrap(), None);
    }
}
