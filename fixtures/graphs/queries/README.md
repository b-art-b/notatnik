# Fixture graph: queries

Graph for query-engine tests, with fixed "today" = 2026-10-03 (Saturday, weeks start on Monday).
`queries/*.query` are query blocks; `expected/*.json` list the expected result ids in order
(`page#block-path`, hand-derived, not generated). Covers every clause, negation, OR lists,
page-level tags, the `work`/`workshop` prefix trap, dates (relative, ranges, none), properties,
lists, diacritics, sorting, grouping and limits.
