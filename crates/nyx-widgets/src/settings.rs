//! The Control Center's layout file as a NyxOS configuration domain (`control-center`):
//! the panel's built-in layout, then NyxOS's defaults (`/etc/nyxos/defaults/control-center.toml`)
//! and an administrator's, then the user's `~/.config/nyx/control-center.toml`, which
//! holds only what the user changed. Shared by the GTK panel and the Quickshell backend.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Schema 1 is the first written by layer: a file from before held the whole layout, so
/// what equals the built-in one is dropped and follows the defaults.
pub const DOMAIN: nyx_config::Domain = nyx_config::Domain { name: "control-center", version: 1, migrations: &[nyx_config::drop_defaults] };

pub fn store<T: Serialize>(builtin: &T) -> nyx_config::Store {
    nyx_config::Store::new(DOMAIN, nyx_config::table_of(builtin))
}

/// The layout through every layer, `builtin` lowest. Problems are reported on stderr; a key
/// of the wrong type falls back on its own.
pub fn load<T: Serialize + DeserializeOwned + Default>(builtin: &T) -> T {
    load_from(&store(builtin))
}

pub fn load_from<T: Serialize + DeserializeOwned + Default>(store: &nyx_config::Store) -> T {
    let loaded = store.load();
    let (value, problems) = nyx_config::parse::<T>(&loaded);
    for p in problems.iter().chain(loaded.problems.iter().map(|p| &p.message)) {
        eprintln!("nyx-control-center: {p}");
    }
    value
}

/// Save what differs from the layers below.
pub fn save<T: Serialize>(builtin: &T, value: &T) -> Result<(), String> {
    store(builtin).save(&nyx_config::table_of(value)).map_err(|e| format!("{e:#}"))
}

/// Called on a background thread when the layout changes in any layer.
pub fn watch(on_change: impl Fn() + Send + 'static) -> Option<nyx_config::watch::Watch> {
    store(&toml::Table::new()).watch(on_change).map_err(|e| eprintln!("nyx-control-center: can't watch the layout: {e:#}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Page, Size, Tile};

    #[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Config {
        #[serde(default)]
        pages: Vec<Page>,
        #[serde(default)]
        remember_page: bool,
    }

    fn builtin() -> Config {
        Config { pages: vec![Page { tiles: vec![Tile::new("wifi", Size::S2X1)] }], remember_page: true }
    }

    #[test]
    fn nyxos_layout_shows_until_the_user_changes_it() {
        let d = tempfile::tempdir().unwrap();
        let layers = nyx_config::Layers { defaults: d.path().join("d"), system: d.path().join("s"), user: d.path().join("u") };
        std::fs::create_dir_all(&layers.defaults).unwrap();
        let store = nyx_config::Store::with_layers(layers.clone(), DOMAIN, nyx_config::table_of(&builtin()));
        assert_eq!(load_from::<Config>(&store), builtin());
        std::fs::write(layers.defaults.join("control-center.toml"), "[[pages]]\ntiles = [{ widget = \"dnd\", size = \"1x1\" }]\n").unwrap();
        let c: Config = load_from(&store);
        assert_eq!(c.pages[0].tiles[0].widget, "dnd");
        assert!(c.remember_page);
        let mut mine = c.clone();
        mine.remember_page = false;
        store.save(&nyx_config::table_of(&mine)).unwrap();
        let user = std::fs::read_to_string(layers.user.join("control-center.toml")).unwrap();
        assert!(!user.contains("pages"), "NyxOS's layout isn't copied: {user}");
        assert!(!load_from::<Config>(&store).remember_page);
    }
}
