//! The layout (`~/.config/nyx/control-center.toml` over NyxOS's defaults, through
//! `nyx_widgets::settings`): pages of tiles, the same file and migrations as the GTK
//! panel, and the tile kinds the panel draws.

use nyx_widgets::layout::{Page, Size, Tile};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub pages: Vec<Page>,
    #[serde(default = "yes")]
    pub remember_page: bool,
    #[serde(default)]
    pub version: u32,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub plugins: std::collections::BTreeMap<String, toml::Table>,
}

const LAYOUT_VERSION: u32 = 3;

fn yes() -> bool {
    true
}

/// Bring an older layout up to date (as the GTK panel's `migrate`).
pub fn migrate(c: &mut Config) -> bool {
    if c.version >= LAYOUT_VERSION {
        return false;
    }
    if c.version < 2 {
        for page in &mut c.pages {
            if let Some(i) = page.tiles.iter().position(|t| t.widget == "vpn") {
                page.tiles[i].widget = "tailscale".into();
                let size = page.tiles[i].size;
                page.tiles.insert(i, Tile::new("vpn", size));
            }
        }
    }
    if c.version < 3 {
        let has = |c: &Config, w: &str| c.pages.iter().any(|p| p.tiles.iter().any(|t| t.widget == w));
        let add: Vec<(&str, Size)> =
            [("upcoming", Size::S4X2), ("microphone", Size::S1X1)].into_iter().filter(|(w, _)| !has(c, w)).collect();
        if let Some(last) = c.pages.last_mut() {
            for (w, size) in add {
                last.tiles.push(Tile::new(w, size));
            }
        }
    }
    c.version = LAYOUT_VERSION;
    true
}

pub fn path() -> PathBuf {
    glib::user_config_dir().join("nyx/control-center.toml")
}

pub fn default_config() -> Config {
    Config { pages: default_pages(), remember_page: true, version: LAYOUT_VERSION, plugins: Default::default() }
}

/// The layout through every layer: the built-in one, NyxOS's, the user's changes.
pub fn load() -> Config {
    let mut c: Config = nyx_widgets::settings::load(&default_config());
    if c.pages.is_empty() {
        c.pages = default_pages();
    }
    if migrate(&mut c) && path().exists() {
        let _ = save(&c);
    }
    c
}

/// Save what differs from the built-in layout and NyxOS's.
pub fn save(c: &Config) -> std::io::Result<()> {
    nyx_widgets::settings::save(&default_config(), c).map_err(std::io::Error::other)
}

/// The first-start layout (as the GTK panel's `default_pages`).
pub fn default_pages() -> Vec<Page> {
    use Size as S;
    let t = Tile::new;
    vec![
        Page {
            tiles: vec![
                t("wifi", S::S2X1),
                t("bluetooth", S::S2X1),
                t("vpn", S::S2X1),
                t("tailscale", S::S2X1),
                t("dnd", S::S1X1),
                t("mode", S::S1X1),
                t("night-light", S::S1X1),
                t("keep-awake", S::S1X1),
                t("sound", S::S4X2),
                t("media", S::S4X2),
                t("power", S::S2X1),
                t("battery", S::S2X1),
                t("devices", S::S1X1),
                t("microphone", S::S1X1),
                t("keyboard-light", S::S1X1),
                t("capture", S::S1X1),
            ],
        },
        Page { tiles: vec![t("upcoming", S::S4X3), t("home", S::S4X3)] },
    ]
}

/// The native tile kinds: id, name, icon, sizes (first is the default).
pub const KINDS: &[(&str, &str, &str, &[&str])] = &[
    ("backup", "Backup", "drive-harddisk-symbolic", &["2x3", "4x3"]),
    ("wifi", "Wi-Fi", "network-wireless-symbolic", &["2x1", "1x1"]),
    ("bluetooth", "Bluetooth", "bluetooth-symbolic", &["2x1", "1x1"]),
    ("devices", "Devices", "phone-symbolic", &["2x1", "1x1"]),
    ("tailscale", "Tailscale", "network-workgroup-symbolic", &["2x1", "1x1"]),
    ("vpn", "VPN", "security-high-symbolic", &["2x1", "1x1"]),
    ("dnd", "Do not disturb", "notifications-disabled-symbolic", &["1x1"]),
    ("mode", "Workspace mode", "view-grid-symbolic", &["1x1"]),
    ("sound", "Sound & brightness", "audio-volume-high-symbolic", &["4x2", "2x2"]),
    ("home", "Home", "go-home-symbolic", &["2x3", "4x3"]),
    ("upcoming", "Up next", "x-office-calendar-symbolic", &["4x3", "2x3"]),
    ("media", "Now playing", "audio-x-generic-symbolic", &["4x2"]),
    ("power", "Power mode", "power-profile-balanced-symbolic", &["2x1", "4x1"]),
    ("battery", "Battery", "battery-good-symbolic", &["1x1", "2x1"]),
    ("microphone", "Microphone", "audio-input-microphone-symbolic", &["1x1"]),
    ("night-light", "Night light", "night-light-symbolic", &["1x1"]),
    ("keep-awake", "Keep awake", "preferences-desktop-screensaver-symbolic", &["1x1"]),
    ("keyboard-light", "Keyboard light", "keyboard-brightness-symbolic", &["1x1"]),
    ("capture", "Screenshot", "camera-photo-symbolic", &["1x1"]),
];

pub fn kinds_json() -> Value {
    Value::Array(
        KINDS.iter().map(|(id, name, icon, sizes)| json!({"id": id, "name": name, "icon": icon, "sizes": sizes})).collect(),
    )
}

/// Pages from the interface, checked: sizes must parse and tiles need an id.
pub fn pages_from(v: &Value) -> Option<Vec<Page>> {
    let pages = serde_json::from_value::<Vec<Page>>(v.clone()).ok()?;
    pages.iter().all(|p| p.tiles.iter().all(|t| !t.widget.is_empty())).then_some(pages)
}
