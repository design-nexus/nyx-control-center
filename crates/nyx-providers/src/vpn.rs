//! VPN services, whichever the user has: NordVPN, ExpressVPN, Private Internet Access,
//! Mullvad and Proton VPN through their own command-line tools, and any other provider
//! through NetworkManager (WireGuard or OpenVPN files imported in Settings › Online
//! Accounts: Surfshark, Windscribe, IVPN, your own server…). Only one carries traffic at
//! a time: connecting one takes the others (and Tor, when it reroutes) down first.
//!
//! Each tool is driven by its documented commands; where a tool documents no status
//! command (ExpressVPN), its state is read from what it prints and treated as unknown
//! when that fails. Read when a view appears and every 4 seconds while one is shown.

use crate::{TimerStop, output, present, run};
use nyx_widgets::live::{Live, Setter, Stop};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Nord,
    Express,
    Pia,
    Mullvad,
    Proton,
    /// A NetworkManager VPN connection (`uuid`; `wireguard` or `vpn`).
    Nm {
        uuid: String,
        wireguard: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Service {
    /// Stable: `nord`, `express`, `pia`, `mullvad`, `proton`, `nm:<uuid>`.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub connected: bool,
    /// "Sweden, Stockholm", or the server, when connected.
    pub location: String,
}

impl Service {
    /// It has places to choose from.
    pub fn has_locations(&self) -> bool {
        !matches!(self.kind, Kind::Nm { .. })
    }
}

/// A place a service can connect to: what to pass it, and what to show.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub arg: String,
    pub name: String,
    /// Indented under its country.
    pub city: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub services: Vec<Service>,
}

impl State {
    pub fn connected(&self) -> Option<&Service> {
        self.services.iter().find(|s| s.connected)
    }
}

// ---------------------------------------------------------------- Reading

/// `Key: value` lines (NordVPN, Mullvad's verbose output).
fn field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let l = l.rsplit('\r').next().unwrap_or(l).trim();
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_string())
    })
}

/// `nordvpn status`.
pub fn parse_nord(text: &str) -> (bool, String) {
    let connected = field(text, "Status").as_deref() == Some("Connected");
    let place = [field(text, "Country"), field(text, "City")].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>();
    (connected, place.join(", "))
}

/// `mullvad status`: "Connected to se-sto-wg-001 in Stockholm, Sweden" / "Disconnected".
pub fn parse_mullvad(text: &str) -> (bool, String) {
    let first = text.lines().next().unwrap_or("").trim();
    if !first.starts_with("Connected") {
        return (false, String::new());
    }
    let place = first.split(" in ").nth(1).map(str::trim).unwrap_or("").to_string();
    (true, place)
}

/// A state word as these tools print it: "Connected" (not "Disconnected", "NotConnected").
pub fn says_connected(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim().to_lowercase();
        (l == "connected" || l.starts_with("connected ") || l.starts_with("connected to") || l.contains("status: connected"))
            && !l.contains("disconnected")
            && !l.contains("not connected")
    })
}

/// `nmcli -t -f NAME,UUID,TYPE,ACTIVE connection show`: VPN and WireGuard connections
/// (Tailscale's own aside).
pub fn parse_nm(text: &str) -> Vec<Service> {
    text.lines()
        .filter_map(|l| {
            let f = crate::terse_fields(l);
            let (name, uuid, kind, active) = (f.first()?, f.get(1)?, f.get(2)?, f.get(3)?);
            let wireguard = kind == "wireguard";
            // Tailscale's and NordVPN's own tunnels aren't services of their own.
            let lower = name.to_lowercase();
            if !(wireguard || kind == "vpn") || lower.contains("tailscale") || lower == "nordlynx" {
                return None;
            }
            Some(Service {
                id: format!("nm:{uuid}"),
                name: name.clone(),
                kind: Kind::Nm { uuid: uuid.clone(), wireguard },
                connected: active == "yes",
                location: String::new(),
            })
        })
        .collect()
}

async fn read_state() -> State {
    let mut services = Vec::new();
    if present("nordvpn") {
        let (connected, location) = output(&["nordvpn", "status"]).await.map(|t| parse_nord(&t)).unwrap_or_default();
        services.push(Service { id: "nord".into(), name: "NordVPN".into(), kind: Kind::Nord, connected, location });
    }
    if present("expressvpnctl") {
        let text = match output(&["expressvpnctl", "get", "connectionstate"]).await {
            Some(t) => t,
            None => output(&["expressvpnctl", "status"]).await.unwrap_or_default(),
        };
        let region = output(&["expressvpnctl", "get", "region"]).await.unwrap_or_default();
        services.push(Service {
            id: "express".into(),
            name: "ExpressVPN".into(),
            kind: Kind::Express,
            connected: says_connected(&text),
            location: region.lines().next().unwrap_or("").trim().to_string(),
        });
    }
    if present("piactl") {
        let state = output(&["piactl", "get", "connectionstate"]).await.unwrap_or_default();
        let region = output(&["piactl", "get", "region"]).await.unwrap_or_default();
        services.push(Service {
            id: "pia".into(),
            name: "Private Internet Access".into(),
            kind: Kind::Pia,
            connected: state.trim() == "Connected",
            location: region.trim().to_string(),
        });
    }
    if present("mullvad") {
        let (connected, location) = output(&["mullvad", "status"]).await.map(|t| parse_mullvad(&t)).unwrap_or_default();
        services.push(Service { id: "mullvad".into(), name: "Mullvad".into(), kind: Kind::Mullvad, connected, location });
    }
    if present("protonvpn") {
        let text = output(&["protonvpn", "status"]).await.unwrap_or_default();
        let location = field(&text, "Server").or_else(|| field(&text, "Country")).unwrap_or_default();
        services.push(Service {
            id: "proton".into(),
            name: "Proton VPN".into(),
            kind: Kind::Proton,
            connected: says_connected(&text),
            location,
        });
    }
    if let Some(t) = output(&["nmcli", "-t", "-f", "NAME,UUID,TYPE,ACTIVE", "connection", "show"]).await {
        services.extend(parse_nm(&t));
    }
    State { services }
}

// ---------------------------------------------------------------- Settings

/// `~/.config/nyx/vpn.toml`: the VPN the Control Center shows (`default`, written by
/// Settings › Online Accounts › VPN) and, per service, favourite servers and the one
/// to connect to (`server`; none is the fastest).
pub fn config_path() -> std::path::PathBuf {
    glib::user_config_dir().join("nyx/vpn.toml")
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Favorite {
    /// What to pass the tool (`Sweden`, `Stockholm`, `se sto`).
    pub arg: String,
    /// What to show ("Stockholm, Sweden").
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ServiceConfig {
    /// Where to connect by default (an `arg`); none: the fastest.
    pub server: Option<String>,
    pub favorites: Vec<Favorite>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub default: Option<String>,
    pub services: std::collections::BTreeMap<String, ServiceConfig>,
}

pub fn load_config() -> Config {
    std::fs::read_to_string(config_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

fn save_config(c: &Config) {
    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = format!(
        "# The VPN the Control Center shows (Settings › Online Accounts › VPN), and each\n# service's favourite servers and default server (none: the fastest).\n{}",
        toml::to_string(c).unwrap_or_default()
    );
    let tmp = path.with_extension("toml.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn chosen_id() -> Option<String> {
    load_config().default
}

/// A service's favourites and default server.
pub fn service_config(id: &str) -> ServiceConfig {
    load_config().services.get(id).cloned().unwrap_or_default()
}

/// Star or unstar a server. Unstarring the default server goes back to the fastest.
pub fn set_favorite(id: &str, place: &Favorite, on: bool) {
    let mut c = load_config();
    let sc = c.services.entry(id.to_string()).or_default();
    sc.favorites.retain(|f| f.arg != place.arg);
    if on {
        sc.favorites.push(place.clone());
    } else if sc.server.as_deref() == Some(place.arg.as_str()) {
        sc.server = None;
    }
    save_config(&c);
}

/// Where to connect by default: a server's `arg`, or `None` for the fastest.
pub fn set_server(id: &str, arg: Option<&str>) {
    let mut c = load_config();
    c.services.entry(id.to_string()).or_default().server = arg.map(String::from);
    save_config(&c);
}

pub fn pick_default(services: &[Service], chosen: Option<&str>) -> Option<Service> {
    chosen.and_then(|id| services.iter().find(|s| s.id == id)).or(services.first()).cloned()
}

/// The default VPN, read now.
pub async fn default_service() -> Option<Service> {
    let st = read_state().await;
    pick_default(&st.services, chosen_id().as_deref())
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    let id = glib::timeout_add_local(Duration::from_secs(4), move || {
        refresh(set.clone());
        glib::ControlFlow::Continue
    });
    Box::new(TimerStop(Some(id)))
}

pub fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        set.set(read_state().await);
    });
}

async fn done(r: Result<(), String>) -> Result<(), String> {
    refresh(live().setter());
    crate::routes::refresh(crate::routes::live().setter());
    r
}

// ---------------------------------------------------------------- Changing

/// Take a service down.
pub async fn disconnect(s: &Service) -> Result<(), String> {
    let r = match &s.kind {
        Kind::Nord => run(&["nordvpn", "disconnect"], None).await,
        Kind::Express => run(&["expressvpnctl", "disconnect"], None).await,
        Kind::Pia => run(&["piactl", "disconnect"], None).await,
        Kind::Mullvad => run(&["mullvad", "disconnect"], None).await,
        Kind::Proton => run(&["protonvpn", "disconnect"], None).await,
        Kind::Nm { uuid, .. } => run(&["nmcli", "connection", "down", "uuid", uuid], None).await,
    };
    done(r).await
}

/// Connect a service, to `place` (from [`places`]) or its default. Whatever else carries
/// traffic goes down first.
pub async fn connect(s: &Service, place: Option<&str>) -> Result<(), String> {
    let st = read_state().await;
    for other in st.services.iter().filter(|o| o.connected && o.id != s.id) {
        disconnect(other).await?;
    }
    if crate::routes::live().get().tor.routing() {
        run(&["tormarchy", "disconnect"], None).await?;
    }
    // No place: the default server, if one is set. An empty place: the fastest, whatever
    // the default.
    let chosen = if place.is_none() { service_config(&s.id).server } else { None };
    let place = match place {
        Some("") => None,
        Some(p) => Some(p),
        None => chosen.as_deref(),
    };
    let r = match (&s.kind, place) {
        (Kind::Nord, Some(p)) => run(&["nordvpn", "connect", p], None).await,
        (Kind::Nord, None) => run(&["nordvpn", "connect"], None).await,
        (Kind::Express, Some(p)) => run(&["expressvpnctl", "connect", p], None).await,
        (Kind::Express, None) => run(&["expressvpnctl", "connect"], None).await,
        (Kind::Pia, p) => {
            run(&["piactl", "set", "region", p.unwrap_or("auto")], None).await?;
            run(&["piactl", "connect"], None).await
        }
        (Kind::Mullvad, p) => {
            if let Some(p) = p {
                let mut args = vec!["mullvad", "relay", "set", "location"];
                args.extend(p.split_whitespace());
                run(&args, None).await?;
            }
            run(&["mullvad", "connect"], None).await
        }
        (Kind::Proton, Some(p)) => run(&["protonvpn", "connect", "--country", p], None).await,
        (Kind::Proton, None) => run(&["protonvpn", "connect"], None).await,
        (Kind::Nm { uuid, .. }, _) => run(&["nmcli", "connection", "up", "uuid", uuid], None).await,
    };
    done(r).await
}

/// Where a service can connect to. Slow for some tools: ask when needed, once.
pub async fn places(kind: &Kind) -> Vec<Place> {
    match kind {
        Kind::Nord => {
            let Some(t) = output(&["nordvpn", "countries"]).await else { return Vec::new() };
            words(&t).into_iter().map(|c| Place { name: c.replace('_', " "), arg: c, city: false }).collect()
        }
        Kind::Express => output(&["expressvpnctl", "get", "regions"])
            .await
            .map(|t| {
                t.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(|l| Place { arg: l.to_string(), name: l.to_string(), city: false })
                    .collect()
            })
            .unwrap_or_default(),
        Kind::Pia => output(&["piactl", "get", "regions"])
            .await
            .map(|t| {
                t.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && *l != "auto")
                    .map(|l| Place { arg: l.to_string(), name: pia_name(l), city: false })
                    .collect()
            })
            .unwrap_or_default(),
        Kind::Mullvad => output(&["mullvad", "relay", "list"]).await.map(|t| parse_mullvad_relays(&t)).unwrap_or_default(),
        // Proton's CLI lists nothing: the countries it serves, by name.
        Kind::Proton => PROTON_COUNTRIES.iter().map(|c| Place { arg: c.to_string(), name: c.to_string(), city: false }).collect(),
        Kind::Nm { .. } => Vec::new(),
    }
}

/// NordVPN's cities in a country.
pub async fn nord_cities(country: &str) -> Vec<Place> {
    let Some(t) = output(&["nordvpn", "cities", country]).await else { return Vec::new() };
    words(&t).into_iter().map(|c| Place { name: c.replace('_', " "), arg: c, city: true }).collect()
}

/// Every NordVPN city ("Atlanta, United States"), for searching. Asked of `nordvpn`
/// fifteen countries at a time and kept for a week in `~/.cache/nyx/nordvpn-cities.json`.
pub async fn nord_all_cities() -> Vec<Place> {
    let cache = glib::user_cache_dir().join("nyx/nordvpn-cities.json");
    let fresh = std::fs::metadata(&cache)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < Duration::from_secs(7 * 86_400));
    if fresh
        && let Some(v) = std::fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str::<Vec<(String, String)>>(&t).ok())
    {
        return v.into_iter().map(|(arg, name)| Place { arg, name, city: true }).collect();
    }
    let countries = places(&Kind::Nord).await;
    let mut out: Vec<(String, String)> = Vec::new();
    for chunk in countries.chunks(15) {
        let handles: Vec<_> = chunk
            .iter()
            .map(|c| {
                let (arg, name) = (c.arg.clone(), c.name.clone());
                glib::MainContext::default().spawn_local(async move { (name, nord_cities(&arg).await) })
            })
            .collect();
        for h in handles {
            if let Ok((country, cities)) = h.await {
                out.extend(cities.into_iter().map(|c| (c.arg, format!("{}, {country}", c.name))));
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    if !out.is_empty() {
        if let Some(dir) = cache.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&cache, serde_json::to_string(&out).unwrap_or_default());
    }
    out.into_iter().map(|(arg, name)| Place { arg, name, city: true }).collect()
}

fn words(t: &str) -> Vec<String> {
    let mut v: Vec<String> = t
        .split([',', '\t', '\n'])
        .map(|s| s.trim().trim_start_matches('-').trim())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .map(String::from)
        .collect();
    v.sort();
    v.dedup();
    v
}

/// "us-chicago" → "US Chicago".
fn pia_name(id: &str) -> String {
    let mut parts = id.split('-');
    let cc = parts.next().unwrap_or("").to_uppercase();
    let rest: Vec<String> = parts
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        })
        .collect();
    format!("{cc} {}", rest.join(" ")).trim().to_string()
}

/// `mullvad relay list`: "Sweden (se)" then indented "Stockholm (sto) @ …" lines.
pub fn parse_mullvad_relays(text: &str) -> Vec<Place> {
    let mut out = Vec::new();
    let mut country: Option<(String, String)> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let depth = line.len() - line.trim_start_matches('\t').len();
        let t = line.trim();
        let Some(open) = t.find('(') else { continue };
        let Some(close) = t[open..].find(')') else { continue };
        let name = t[..open].trim().to_string();
        let code = t[open + 1..open + close].trim().to_string();
        match depth {
            0 => {
                out.push(Place { arg: code.clone(), name: name.clone(), city: false });
                country = Some((code, name));
            }
            1 => {
                if let Some((cc, cname)) = &country {
                    out.push(Place { arg: format!("{cc} {code}"), name: format!("{name}, {cname}"), city: true });
                }
            }
            _ => {}
        }
    }
    out
}

const PROTON_COUNTRIES: &[&str] = &[
    "Argentina",
    "Australia",
    "Austria",
    "Belgium",
    "Brazil",
    "Bulgaria",
    "Canada",
    "Chile",
    "Colombia",
    "Croatia",
    "Czechia",
    "Denmark",
    "Estonia",
    "Finland",
    "France",
    "Germany",
    "Greece",
    "Hong Kong",
    "Hungary",
    "Iceland",
    "India",
    "Indonesia",
    "Ireland",
    "Israel",
    "Italy",
    "Japan",
    "Latvia",
    "Lithuania",
    "Luxembourg",
    "Malaysia",
    "Mexico",
    "Netherlands",
    "New Zealand",
    "Nigeria",
    "Norway",
    "Peru",
    "Philippines",
    "Poland",
    "Portugal",
    "Romania",
    "Serbia",
    "Singapore",
    "Slovakia",
    "Slovenia",
    "South Africa",
    "South Korea",
    "Spain",
    "Sweden",
    "Switzerland",
    "Taiwan",
    "Thailand",
    "Turkey",
    "Ukraine",
    "United Kingdom",
    "United States",
    "Vietnam",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tools() {
        assert_eq!(parse_nord("Status: Connected\nCountry: Sweden\nCity: Stockholm\n"), (true, "Sweden, Stockholm".into()));
        assert!(!parse_nord("Status: Disconnected\n").0);
        assert_eq!(parse_mullvad("Connected to se-sto-wg-001 in Stockholm, Sweden\n"), (true, "Stockholm, Sweden".into()));
        assert!(!parse_mullvad("Disconnected\n").0);
        assert!(says_connected("Connected\n") && says_connected("Status: Connected to US"));
        assert!(!says_connected("Disconnected") && !says_connected("Not connected"));
        assert_eq!(pia_name("us-new-york"), "US New York");
    }

    #[test]
    fn reads_networkmanager_vpns() {
        let s = parse_nm(
            "Pandora:aaa:802-11-wireless:yes\nwg-surfshark:bbb:wireguard:no\nWork:ccc:vpn:yes\ntailscale0:ddd:tun:yes\n",
        );
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].kind, Kind::Nm { uuid: "bbb".into(), wireguard: true });
        assert!(s[1].connected);
    }

    #[test]
    fn config_round_trip() {
        let mut c = Config { default: Some("nord".into()), ..Default::default() };
        let sc = c.services.entry("nord".into()).or_default();
        sc.server = Some("Sweden".into());
        sc.favorites.push(Favorite { arg: "Atlanta".into(), name: "Atlanta, United States".into() });
        let text = toml::to_string(&c).unwrap();
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), c);
        assert_eq!(
            toml::from_str::<Config>("default = \"mullvad\"\n").unwrap().default.as_deref(),
            Some("mullvad"),
            "Settings' one-line file"
        );
    }

    #[test]
    fn picks_the_default() {
        let s = parse_nm("wg-a:aaa:wireguard:no\nwg-b:bbb:wireguard:no\nnordlynx:ccc:wireguard:yes\n");
        assert_eq!(s.len(), 2, "NordVPN's own tunnel isn't a service");
        assert_eq!(pick_default(&s, Some("nm:bbb")).unwrap().name, "wg-b");
        assert_eq!(pick_default(&s, Some("gone")).unwrap().name, "wg-a", "the first when the chosen one is gone");
        assert!(pick_default(&[], None).is_none());
    }

    #[test]
    fn reads_mullvad_relays() {
        let p = parse_mullvad_relays(
            "Sweden (se)\n\tStockholm (sto) @ 59.33°N\n\t\tse-sto-wg-001 (1.2.3.4) - WireGuard\nAlbania (al)\n\tTirana (tia) @ 41°N\n",
        );
        let names: Vec<&str> = p.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["Sweden", "Stockholm, Sweden", "Albania", "Tirana, Albania"]);
        assert_eq!(p[1].arg, "se sto");
        assert!(p[1].city && !p[0].city);
    }
}
