//! Logging to the terminal. The libraries only use the `log` macros; each binary (the desktop app, the dev
//! server, the CLI) calls [`init`] once. Without `RUST_LOG`, our own crates log at `default_level` and everything
//! else at `warn`.
//!
//! What is logged where, and what never is: `docs/logging.md`.

use std::io::Write;

/// The crates of this workspace, by the name `log` uses for their modules.
const OWN_CRATES: &[&str] = &[
    "notatnik_app",
    "notatnik_caldav",
    "notatnik_core",
    "notatnik_crypt",
    "notatnik_desktop",
    "notatnik_dev_server",
    "notatnik_graph",
    "notatnik_index",
    "notatnik_sync",
];

/// The filter used when `RUST_LOG` is not set: `warn,notatnik_app=info,...`.
fn default_filter(default_level: &str) -> String {
    let mut f = String::from("warn");
    for c in OWN_CRATES {
        f.push_str(&format!(",{c}={default_level}"));
    }
    f
}

/// Starts logging to stderr: `12:30:05.123 INFO  sync  pushed 2, pulled 1`. Safe to call twice (the second call
/// does nothing), which tests rely on.
pub fn init(default_level: &str) {
    let env = env_logger::Env::default().default_filter_or(default_filter(default_level));
    let _ = env_logger::Builder::from_env(env)
        .format(|buf, r| {
            let style = buf.default_level_style(r.level());
            // `notatnik_app::state::syncing` → `app::state::syncing`; other crates keep their path
            let target = r.target();
            let target = target.strip_prefix("notatnik_").unwrap_or(target);
            writeln!(
                buf,
                "{} {style}{:<5}{style:#} {target}  {}",
                chrono::Local::now().format("%H:%M:%S%.3f"),
                r.level(),
                r.args()
            )
        })
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::default_filter;

    #[test]
    fn own_crates_get_the_level_and_the_rest_warns() {
        let f = default_filter("info");
        assert!(f.starts_with("warn,"));
        assert!(f.contains("notatnik_sync=info"));
        assert!(!f.contains("ureq"));
    }
}
