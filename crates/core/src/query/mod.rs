//! The query language of ```` ```query ```` blocks (file-format.md §6): AST, parser, relative dates.
//! Compiling to SQL happens in `notatnik-index`.

pub mod ast;
pub mod dates;
pub mod parse;

pub use ast::*;
pub use dates::{DateRange, WeekStart};
pub use parse::{QueryError, parse_query, parse_query_partial};
