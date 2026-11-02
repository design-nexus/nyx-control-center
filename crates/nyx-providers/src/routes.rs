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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Nord {
    pub installed: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default, PartialEq)]
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub nord: Nord,
    pub tor: Tor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        let nord = match output(&["nordvpn", "status"]).await {
            Some(t) => parse_nord(&t),
            None => Nord { installed: crate::present("nordvpn"), ..Default::default() },
        };
        let tor = output(&["tormarchy", "status", "--json"]).await.and_then(|t| parse_tor(&t)).unwrap_or_default();
        set.set(State { nord, tor });
    });
}

async fn done(r: Result<(), String>) -> Result<(), String> {
    refresh(live().setter());
    r
}

/// Go direct: both off.
pub async fn direct() -> Result<(), String> {
    let st = live().get();
    if st.nord.connected() {
        run(&["nordvpn", "disconnect"], None).await?;
    }
    if st.tor.connected {
        run(&["tormarchy", "disconnect"], None).await?;
    }
    done(Ok(())).await
}

/// NordVPN, to the fastest server or one in `place` (a country or city, `_` for spaces).
pub async fn nord(place: Option<&str>) -> Result<(), String> {
    let st = live().get();
    if st.tor.routing() {
        run(&["tormarchy", "disconnect"], None).await?;
    }
    let mut args = vec!["nordvpn", "connect"];
    if let Some(p) = place {
        args.push(p);
    }
    done(run(&args, None).await).await
}

pub async fn nord_off() -> Result<(), String> {
    done(run(&["nordvpn", "disconnect"], None).await).await
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

/// Tor, in `mode`. A mode that carries traffic takes NordVPN down first.
pub async fn tor(mode: TorMode) -> Result<(), String> {
    let st = live().get();
    if mode != TorMode::Socks && st.nord.connected() {
        run(&["nordvpn", "disconnect"], None).await?;
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
