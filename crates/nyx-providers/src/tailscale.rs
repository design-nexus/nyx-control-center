//! Tailscale: whether it's up, this machine, the tailnet's machines, the exit node and
//! the accounts — what Omarchy's Tailscale panel offers. Read from `tailscale status
//! --json` and `tailscale switch --list` when a view appears and every 5 seconds while
//! one is shown; changed with `tailscale up/down/set/switch`.

use crate::{TimerStop, output, run};
use nyx_widgets::live::{Live, Setter, Stop};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Machine {
    pub name: String,
    /// `name.tailnet.ts.net` (without the trailing dot).
    pub dns: String,
    pub ips: Vec<String>,
    pub online: bool,
    pub os: String,
    /// It offers itself as an exit node.
    pub exit_option: bool,
    /// This machine's traffic goes through it now.
    pub exit_active: bool,
    /// "Sweden, Stockholm" for Mullvad and other located exit nodes.
    pub location: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Account {
    pub id: String,
    pub tailnet: String,
    pub account: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub installed: bool,
    /// `Running`, `Stopped`, `NeedsLogin`, `Starting`…
    pub backend: String,
    pub tailnet: String,
    pub me: Machine,
    /// Everyone else, online first, then by name.
    pub peers: Vec<Machine>,
    pub accounts: Vec<Account>,
}

impl State {
    pub fn running(&self) -> bool {
        self.backend == "Running"
    }

    pub fn exit_node(&self) -> Option<&Machine> {
        self.peers.iter().find(|p| p.exit_active)
    }

    pub fn exit_nodes(&self) -> Vec<&Machine> {
        self.peers.iter().filter(|p| p.exit_option).collect()
    }
}

fn machine(v: &Value) -> Machine {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let loc = &v["Location"];
    let country = loc["Country"].as_str().map(String::from);
    let location = country.as_ref().map(|c| match loc["City"].as_str() {
        Some(city) if !city.is_empty() => format!("{c}, {city}"),
        _ => c.clone(),
    });
    Machine {
        name: s("HostName"),
        dns: s("DNSName").trim_end_matches('.').to_string(),
        ips: v["TailscaleIPs"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        online: v["Online"].as_bool().unwrap_or(false),
        os: s("OS"),
        exit_option: v["ExitNodeOption"].as_bool().unwrap_or(false),
        exit_active: v["ExitNode"].as_bool().unwrap_or(false),
        location,
        country,
    }
}

/// `tailscale status --json`.
pub fn parse_status(json: &str) -> Option<State> {
    let v: Value = serde_json::from_str(json).ok()?;
    let mut peers: Vec<Machine> = v["Peer"].as_object().map(|m| m.values().map(machine).collect()).unwrap_or_default();
    peers.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Some(State {
        installed: true,
        backend: v["BackendState"].as_str().unwrap_or_default().to_string(),
        tailnet: v["CurrentTailnet"]["Name"].as_str().unwrap_or_default().to_string(),
        me: machine(&v["Self"]),
        peers,
        accounts: Vec::new(),
    })
}

/// `tailscale switch --list`: `ID  Tailnet  Account`, the active one ending in `*`.
pub fn parse_accounts(text: &str) -> Vec<Account> {
    text.lines()
        .filter(|l| !l.starts_with("Warning") && !l.trim().is_empty())
        .skip_while(|l| l.starts_with("ID"))
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let (id, tailnet, account) = (f.first()?, f.get(1)?, f.get(2)?);
            Some(Account {
                id: id.to_string(),
                tailnet: tailnet.to_string(),
                active: account.ends_with('*'),
                account: account.trim_end_matches('*').to_string(),
            })
        })
        .collect()
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    let id = glib::timeout_add_local(Duration::from_secs(5), move || {
        refresh(set.clone());
        glib::ControlFlow::Continue
    });
    Box::new(TimerStop(Some(id)))
}

pub fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        let Some(json) = output(&["tailscale", "status", "--json"]).await else {
            // Not installed, or tailscaled isn't running.
            let installed = output(&["tailscale", "version"]).await.is_some();
            set.set(State { installed, backend: if installed { "Stopped".into() } else { String::new() }, ..Default::default() });
            return;
        };
        let Some(mut st) = parse_status(&json) else { return };
        if let Some(list) = output(&["tailscale", "switch", "--list"]).await {
            st.accounts = parse_accounts(&list);
        }
        set.set(st);
    });
}

async fn act(args: &[&str]) -> Result<(), String> {
    let r = run(args, None).await;
    refresh(live().setter());
    r
}

pub async fn set_up(on: bool) -> Result<(), String> {
    act(&["tailscale", if on { "up" } else { "down" }]).await
}

/// Send this machine's traffic through `ip` (an exit node), or nowhere special.
pub async fn set_exit(ip: Option<&str>) -> Result<(), String> {
    let arg = format!("--exit-node={}", ip.unwrap_or(""));
    act(&["tailscale", "set", &arg]).await
}

/// Let other machines use this one as an exit node.
pub async fn set_advertise_exit(on: bool) -> Result<(), String> {
    let arg = format!("--advertise-exit-node={on}");
    act(&["tailscale", "set", &arg]).await
}

pub async fn switch(account_id: &str) -> Result<(), String> {
    act(&["tailscale", "switch", account_id]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_status() {
        let st = parse_status(
            r#"{"BackendState":"Running","CurrentTailnet":{"Name":"example.github"},
                "Self":{"HostName":"Hades","DNSName":"hades.tail.ts.net.","TailscaleIPs":["100.68.211.7","fd7a::1"],"Online":true},
                "Peer":{"a":{"HostName":"surface","DNSName":"surface.tail.ts.net.","TailscaleIPs":["100.73.1.1"],"Online":false,"OS":"windows"},
                        "b":{"HostName":"se-sto-wg-001","DNSName":"se.mullvad.ts.net.","TailscaleIPs":["100.90.1.1"],"Online":true,"ExitNodeOption":true,"ExitNode":true,
                             "Location":{"Country":"Sweden","City":"Stockholm"}},
                        "c":{"HostName":"Prometheus","DNSName":"prometheus.tail.ts.net.","TailscaleIPs":["100.100.1.1"],"Online":true,"OS":"macOS"}}}"#,
        )
        .unwrap();
        assert!(st.running());
        assert_eq!(st.me.dns, "hades.tail.ts.net");
        let names: Vec<&str> = st.peers.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["Prometheus", "se-sto-wg-001", "surface"], "online first, then by name");
        assert_eq!(st.exit_node().unwrap().location.as_deref(), Some("Sweden, Stockholm"));
        assert_eq!(st.exit_nodes().len(), 1);
        assert!(parse_status("nope").is_none());
    }

    #[test]
    fn reads_accounts() {
        let a = parse_accounts(
            "Warning: client version \"1.104.2\" != tailscaled server version \"1.104.1\"\nID    Tailnet              Account\n69ed  design-nexus.github  design-nexus@github*\n12ab  work.example  me@work.example\n",
        );
        assert_eq!(a.len(), 2);
        assert!(a[0].active && a[0].account == "design-nexus@github");
        assert!(!a[1].active);
    }
}
