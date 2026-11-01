//! Wi-Fi and VPNs through NetworkManager (`nmcli`), plus Tailscale when it's installed.
//! `nmcli monitor` says when something changed; the state is then read again.

use crate::{Debounce, Watch, output, run, terse_fields};
use nyx_widgets::live::{Live, Setter, Stop};
use std::cell::RefCell;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Network {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    /// A saved connection exists for it.
    pub known: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Vpn {
    pub name: String,
    /// NetworkManager's connection uuid, or `tailscale`.
    pub id: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// Whether NetworkManager answered at all.
    pub available: bool,
    pub has_wifi: bool,
    pub wifi_on: bool,
    /// The Wi-Fi network in use.
    pub ssid: Option<String>,
    pub signal: u8,
    /// The active Wi-Fi connection's uuid.
    pub wifi_uuid: Option<String>,
    pub metered: bool,
    pub wired: bool,
    /// Networks in range, strongest first; known ones first among equals.
    pub networks: Vec<Network>,
    pub vpns: Vec<Vpn>,
}

impl State {
    pub fn vpn_active(&self) -> Option<&Vpn> {
        self.vpns.iter().find(|v| v.active)
    }
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

/// The shared network state.
pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    let debounce = Debounce::new(250);
    let s = set.clone();
    refresh(s.clone());
    let watch = Watch::lines(&["nmcli", "monitor"], move |_| {
        let s = s.clone();
        debounce.call(move || refresh(s));
    });
    Box::new(watch)
}

thread_local! {
    static REFRESHING: RefCell<bool> = const { RefCell::new(false) };
}

/// Read everything again (in the background) and publish it.
pub fn refresh(set: Setter<State>) {
    if REFRESHING.with(|r| r.replace(true)) {
        return;
    }
    glib::MainContext::default().spawn_local(async move {
        let state = read().await;
        REFRESHING.with(|r| *r.borrow_mut() = false);
        set.set(state);
    });
}

async fn read() -> State {
    let mut st = State::default();
    let Some(radio) = output(&["nmcli", "-t", "-f", "WIFI-HW,WIFI", "radio"]).await else { return st };
    st.available = true;
    let r = terse_fields(radio.trim());
    // WIFI-HW is "missing" without a Wi-Fi device ("disabled" is a hardware switch).
    st.has_wifi = r.first().is_some_and(|v| v != "missing");
    st.wifi_on = r.get(1).map(String::as_str) == Some("enabled");

    let saved = output(&["nmcli", "-t", "-f", "NAME,UUID,TYPE", "connection", "show"]).await.unwrap_or_default();
    let saved: Vec<Vec<String>> = saved.lines().map(terse_fields).collect();
    let active =
        output(&["nmcli", "-t", "-f", "NAME,UUID,TYPE,DEVICE", "connection", "show", "--active"]).await.unwrap_or_default();
    for f in active.lines().map(terse_fields) {
        match f.get(2).map(String::as_str) {
            Some("802-11-wireless") => {
                st.wifi_uuid = f.get(1).cloned();
            }
            Some("802-3-ethernet") => st.wired = true,
            _ => {}
        }
    }
    for f in &saved {
        let (Some(name), Some(uuid), Some(kind)) = (f.first(), f.get(1), f.get(2)) else { continue };
        if kind == "vpn" || kind == "wireguard" {
            let on = active.lines().map(terse_fields).any(|a| a.get(1) == Some(uuid));
            st.vpns.push(Vpn { name: name.clone(), id: uuid.clone(), active: on });
        }
    }
    if let Some(ts) = tailscale().await {
        st.vpns.push(ts);
    }
    if let Some(uuid) = &st.wifi_uuid {
        let m = output(&["nmcli", "-t", "-f", "connection.metered", "connection", "show", uuid]).await.unwrap_or_default();
        st.metered = m.trim().ends_with("yes");
    }
    if st.wifi_on {
        let list = output(&["nmcli", "-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "no"])
            .await
            .unwrap_or_default();
        let known: Vec<&String> =
            saved.iter().filter(|f| f.get(2).map(String::as_str) == Some("802-11-wireless")).filter_map(|f| f.first()).collect();
        st.networks = networks(&list, &known);
        if let Some(n) = st.networks.iter().find(|n| n.active) {
            st.ssid = Some(n.ssid.clone());
            st.signal = n.signal;
        }
    }
    st
}

/// Networks from `nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY device wifi list`: one entry
/// per name (the strongest), hidden networks left out.
pub fn networks(list: &str, known: &[&String]) -> Vec<Network> {
    let mut out: Vec<Network> = Vec::new();
    for f in list.lines().map(terse_fields) {
        let (Some(inuse), Some(ssid), Some(signal), Some(sec)) = (f.first(), f.get(1), f.get(2), f.get(3)) else { continue };
        if ssid.is_empty() {
            continue;
        }
        let n = Network {
            ssid: ssid.clone(),
            signal: signal.parse().unwrap_or(0),
            secure: !sec.trim().is_empty() && sec.trim() != "--",
            known: known.contains(&ssid),
            active: inuse.trim() == "*",
        };
        match out.iter_mut().find(|o| o.ssid == n.ssid) {
            Some(o) => {
                o.active |= n.active;
                o.signal = o.signal.max(n.signal);
            }
            None => out.push(n),
        }
    }
    out.sort_by(|a, b| b.active.cmp(&a.active).then(b.known.cmp(&a.known)).then(b.signal.cmp(&a.signal)));
    out
}

async fn tailscale() -> Option<Vpn> {
    let json = output(&["tailscale", "status", "--json", "--peers=false"]).await?;
    let v: serde_json::Value = serde_json::from_str(&json).ok()?;
    let state = v.get("BackendState")?.as_str()?;
    Some(Vpn { name: "Tailscale".into(), id: "tailscale".into(), active: state == "Running" })
}

/// Signal bars, 0–4.
pub fn bars(signal: u8) -> u8 {
    match signal {
        0..=19 => 1,
        20..=44 => 2,
        45..=69 => 3,
        _ => 4,
    }
}

// ---------- Actions ----------

async fn after(result: Result<(), String>) -> Result<(), String> {
    let l = live();
    refresh(l.setter());
    result
}

pub async fn set_wifi(on: bool) -> Result<(), String> {
    after(run(&["nmcli", "radio", "wifi", if on { "on" } else { "off" }], None).await).await
}

/// Join a network: a saved one by name, a new open one directly, a new secured one
/// with `password` (passed on stdin, never on the command line).
pub async fn connect(n: &Network, password: Option<&str>) -> Result<(), String> {
    let r = if n.known {
        run(&["nmcli", "connection", "up", "id", &n.ssid], None).await
    } else if n.secure {
        let pw = password.ok_or("this network needs a password")?;
        run(&["nmcli", "--ask", "device", "wifi", "connect", &n.ssid], Some(&format!("{pw}\n"))).await
    } else {
        run(&["nmcli", "device", "wifi", "connect", &n.ssid], None).await
    };
    after(r).await
}

pub async fn disconnect(uuid: &str) -> Result<(), String> {
    after(run(&["nmcli", "connection", "down", "uuid", uuid], None).await).await
}

pub async fn set_metered(uuid: &str, on: bool) -> Result<(), String> {
    after(run(&["nmcli", "connection", "modify", "uuid", uuid, "connection.metered", if on { "yes" } else { "no" }], None).await)
        .await
}

pub async fn rescan() -> Result<(), String> {
    after(run(&["nmcli", "device", "wifi", "rescan"], None).await).await
}

pub async fn set_vpn(vpn: &Vpn, on: bool) -> Result<(), String> {
    let r = if vpn.id == "tailscale" {
        run(&["tailscale", if on { "up" } else { "down" }], None).await
    } else {
        run(&["nmcli", "connection", if on { "up" } else { "down" }, "uuid", &vpn.id], None).await
    };
    after(r).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_list() {
        let list = "*:Pandora :83:WPA2\n :FPF - Sales:55:WPA2\n :FPF - Sales:70:WPA2\n ::40:WPA2\n :Cafe Lumen:60:\n";
        let pandora = "Pandora ".to_string();
        let n = networks(list, &[&pandora]);
        let names: Vec<&str> = n.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, vec!["Pandora ", "FPF - Sales", "Cafe Lumen"]);
        assert!(n[0].active && n[0].known && n[0].secure);
        assert_eq!(n[1].signal, 70, "the strongest of the same name");
        assert!(!n[2].secure);
    }

    #[test]
    fn signal_bars() {
        assert_eq!(bars(10), 1);
        assert_eq!(bars(83), 4);
    }
}
