//! The Wi-Fi extras Omarchy's network panel offers: the DNS provider (`omarchy-dns`:
//! Cloudflare, Google, the network's own, or custom servers), the Wi-Fi band
//! (`omarchy-network-band`), whether the network wants a sign-in (a captive portal),
//! a speed test and a QR code to share the network (Omarchy's own panels). Read when a
//! view appears; changed through those commands.

use crate::{output, run};
use nyx_widgets::live::{Live, Setter, Stop};

/// What Omarchy calls them, and what to show.
pub const PROVIDERS: &[(&str, &str, &str)] = &[
    ("DHCP", "Network's own", "Whatever the network you're on hands out"),
    ("Cloudflare", "Cloudflare", "1.1.1.1, with DNS over TLS"),
    ("Google", "Google", "8.8.8.8, with DNS over TLS"),
    ("Custom", "Custom", "Your own servers, typed in a terminal"),
];

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Band {
    /// The band the radio is on now: `2.4`, `5`, `6`.
    pub current: String,
    pub available: Vec<String>,
    /// `auto`, or the band pinned for this network.
    pub selected: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    /// `DHCP`, `Cloudflare`, `Google`, `Custom`; empty if Omarchy's command is missing.
    pub dns: String,
    pub band: Option<Band>,
    /// The network sends you to a sign-in page first.
    pub portal: bool,
}

/// `omarchy-network-band`: `band 5`, `available 2.4 5`, `selected auto`.
pub fn parse_band(text: &str) -> Option<Band> {
    let mut b = Band::default();
    for line in text.lines() {
        let mut f = line.split_whitespace();
        match (f.next(), f.collect::<Vec<_>>()) {
            (Some("band"), v) => b.current = v.join(" "),
            (Some("available"), v) => b.available = v.iter().map(|s| s.to_string()).collect(),
            (Some("selected"), v) => b.selected = v.join(" "),
            _ => {}
        }
    }
    (!b.current.is_empty() || !b.available.is_empty()).then_some(b)
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set);
    Box::new(crate::TimerStop(None))
}

pub fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        let dns = output(&["omarchy-dns"]).await.map(|s| s.trim().to_string()).unwrap_or_default();
        let band = output(&["omarchy-network-band"]).await.and_then(|t| parse_band(&t));
        let portal = output(&["nmcli", "networking", "connectivity"]).await.is_some_and(|s| s.trim() == "portal");
        set.set(State { dns, band, portal });
    });
}

/// Use a DNS provider. Asks for permission (polkit) unless sudo needs no password.
/// `Custom` opens a terminal to type the servers in.
pub async fn set_dns(provider: &str) -> Result<(), String> {
    let r = if provider == "Custom" {
        crate::spawn(&["omarchy-launch-floating-terminal-with-presentation", "omarchy-dns Custom"]);
        Ok(())
    } else {
        run(&["omarchy-dns", provider], None).await
    };
    refresh(live().setter());
    r
}

/// Pin the Wi-Fi band (`2.4`, `5`, `6`) or let it choose (`auto`).
pub async fn set_band(band: &str) -> Result<(), String> {
    let r = run(&["omarchy-network-band", band], None).await;
    refresh(live().setter());
    r
}

/// Open the network's sign-in page.
pub fn sign_in() {
    crate::spawn(&["omarchy-network-captive-portal", "http://ping.omacom.io/nm-check.txt"]);
}

/// Omarchy's speed test and share-by-QR panels.
pub fn speed_test() {
    crate::spawn(&["omarchy-shell", "shell", "summon", "omarchy.speedtest", "{}"]);
}

pub fn share_qr() {
    crate::spawn(&["omarchy-shell", "shell", "summon", "omarchy.wifiqr", "{}"]);
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_band() {
        let b = super::parse_band("band\t5\navailable\t2.4 5\nselected\tauto\n").unwrap();
        assert_eq!((b.current.as_str(), b.selected.as_str()), ("5", "auto"));
        assert_eq!(b.available, vec!["2.4", "5"]);
        assert!(super::parse_band("").is_none());
    }
}

// ---------------------------------------------------------------- Connection details

/// The active connection, as Omarchy's network panel shows it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Details {
    pub iface: String,
    /// `wifi`, `ethernet`.
    pub kind: String,
    pub ip: String,
    pub prefix: String,
    pub gateway: String,
    pub ssid: String,
    pub signal_dbm: Option<i32>,
    /// MHz.
    pub freq: Option<f64>,
    pub bitrate: String,
    pub router_ping_ms: Option<f64>,
    pub internet_ping_ms: Option<f64>,
    /// Bytes a second, from two readings in a row.
    pub down: Option<f64>,
    pub up: Option<f64>,
    rx: u64,
    tx: u64,
    at: f64,
}

/// `omarchy-network-status --verbose`: `key<TAB>value` lines.
pub fn parse_details(text: &str) -> Details {
    let mut d = Details::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('\t') else { continue };
        let v = v.trim();
        match k {
            "iface" => d.iface = v.into(),
            "type" => d.kind = v.into(),
            "ip" => d.ip = v.into(),
            "prefix" => d.prefix = v.into(),
            "gateway" => d.gateway = v.into(),
            // `\x20` and friends are escaped bytes.
            "ssid" => d.ssid = v.replace("\\x20", " ").trim().to_string(),
            "signal_dbm" => d.signal_dbm = v.parse().ok(),
            "freq" => d.freq = v.parse().ok(),
            "bitrate" => d.bitrate = v.into(),
            "router_ping_ms" => d.router_ping_ms = v.parse().ok(),
            "internet_ping_ms" => d.internet_ping_ms = v.parse().ok(),
            "rx_bytes" => d.rx = v.parse().unwrap_or(0),
            "tx_bytes" => d.tx = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    d
}

/// "54.3 Mbit/s", from bytes a second.
pub fn rate(bytes_per_sec: f64) -> String {
    let bits = bytes_per_sec * 8.0;
    if bits >= 1e9 {
        format!("{:.1} Gbit/s", bits / 1e9)
    } else if bits >= 1e6 {
        format!("{:.1} Mbit/s", bits / 1e6)
    } else {
        format!("{:.0} kbit/s", bits / 1e3)
    }
}

thread_local! {
    static DETAILS: Live<Option<Details>> = Live::new(None, start_details);
}

pub fn details() -> Live<Option<Details>> {
    DETAILS.with(|l| l.clone())
}

fn start_details(set: Setter<Option<Details>>) -> Box<dyn Stop> {
    read_details(set.clone());
    let id = glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
        read_details(set.clone());
        glib::ControlFlow::Continue
    });
    Box::new(crate::TimerStop(Some(id)))
}

fn read_details(set: Setter<Option<Details>>) {
    glib::MainContext::default().spawn_local(async move {
        let Some(text) = output(&["omarchy-network-status", "--verbose"]).await else {
            set.set(None);
            return;
        };
        let mut d = parse_details(&text);
        d.at = glib::monotonic_time() as f64 / 1e6;
        // Throughput from the last reading on the same interface.
        if let Some(prev) = details().get().filter(|p| p.iface == d.iface && p.at > 0.0) {
            let secs = d.at - prev.at;
            if secs > 0.2 && d.rx >= prev.rx && d.tx >= prev.tx {
                d.down = Some((d.rx - prev.rx) as f64 / secs);
                d.up = Some((d.tx - prev.tx) as f64 / secs);
            }
        }
        set.set((!d.iface.is_empty()).then_some(d));
    });
}

#[cfg(test)]
mod detail_tests {
    #[test]
    fn reads_details() {
        let d = super::parse_details(
            "iface\twlo1\nip\t10.0.0.5\nprefix\t24\ntype\twifi\nssid\tPandora\\x20\nsignal_dbm\t-47\nfreq\t5745.0\nrouter_ping_ms\t2.97\nrx_bytes\t100\n",
        );
        assert_eq!((d.iface.as_str(), d.ssid.as_str(), d.signal_dbm), ("wlo1", "Pandora", Some(-47)));
        assert_eq!(d.router_ping_ms, Some(2.97));
        assert_eq!(super::rate(1_000_000.0), "8.0 Mbit/s");
    }
}
