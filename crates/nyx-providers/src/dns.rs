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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Band {
    /// The band the radio is on now: `2.4`, `5`, `6`.
    pub current: String,
    pub available: Vec<String>,
    /// `auto`, or the band pinned for this network.
    pub selected: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
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
