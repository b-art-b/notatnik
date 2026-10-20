//! Which files of a graph are stored encrypted on a server (EN-11, EN-12): the list of paths in
//! `[encryption] paths`, the matching, and the name of the encrypted copy. Pure; the encryption itself is
//! `notatnik-crypt`.

/// The ending of the name an encrypted copy has on the server: `pages/a.md` is stored as `pages/a.md.enc`.
pub const ENC_SUFFIX: &str = ".enc";

/// Why an entry of `[encryption] paths` cannot be used, if it cannot.
pub fn check_path(p: &str) -> Result<(), &'static str> {
    let body = p.strip_suffix('/').unwrap_or(p);
    if body.is_empty() {
        Err("is empty")
    } else if p.starts_with('/') || p.contains('\\') {
        Err("must be relative to the graph folder, with `/` between the parts")
    } else if body
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..")
    {
        Err("has an empty part, `.` or `..`")
    } else if body == ".notatnik" || body.starts_with(".notatnik/") {
        Err("is inside `.notatnik/`, which is never encrypted")
    } else {
        Ok(())
    }
}

/// The server name of the encrypted copy of `path`.
pub fn remote_name(path: &str) -> String {
    format!("{path}{ENC_SUFFIX}")
}

/// The folder on the server that holds the objects of marked files when their names are hidden (EN-25). A local
/// file below it is never synced.
pub const HIDDEN_DIR: &str = ".notatnik-enc";

/// Where the object `name` (the 32 hex characters of `Keys::file_name`) lives on the server.
pub fn hidden_object(name: &str) -> String {
    format!(
        "{HIDDEN_DIR}/{}/{name}{ENC_SUFFIX}",
        &name[..2.min(name.len())]
    )
}

/// The object name of a server path that has the shape of [`hidden_object`], else `None`.
pub fn hidden_name(remote: &str) -> Option<&str> {
    let rest = remote.strip_prefix(HIDDEN_DIR)?.strip_prefix('/')?;
    let (shard, file) = rest.split_once('/')?;
    let name = file.strip_suffix(ENC_SUFFIX)?;
    let hex = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    (shard.len() == 2 && name.len() == 32 && hex(shard) && hex(name) && name.starts_with(shard))
        .then_some(name)
}

/// The path an encrypted server file is the copy of, or `None` for a name that does not end in `.enc`.
pub fn plain_name(remote: &str) -> Option<&str> {
    remote.strip_suffix(ENC_SUFFIX).filter(|p| !p.is_empty())
}

/// The file a conflict copy was made from (`a (conflicted copy 2026-10-03 120000).md` is a copy of `a.md`), or
/// `None` when the name is not that of a conflict copy. The copy is stored like its original (EN-12).
pub fn conflict_original(path: &str) -> Option<String> {
    let (dir, file) = path.rsplit_once('/').map_or(("", path), |(d, f)| (d, f));
    let marker = " (conflicted copy ";
    let i = file.rfind(marker)?;
    let rest = &file[i + marker.len()..];
    let close = rest.find(')')?;
    // `YYYY-MM-DD HHMMSS` and perhaps a number: starts with a year
    if !rest[..close].chars().take(4).all(|c| c.is_ascii_digit()) || close < 4 {
        return None;
    }
    let original = format!("{}{}", &file[..i], &rest[close + 1..]);
    Some(if dir.is_empty() {
        original
    } else {
        format!("{dir}/{original}")
    })
}

/// The paths marked as encrypted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncryptedPaths {
    files: Vec<String>,
    /// With the trailing `/`.
    folders: Vec<String>,
}

impl EncryptedPaths {
    /// The rules of `[encryption] paths`; entries that cannot be used are left out.
    pub fn new(paths: &[String]) -> EncryptedPaths {
        let mut r = EncryptedPaths::default();
        for p in paths {
            if check_path(p).is_err() {
                continue;
            }
            if p.ends_with('/') {
                r.folders.push(p.clone());
            } else {
                r.files.push(p.clone());
            }
        }
        r
    }

    /// Nothing is marked.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.folders.is_empty()
    }

    /// Whether the file at `path` (relative to the graph folder) is stored encrypted. Nothing below `.notatnik/` is,
    /// and a conflict copy follows the file it came from.
    pub fn is_encrypted(&self, path: &str) -> bool {
        if path == ".notatnik" || path.starts_with(".notatnik/") {
            return false;
        }
        self.matches(path) || conflict_original(path).is_some_and(|o| self.matches(&o))
    }

    fn matches(&self, path: &str) -> bool {
        self.files.iter().any(|f| f == path) || self.folders.iter().any(|d| path.starts_with(d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_objects_have_a_fixed_shape() {
        let n = "ab0123456789abcdef0123456789abcd";
        let o = hidden_object(n);
        assert_eq!(o, ".notatnik-enc/ab/ab0123456789abcdef0123456789abcd.enc");
        assert_eq!(hidden_name(&o), Some(n));
        for bad in [
            "pages/a.md.enc",
            ".notatnik-enc/ab/short.enc",
            ".notatnik-enc/cd/ab0123456789abcdef0123456789abcd.enc",
            ".notatnik-enc/ab/AB0123456789abcdef0123456789abcd.enc",
            ".notatnik-enc/ab0123456789abcdef0123456789abcd.enc",
        ] {
            assert_eq!(hidden_name(bad), None, "{bad}");
        }
    }

    fn rules(p: &[&str]) -> EncryptedPaths {
        EncryptedPaths::new(&p.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_folder_covers_everything_below_and_a_file_only_itself() {
        let r = rules(&["pages/private/", "pages/Health.md"]);
        for yes in [
            "pages/private/a.md",
            "pages/private/deep/er/b.md",
            "pages/Health.md",
        ] {
            assert!(r.is_encrypted(yes), "{yes}");
        }
        for no in [
            "pages/private-x.md",
            "pages/privateer/a.md",
            "pages/private",
            "pages/Health.md.bak",
            "pages/health.md",
            "pages/Health/sub.md",
            "journals/2026-10-03.md",
            "assets/private/x.png",
        ] {
            assert!(!r.is_encrypted(no), "{no}");
        }
    }

    #[test]
    fn nothing_below_the_graph_settings_is_encrypted_and_bad_entries_are_ignored() {
        let r = rules(&[
            ".notatnik/",
            ".notatnik/config.toml",
            "/etc/",
            "../x/",
            "a//b/",
            "",
            "/",
            "pages\\x.md",
            "ok/",
        ]);
        assert!(!r.is_encrypted(".notatnik/config.toml"));
        assert!(!r.is_encrypted(".notatnik/vault.json"));
        assert!(r.is_encrypted("ok/x.md"));
        assert!(!r.is_encrypted("etc/x"));
        assert!(!rules(&[]).is_encrypted("pages/a.md"));
        assert!(rules(&[]).is_empty() && !r.is_empty());
        assert!(check_path("a/b.md").is_ok());
        assert!(check_path("a/./b").is_err());
    }

    #[test]
    fn unicode_names_and_case_are_compared_as_written() {
        let r = rules(&["pages/Zażółć/", "pages/Straße.md"]);
        assert!(r.is_encrypted("pages/Zażółć/gęślą.md"));
        assert!(r.is_encrypted("pages/Straße.md"));
        assert!(!r.is_encrypted("pages/STRASSE.md"));
        assert!(!r.is_encrypted("pages/zażółć/x.md"));
    }

    #[test]
    fn a_conflict_copy_is_stored_like_the_file_it_came_from() {
        let r = rules(&["pages/Health.md", "pages/private/"]);
        assert_eq!(
            conflict_original("pages/Health (conflicted copy 2026-10-03 120000).md").as_deref(),
            Some("pages/Health.md")
        );
        assert_eq!(
            conflict_original("a (conflicted copy 2026-10-03 120000 2).md").as_deref(),
            Some("a.md")
        );
        assert_eq!(
            conflict_original("noext (conflicted copy 2026-10-03 120000)").as_deref(),
            Some("noext")
        );
        assert!(conflict_original("pages/a (conflicted copy of my life).md").is_none());
        assert!(conflict_original("pages/a.md").is_none());
        assert!(r.is_encrypted("pages/Health (conflicted copy 2026-10-03 120000).md"));
        assert!(r.is_encrypted("pages/private/x (conflicted copy 2026-10-03 120000).md"));
        assert!(!r.is_encrypted("pages/Other (conflicted copy 2026-10-03 120000).md"));
    }

    #[test]
    fn the_server_name_of_an_encrypted_copy() {
        assert_eq!(remote_name("pages/a.md"), "pages/a.md.enc");
        assert_eq!(plain_name("pages/a.md.enc"), Some("pages/a.md"));
        assert_eq!(plain_name("pages/a.md"), None);
        assert_eq!(plain_name(".enc"), None);
    }
}
