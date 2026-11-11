//! Providers as topics: a provider runs only while the interface subscribes to its
//! topic (a tile or detail view showing it), as the GTK tiles subscribed only while
//! mapped.

use crate::App;
use nyx_providers::{
    audio, backup, bluetooth, brightness, calendar, devices, display, dns, home, media, network, omarchy, power, routes,
    tailscale, vpn,
};
use nyx_widgets::live::{Live, Subscription};
use serde::Serialize;
use serde_json::{Value, json};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

#[derive(Default)]
pub struct Topics {
    running: RefCell<HashMap<String, Box<dyn Any>>>,
}

fn bind<T: Clone + PartialEq + Serialize + 'static>(
    app: &Rc<App>,
    topic: &'static str,
    live: Live<T>,
    shape: fn(&T) -> Value,
) -> Box<dyn Any> {
    let link = app.link.clone();
    let sub: Subscription = live.subscribe(move |st| link.publish(topic, shape(st)));
    Box::new(sub)
}

fn plain<T: Serialize>(st: &T) -> Value {
    serde_json::to_value(st).unwrap_or(Value::Null)
}

/// Now playing with where it is now (players don't announce progress).
fn media_json(st: &media::State) -> Value {
    let mut v = plain(st);
    v["position_now"] = json!(st.position_now());
    v["player_name"] = json!(player_name(&st.player));
    v
}

/// A player's name from its bus name (as the GTK tile's `player_name`).
pub fn player_name(bus: &str) -> String {
    let rest = bus.strip_prefix("org.mpris.MediaPlayer2.").unwrap_or(bus);
    let name = if rest.starts_with("io.github.") || rest.starts_with("org.") || rest.starts_with("com.") {
        rest.split('.').next_back().unwrap_or(rest)
    } else {
        rest.split('.').next().unwrap_or(rest)
    };
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// When an event is, briefly (as the GTK tile's `event_when`).
pub fn event_when(e: &calendar::Event, now: i64) -> String {
    let local = |t: i64| glib::DateTime::from_unix_local(t).ok();
    let (Some(start), Some(today)) = (local(e.starts), local(now)) else { return String::new() };
    let days = (start.ymd().0 - today.ymd().0) * 366 + start.day_of_year() - today.day_of_year();
    let fmt = |f: &str| start.format(f).map(|s| s.to_string()).unwrap_or_default();
    if e.all_day {
        return match days {
            ..=0 => "All day".into(),
            1 => "Tomorrow".into(),
            2..=6 => fmt("%A"),
            _ => fmt("%b %-d"),
        };
    }
    if e.starts <= now {
        return "Now".into();
    }
    let mins = (e.starts - now) / 60;
    if mins < 60 {
        return format!("in {} min", mins.max(1));
    }
    let time = fmt("%H:%M");
    match days {
        0 => time,
        1 => format!("Tomorrow {time}"),
        2..=6 => format!("{} {time}", fmt("%a")),
        _ => fmt("%b %-d"),
    }
}

fn calendar_json(st: &calendar::State) -> Value {
    let now = glib::real_time() / 1_000_000;
    let mut v = plain(st);
    if let Some(events) = v.get_mut("events").and_then(Value::as_array_mut) {
        for (e, src) in events.iter_mut().zip(st.events.iter()) {
            e["when"] = json!(event_when(src, now));
        }
    }
    v
}

fn power_json(st: &power::State) -> Value {
    let mut v = plain(st);
    if let Some(b) = &st.battery {
        v["battery"]["duration"] = json!(b.time.map(power::duration));
    }
    v["profiles"] = json!(st.profiles.iter().map(|p| p.id()).collect::<Vec<_>>());
    v["profile"] = json!(st.profile.map(|p| p.id()));
    v
}

fn network_json(st: &network::State) -> Value {
    let mut v = plain(st);
    v["bars"] = json!(network::bars(st.signal));
    v
}

fn mode_json(desktop: &nyx_desktop::Desktop) -> Value {
    use nyx_desktop::omarchy::Mode as M;
    let Some(m) = desktop.focused_monitor() else { return json!({"workspace": 0, "mode": "tiling", "label": "Tiling"}) };
    let ws = m.active_workspace;
    let layout = desktop.workspaces.iter().find(|w| w.id == ws).map(|w| w.tiled_layout.clone()).unwrap_or_default();
    let mode = nyx_desktop::omarchy::workspace_mode(ws, &layout);
    let id = match mode {
        M::Tiling => "tiling",
        M::Scrolling => "scrolling",
        M::Floating => "floating",
    };
    let modes: Vec<&str> = nyx_desktop::omarchy::modes().iter().map(|m| m.label()).collect();
    json!({"workspace": ws, "mode": id, "label": mode.label(), "modes": modes})
}

fn start(app: &Rc<App>, topic: &str) -> Option<Box<dyn Any>> {
    Some(match topic {
        "audio" => bind(app, "audio", audio::live(), plain),
        "network" => bind(app, "network", network::live(), network_json),
        "bluetooth" => bind(app, "bluetooth", bluetooth::live(), plain),
        "devices" => bind(app, "devices", devices::live(), plain),
        "brightness" => bind(app, "brightness", brightness::live(), plain),
        "display" => bind(app, "display", display::live(), plain),
        "dns" => bind(app, "dns", dns::live(), plain),
        "dns-details" => bind(app, "dns-details", dns::details(), |d: &Option<dns::Details>| {
            let mut v = plain(d);
            if let Some(d) = d {
                v["down_text"] = json!(d.down.map(dns::rate));
                v["up_text"] = json!(d.up.map(dns::rate));
            }
            v
        }),
        "power" => bind(app, "power", power::live(), power_json),
        "power-details" => bind(app, "power-details", power::details(), plain),
        "power-history" => bind(app, "power-history", power::history(), plain),
        "calendar" => bind(app, "calendar", calendar::live(), calendar_json),
        "home" => bind(app, "home", home::live(), plain),
        "omarchy" => bind(app, "omarchy", omarchy::live(), plain),
        "routes" => bind(app, "routes", routes::live(), |st: &routes::State| {
            let mut v = plain(st);
            v["route"] = json!(format!("{:?}", st.route()).to_lowercase());
            v["nord"]["connected"] = json!(st.nord.connected());
            v["tor"]["routing"] = json!(st.tor.routing());
            v
        }),
        "tailscale" => bind(app, "tailscale", tailscale::live(), |st: &tailscale::State| {
            let mut v = plain(st);
            v["running"] = json!(st.running());
            v["exit_node"] = plain(&st.exit_node());
            v
        }),
        "vpn" => bind(app, "vpn", vpn::live(), plain),
        "backup" => bind(app, "backup", backup::live(), plain),
        "media" => {
            let sub = bind(app, "media", media::live(), media_json);
            // The bar and times move on their own while playing.
            let link = app.link.clone();
            let tick = glib::timeout_add_local(Duration::from_secs(1), move || {
                let st = media::live().get();
                if st.playing {
                    link.publish("media", media_json(&st));
                }
                glib::ControlFlow::Continue
            });
            Box::new((sub, Timer(Some(tick))))
        }
        "mode" => {
            use gio::prelude::*;
            let last: Rc<RefCell<Option<std::sync::Arc<nyx_desktop::Desktop>>>> = Rc::default();
            let (link, seen) = (app.link.clone(), last.clone());
            let w = nyx_desktop::watch::watch_local(move |u| {
                *seen.borrow_mut() = Some(u.desktop.clone());
                link.publish("mode", mode_json(&u.desktop));
            });
            // Omarchy saves a workspace's mode after Hyprland has heard of the change, so
            // its events can come before the file: the file changing says it again.
            let dir = nyx_desktop::omarchy::layouts_dir();
            let _ = std::fs::create_dir_all(&dir);
            let monitor = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE).ok();
            if let Some(m) = &monitor {
                let link = app.link.clone();
                m.connect_changed(move |_, _, _, _| {
                    if let Some(desktop) = last.borrow().as_ref() {
                        link.publish("mode", mode_json(desktop));
                    }
                });
            }
            Box::new((w, monitor))
        }
        _ => return None,
    })
}

struct Timer(Option<glib::SourceId>);

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(id) = self.0.take() {
            id.remove();
        }
    }
}

/// A topic gained its first subscriber or lost its last.
pub fn watched(app: &Rc<App>, topic: &str, n: usize) {
    let running = app.topics.running.borrow().contains_key(topic);
    if n > 0 && !running {
        if let Some(h) = start(app, topic) {
            app.topics.running.borrow_mut().insert(topic.to_string(), h);
        }
    } else if n == 0 && running {
        let h = app.topics.running.borrow_mut().remove(topic);
        drop(h);
    }
}
