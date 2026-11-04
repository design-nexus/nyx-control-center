//! Omarchy's own switches, read from its state files and changed only through its own
//! commands, so the bar, menus and keybindings stay in step:
//!
//! - Do Not Disturb: `~/.local/state/omarchy/notifications/quiet.json` (`dnd`),
//!   toggled with `omarchy-toggle-notification-silencing`.
//! - Keep awake: `~/.local/state/omarchy/indicators/stay-awake`, `omarchy-toggle-idle`.
//! - Night light: hyprsunset's temperature below 6000 K, `omarchy-toggle-nightlight`.
//! - Workspace mode: see `nyx_desktop::omarchy` (read in the app, which has the desktop).
//!
//! Without Omarchy (NyxOS), Nyx's own services stand in: Do Not Disturb is
//! nyx-notifyd's, keep awake an idle inhibitor in a user unit, and night light
//! hyprsunset, started when it's first turned on.

use crate::{MonitorStop, Stops, output, present, run};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub dnd: bool,
    pub keep_awake: bool,
    pub night_light: bool,
}

/// Whether Omarchy's commands are installed.
pub fn here() -> bool {
    present("omarchy-menu")
}

const NOTIFYD: [&str; 6] = [
    "busctl",
    "--user",
    "call",
    "io.github.design_nexus.nyx.Notifications",
    "/io/github/design_nexus/nyx/Notifications",
    "io.github.design_nexus.nyx.Notifications1",
];
/// The user unit that holds the idle inhibitor while keep awake is on.
const KEEP_AWAKE: &str = "nyx-keep-awake.service";
/// Night light's temperature, as Omarchy's.
const NIGHT_KELVIN: &str = "4000";

async fn notifyd(method: &str) -> Option<bool> {
    let mut args = NOTIFYD.to_vec();
    args.push(method);
    // `b true`
    output(&args).await.map(|o| o.trim().ends_with("true"))
}

fn state_dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::home_dir().join(".local/state"))
        .join("omarchy")
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn read_files() -> (bool, bool) {
    let dir = state_dir();
    let dnd = std::fs::read_to_string(dir.join("notifications/quiet.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("dnd")?.as_bool())
        .unwrap_or(false);
    let awake = dir.join("indicators/stay-awake").exists();
    (dnd, awake)
}

/// hyprsunset's temperature: night light is on below 6000 K (as Omarchy decides).
pub fn night_light_on(reply: &str) -> bool {
    // The reply is just the number; anything else (an error) means it isn't running.
    reply.trim().parse::<u32>().is_ok_and(|t| t < 6000)
}

pub fn refresh(set: Setter<State>) {
    if here() {
        let (dnd, keep_awake) = read_files();
        set.update(|s| {
            s.dnd = dnd;
            s.keep_awake = keep_awake;
        });
    } else {
        let set = set.clone();
        glib::MainContext::default().spawn_local(async move {
            let dnd = notifyd("Dnd").await.unwrap_or(false);
            let keep_awake = run(&["systemctl", "--user", "is-active", "--quiet", KEEP_AWAKE], None).await.is_ok();
            set.update(|s| {
                s.dnd = dnd;
                s.keep_awake = keep_awake;
            });
        });
    }
    glib::MainContext::default().spawn_local(async move {
        let night = output(&["hyprctl", "hyprsunset", "temperature"]).await.is_some_and(|r| night_light_on(&r));
        set.update(|s| s.night_light = night);
    });
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    let mut stops: Vec<Box<dyn Stop>> = Vec::new();
    if !here() {
        return Box::new(Stops(stops));
    }
    for sub in ["notifications", "indicators"] {
        let dir = state_dir().join(sub);
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(m) = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
            let set = set.clone();
            m.connect_changed(move |_, _, _, _| {
                let (dnd, keep_awake) = read_files();
                set.update(|s| {
                    s.dnd = dnd;
                    s.keep_awake = keep_awake;
                });
            });
            stops.push(Box::new(MonitorStop(m)));
        }
    }
    Box::new(Stops(stops))
}

async fn toggle(cmd: &str) -> Result<(), String> {
    let r = run(&[cmd], None).await;
    refresh(live().setter());
    r
}

pub async fn toggle_dnd() -> Result<(), String> {
    if here() {
        return toggle("omarchy-toggle-notification-silencing").await;
    }
    let r = notifyd("ToggleDnd").await.map(|_| ()).ok_or_else(|| "the notification service didn't answer".to_string());
    refresh(live().setter());
    r
}

pub async fn toggle_keep_awake() -> Result<(), String> {
    if here() {
        return toggle("omarchy-toggle-idle").await;
    }
    let r = if live().get().keep_awake {
        run(&["systemctl", "--user", "stop", KEEP_AWAKE], None).await
    } else {
        run(
            &[
                "systemd-run",
                "--user",
                "--unit",
                KEEP_AWAKE,
                "--description",
                "Keep awake (Control Center)",
                "systemd-inhibit",
                "--what=idle",
                "--who=Control Center",
                "--why=Keep awake",
                "sleep",
                "infinity",
            ],
            None,
        )
        .await
    };
    refresh(live().setter());
    r
}

pub async fn toggle_night_light() -> Result<(), String> {
    if here() {
        return toggle("omarchy-toggle-nightlight").await;
    }
    let r = if live().get().night_light {
        run(&["hyprctl", "hyprsunset", "identity"], None).await
    } else {
        // hyprsunset answers hyprctl once it runs; start it the first time.
        if output(&["hyprctl", "hyprsunset", "temperature"]).await.is_none_or(|r| r.trim().parse::<u32>().is_err()) {
            let _ = run(&["systemd-run", "--user", "--unit", "nyx-hyprsunset", "hyprsunset"], None).await;
            glib::timeout_future(std::time::Duration::from_millis(600)).await;
        }
        run(&["hyprctl", "hyprsunset", "temperature", NIGHT_KELVIN], None).await
    };
    refresh(live().setter());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_light() {
        assert!(night_light_on("4000"));
        assert!(!night_light_on("6500\n"));
        assert!(!night_light_on("Couldn't connect to /run/user/1000/hypr/x/.hyprsunset.sock. (3)"), "not running: off");
    }
}
