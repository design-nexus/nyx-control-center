//! Where this computer's traffic goes: directly, through NordVPN, or through Tor (the
//! Privacy Route plugin's job). Only one of NordVPN and Tor carries traffic at a time:
//! starting one takes the other down first. Tor's "browser only" mode reroutes nothing,
//! so it can run beside NordVPN.
//!
//! Read from `nordvpn status` and `tormarchy status --json` when a view appears and every
//! 4 seconds while one is shown.

use crate::{TimerStop, output, run};
use nyx_widgets::live::{Live, Setter, Stop};
use std::time::Duration;

/// The default VPN (see [`crate::vpn::default_service`]). Named `Nord` as it was
/// NordVPN's first; it describes whichever service is the default.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Nord {
    pub installed: bool,
    /// "NordVPN", "Mullvad", a connection's name…
    pub name: String,
    pub service: Option<crate::vpn::Service>,
    /// `Connected`, `Disconnected`, `Connecting`…
    pub status: String,
    pub country: String,
    pub city: String,
    pub server: String,
    pub ip: String,
    pub technology: String,
    pub uptime: String,
}

impl Nord {
    pub fn connected(&self) -> bool {
        self.status == "Connected"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum TorMode {
    /// Every packet; the local network is dropped too.
    Strict,
    /// All internet traffic; the local network stays reachable.
    Lan,
    /// Nothing system-wide: apps that speak SOCKS use 127.0.0.1:9050.
    Socks,
}

impl TorMode {
    pub fn arg(self) -> &'static str {
        match self {
            TorMode::Strict => "strict",
            TorMode::Lan => "lan",
            TorMode::Socks => "socks",
        }
    }

    pub fn parse(s: &str) -> Option<TorMode> {
        match s {
            "strict" => Some(TorMode::Strict),
            "lan" => Some(TorMode::Lan),
            "socks" => Some(TorMode::Socks),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TorMode::Strict => "Maximum",
            TorMode::Lan => "Standard",
            TorMode::Socks => "Browser only",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Tor {
    pub installed: bool,
    pub running: bool,
    pub connected: bool,
    /// 0–100 while starting.
    pub bootstrap: u32,
    pub mode: Option<TorMode>,
    pub exit_country: String,
    pub exit_ip: String,
    pub requested_exit: String,
    pub status: String,
}

impl Tor {
    /// Tor carries the system's traffic (any mode but browser only).
    pub fn routing(&self) -> bool {
        self.connected && self.mode != Some(TorMode::Socks)
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    pub nord: Nord,
    pub tor: Tor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Route {
    Direct,
    Nord,
    Tor,
    /// Both carry traffic: something went wrong.
    Both,
}

impl State {
    pub fn route(&self) -> Route {
        match (self.nord.connected(), self.tor.routing()) {
            (true, true) => Route::Both,
            (true, false) => Route::Nord,
            (false, true) => Route::Tor,
            (false, false) => Route::Direct,
        }
    }
}

/// `nordvpn status`: `Key: value` lines.
pub fn parse_nord(text: &str) -> Nord {
    let mut n = Nord { installed: true, ..Default::default() };
    for line in text.lines() {
        // Older versions start with a spinner ("-\r  \r").
        let line = line.rsplit('\r').next().unwrap_or(line).trim();
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.trim().to_string();
        match k.trim() {
            "Status" => n.status = v,
            "Country" => n.country = v,
            "City" => n.city = v,
            "Server" | "Hostname" if n.server.is_empty() => n.server = v,
            "IP" | "Your new IP" => n.ip = v,
            "Current technology" => n.technology = v,
            "Uptime" => n.uptime = v,
            _ => {}
        }
    }
    n
}

/// `tormarchy status --json`.
pub fn parse_tor(json: &str) -> Option<Tor> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    Some(Tor {
        installed: v["installed"].as_bool().unwrap_or(false),
        running: v["torRunning"].as_bool().unwrap_or(false),
        connected: v["connected"].as_bool().unwrap_or(false),
        bootstrap: v["bootstrap"].as_u64().unwrap_or(0) as u32,
        mode: TorMode::parse(&s("mode")),
        exit_country: s("exitCountry"),
        exit_ip: s("exitIp"),
        requested_exit: s("requestedExit"),
        status: s("statusText"),
    })
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
        let nord = match crate::vpn::default_service().await {
            None => Nord::default(),
            Some(svc) if svc.kind == crate::vpn::Kind::Nord => {
                let mut n = output(&["nordvpn", "status"]).await.map(|t| parse_nord(&t)).unwrap_or_default();
                n.installed = true;
                n.name = svc.name.clone();
                n.service = Some(svc);
                n
            }
            Some(svc) => {
                let mut parts = svc.location.splitn(2, ", ");
                Nord {
                    installed: true,
                    name: svc.name.clone(),
                    status: if svc.connected { "Connected".into() } else { "Disconnected".into() },
                    country: parts.next().unwrap_or("").to_string(),
                    city: parts.next().unwrap_or("").to_string(),
                    service: Some(svc),
                    ..Default::default()
                }
            }
        };
        let tor = output(&["tormarchy", "status", "--json"]).await.and_then(|t| parse_tor(&t)).unwrap_or_default();
        set.set(State { nord, tor });
    });
}

async fn done(r: Result<(), String>) -> Result<(), String> {
    refresh(live().setter());
    r
}

/// Go direct: every VPN service and Tor off.
pub async fn direct() -> Result<(), String> {
    for s in crate::vpn::live().get().services.iter().filter(|s| s.connected) {
        crate::vpn::disconnect(s).await?;
    }
    if live().get().tor.connected {
        run(&["tormarchy", "disconnect"], None).await?;
    }
    crate::vpn::refresh(crate::vpn::live().setter());
    done(Ok(())).await
}

/// The default VPN, to its fastest place or `place`.
pub async fn nord(place: Option<&str>) -> Result<(), String> {
    let Some(svc) = crate::vpn::default_service().await else { return Err("No VPN set up".into()) };
    let r = crate::vpn::connect(&svc, place).await;
    done(r).await
}

pub async fn nord_off() -> Result<(), String> {
    let Some(svc) = crate::vpn::default_service().await else { return Ok(()) };
    done(crate::vpn::disconnect(&svc).await).await
}

/// A NordVPN setting: `killswitch`, `autoconnect`, `threatprotectionlite`… on or off.
pub async fn nord_set(name: &str, on: bool) -> Result<(), String> {
    done(run(&["nordvpn", "set", name, if on { "on" } else { "off" }], None).await).await
}

/// NordVPN's settings as `(name, on)`, for the switches it shows.
pub async fn nord_settings() -> Vec<(String, bool)> {
    let Some(t) = output(&["nordvpn", "settings"]).await else { return Vec::new() };
    t.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            let on = match v.trim() {
                "enabled" => true,
                "disabled" => false,
                _ => return None,
            };
            Some((k.trim().to_string(), on))
        })
        .collect()
}

/// The countries NordVPN offers.
pub async fn nord_countries() -> Vec<String> {
    let Some(t) = output(&["nordvpn", "countries"]).await else { return Vec::new() };
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

/// NordVPN's cities in a country (`Sweden`, `United_States`).
pub async fn nord_cities(country: &str) -> Vec<String> {
    let Some(t) = output(&["nordvpn", "cities", country]).await else { return Vec::new() };
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

/// Pause NordVPN for `5m`, `15m`, `30m`, `1h` or `24h` (it reconnects by itself).
pub async fn nord_pause(duration: &str) -> Result<(), String> {
    done(run(&["nordvpn", "pause", duration], None).await).await
}

/// A NordVPN setting with a value: `technology NORDLYNX`, `protocol udp`.
pub async fn nord_set_value(name: &str, value: &str) -> Result<(), String> {
    done(run(&["nordvpn", "set", name, value], None).await).await
}

/// NordVPN's current technology (`NORDLYNX`, `OPENVPN`, `NORDWHISPER`).
pub async fn nord_technology() -> Option<String> {
    let t = output(&["nordvpn", "settings"]).await?;
    t.lines().find_map(|l| l.strip_prefix("Technology:").map(|v| v.trim().to_uppercase()))
}

/// NordVPN's protocol (`UDP`, `TCP`): only for the OpenVPN technology.
pub async fn nord_protocol() -> Option<String> {
    let t = output(&["nordvpn", "settings"]).await?;
    t.lines().find_map(|l| l.strip_prefix("Protocol:").map(|v| v.trim().to_uppercase())).filter(|p| p == "UDP" || p == "TCP")
}

/// The countries Tor has exit relays in, most relays first: `(code, relays)`. Slow (it
/// asks Tor about every relay); only while Tor runs.
pub async fn tor_exit_countries() -> Vec<(String, usize)> {
    let Some(t) = output(&["tormarchy", "exit", "--list"]).await else { return Vec::new() };
    parse_exit_countries(&t)
}

/// Lines like `250-ip-to-country/1.2.3.4=de`.
pub fn parse_exit_countries(text: &str) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for line in text.lines() {
        if let Some((_, cc)) = line.split_once('=') {
            let cc = cc.trim().to_lowercase();
            if cc.len() == 2 && cc.chars().all(|c| c.is_ascii_alphabetic()) && cc != "??" {
                *counts.entry(cc).or_default() += 1;
            }
        }
    }
    let mut v: Vec<(String, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}

/// One latency measurement through the circuit, in ms.
pub async fn tor_latency() -> Option<f64> {
    let t = output(&["tormarchy", "speed", "--json"]).await?;
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    if v["ok"].as_bool() != Some(true) {
        return None;
    }
    v["latencyMs"].as_f64()
}

/// Bridges (for networks that block Tor): on, off, and whether any are set up.
pub async fn tor_bridges() -> Option<(bool, bool)> {
    let t = output(&["tormarchy", "bridge", "status"]).await?;
    let on = t.contains("Bridges: on");
    Some((on, !t.contains("none configured")))
}

pub async fn tor_set_bridges(on: bool) -> Result<(), String> {
    done(run(&["tormarchy", "bridge", if on { "on" } else { "off" }], None).await).await
}

/// A browser that can reach nothing but Tor.
pub fn tor_browser() {
    crate::spawn(&["tormarchy", "browser"]);
}

/// Remove every Tor firewall rule now: the escape hatch if the network is stuck.
pub async fn tor_panic() -> Result<(), String> {
    done(run(&["tormarchy", "panic"], None).await).await
}

/// Tor, in `mode`. A mode that carries traffic takes any VPN down first.
pub async fn tor(mode: TorMode) -> Result<(), String> {
    let st = live().get();
    if mode != TorMode::Socks {
        for s in crate::vpn::live().get().services.iter().filter(|s| s.connected) {
            crate::vpn::disconnect(s).await?;
        }
    }
    let r = if st.tor.connected {
        run(&["tormarchy", "mode", mode.arg()], None).await
    } else {
        run(&["tormarchy", "connect", mode.arg()], None).await
    };
    done(r).await
}

pub async fn tor_off() -> Result<(), String> {
    done(run(&["tormarchy", "disconnect"], None).await).await
}

/// A new circuit (a new exit).
pub async fn tor_new_circuit() -> Result<(), String> {
    done(run(&["tormarchy", "newnym"], None).await).await
}

/// Pin the exit to a country code (`se`), or let Tor choose (`auto`).
pub async fn tor_exit(cc: &str) -> Result<(), String> {
    done(run(&["tormarchy", "exit", cc], None).await).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nord() {
        let n = parse_nord(
            "Status: Connected\nServer: Sweden #512\nHostname: se512.nordvpn.com\nIP: 1.2.3.4\nCountry: Sweden\nCity: Stockholm\nCurrent technology: NORDLYNX\nUptime: 5 minutes\n",
        );
        assert!(n.connected());
        assert_eq!((n.country.as_str(), n.city.as_str(), n.server.as_str()), ("Sweden", "Stockholm", "Sweden #512"));
        assert!(!parse_nord("Status: Disconnected\n").connected());
    }

    #[test]
    fn counts_exit_countries() {
        let v = parse_exit_countries(
            "250-ip-to-country/1.1.1.1=de\n250-ip-to-country/2.2.2.2=de\n250-ip-to-country/3.3.3.3=se\n250-ip-to-country/4.4.4.4=??\n250 OK\n",
        );
        assert_eq!(v, vec![("de".to_string(), 2), ("se".to_string(), 1)]);
    }

    #[test]
    fn reads_tor_and_the_route() {
        let t = parse_tor(r#"{"installed":true,"torRunning":true,"connected":true,"bootstrap":100,"mode":"socks","exitCountry":"de","statusText":"Connected"}"#).unwrap();
        assert_eq!(t.mode, Some(TorMode::Socks));
        assert!(!t.routing(), "browser only reroutes nothing");
        let mut st = State { tor: t, ..Default::default() };
        assert_eq!(st.route(), Route::Direct);
        st.tor.mode = Some(TorMode::Lan);
        assert_eq!(st.route(), Route::Tor);
        st.nord.status = "Connected".into();
        assert_eq!(st.route(), Route::Both);
    }
}
