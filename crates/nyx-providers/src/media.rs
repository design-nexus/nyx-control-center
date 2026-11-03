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
    pub album: String,
    /// Microseconds: the track's length, and where it was when read.
    pub length: i64,
    pub position: i64,
    /// When `position` was read (glib monotonic time, µs), to count on from.
    pub read_at: i64,
    pub can_seek: bool,
    /// The track's id, which seeking needs.
    pub track: String,
}

impl State {
    /// Where playback is now (counting on from the last reading while playing).
    pub fn position_now(&self) -> i64 {
        let p = if self.playing { self.position + (glib::monotonic_time() - self.read_at) } else { self.position };
        if self.length > 0 { p.clamp(0, self.length) } else { p.max(0) }
    }
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
        can_seek: p.lookup::<bool>("CanSeek").ok().flatten().unwrap_or(false),
        position: p.lookup::<i64>("Position").ok().flatten().unwrap_or(0),
        read_at: glib::monotonic_time(),
        ..Default::default()
    };
    if let Some(meta) = p.lookup_value("Metadata", None) {
        let m = glib::VariantDict::new(Some(&meta));
        st.title = m.lookup::<String>("xesam:title").ok().flatten().unwrap_or_default();
        st.artist = m.lookup::<Vec<String>>("xesam:artist").ok().flatten().unwrap_or_default().join(", ");
        st.art = m.lookup::<String>("mpris:artUrl").ok().flatten().filter(|u| u.starts_with("file://"));
        st.album = m.lookup::<String>("xesam:album").ok().flatten().unwrap_or_default();
        st.length = m
            .lookup::<i64>("mpris:length")
            .ok()
            .flatten()
            .or_else(|| m.lookup::<u64>("mpris:length").ok().flatten().map(|v| v as i64))
            .unwrap_or(0);
        st.track = m
            .lookup_value("mpris:trackid", None)
            .and_then(|v| {
                v.str().map(String::from).or_else(|| v.get::<glib::variant::ObjectPath>().map(|o| o.as_str().to_string()))
            })
            .unwrap_or_default();
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
        (None, PLAYER, "Seeked", Some(PATH)),
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

/// Jump to `position` (µs) in the current track.
pub fn seek(position: i64) {
    let st = live().get();
    let Some(bus) = session_bus() else { return };
    if st.player.is_empty() || !st.can_seek || st.track.is_empty() {
        return;
    }
    let Ok(track) = glib::variant::ObjectPath::try_from(st.track.clone()) else { return };
    let args = glib::Variant::tuple_from_iter([track.to_variant(), position.to_variant()]);
    bus.call(
        Some(&st.player),
        PATH,
        PLAYER,
        "SetPosition",
        Some(&args),
        None,
        gio::DBusCallFlags::NONE,
        1000,
        gio::Cancellable::NONE,
        |_| {},
    );
    // Players announce a seek, not the new position: read it again shortly.
    let set = live().setter();
    glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
        if let Some(bus) = session_bus() {
            let last = set.get().map(|s| s.player).unwrap_or_default();
            set.set(read(&bus, &last));
        }
    });
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
