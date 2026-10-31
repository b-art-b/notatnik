// Example queries, offered by `/query` in the editor and listed in Settings → Templates. One list for both; the
// Rust test `example_queries_parse` (crates/core/tests/query_parse.rs) reads the same file and parses every query.
import examples from './queryExamples.json'

export interface QueryExample {
  id: string
  title: string
  /** One sentence on what it does or what to change; may be empty. */
  description: string
  /** The query text, one clause per line (placeholders such as `{{page}}` are filled in when it runs). */
  query: string
}

export const QUERY_EXAMPLES: QueryExample[] = examples

/** The block text of a query block with this query. */
export const queryBlockText = (query: string): string => '```query\n' + query + '\n```'
