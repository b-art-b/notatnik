//! Favourite pages (NS-26): a list of page names in `favorites = [...]` of `.notatnik/config.toml`, so it syncs with
//! the graph. The sidebar shows it above the recent pages.

use super::*;

/// The most favourites there can be: a list of shortcuts, not a second page list.
pub(crate) const MAX_FAVORITES: usize = 20;

fn same(a: &str, b: &str) -> bool {
    PageName::parse(a)
        .and_then(|a| PageName::parse(b).map(|b| a.lookup_key() == b.lookup_key()))
        .unwrap_or(a == b)
}

/// A list of page names after `old` became `new`; with `subpages` the names below it move along.
pub(super) fn renamed_list(list: &[String], old: &str, new: &str, subpages: bool) -> Vec<String> {
    let prefix = format!("{old}/");
    list.iter()
        .map(|f| {
            if same(f, old) {
                new.to_string()
            } else if subpages
                && f.len() > prefix.len()
                && f.is_char_boundary(prefix.len())
                && f[..prefix.len()].to_lowercase() == prefix.to_lowercase()
            {
                format!("{new}/{}", &f[prefix.len()..])
            } else {
                f.clone()
            }
        })
        .collect()
}

impl AppState {
    fn favorites_of(o: &Open) -> Vec<FavoriteView> {
        o.graph
            .config()
            .favorites
            .into_iter()
            .map(|name| FavoriteView {
                path: o
                    .graph
                    .with_index(|i| i.page_info(&name))
                    .ok()
                    .flatten()
                    .and_then(|p| p.file_path),
                name,
            })
            .collect()
    }

    /// `favorites`
    pub fn favorites(&self, _a: NoArgs) -> ApiResult<Vec<FavoriteView>> {
        self.with_open(|o| Ok(Self::favorites_of(o)))
    }

    /// `favorite_set`: adds a page at the end of the list, or removes it. At most [`MAX_FAVORITES`].
    pub fn favorite_set(&self, a: FavoriteSetArgs) -> ApiResult<Vec<FavoriteView>> {
        let name = Self::parse_name(&a.page)?;
        self.with_open(|o| {
            let mut list = o.graph.config().favorites;
            let at = list.iter().position(|x| same(x, name.as_str()));
            match (a.favorite, at) {
                (true, None) => {
                    if list.len() >= MAX_FAVORITES {
                        return Err(ApiError::new(
                            "limit",
                            format!(
                                "there can be at most {MAX_FAVORITES} favorites; remove one first"
                            ),
                        ));
                    }
                    list.push(name.as_str().to_string());
                }
                (false, Some(i)) => {
                    list.remove(i);
                }
                _ => return Ok(Self::favorites_of(o)),
            }
            o.graph.update_config(|c| c.favorites = list)?;
            Ok(Self::favorites_of(o))
        })
    }

    /// `favorite_move`: puts a favourite at another place of the list.
    pub fn favorite_move(&self, a: FavoriteMoveArgs) -> ApiResult<Vec<FavoriteView>> {
        self.with_open(|o| {
            let mut list = o.graph.config().favorites;
            let Some(from) = list.iter().position(|x| same(x, &a.page)) else {
                return Err(ApiError::new(
                    "not_found",
                    format!("`{}` is not a favorite", a.page),
                ));
            };
            let item = list.remove(from);
            list.insert((a.to as usize).min(list.len()), item);
            o.graph.update_config(|c| c.favorites = list)?;
            Ok(Self::favorites_of(o))
        })
    }

    /// After a rename: the favourite `old` is `new`, and with `subpages` the favourites below it move along.
    pub(super) fn rename_favorites(o: &Open, old: &str, new: &str, subpages: bool) {
        let list = o.graph.config().favorites;
        let renamed = renamed_list(&list, old, new, subpages);
        if renamed != list {
            if let Err(e) = o.graph.update_config(|c| c.favorites = renamed) {
                log::warn!("favorites: could not follow a rename: {e}");
            }
        }
    }

    /// After a delete: the deleted pages are no favourites.
    pub(super) fn remove_favorites(o: &Open, names: &[String]) {
        let list = o.graph.config().favorites;
        let kept: Vec<String> = list
            .iter()
            .filter(|f| !names.iter().any(|n| same(f, n)))
            .cloned()
            .collect();
        if kept.len() != list.len() {
            if let Err(e) = o.graph.update_config(|c| c.favorites = kept) {
                log::warn!("favorites: could not drop a deleted page: {e}");
            }
        }
    }
}
