//! Application service: the typed API used by the desktop app (Tauri) and by the dev server that
//! the UI tests run against. Both call [`dispatch`], so there is exactly one implementation.

pub mod dto;
pub mod render;
mod state;
mod templates;
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
    list_graphs(NoArgs) -> Vec<GraphHistoryEntry> => list_graphs,
    forget_graph(ForgetGraphArgs) -> Vec<GraphHistoryEntry> => forget_graph,
    graph_info(NoArgs) -> GraphInfo => graph_info,
    today(NoArgs) -> StringValue => today_cmd,
    get_page(GetPageArgs) -> PageView => get_page,
    apply_ops(ApplyOpsArgs) -> ApplyResult => apply_ops,
    undo(NoArgs) -> UndoResult => undo,
    redo(NoArgs) -> UndoResult => redo,
    search(SearchArgs) -> Vec<SearchHitView> => search,
    search_titles(SearchArgs) -> Vec<TitleHit> => search_titles,
    backlinks(PageArgs) -> Vec<BacklinkView> => backlinks,
    unlinked_refs(PageArgs) -> Vec<UnlinkedView> => unlinked_refs,
    subpages(PageArgs) -> Vec<TitleHit> => subpages,
    page_icons(NoArgs) -> Vec<PageIconView> => page_icons,
    segment_view(SegmentArgs) -> SegmentView => segment_view,
    segment_places(SegmentPlacesArgs) -> SegmentPlacesView => segment_places,
    all_pages(NoArgs) -> Vec<PageListRow> => all_pages,
    page_property_keys(NoArgs) -> Vec<PageKeyView> => page_property_keys,
    page_groups(PageGroupsArgs) -> Vec<PageGroupView> => page_groups,
    list_namespace(ListNamespaceArgs) -> Vec<NamespaceEntry> => list_namespace,
    journals(JournalsArgs) -> Vec<JournalDay> => journals,
    rename_plan(RenameArgs) -> RenamePlanView => rename_plan,
    rename_revert(NoArgs) -> StringList => rename_revert,
    rename_apply(RenameArgs) -> PageView => rename_apply,
    delete_pages(DeletePagesArgs) -> DeletePagesResult => delete_pages,
    restore_pages(RestorePagesArgs) -> StringList => restore_pages,
    archived_pages(NoArgs) -> StringList => archived_pages,
    ensure_block_id(EnsureBlockIdArgs) -> StringValue => ensure_block_id,
    get_block(GetBlockArgs) -> Option<BlockLookup> => get_block,
    set_collapsed(SetCollapsedArgs) -> NoArgs => set_collapsed,
    list_conflicts(NoArgs) -> StringList => list_conflicts,
    read_conflict(ReadConflictArgs) -> ConflictView => read_conflict,
    task_chain(ChainArgs) -> ChainView => task_chain,
    new_block_id(NoArgs) -> StringValue => new_block_id,
    template_info(PageArgs) -> TemplateInfo => template_info,
    apply_template(PageArgs) -> ApplyResult => apply_template,
    save_template(SaveTemplateArgs) -> TemplateEntry => save_template,
    list_templates(NoArgs) -> Vec<TemplateEntry> => list_templates,
    delete_template(NameArgs) -> Vec<TemplateEntry> => delete_template,
    deleted_templates(NoArgs) -> Vec<DeletedTemplate> => deleted_templates,
    restore_template(RestoreTemplateArgs) -> Vec<TemplateEntry> => restore_template,
    template_blocks(TemplateBlocksArgs) -> TemplateContent => template_blocks,
    run_query(RunQueryArgs) -> QueryView => run_query,
    agenda(AgendaArgs) -> AgendaView => agenda,
    list_saved_queries(NoArgs) -> Vec<SavedQueryView> => list_saved_queries,
    save_query(SavedQueryView) -> Vec<SavedQueryView> => save_query,
    delete_saved_query(NameArgs) -> Vec<SavedQueryView> => delete_saved_query,
    property_keys(NoArgs) -> StringList => property_keys,
    property_values(PropKeyArgs) -> StringList => property_values,
    get_settings(NoArgs) -> SettingsView => get_settings,
    set_settings(SetSettingsArgs) -> SettingsView => set_settings,
    list_backups(NoArgs) -> BackupList => list_backups,
    backup_now(NoArgs) -> BackupList => backup_now,
    restore_backup(RestoreBackupArgs) -> StringValue => restore_backup,
    restore_backup_page(RestoreBackupPageArgs) -> StringList => restore_backup_page,
    backup_compare(BackupNameArgs) -> BackupCompare => backup_compare,
    backup_file_diff(BackupFileArgs) -> BackupFileDiff => backup_file_diff,
    backup_device_preview(BackupNameArgs) -> DeviceRestorePreview => backup_device_preview,
    restore_device_settings(BackupNameArgs) -> SettingsView => restore_device_settings,
    page_versions(PageArgs) -> Vec<PageVersionView> => page_versions,
    page_version_text(PageVersionArgs) -> StringValue => page_version_text,
    restore_page_version(PageVersionArgs) -> StringList => restore_page_version,
}
