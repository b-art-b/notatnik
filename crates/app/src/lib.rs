//! Application service: the typed API used by the desktop app (Tauri) and by the dev server that
//! the UI tests run against. Both call [`dispatch`], so there is exactly one implementation.

pub mod dto;
pub mod render;
mod state;
mod ts;
pub mod views;

pub use state::{ApiError, ApiResult, AppOptions, AppState, Emitter};
pub use ts::typescript_bindings;

use dto::*;
use serde_json::Value;

/// Declares every command once: its name, argument type, result type and implementation. Generates
/// [`dispatch`] and the TypeScript `Commands` map.
macro_rules! commands {
    ($( $name:ident ( $args:ty ) -> $ret:ty => $method:ident ),* $(,)?) => {
        /// Calls a command by name with JSON arguments; returns the JSON result.
        pub fn dispatch(state: &AppState, name: &str, args: Value) -> Result<Value, ApiError> {
            match name {
                $( stringify!($name) => {
                    let a: $args = serde_json::from_value(args)
                        .map_err(|e| ApiError { code: "invalid".into(), message: format!("bad arguments for `{}`: {e}", name) })?;
                    let r: $ret = state.$method(a)?;
                    serde_json::to_value(r).map_err(|e| ApiError { code: "internal".into(), message: e.to_string() })
                } )*
                other => Err(ApiError { code: "invalid".into(), message: format!("unknown command `{other}`") }),
            }
        }

        /// The TypeScript `Commands` map for `bindings.ts`.
        pub(crate) fn commands_ts() -> String {
            let mut s = String::from("export type Commands = {\n");
            $( s.push_str(&format!("  {}: {{ args: {}; result: {} }};\n", stringify!($name), stringify!($args), ts::ts_name(stringify!($ret)))); )*
            s.push_str("};\n");
            s
        }
    };
}

commands! {
    open_graph(OpenGraphArgs) -> GraphInfo => open_graph,
    graph_info(NoArgs) -> GraphInfo => graph_info,
    today(NoArgs) -> StringValue => today_cmd,
    get_page(GetPageArgs) -> PageView => get_page,
    apply_ops(ApplyOpsArgs) -> ApplyResult => apply_ops,
    undo(NoArgs) -> UndoResult => undo,
    redo(NoArgs) -> UndoResult => redo,
    search(SearchArgs) -> Vec<SearchHitView> => search,
    search_titles(SearchArgs) -> Vec<TitleHit> => search_titles,
    backlinks(PageArgs) -> Vec<BacklinkView> => backlinks,
    list_namespace(ListNamespaceArgs) -> Vec<NamespaceEntry> => list_namespace,
    journals(JournalsArgs) -> Vec<JournalDay> => journals,
    rename_plan(RenameArgs) -> RenamePlanView => rename_plan,
    rename_apply(RenameArgs) -> PageView => rename_apply,
    ensure_block_id(EnsureBlockIdArgs) -> StringValue => ensure_block_id,
    get_block(GetBlockArgs) -> Option<BlockLookup> => get_block,
    set_collapsed(SetCollapsedArgs) -> NoArgs => set_collapsed,
    list_conflicts(NoArgs) -> StringList => list_conflicts,
    read_conflict(ReadConflictArgs) -> ConflictView => read_conflict,
}
