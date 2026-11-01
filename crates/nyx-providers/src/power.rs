//! Power mode (power-profiles-daemon) and battery (UPower), both on the system bus.

use crate::{SignalStop, Stops, system_bus};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profile {
    PowerSaver,
    #[default]
    Balanced,
    Performance,
}

impl Profile {
    pub fn id(self) -> &'static str {
        match self {
            Profile::PowerSaver => "power-saver",
            Profile::Balanced => "balanced",
            Profile::Performance => "performance",
        }
    }

    pub fn parse(s: &str) -> Option<Profile> {
        match s {
            "power-saver" => Some(Profile::PowerSaver),
            "balanced" => Some(Profile::Balanced),
            "performance" => Some(Profile::Performance),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Profile::PowerSaver => "Power saver",
            Profile::Balanced => "Balanced",
            Profile::Performance => "Performance",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    /// 0–100
    pub percent: f64,
    pub charging: bool,
    pub full: bool,
    /// Seconds to empty (discharging) or full (charging), when UPower knows.
    pub time: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub profiles: Vec<Profile>,
    pub profile: Option<Profile>,
    pub battery: Option<Battery>,
}

const PPD: (&str, &str, &str) = ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles", "net.hadess.PowerProfiles");
const UPOWER: (&str, &str, &str) =
    ("org.freedesktop.UPower", "/org/freedesktop/UPower/devices/DisplayDevice", "org.freedesktop.UPower.Device");

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn props(bus: &gio::DBusConnection, (name, path, iface): (&str, &str, &str)) -> Option<glib::VariantDict> {
    let reply = bus
        .call_sync(
            Some(name),
            path,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            Some(&(iface,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
        .ok()?;
    Some(glib::VariantDict::new(Some(&reply.child_value(0))))
}

fn read(bus: &gio::DBusConnection) -> State {
    let mut st = State::default();
    if let Some(p) = props(bus, PPD) {
        st.profile = p.lookup::<String>("ActiveProfile").ok().flatten().and_then(|s| Profile::parse(&s));
        if let Some(list) = p.lookup_value("Profiles", None) {
            for i in 0..list.n_children() {
                let d = glib::VariantDict::new(Some(&list.child_value(i)));
                if let Some(p) = d.lookup::<String>("Profile").ok().flatten().and_then(|s| Profile::parse(&s)) {
                    st.profiles.push(p);
                }
            }
        }
    }
    if let Some(u) = props(bus, UPOWER)
        && u.lookup::<bool>("IsPresent").ok().flatten().unwrap_or(false)
    {
        // UPower states: 1 charging, 2 discharging, 4 fully charged, 5 pending charge.
        let state = u.lookup::<u32>("State").ok().flatten().unwrap_or(0);
        let charging = state == 1;
        let (empty, full) = (
            u.lookup::<i64>("TimeToEmpty").ok().flatten().unwrap_or(0),
            u.lookup::<i64>("TimeToFull").ok().flatten().unwrap_or(0),
        );
        st.battery = Some(Battery {
            percent: u.lookup::<f64>("Percentage").ok().flatten().unwrap_or(0.0),
            charging,
            full: state == 4,
            time: Some(if charging { full } else { empty }).filter(|t| *t > 0),
        });
    }
    st
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    let Some(bus) = system_bus() else { return Box::new(()) };
    set.set(read(&bus));
    let mut stops: Vec<Box<dyn Stop>> = Vec::new();
    for (name, path, _) in [PPD, UPOWER] {
        let (set, b) = (set.clone(), bus.clone());
        let sub = bus.subscribe_to_signal(
            Some(name),
            Some("org.freedesktop.DBus.Properties"),
            Some("PropertiesChanged"),
            Some(path),
            None,
            gio::DBusSignalFlags::NONE,
            move |_| set.set(read(&b)),
        );
        stops.push(Box::new(SignalStop(sub)));
    }
    Box::new(Stops(stops))
}

pub async fn set_profile(p: Profile) -> Result<(), String> {
    let bus = system_bus().ok_or("no system bus")?;
    bus.call_future(
        Some(PPD.0),
        PPD.1,
        "org.freedesktop.DBus.Properties",
        "Set",
        Some(&(PPD.2, "ActiveProfile", p.id().to_variant()).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        3000,
    )
    .await
    .map(|_| ())
    .map_err(|e| e.message().to_string())
}

/// "3 h 10 min", "45 min".
pub fn duration(secs: i64) -> String {
    let m = (secs + 30) / 60;
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration(11_400), "3 h 10 min");
        assert_eq!(duration(2_700), "45 min");
        assert_eq!(duration(7_200), "2 h");
    }

    #[test]
    fn profiles() {
        for p in [Profile::PowerSaver, Profile::Balanced, Profile::Performance] {
            assert_eq!(Profile::parse(p.id()), Some(p));
        }
    }
}
