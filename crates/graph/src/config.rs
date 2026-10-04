//! `.notatnik/config.toml` and per-device settings (file-format.md §8).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use notatnik_core::{PropType, WeekStart};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Parsed `config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
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
    /// `[[saved_queries]]`: queries pinned to the sidebar (they sync with the graph).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saved_queries: Vec<SavedQuery>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TasksSection {
    /// 0 = keep all completion dates of repeating tasks; N = keep the last N.
    #[serde(default)]
    pub completed_log_limit: u32,
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
}

fn default_tasks_as() -> String {
    "vtodo".into()
}
fn default_on_delete() -> String {
    "complete".into()
}

impl Default for CalendarSection {
    fn default() -> Self {
        CalendarSection {
            tasks_as: default_tasks_as(),
            on_delete: default_on_delete(),
        }
    }
}

const KNOWN: &[(&str, &[&str])] = &[
    ("graph", &["id", "journal_template", "week_start"]),
    ("properties", &["hidden", "types"]),
    ("tasks", &["completed_log_limit"]),
    ("calendar", &["tasks_as", "on_delete"]),
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
                None if k == "saved_queries" => {}
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
    /// Absolute folder path on this device.
    pub path: String,
}

/// `~/.config/notatnik/settings.toml`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeviceSettings {
    /// Known graphs.
    #[serde(default)]
    pub graphs: Vec<GraphEntry>,
}

impl DeviceSettings {
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

    /// Adds or updates a graph entry.
    pub fn register(&mut self, id: &str, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        match self.graphs.iter_mut().find(|g| g.id == id) {
            Some(g) => g.path = path,
            None => self.graphs.push(GraphEntry {
                id: id.to_string(),
                path,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
