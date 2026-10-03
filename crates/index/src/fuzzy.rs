//! Fuzzy page-title search (P1.25), kept in memory and refreshed after index changes.

use notatnik_core::search_fold;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use serde::Serialize;

use crate::{Index, Result};

#[derive(Default)]
pub(crate) struct FuzzyCache {
    dirty: bool,
    loaded: bool,
    titles: Vec<(String, String, bool)>, // name, folded name, virtual
}

impl FuzzyCache {
    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }
}

/// A fuzzy page-name match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuzzyHit {
    /// Page name.
    pub name: String,
    /// Match score (higher is better).
    pub score: u32,
    /// The page has no file.
    pub is_virtual: bool,
}

impl Index {
    fn refresh_fuzzy(&mut self) -> Result<()> {
        if self.fuzzy.loaded && !self.fuzzy.dirty {
            return Ok(());
        }
        let mut st = self
            .conn
            .prepare("SELECT name, is_virtual FROM pages ORDER BY key")?;
        let rows: Vec<(String, bool)> = st
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))
            })?
            .collect::<std::result::Result<_, _>>()?;
        drop(st);
        self.fuzzy.titles = rows
            .into_iter()
            .map(|(n, v)| (search_fold(&n), n, v))
            .map(|(f, n, v)| (n, f, v))
            .collect();
        self.fuzzy.loaded = true;
        self.fuzzy.dirty = false;
        Ok(())
    }

    /// Fuzzy-matches `query` against all page names. An exact leaf name or a prefix of the leaf ranks
    /// above looser matches (`alpha` finds `work/projects/alpha` before `alphabet-soup`).
    pub fn fuzzy_pages(&mut self, query: &str, limit: usize) -> Result<Vec<FuzzyHit>> {
        self.refresh_fuzzy()?;
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let fq = search_fold(q);
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(q, CaseMatching::Ignore, Normalization::Smart);
        let mut buf = Vec::new();
        let mut hits: Vec<FuzzyHit> = Vec::new();
        for (name, folded, is_virtual) in &self.fuzzy.titles {
            let Some(score) = pattern.score(Utf32Str::new(name, &mut buf), &mut matcher) else {
                continue;
            };
            let leaf = folded.rsplit('/').next().unwrap_or(folded);
            let bonus = if leaf == fq {
                10_000
            } else if leaf.starts_with(&fq) {
                3_000
            } else if folded.starts_with(&fq) {
                1_000
            } else {
                0
            };
            hits.push(FuzzyHit {
                name: name.clone(),
                score: score + bonus,
                is_virtual: *is_virtual,
            });
        }
        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.name.len().cmp(&b.name.len()))
                .then(a.name.cmp(&b.name))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}
