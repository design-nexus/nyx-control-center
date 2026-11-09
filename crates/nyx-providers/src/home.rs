//! Home: lights, switches, climate and scenes from Nyx Home's service (`nyx-homed`,
//! `io.github.design_nexus.nyx.Home.Devices1`), so the Home app needn't be open. Read
//! when a view appears, every 3 seconds while one is shown, and right after an action.

use crate::{TimerStop, session_bus};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};
use serde::Deserialize;
use std::time::Duration;

const BUS: &str = "io.github.design_nexus.nyx.Home.Automation";
const PATH: &str = "/io/github/design_nexus/nyx/Home/Devices";
const IFACE: &str = "io.github.design_nexus.nyx.Home.Devices1";

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub on: bool,
    #[serde(default = "yes")]
    pub available: bool,
    pub room: Option<String>,
    pub brightness: Option<f64>,
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub dimmable: bool,
    #[serde(default)]
    pub colorable: bool,
    pub target: Option<f64>,
    pub current: Option<f64>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub room: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, serde::Serialize)]
pub struct State {
    /// Home's service answered (it's installed and connected to Home Assistant).
    #[serde(skip)]
    pub available: bool,
    /// Why not, when it isn't.
    #[serde(skip)]
    pub problem: Option<String>,
    #[serde(default)]
    pub rooms: Vec<String>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
}

impl State {
    /// A room's devices: lights first, then the rest.
    pub fn in_room(&self, room: Option<&str>) -> Vec<&Device> {
        let mut v: Vec<&Device> = self.devices.iter().filter(|d| room.is_none() || d.room.as_deref() == room).collect();
        v.sort_by_key(|d| match d.domain.as_str() {
            "light" => 0,
            "switch" | "fan" => 1,
            _ => 2,
        });
        v
    }

    pub fn scenes_in(&self, room: Option<&str>) -> Vec<&Scene> {
        let here: Vec<&Scene> = self.scenes.iter().filter(|s| room.is_some() && s.room.as_deref() == room).collect();
        if here.is_empty() { self.scenes.iter().filter(|s| s.room.is_none()).collect() } else { here }
    }
}

/// Parse `Snapshot()`'s JSON.
pub fn parse(json: &str) -> Option<State> {
    let mut st: State = serde_json::from_str(json).ok()?;
    st.available = true;
    Some(st)
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    let id = glib::timeout_add_local(Duration::from_secs(3), move || {
        refresh(set.clone());
        glib::ControlFlow::Continue
    });
    Box::new(TimerStop(Some(id)))
}

pub fn refresh(set: Setter<State>) {
    let Some(bus) = session_bus() else { return };
    glib::MainContext::default().spawn_local(async move {
        let reply = bus
            .call_future(
                Some(BUS),
                PATH,
                IFACE,
                "Snapshot",
                None,
                Some(glib::VariantTy::new("(s)").unwrap()),
                gio::DBusCallFlags::NO_AUTO_START,
                8000,
            )
            .await;
        let state = match reply {
            Ok(v) => v.child_value(0).str().and_then(parse).unwrap_or_default(),
            Err(e) => State {
                problem: Some(if e.matches(gio::DBusError::ServiceUnknown) || e.matches(gio::DBusError::NameHasNoOwner) {
                    "Home isn't running".into()
                } else {
                    e.message().to_string()
                }),
                ..Default::default()
            },
        };
        set.set(state);
    });
}

/// Run an action (`turn_on`, `turn_off`, `set_temperature`) with service data.
pub async fn call(entity: &str, service: &str, data: serde_json::Value) -> Result<(), String> {
    let bus = session_bus().ok_or("no session bus")?;
    let data = if data.is_null() { String::new() } else { data.to_string() };
    let r = bus
        .call_future(
            Some(BUS),
            PATH,
            IFACE,
            "Call",
            Some(&(entity, service, data.as_str()).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            10_000,
        )
        .await
        .map(|_| ())
        .map_err(|e| e.message().to_string());
    refresh(live().setter());
    r
}

/// Turn a device on or off (scenes: run).
pub async fn toggle(d: &Device) -> Result<(), String> {
    call(&d.id, if d.on { "turn_off" } else { "turn_on" }, serde_json::Value::Null).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_snapshot() {
        let st = parse(
            r#"{"rooms":["Living room"],"devices":[
                {"id":"switch.fan","name":"Fan","domain":"switch","on":true,"available":true,"room":"Living room","dimmable":false,"colorable":false},
                {"id":"light.lamp","name":"Lamp","domain":"light","on":true,"available":true,"room":"Living room","brightness":0.4,"color":[157,123,255],"dimmable":true,"colorable":true},
                {"id":"climate.heat","name":"Heating","domain":"climate","on":true,"available":true,"room":null,"target":21.5,"current":20.0,"dimmable":false,"colorable":false}],
              "scenes":[{"id":"scene.evening","name":"Evening","room":"Living room"},{"id":"scene.away","name":"Away","room":null}]}"#,
        )
        .unwrap();
        assert!(st.available);
        let here: Vec<&str> = st.in_room(Some("Living room")).iter().map(|d| d.id.as_str()).collect();
        assert_eq!(here, vec!["light.lamp", "switch.fan"], "lights first");
        assert_eq!(st.in_room(None).len(), 3, "everything when no room is picked");
        assert_eq!(st.scenes_in(Some("Living room"))[0].name, "Evening");
        assert_eq!(st.scenes_in(Some("Attic"))[0].name, "Away", "a room without scenes gets the house-wide ones");
        assert!(parse("not json").is_none());
    }
}
