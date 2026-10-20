//! Application service: the typed API used by the desktop app (Tauri) and by the dev server that
//! the UI tests run against. Both call [`dispatch`], so there is exactly one implementation.

pub mod dto;
pub mod render;
mod state;
mod templates;
mod ts;
pub mod views;

pub use state::{
    ApiError, ApiResult, AppOptions, AppState, Emitter, MemorySecrets, Secrets, asset_content_type,
};
pub use ts::typescript_bindings;

use dto::*;
use serde_json::Value;

/// A command that takes this long is worth a line at `info`; faster ones are only logged at `debug`.
const SLOW_COMMAND: std::time::Duration = std::time::Duration::from_millis(250);

/// Calls a command by name with JSON arguments; returns the JSON result. Every call is logged at `debug` with its
/// time (never its arguments or result: those are the user's notes). A slow call is logged at `info`, a failure
/// by its code: `internal` is a bug (`error`), `invalid` is the caller's mistake (`warn`), the others (`not_found`,
/// `conflict`, `no_graph`, ...) are ordinary answers (`debug`).
pub fn dispatch(state: &AppState, name: &str, args: Value) -> Result<Value, ApiError> {
    let started = std::time::Instant::now();
    let result = run_command(state, name, args);
    let took = started.elapsed();
    match &result {
        Ok(_) if took >= SLOW_COMMAND => {
            log::info!("command {name} was slow: {} ms", took.as_millis())
        }
        Ok(_) => log::debug!("command {name} ok ({} ms)", took.as_millis()),
        Err(e) if e.code == "internal" => log::error!("command {name} failed: {e}"),
        Err(e) if e.code == "invalid" => log::warn!("command {name} was refused: {}", e.message),
        Err(e) => log::debug!(
            "command {name} answered {} ({} ms)",
            e.code,
            took.as_millis()
        ),
    }
    result
}

/// Declares every command once: its name, argument type, result type and implementation. Generates
/// [`dispatch`] and the TypeScript `Commands` map.
macro_rules! commands {
    ($( $name:ident ( $args:ty ) -> $ret:ty => $method:ident ),* $(,)?) => {
        fn run_command(state: &AppState, name: &str, args: Value) -> Result<Value, ApiError> {
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
    page_files(PageFilesArgs) -> PageFilesView => page_files,
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
    log_client(LogClientArgs) -> NoArgs => log_client,
    encryption_status(NoArgs) -> EncryptionStatus => graph_encryption_status,
    encryption_setup(EncryptionSetupArgs) -> RecoveryKeyResult => encryption_setup,
    encryption_confirm(NoArgs) -> EncryptionStatus => encryption_confirm,
    encryption_unlock(EncryptionUnlockArgs) -> EncryptionStatus => encryption_unlock,
    encryption_lock(NoArgs) -> EncryptionStatus => encryption_lock,
    encryption_change_password(EncryptionChangePasswordArgs) -> EncryptionStatus => encryption_change_password,
    encryption_new_recovery_key(EncryptionPasswordArgs) -> RecoveryKeyResult => encryption_new_recovery_key,
    encryption_keyring(EncryptionKeyringArgs) -> EncryptionStatus => encryption_keyring,
    encryption_calendar(EncryptionCalendarArgs) -> EncryptionStatus => encryption_calendar,
    encryption_page_paths(EncryptionPagePathsArgs) -> StringList => encryption_page_paths,
    encryption_preview(EncryptionPathsArgs) -> EncryptionPreview => encryption_preview,
    encryption_mark(EncryptionPathsArgs) -> EncryptionStatus => encryption_mark,
    conflict_merge(ConflictMergeArgs) -> ConflictMergeView => conflict_merge,
    resolve_conflict(ResolveConflictArgs) -> StringList => resolve_conflict,
    list_conflicts(NoArgs) -> StringList => list_conflicts,
    read_conflict(ReadConflictArgs) -> ConflictView => read_conflict,
    task_chain(ChainArgs) -> ChainView => task_chain,
    new_block_id(NoArgs) -> StringValue => new_block_id,
    template_info(PageArgs) -> TemplateInfo => template_info,
    apply_template(PageArgs) -> ApplyResult => apply_template,
    save_template(SaveTemplateArgs) -> TemplateEntry => save_template,
    list_templates(NoArgs) -> Vec<TemplateEntry> => list_templates,
    read_template(NameArgs) -> StringValue => read_template,
    write_template(WriteTemplateArgs) -> TemplateEntry => write_template,
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
    backup_encryption_status(NoArgs) -> BackupEncryption => backup_encryption_status,
    backup_encryption_setup(BackupSetupArgs) -> RecoveryKeyResult => backup_encryption_setup,
    backup_encryption_confirm(NoArgs) -> BackupEncryption => backup_encryption_confirm,
    backup_unlock(BackupUnlockArgs) -> BackupEncryption => backup_unlock,
    asset_link(AssetLinkArgs) -> StringValue => asset_link,
    save_asset(SaveAssetArgs) -> SavedAsset => save_asset,
    attach_file(AttachFileArgs) -> SavedAsset => attach_file,
    list_orphan_assets(NoArgs) -> OrphanAssets => orphan_assets,
    delete_orphan_assets(DeleteAssetsArgs) -> OrphanAssets => delete_orphan_assets,
    sync_status(NoArgs) -> SyncStatus => sync_status,
    sync_set_merge(SyncMergeArgs) -> SyncStatus => sync_set_merge,
    sync_now(SyncNowArgs) -> SyncStatus => sync_now,
    sync_set_password(SyncPasswordArgs) -> SyncStatus => sync_set_password,
    webdav_browse(WebdavArgs) -> WebdavFolders => webdav_browse,
    file_sources(NoArgs) -> Vec<FileSourceView> => file_sources,
    file_source_save(FileSourceArgs) -> FileSourceView => file_source_save,
    file_source_delete(FileSourceIdArgs) -> Vec<FileSourceView> => file_source_delete,
    file_browse(FileBrowseArgs) -> FileBrowse => file_browse,
    calendar_status(NoArgs) -> CalendarStatus => calendar_status,
    calendar_connect(CalendarConnectArgs) -> CalendarList => calendar_connect,
    calendar_save(CalendarSaveArgs) -> CalendarStatus => calendar_save,
    calendar_sync(NoArgs) -> CalendarSyncReport => calendar_sync,
    calendar_events(CalendarEventsArgs) -> Vec<CalendarEventView> => calendar_events,
    calendar_inbox_copies(NoArgs) -> InboxCopies => calendar_inbox_copies,
    calendar_move_inbox_copies(NoArgs) -> MovedCopies => calendar_move_inbox_copies,
    calendar_pause(CalendarPauseArgs) -> CalendarStatus => calendar_pause,
    calendar_set_color(CalendarColorArgs) -> CalendarStatus => calendar_set_color,
    calendar_log(NoArgs) -> Vec<CalendarLogEntry> => calendar_log,
    calendar_use_other_value(CalendarUseOtherArgs) -> CalendarSyncReport => calendar_use_other_value,
    calendar_forget(NoArgs) -> CalendarStatus => calendar_forget,
    webdav_test(WebdavArgs) -> WebdavTest => webdav_test,
    create_remote_graph(WebdavArgs) -> GraphInfo => create_remote_graph,
    connect_remote(WebdavArgs) -> GraphInfo => connect_remote,
    disconnect_remote(NoArgs) -> GraphInfo => disconnect_remote,
    backup_lock(NoArgs) -> BackupEncryption => backup_lock,
    backup_encryption_change_password(BackupChangePasswordArgs) -> BackupEncryption => backup_encryption_change_password,
    backup_encryption_new_recovery_key(BackupPasswordArgs) -> RecoveryKeyResult => backup_encryption_new_recovery_key,
    backup_encrypt_existing(NoArgs) -> BackupEncryption => backup_encrypt_existing,
    backup_compare(BackupNameArgs) -> BackupCompare => backup_compare,
    backup_file_diff(BackupFileArgs) -> BackupFileDiff => backup_file_diff,
    backup_device_preview(BackupNameArgs) -> DeviceRestorePreview => backup_device_preview,
    restore_device_settings(BackupNameArgs) -> SettingsView => restore_device_settings,
    page_versions(PageArgs) -> Vec<PageVersionView> => page_versions,
    page_version_text(PageVersionArgs) -> StringValue => page_version_text,
    restore_page_version(PageVersionArgs) -> StringList => restore_page_version,
}
