//! Screen and keyboard brightness. The screen is the focused one, read and set through
//! Omarchy's `omarchy-brightness-display` (the laptop's backlight, or an external
//! monitor over DDC, the way Omarchy's brightness keys do); without it, `brightnessctl`
//! on the backlight Omarchy names (`omarchy-hw-display`), else the first. The keyboard
//! light is `brightnessctl`. Nothing announces brightness changes, so the value is read
//! when a view appears and every 2 seconds while one is shown.

use crate::{TimerStop, output, run};
use nyx_widgets::live::{Live, Setter, Stop};
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Level {
    pub device: String,
    /// 0.0–1.0
    pub value: f64,
    /// Steps the hardware has (keyboard backlights have a few).
    pub max: u32,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    pub screen: Option<Level>,
    pub keyboard: Option<Level>,
    /// False until the first reading, so "no screen backlight" isn't assumed before it.
    pub read: bool,
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    let id = glib::timeout_add_local(Duration::from_secs(2), move || {
        refresh(set.clone());
        glib::ControlFlow::Continue
    });
    Box::new(TimerStop(Some(id)))
}

/// `brightnessctl -m -l`: `device,class,current,percent,max` per line.
pub fn parse(list: &str) -> State {
    let mut st = State::default();
    for line in list.lines() {
        let f: Vec<&str> = line.split(',').collect();
        let (Some(dev), Some(class), Some(cur), Some(max)) = (f.first(), f.get(1), f.get(2), f.get(4)) else { continue };
        let (Ok(cur), Ok(max)) = (cur.parse::<f64>(), max.parse::<u32>()) else { continue };
        if max == 0 {
            continue;
        }
        let level = Level { device: dev.to_string(), value: cur / max as f64, max };
        if *class == "backlight" && st.screen.is_none() {
            st.screen = Some(level);
        } else if *class == "leds" && dev.contains("kbd_backlight") && st.keyboard.is_none() {
            st.keyboard = Some(level);
        }
    }
    st
}

/// Pick the backlight: the one Omarchy names, if listed.
pub fn prefer(mut st: State, list: &str, device: Option<&str>) -> State {
    let Some(want) = device.map(str::trim).filter(|d| !d.is_empty()) else { return st };
    for line in list.lines() {
        let f: Vec<&str> = line.split(',').collect();
        if f.first() == Some(&want)
            && f.get(1) == Some(&"backlight")
            && let (Some(Ok(cur)), Some(Ok(max))) = (f.get(2).map(|c| c.parse::<f64>()), f.get(4).map(|m| m.parse::<u32>()))
            && max > 0
        {
            st.screen = Some(Level { device: want.to_string(), value: cur / max as f64, max });
        }
    }
    st
}

fn omarchy_script() -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("omarchy-brightness-display").is_file()))
}

pub fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        let list = output(&["brightnessctl", "-m", "-l"]).await.unwrap_or_default();
        let mut st = parse(&list);
        st.read = true;
        if omarchy_script() {
            // The focused screen's level, as Omarchy's keys see it.
            let pct = output(&["omarchy-brightness-display"]).await.and_then(|o| o.trim().parse::<f64>().ok());
            let name = output(&["omarchy-hyprland-monitor-focused"]).await.map(|n| n.trim().to_string());
            st.screen = pct.map(|p| Level {
                device: name.filter(|n| !n.is_empty()).unwrap_or_else(|| "the screen".into()),
                value: (p / 100.0).clamp(0.0, 1.0),
                max: 100,
            });
        } else {
            let device = output(&["omarchy-hw-display"]).await;
            st = prefer(st, &list, device.as_deref());
        }
        set.set(st);
    });
}

/// Set the focused screen's brightness (0.0–1.0; never fully off).
pub async fn set_screen(value: f64) -> Result<(), String> {
    let pct = format!("{}%", (value.clamp(0.01, 1.0) * 100.0).round().max(1.0) as u32);
    let r = if omarchy_script() {
        run(&["omarchy-brightness-display", "--no-osd", &pct], None).await
    } else {
        match live().get().screen {
            Some(s) => run(&["brightnessctl", "-q", "-d", &s.device, "set", &pct], None).await,
            None => Err("no screen backlight".into()),
        }
    };
    refresh(live().setter());
    r
}

pub async fn set(device: &str, value: f64) -> Result<(), String> {
    let pct = format!("{}%", (value.clamp(0.0, 1.0) * 100.0).round() as u32);
    let r = run(&["brightnessctl", "-q", "-d", device, "set", &pct], None).await;
    refresh(live().setter());
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_devices() {
        let st = parse("nvidia_0,backlight,33,33%,100\nasus::kbd_backlight,leds,2,66%,3\ninput3::capslock,leds,0,0%,1\n");
        assert_eq!(st.screen.as_ref().unwrap().device, "nvidia_0");
        assert!((st.screen.unwrap().value - 0.33).abs() < 1e-9);
        assert_eq!(st.keyboard.unwrap().max, 3);
        assert_eq!(parse("").screen, None);
    }

    #[test]
    fn prefers_the_backlight_omarchy_names() {
        let list = "nvidia_0,backlight,33,33%,100\nintel_backlight,backlight,77,15%,512\n";
        let st = prefer(parse(list), list, Some("intel_backlight\n"));
        let s = st.screen.unwrap();
        assert_eq!(s.device, "intel_backlight");
        assert_eq!(s.max, 512);
        assert_eq!(prefer(parse(list), list, None).screen.unwrap().device, "nvidia_0");
    }
}
