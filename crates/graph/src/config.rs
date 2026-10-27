//! `.notatnik/config.toml` and per-device settings (file-format.md §8).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use notatnik_core::{PropType, WeekStart};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Parsed `config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    /// `favorites = [...]`: page names pinned to the top of the sidebar, in the order shown (they sync with the graph).
    /// First in the struct: a plain value must come before the tables in the file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub favorites: Vec<String>,
    /// `hidden = [...]`: page names that are hidden, each with everything below it (NS-27). A page that has a file is
    /// hidden by `archived: true` in the file; this list is for names without one (a folder that exists because a page
    /// below it does) and syncs with the graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// `[graph]`
    #[serde(default)]
    pub graph: GraphSection,
    /// `[properties]`
    #[serde(default)]
    pub properties: PropertiesSection,
    /// `[namespaces."people"]`
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub namespaces: BTreeMap<String, NamespaceSection>,
    /// `[tasks]`
    #[serde(default)]
    pub tasks: TasksSection,
    /// `[calendar]`
    #[serde(default)]
    pub calendar: CalendarSection,
    /// `[encryption]`: which files are stored encrypted on a WebDAV server (EN-11).
    #[serde(default, skip_serializing_if = "EncryptionSection::is_default")]
    pub encryption: EncryptionSection,
    /// `[[saved_queries]]`: queries pinned to the sidebar (they sync with the graph).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saved_queries: Vec<SavedQuery>,
    /// `[[page_icons]]`: an emoji for every page whose property has a value (`type: person` → 👤). They sync with the
    /// graph. A page's own `icon:` property wins over these rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_icons: Vec<PageIconRule>,
}

/// One page-icon rule: pages with `key: value` get `icon`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageIconRule {
    /// Page property (frontmatter key), e.g. `type`.
    pub key: String,
    /// The value that selects the pages, compared without regard to case or accents, e.g. `person`.
    pub value: String,
    /// The emoji (or a few characters) shown with those pages.
    pub icon: String,
}

/// A query pinned to the sidebar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedQuery {
    /// Name shown in the sidebar.
    pub name: String,
    /// Query text.
    pub query: String,
}

/// `[graph]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSection {
    /// Random ID, generated once.
    #[serde(default)]
    pub id: String,
    /// Journal template path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal_template: Option<String>,
    /// `monday` or `sunday`.
    #[serde(default = "default_week_start")]
    pub week_start: String,
}

fn default_week_start() -> String {
    "monday".into()
}

impl Default for GraphSection {
    fn default() -> Self {
        GraphSection {
            id: String::new(),
            journal_template: None,
            week_start: default_week_start(),
        }
    }
}

/// `[properties]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertiesSection {
    /// Properties hidden in the rendered view.
    #[serde(default = "default_hidden")]
    pub hidden: Vec<String>,
    /// Declared types: `number`, `date`, `datetime`, `bool`, `page`, `text`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<String, String>,
}

fn default_hidden() -> Vec<String> {
    vec!["done".into(), "completed".into()]
}

impl Default for PropertiesSection {
    fn default() -> Self {
        PropertiesSection {
            hidden: default_hidden(),
            types: BTreeMap::new(),
        }
    }
}

/// `[namespaces."name"]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NamespaceSection {
    /// Template for new pages below this namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

/// `[tasks]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TasksSection {
    /// 0 = keep all completion dates of repeating tasks; N = keep the last N.
    #[serde(default)]
    pub completed_log_limit: u32,
    /// The priority labels, most important first (one to nine). Shown after the task marker (`TODO [#A] ...`); in a
    /// file `priority::` is one of them (`high`/`medium`/`low` still work). A task has no priority unless one is set.
    #[serde(
        default = "default_priorities",
        skip_serializing_if = "is_default_priorities"
    )]
    pub priorities: Vec<String>,
}

/// The default priority labels.
pub fn default_priorities() -> Vec<String> {
    vec!["A".into(), "B".into(), "C".into()]
}
fn is_default_priorities(p: &[String]) -> bool {
    p == default_priorities().as_slice()
}

impl Default for TasksSection {
    fn default() -> Self {
        TasksSection {
            completed_log_limit: 0,
            priorities: default_priorities(),
        }
    }
}

/// Checks priority labels: one to nine, each 1 to 8 characters without spaces, all different (ignoring case).
pub fn validate_priorities(labels: &[String]) -> std::result::Result<(), String> {
    if labels.is_empty() || labels.len() > 9 {
        return Err(
            "give between one and nine priority labels, most important first (A, B, C)".into(),
        );
    }
    let mut seen: Vec<String> = Vec::new();
    for l in labels {
        let l = l.trim();
        if l.is_empty()
            || l.chars().count() > 8
            || l.chars().any(char::is_whitespace)
            || l.contains(['[', ']', '#', ':', ','])
        {
            return Err(format!(
                "`{l}` is not a usable priority label (1 to 8 characters, no spaces or `[]#:,`)"
            ));
        }
        let k = l.to_lowercase();
        if seen.contains(&k) {
            return Err(format!("the label `{l}` is used twice"));
        }
        seen.push(k);
    }
    Ok(())
}

/// `[calendar]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarSection {
    /// `vtodo` or `vevent`.
    #[serde(default = "default_tasks_as")]
    pub tasks_as: String,
    /// `complete` or `delete`.
    #[serde(default = "default_on_delete")]
    pub on_delete: String,
    /// `off` (default), `journal` or `inbox`: tasks created in other apps become blocks in a section of today's
    /// journal, or on the inbox page (CA-5, CA-12).
    #[serde(default = "default_import")]
    pub import: String,
    /// The page those tasks go to with `import = "inbox"`.
    #[serde(default = "default_inbox")]
    pub inbox: String,
    /// The heading of the section of today's journal they go under with `import = "journal"`.
    #[serde(default = "default_import_section")]
    pub import_section: String,
    /// `open` (default `none`): open tasks without a date are sent too, so the calendar holds every open task.
    #[serde(default = "default_undated")]
    pub undated: String,
    /// Colours of calendars in the calendar view, `#rrggbb` by calendar `href`.
    #[serde(default)]
    pub colors: std::collections::BTreeMap<String, String>,
}

fn default_tasks_as() -> String {
    "vtodo".into()
}
fn default_on_delete() -> String {
    "complete".into()
}
fn default_import() -> String {
    "off".into()
}
fn default_undated() -> String {
    "none".into()
}
fn default_import_section() -> String {
    "Synced tasks".into()
}
fn default_inbox() -> String {
    "inbox".into()
}

impl Default for CalendarSection {
    fn default() -> Self {
        CalendarSection {
            tasks_as: default_tasks_as(),
            on_delete: default_on_delete(),
            import: default_import(),
            inbox: default_inbox(),
            import_section: default_import_section(),
            undated: default_undated(),
            colors: Default::default(),
        }
    }
}

/// `[encryption]` (EN-11, EN-17)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptionSection {
    /// `marked` (default): only `paths` are stored encrypted; `all`: everything is, except `except` (EN-27).
    #[serde(default = "default_encryption_scope")]
    pub scope: String,
    /// With `scope = "all"`: the paths that stay plain. Same entries as `paths`. Not used with `marked`, but kept.
    #[serde(default)]
    pub except: Vec<String>,
    /// Paths relative to the graph folder that are stored encrypted on the server: a path ending in `/` is a folder
    /// with everything below it, any other path is one file.
    #[serde(default)]
    pub paths: Vec<String>,
    /// `exclude` (default): tasks of encrypted pages are not sent to the calendar; `allow` sends them.
    #[serde(default = "default_encryption_calendar")]
    pub calendar: String,
    /// `visible` (default): an encrypted file is stored as `<path>.enc`; `hidden`: as an object with a name that
    /// reveals nothing, the path being inside (EN-25).
    #[serde(default = "default_encryption_names")]
    pub names: String,
}

fn default_encryption_scope() -> String {
    "marked".into()
}

fn default_encryption_names() -> String {
    "visible".into()
}

fn default_encryption_calendar() -> String {
    "exclude".into()
}

impl Default for EncryptionSection {
    fn default() -> Self {
        EncryptionSection {
            scope: default_encryption_scope(),
            except: Vec::new(),
            paths: Vec::new(),
            calendar: default_encryption_calendar(),
            names: default_encryption_names(),
        }
    }
}

impl EncryptionSection {
    /// Which files are stored encrypted on the server (EN-11, EN-27).
    pub fn rules(&self) -> notatnik_core::encrypted::EncryptedPaths {
        use notatnik_core::encrypted::EncryptedPaths;
        if self.scope == "all" {
            EncryptedPaths::everything_except(&self.except)
        } else {
            EncryptedPaths::new(&self.paths)
        }
    }

    fn is_default(&self) -> bool {
        *self == EncryptionSection::default()
    }
}

const KNOWN: &[(&str, &[&str])] = &[
    (
        "encryption",
        &["scope", "except", "paths", "calendar", "names"],
    ),
    ("graph", &["id", "journal_template", "week_start"]),
    ("properties", &["hidden", "types"]),
    ("tasks", &["completed_log_limit", "priorities"]),
    (
        "calendar",
        &[
            "tasks_as",
            "on_delete",
            "import",
            "inbox",
            "import_section",
            "undated",
            "colors",
        ],
    ),
];

impl Config {
    /// Parses `config.toml` text. Unknown keys don't fail the parse; they are returned as warnings.
    pub fn parse(text: &str) -> Result<(Config, Vec<String>)> {
        let table: toml::Table = text
            .parse()
            .map_err(|e: toml::de::Error| Error::Config(e.to_string()))?;
        let mut warnings = Vec::new();
        for (k, v) in &table {
            match KNOWN.iter().find(|(name, _)| name == k) {
                Some((_, keys)) => {
                    if let Some(t) = v.as_table() {
                        for sub in t.keys() {
                            if !keys.contains(&sub.as_str()) {
                                warnings.push(format!("unknown config key `{k}.{sub}`"));
                            }
                        }
                    }
                }
                None if k == "saved_queries"
                    || k == "page_icons"
                    || k == "favorites"
                    || k == "hidden" => {}
                None if k == "namespaces" => {
                    if let Some(t) = v.as_table() {
                        for (ns, nv) in t {
                            if let Some(nt) = nv.as_table() {
                                for sub in nt.keys().filter(|s| *s != "template") {
                                    warnings.push(format!(
                                        "unknown config key `namespaces.{ns}.{sub}`"
                                    ));
                                }
                            }
                        }
                    }
                }
                None => warnings.push(format!("unknown config key `{k}`")),
            }
        }
        let cfg: Config = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        for (k, v) in &cfg.properties.types {
            if parse_prop_type(v).is_none() {
                warnings.push(format!("unknown property type `{v}` for `{k}`"));
            }
        }
        if !matches!(cfg.encryption.calendar.as_str(), "exclude" | "allow") {
            warnings.push(format!(
                "`encryption.calendar` must be exclude or allow, found `{}`",
                cfg.encryption.calendar
            ));
        }
        if !matches!(cfg.encryption.names.as_str(), "visible" | "hidden") {
            warnings.push(format!(
                "`encryption.names` must be visible or hidden, found `{}`",
                cfg.encryption.names
            ));
        }
        if !matches!(cfg.encryption.scope.as_str(), "marked" | "all") {
            warnings.push(format!(
                "`encryption.scope` must be marked or all, found `{}`",
                cfg.encryption.scope
            ));
        }
        for (key, list) in [
            ("paths", &cfg.encryption.paths),
            ("except", &cfg.encryption.except),
        ] {
            for p in list {
                if let Err(why) = notatnik_core::encrypted::check_path(p) {
                    warnings.push(format!("`encryption.{key}`: `{p}` {why}; it is ignored"));
                }
            }
        }
        if !matches!(cfg.graph.week_start.as_str(), "monday" | "sunday") {
            warnings.push(format!(
                "`graph.week_start` must be monday or sunday, found `{}`",
                cfg.graph.week_start
            ));
        }
        Ok((cfg, warnings))
    }

    /// Serialises to TOML.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// The priority labels in use: `[tasks] priorities` when valid, else `A`, `B`, `C`.
    pub fn priority_labels(&self) -> Vec<String> {
        if validate_priorities(&self.tasks.priorities).is_ok() {
            self.tasks
                .priorities
                .iter()
                .map(|l| l.trim().to_string())
                .collect()
        } else {
            default_priorities()
        }
    }

    /// Parser options derived from the config.
    pub fn parse_options(&self) -> notatnik_core::ParseOptions {
        notatnik_core::ParseOptions {
            property_types: self.property_types(),
            priority_labels: self.priority_labels(),
        }
    }

    /// Declared property types as the parser wants them.
    pub fn property_types(&self) -> BTreeMap<String, PropType> {
        self.properties
            .types
            .iter()
            .filter_map(|(k, v)| parse_prop_type(v).map(|t| (k.clone(), t)))
            .collect()
    }

    /// First day of the week.
    pub fn week_start(&self) -> WeekStart {
        if self.graph.week_start == "sunday" {
            WeekStart::Sunday
        } else {
            WeekStart::Monday
        }
    }
}

fn parse_prop_type(s: &str) -> Option<PropType> {
    Some(match s {
        "text" => PropType::Text,
        "number" => PropType::Number,
        "date" => PropType::Date,
        "datetime" => PropType::DateTime,
        "bool" => PropType::Bool,
        "page" => PropType::Page,
        _ => return None,
    })
}

/// A random-looking ID like `g7f3k2` (time, pid and a counter hashed; no RNG dependency).
pub fn new_graph_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let h = blake3::hash(format!("{nanos}-{}-{n}", std::process::id()).as_bytes());
    const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut id = String::from("g");
    for b in h.as_bytes().iter().take(6) {
        id.push(char::from(ALPHABET[usize::from(*b) % 36]));
    }
    id
}

/// Per-device directories. Nothing here is ever synced.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// `~/.config/notatnik`
    pub config: PathBuf,
    /// `~/.local/share/notatnik`
    pub data: PathBuf,
}

impl Dirs {
    /// XDG locations from the environment (`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `HOME`).
    pub fn from_env() -> Dirs {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let var = |name: &str, fallback: &str| -> PathBuf {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map_or_else(|| home.join(fallback), PathBuf::from)
        };
        Dirs {
            config: var("XDG_CONFIG_HOME", ".config").join("notatnik"),
            data: var("XDG_DATA_HOME", ".local/share").join("notatnik"),
        }
    }

    /// Both directories below one root (tests).
    pub fn under(root: &Path) -> Dirs {
        Dirs {
            config: root.join("config"),
            data: root.join("data"),
        }
    }

    /// Directory with the cache of one graph.
    pub fn graph_data(&self, graph_id: &str) -> PathBuf {
        self.data.join("graphs").join(graph_id)
    }
}

/// One known graph in `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEntry {
    /// Graph ID.
    pub id: String,
    /// Absolute folder path on this device. For a graph that lives on a server this is its local copy (the replica).
    pub path: String,
    /// The server folder the graph is kept in step with, if any (SY-1). The password is not here: it is in the
    /// system's keyring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteConfig>,
    /// Calendar sync of this graph's dated tasks (phase 4), if set up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<CalendarConfig>,
    /// The color the person gave the graph (an index into the palette, `0..GRAPH_COLORS`); without one the color
    /// comes from the graph's ID (MG-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
}

/// How many colors a graph can have (the palette is in the UI).
pub const GRAPH_COLORS: u8 = 8;

/// The color of a graph that has none chosen: the same every time, from its ID.
pub fn default_graph_color(id: &str) -> u8 {
    let mut h: u32 = 2_166_136_261;
    for b in id.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    (h % u32::from(GRAPH_COLORS)) as u8
}

/// The calendar server login of a graph that has its own (the default is the login of the graph's sync).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarAccount {
    /// The server, e.g. `https://cloud.example.org`.
    pub server: String,
    /// The login name; the password is in the system's keyring.
    pub user: String,
}

/// Calendar sync settings of a graph on this device (CA-1, CA-2). What a deleted task does and whether tasks are
/// VTODO or VEVENT is the graph's `[calendar]` section, which syncs with the graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarConfig {
    /// Sync is on.
    #[serde(default)]
    pub enabled: bool,
    /// Switched off for now: nothing is sent or read until it is resumed; the choices stay.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// Calendars (their `href`) to read events from.
    #[serde(default)]
    pub read: Vec<String>,
    /// The calendar (its `href`) that receives the tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<String>,
    /// More calendars (their `href`) that tasks can go to; a task names one with `cal:: <name>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more: Vec<String>,
    /// The names of the calendars of `tasks` and `more`, by `href` (what `cal::` is matched against).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub names: std::collections::BTreeMap<String, String>,
    /// A login of its own; none means the login of the graph's sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<CalendarAccount>,
}

/// A WebDAV folder a graph is synced with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// The address of the folder, e.g. `https://cloud.example.org/remote.php/dav/files/me/Notes/graph`.
    pub url: String,
    /// The login name.
    pub user: String,
    /// The server's folder ETags change whenever something below changes (found by the connection test).
    #[serde(default)]
    pub trust_folder_etags: bool,
    /// Merge a page that changed on both sides when the merge is clean (ST-6); off keeps a "conflicted copy".
    #[serde(default = "yes")]
    pub merge: bool,
}

fn yes() -> bool {
    true
}

/// A server the "link a file" picker can browse (a device setting; the password is in the system's keyring).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSource {
    /// Stable ID, the keyring entry is named after it.
    pub id: String,
    /// The name shown in the picker.
    pub name: String,
    /// The server (`https://cloud.example.org`) or the full WebDAV address up to `…/remote.php/dav/files/<user>`.
    pub server: String,
    /// The login name.
    pub user: String,
}

/// Per-device appearance settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettings {
    /// `system`, `light` or `dark`.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// BCP 47 locale used to format dates (`en-GB`, `pl-PL`, ...).
    #[serde(default = "default_date_locale")]
    pub date_locale: String,
    /// Show `[[ ]]` around rendered page links (they are always visible while a block is edited).
    #[serde(default)]
    pub link_brackets: bool,
    /// Pasting a web address on its own looks up the page's title and writes `[title](address)`. Off by default:
    /// the app then contacts the site.
    #[serde(default)]
    pub paste_titles: bool,
    /// How page icons (emoji) are drawn: `color` as they are, `muted` (default) quieter in lists and in full color in
    /// titles, on hover and on the current page, or `mono` grey everywhere.
    #[serde(default = "default_icon_style")]
    pub icon_style: String,
    /// Task details (schedule, deadline, repeat, logbook) under a task on pages: `full` (always shown) or `compact`
    /// (one small indicator in the task's row, the details on hover, focus or tap).
    #[serde(default = "default_task_details")]
    pub task_details: String,
    /// Block properties (`key:: value` lines) under a block on pages: `full` or `compact` (one small indicator in
    /// the block's row, the property card in a popover).
    #[serde(default = "default_task_details")]
    pub block_properties: String,
    /// How dates are written: `full` (Sat, 3 Oct 2026), `long` (3 October 2026), `short` (03/10/2026, order by
    /// locale), `iso` (2026-10-03) or `custom` (use `date_pattern`).
    #[serde(default = "default_date_format")]
    pub date_format: String,
    /// Pattern for `date_format = "custom"`, e.g. `dddd, D MMMM YYYY`. Tokens: see `docs/date-formats.md`.
    #[serde(default)]
    pub date_pattern: String,
    /// Folder for graph snapshots (BK-2); empty = `<data dir>/backups`. Never inside a graph folder.
    #[serde(default)]
    pub backup_dir: String,
    /// Take a snapshot at start and then daily while the app is open, when something changed (BK-3).
    #[serde(default = "default_true")]
    pub backup_auto: bool,
    /// `compressed` (zip deflate) or `none` (stored without compression) for new snapshots (BK-7).
    #[serde(default = "default_compression")]
    pub backup_compression: String,
    /// Seal new snapshots to the backup vault (EN-5). Turned on only after the vault was set up and its recovery
    /// key confirmed; while it is on, a backup that cannot be sealed fails instead of being written in plain.
    #[serde(default)]
    pub backup_encrypt: bool,
    /// A second folder that gets a copy of every snapshot (BK-9); empty = none. Never inside a graph folder.
    #[serde(default)]
    pub backup_mirror_dir: String,
    /// Setting `type` on a page adds the properties of `templates/<type>.md` it lacks (PR-10).
    #[serde(default = "default_true")]
    pub templates_auto: bool,
    /// Open the last graph at start without showing the start screen (false: the start screen lists the graphs with
    /// the last one selected).
    #[serde(default)]
    pub open_last_graph: bool,
}

/// The accepted values of `UiSettings::date_format`.
pub const DATE_FORMATS: [&str; 5] = ["full", "long", "short", "iso", "custom"];

fn default_true() -> bool {
    true
}

fn default_compression() -> String {
    "compressed".into()
}

fn default_icon_style() -> String {
    "muted".into()
}

fn default_task_details() -> String {
    "full".into()
}

fn default_date_format() -> String {
    "full".into()
}

fn default_theme() -> String {
    "system".into()
}
fn default_date_locale() -> String {
    "en-GB".into()
}

impl Default for UiSettings {
    fn default() -> Self {
        UiSettings {
            theme: default_theme(),
            date_locale: default_date_locale(),
            link_brackets: false,
            paste_titles: false,
            icon_style: default_icon_style(),
            task_details: default_task_details(),
            block_properties: default_task_details(),
            date_format: default_date_format(),
            date_pattern: String::new(),
            backup_dir: String::new(),
            backup_auto: true,
            backup_compression: default_compression(),
            backup_encrypt: false,
            backup_mirror_dir: String::new(),
            templates_auto: true,
            open_last_graph: false,
        }
    }
}

/// `~/.config/notatnik/settings.toml`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeviceSettings {
    /// Known graphs: the history of the graphs opened in the app, most recently opened last.
    #[serde(default)]
    pub graphs: Vec<GraphEntry>,
    /// The ID of the graph that was open last (the app starts with it selected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_graph: Option<String>,
    /// Appearance.
    #[serde(default)]
    pub ui: UiSettings,
    /// Servers to link files from (besides the one the open graph is synced with).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_sources: Vec<FileSource>,
}

impl DeviceSettings {
    /// Parses the text of a settings file.
    pub fn parse(text: &str) -> Result<DeviceSettings> {
        toml::from_str(text).map_err(|e| Error::Config(e.to_string()))
    }

    /// Reads the settings (empty if missing).
    pub fn load(dirs: &Dirs) -> Result<DeviceSettings> {
        let path = dirs.config.join("settings.toml");
        match std::fs::read_to_string(&path) {
            Ok(t) => {
                toml::from_str(&t).map_err(|e| Error::Config(format!("{}: {e}", path.display())))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DeviceSettings::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Writes the settings.
    pub fn save(&self, dirs: &Dirs) -> Result<()> {
        std::fs::create_dir_all(&dirs.config)?;
        std::fs::write(
            dirs.config.join("settings.toml"),
            toml::to_string_pretty(self).unwrap_or_default(),
        )?;
        Ok(())
    }

    /// Adds or updates a graph entry and makes it the last opened: it moves to the end of the history.
    pub fn register(&mut self, id: &str, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        let remote = self.remote_of(id);
        let calendar = self.calendar_of(id);
        let color = self
            .graphs
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| g.color);
        self.graphs.retain(|g| g.id != id);
        self.graphs.push(GraphEntry {
            id: id.to_string(),
            path,
            remote,
            calendar,
            color,
        });
        self.last_graph = Some(id.to_string());
    }

    /// The server folder of a graph, if it has one.
    pub fn remote_of(&self, id: &str) -> Option<RemoteConfig> {
        self.graphs
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| g.remote.clone())
    }

    /// The calendar settings of a graph, if it has any.
    pub fn calendar_of(&self, id: &str) -> Option<CalendarConfig> {
        self.graphs
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| g.calendar.clone())
    }

    /// Sets the calendar settings of a graph that is in the history.
    pub fn set_calendar(&mut self, id: &str, calendar: Option<CalendarConfig>) {
        if let Some(g) = self.graphs.iter_mut().find(|g| g.id == id) {
            g.calendar = calendar;
        }
    }

    /// Sets or removes the server folder of a graph that is in the history.
    pub fn set_remote(&mut self, id: &str, remote: Option<RemoteConfig>) {
        if let Some(g) = self.graphs.iter_mut().find(|g| g.id == id) {
            g.remote = remote;
        }
    }

    /// The color of a graph: the one the person chose, otherwise one from the graph's ID (MG-5).
    pub fn color_of(&self, id: &str) -> u8 {
        self.graphs
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| g.color)
            .filter(|c| *c < GRAPH_COLORS)
            .unwrap_or_else(|| default_graph_color(id))
    }

    /// Sets (`Some`) or clears (`None`) the color of a known graph. `false` when the color is not in the palette.
    pub fn set_color(&mut self, id: &str, color: Option<u8>) -> bool {
        if color.is_some_and(|c| c >= GRAPH_COLORS) {
            return false;
        }
        if let Some(g) = self.graphs.iter_mut().find(|g| g.id == id) {
            g.color = color;
        }
        true
    }

    /// Removes a graph from the history (its files are not touched).
    pub fn forget(&mut self, id: &str) {
        self.graphs.retain(|g| g.id != id);
        if self.last_graph.as_deref() == Some(id) {
            self.last_graph = None;
        }
    }

    /// The folder of the last opened graph, if it is still in the history.
    pub fn last_path(&self) -> Option<&str> {
        let id = self.last_graph.as_deref()?;
        self.graphs
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.path.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graph_color_is_the_same_every_time_and_in_the_palette() {
        let ids = ["gvu3dno", "a1b2c3d", "zzzzzzz", "0000000", "x9y8z7w", "q"];
        let colors: Vec<u8> = ids.iter().map(|i| default_graph_color(i)).collect();
        assert!(colors.iter().all(|c| *c < GRAPH_COLORS));
        assert_eq!(
            colors,
            ids.iter()
                .map(|i| default_graph_color(i))
                .collect::<Vec<_>>()
        );
        assert!(
            colors
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                >= 3,
            "ids spread over the palette: {colors:?}"
        );
        let mut dev = DeviceSettings::default();
        dev.register("gvu3dno", Path::new("/a"));
        assert_eq!(dev.color_of("gvu3dno"), default_graph_color("gvu3dno"));
        assert!(dev.set_color("gvu3dno", Some(7)));
        dev.register("gvu3dno", Path::new("/moved"));
        assert_eq!(
            dev.color_of("gvu3dno"),
            7,
            "registering again keeps the choice"
        );
        assert!(!dev.set_color("gvu3dno", Some(8)));
        assert_eq!(dev.color_of("gvu3dno"), 7);
        assert!(dev.set_color("gvu3dno", None));
        assert_eq!(dev.color_of("gvu3dno"), default_graph_color("gvu3dno"));
    }

    #[test]
    fn parses_and_warns_about_unknown_keys() {
        let text = "[graph]\nid = \"g1\"\nweek_start = \"sunday\"\nbogus = 1\n[properties]\ntypes = { rating = \"number\", x = \"blob\" }\n[nonsense]\na = 1\n[namespaces.people]\ntemplate = \"t.md\"\nzzz = 1\n";
        let (c, w) = Config::parse(text).unwrap();
        assert_eq!(c.graph.id, "g1");
        assert_eq!(c.week_start(), WeekStart::Sunday);
        assert_eq!(c.property_types().len(), 1);
        assert!(w.iter().any(|m| m.contains("graph.bogus")));
        assert!(w.iter().any(|m| m.contains("`nonsense`")));
        assert!(w.iter().any(|m| m.contains("namespaces.people.zzz")));
        assert!(w.iter().any(|m| m.contains("blob")));
    }

    #[test]
    fn defaults_and_round_trip() {
        let (c, w) = Config::parse("").unwrap();
        assert!(w.is_empty());
        assert_eq!(c.properties.hidden, ["done", "completed"]);
        assert_eq!(c.graph.week_start, "monday");
        let mut c2 = c.clone();
        c2.graph.id = new_graph_id();
        assert_eq!(Config::parse(&c2.to_toml()).unwrap().0, c2);
        assert_ne!(new_graph_id(), new_graph_id());
        assert!(Config::parse("[graph\nbroken").is_err());
    }
}

#[cfg(test)]
mod priority_tests {
    use super::*;

    #[test]
    fn labels_are_validated_and_default_to_abc() {
        assert!(validate_priorities(&["A".into(), "B".into(), "C".into()]).is_ok());
        assert!(validate_priorities(&["1".into(), "2".into(), "3".into()]).is_ok());
        assert!(validate_priorities(&["A".into(), "B".into()]).is_ok());
        assert!(validate_priorities(&[]).is_err());
        assert!(validate_priorities(&(1..=10).map(|n| n.to_string()).collect::<Vec<_>>()).is_err());
        assert!(validate_priorities(&["A".into(), "a".into(), "C".into()]).is_err());
        assert!(validate_priorities(&["A B".into(), "B".into(), "C".into()]).is_err());
        let (c, _) = Config::parse("[tasks]\npriorities = [\"!\", \"!!\", \"!!!\"]\n").unwrap();
        assert_eq!(c.priority_labels(), ["!", "!!", "!!!"]);
        // an invalid list falls back to A, B, C instead of breaking the graph
        let (c, _) = Config::parse("[tasks]\npriorities = [\"a b\"]\n").unwrap();
        assert_eq!(c.priority_labels(), ["A", "B", "C"]);
        let (c, _) = Config::parse("[tasks]\npriorities = [\"1\", \"2\", \"3\", \"4\"]\n").unwrap();
        assert_eq!(c.priority_labels().len(), 4);
        // the default is not written out
        assert!(!Config::default().to_toml().contains("priorities"));
    }
}

#[cfg(test)]
mod encryption_tests {
    use super::*;

    #[test]
    fn hidden_is_a_list_before_the_tables() {
        let (c, w) =
            Config::parse("hidden = [\"work\", \"ref/old\"]\n\n[graph]\nid = \"g1\"\n").unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.hidden, ["work", "ref/old"]);
        let text = c.to_toml();
        assert!(
            text.find("hidden").unwrap() < text.find("[graph]").unwrap(),
            "{text}"
        );
        let empty = Config::default().to_toml();
        let top = empty.split("\n[").next().unwrap_or("");
        assert!(!top.contains("hidden"), "{top}");
    }

    #[test]
    fn favorites_are_a_list_before_the_tables() {
        let (c, w) =
            Config::parse("favorites = [\"Notatki\", \"P/Capgemini\"]\n\n[graph]\nid = \"g1\"\n")
                .unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.favorites, ["Notatki", "P/Capgemini"]);
        let text = c.to_toml();
        assert!(
            text.find("favorites").unwrap() < text.find("[graph]").unwrap(),
            "{text}"
        );
        assert_eq!(Config::parse(&text).unwrap().0, c);
        // none: not written
        assert!(!Config::default().to_toml().contains("favorites"));
    }

    #[test]
    fn the_scope_chooses_which_list_decides() {
        let text = "[encryption]\nscope = \"all\"\nexcept = [\"pages/public/\"]\npaths = [\"pages/private/\"]\n";
        let (c, w) = Config::parse(text).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(
            (c.encryption.scope.as_str(), c.encryption.except.len()),
            ("all", 1)
        );
        let r = c.encryption.rules();
        assert!(r.is_encrypted("pages/private/a.md") && r.is_encrypted("pages/a.md"));
        assert!(!r.is_encrypted("pages/public/a.md"));
        assert_eq!(Config::parse(&c.to_toml()).unwrap().0, c);
        // `marked` (the default) ignores `except` and keeps `paths`
        let (c, _) =
            Config::parse("[encryption]\nexcept = [\"x/\"]\npaths = [\"pages/private/\"]\n")
                .unwrap();
        assert_eq!(c.encryption.scope, "marked");
        assert!(!c.encryption.rules().is_encrypted("pages/a.md"));
        assert!(c.encryption.rules().is_encrypted("pages/private/a.md"));
        let (_, w) =
            Config::parse("[encryption]\nscope = \"some\"\nexcept = [\"../x\", \"ok/\"]\n")
                .unwrap();
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(
            w.iter().any(|m| m.contains("encryption.scope"))
                && w.iter().any(|m| m.contains("encryption.except"))
        );
    }

    #[test]
    fn the_section_is_read_written_and_checked() {
        let text = "[encryption]\npaths = [\"pages/private/\", \"pages/Health.md\"]\ncalendar = \"allow\"\nnames = \"hidden\"\n";
        let (c, w) = Config::parse(text).unwrap();
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(c.encryption.paths, ["pages/private/", "pages/Health.md"]);
        assert_eq!(c.encryption.calendar, "allow");
        assert_eq!(c.encryption.names, "hidden");
        assert_eq!(Config::default().encryption.names, "visible");
        assert_eq!(Config::parse(&c.to_toml()).unwrap().0, c);
        // the default is not written out, so a graph without encryption keeps its file as it was
        assert!(!Config::default().to_toml().contains("encryption"));
        assert_eq!(Config::default().encryption.calendar, "exclude");
        let (_, w) = Config::parse(
            "[encryption]\npaths = [\"../x/\", \".notatnik/a\", \"ok/\"]\ncalendar = \"maybe\"\nnames = \"secret\"\nmore = 1\n",
        )
        .unwrap();
        assert_eq!(w.len(), 5, "{w:?}");
    }
}
