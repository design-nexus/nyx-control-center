//! Phones and other devices paired through Flux (Nyx Connect): which are connected,
//! their battery, and what each can do now. Follows fluxd (`nyx-connect`'s watch) only
//! while a tile or view shows it; pairing and access live in Settings › Connected Devices.

use nyx_connect::{Access, Client, Event, Feature, Snapshot, Watch};
use nyx_widgets::live::{Live, OnStop, Setter, Stop};

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Device {
    /// Flux's device ID.
    pub id: String,
    pub name: String,
    pub icon: String,
    pub connected: bool,
    pub battery: Option<i32>,
    pub charging: bool,
    /// It can be rung (and is connected).
    pub can_ring: bool,
    /// The clipboard can be sent to it (connected, and its clipboard access is on).
    pub can_clipboard: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    /// fluxd is installed.
    pub installed: bool,
    /// fluxd answers.
    pub available: bool,
    /// Paired devices, connected first.
    pub devices: Vec<Device>,
    /// Devices asking to pair, or waiting for this computer's confirmation.
    pub pairing: Vec<String>,
}

impl State {
    pub fn connected(&self) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.connected).collect()
    }
}

thread_local! {
    static LIVE: Live<State> = Live::new(State { installed: installed(), ..State::default() }, start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn installed() -> bool {
    crate::present("fluxd") || nyx_connect::socket_path().exists()
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    let (tx, rx) = async_channel::unbounded::<Event>();
    let watch = Watch::start(move |e| {
        let _ = tx.send_blocking(e);
    });
    glib::spawn_future_local(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Event::State(s) => set.set(read(&s)),
                Event::Unavailable(_) => set.set(State { installed: installed(), ..State::default() }),
                Event::Toast(_) => {}
            }
        }
    });
    Box::new(OnStop(Some(Box::new(move || drop(watch)))))
}

pub fn read(s: &Snapshot) -> State {
    let mut devices: Vec<Device> = s
        .paired()
        .map(|d| Device {
            id: d.id.clone(),
            name: d.display_name(&s.devices),
            icon: d.icon().into(),
            connected: d.online,
            battery: d.battery.filter(|_| d.online).map(|b| b.charge),
            charging: d.battery.is_some_and(|b| b.charging),
            can_ring: d.online && d.has(Feature::Ring),
            can_clipboard: d.online && d.has(Feature::Clipboard) && s.allowed(&d.id, Access::Clipboard),
        })
        .collect();
    devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    let pairing =
        s.devices.iter().filter(|d| matches!(d.pair_state.as_str(), "incoming" | "confirm")).map(|d| d.name.clone()).collect();
    State { installed: true, available: true, devices, pairing }
}

async fn blocking(work: impl FnOnce(&Client) -> Result<(), nyx_connect::Error> + Send + 'static) -> Result<(), String> {
    gio::spawn_blocking(move || work(&Client::new()).map_err(|e| e.message))
        .await
        .unwrap_or_else(|_| Err("Flux didn't answer".into()))
}

/// Ring a phone, to find it.
pub async fn ring(id: &str) -> Result<(), String> {
    let id = id.to_string();
    blocking(move |c| c.ring(&id)).await
}

/// Send what's on this computer's clipboard (text or an image) to the device.
pub async fn send_clipboard(id: &str) -> Result<(), String> {
    let id = id.to_string();
    blocking(move |c| c.send_clipboard(&id, "")).await
}

/// Start fluxd's user service.
pub async fn start_flux() -> Result<(), String> {
    crate::run(&["systemctl", "--user", "start", "fluxd.service"], None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_first_and_actions_follow_access() {
        let s: Snapshot = serde_json::from_value(serde_json::json!({
            "devices": [
                { "id": "b", "name": "Tablet", "type": "tablet", "paired": true, "online": false, "plugins": ["findmyphone"] },
                { "id": "a", "name": "Pandora", "type": "phone", "paired": true, "online": true,
                  "battery": { "charge": 80, "charging": true }, "plugins": ["findmyphone", "clipboard"] },
                { "id": "c", "name": "New", "paired": false, "online": true, "pairState": "incoming" }
            ],
            "settings": { "deviceRules": { "a": { "clipboard": false } } }
        }))
        .unwrap();
        let st = read(&s);
        assert_eq!(st.devices.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["Pandora", "Tablet"]);
        let p = &st.devices[0];
        assert!(p.can_ring && !p.can_clipboard, "clipboard access is off for it");
        assert_eq!((p.battery, p.charging), (Some(80), true));
        assert!(!st.devices[1].can_ring, "not connected");
        assert_eq!(st.pairing, ["New"]);
    }
}
