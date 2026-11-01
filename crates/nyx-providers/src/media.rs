//! What's playing, from MPRIS players on the session bus: the one playing, else the one
//! that played last.

use crate::{Debounce, SignalStop, Stops, session_bus};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};

const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// The player's bus name; empty when nothing is playing anywhere.
    pub player: String,
    /// The player's app (desktop id, when it says).
    pub app: String,
    pub title: String,
    pub artist: String,
    /// A local file (`file://…`).
    pub art: Option<String>,
    pub playing: bool,
    pub can_previous: bool,
    pub can_next: bool,
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn call(
    bus: &gio::DBusConnection,
    dest: &str,
    path: &str,
    iface: &str,
    method: &str,
    args: Option<&glib::Variant>,
) -> Option<glib::Variant> {
    bus.call_sync(Some(dest), path, iface, method, args, None, gio::DBusCallFlags::NONE, 400, gio::Cancellable::NONE).ok()
}

fn player_state(bus: &gio::DBusConnection, name: &str) -> Option<State> {
    let all = call(bus, name, PATH, "org.freedesktop.DBus.Properties", "GetAll", Some(&(PLAYER,).to_variant()))?;
    let p = glib::VariantDict::new(Some(&all.child_value(0)));
    let mut st = State {
        player: name.to_string(),
        playing: p.lookup::<String>("PlaybackStatus").ok().flatten().as_deref() == Some("Playing"),
        can_previous: p.lookup::<bool>("CanGoPrevious").ok().flatten().unwrap_or(false),
        can_next: p.lookup::<bool>("CanGoNext").ok().flatten().unwrap_or(false),
        ..Default::default()
    };
    if let Some(meta) = p.lookup_value("Metadata", None) {
        let m = glib::VariantDict::new(Some(&meta));
        st.title = m.lookup::<String>("xesam:title").ok().flatten().unwrap_or_default();
        st.artist = m.lookup::<Vec<String>>("xesam:artist").ok().flatten().unwrap_or_default().join(", ");
        st.art = m.lookup::<String>("mpris:artUrl").ok().flatten().filter(|u| u.starts_with("file://"));
    }
    let entry = call(
        bus,
        name,
        PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        Some(&("org.mpris.MediaPlayer2", "DesktopEntry").to_variant()),
    );
    st.app = entry.and_then(|v| v.child_value(0).as_variant()).and_then(|v| v.get::<String>()).unwrap_or_default();
    Some(st)
}

fn read(bus: &gio::DBusConnection, last: &str) -> State {
    let Some(names) = call(bus, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", None)
        .and_then(|v| v.child_value(0).get::<Vec<String>>())
    else {
        return State::default();
    };
    let players: Vec<State> =
        names.iter().filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).filter_map(|n| player_state(bus, n)).collect();
    players
        .iter()
        .find(|p| p.playing)
        .or_else(|| players.iter().find(|p| p.player == last))
        .or(players.first())
        .cloned()
        .unwrap_or_default()
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    let Some(bus) = session_bus() else { return Box::new(()) };
    set.set(read(&bus, ""));
    let debounce = Debounce::new(100);
    let mut stops: Vec<Box<dyn Stop>> = Vec::new();
    // Players changing, and players coming and going.
    for (sender, iface, member, path) in [
        (None, "org.freedesktop.DBus.Properties", "PropertiesChanged", Some(PATH)),
        (Some("org.freedesktop.DBus"), "org.freedesktop.DBus", "NameOwnerChanged", None),
    ] {
        let (set, b, d) = (set.clone(), bus.clone(), debounce.clone());
        let sub =
            bus.subscribe_to_signal(sender, Some(iface), Some(member), path, None, gio::DBusSignalFlags::NONE, move |sig| {
                if member == "NameOwnerChanged"
                    && !sig.parameters.child_value(0).str().is_some_and(|n| n.starts_with("org.mpris.MediaPlayer2."))
                {
                    return;
                }
                let (set, b) = (set.clone(), b.clone());
                d.call(move || {
                    let last = set.get().map(|s| s.player).unwrap_or_default();
                    set.set(read(&b, &last));
                });
            });
        stops.push(Box::new(SignalStop(sub)));
    }
    Box::new(Stops(stops))
}

/// `Previous`, `PlayPause` or `Next` on the current player.
pub fn command(method: &str) {
    let player = live().get().player;
    let Some(bus) = session_bus() else { return };
    if player.is_empty() {
        return;
    }
    bus.call(Some(&player), PATH, PLAYER, method, None, None, gio::DBusCallFlags::NONE, 1000, gio::Cancellable::NONE, |_| {});
}
