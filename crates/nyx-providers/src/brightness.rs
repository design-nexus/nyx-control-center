//! Screen and keyboard brightness through `brightnessctl` (which goes through logind,
//! so no special permissions). The kernel doesn't announce brightness changes, so the
//! value is read when a view appears and every 2 seconds while one is shown.

use crate::{TimerStop, output, run};
use nyx_widgets::live::{Live, Setter, Stop};
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Level {
    pub device: String,
    /// 0.0–1.0
    pub value: f64,
    /// Steps the hardware has (keyboard backlights have a few).
    pub max: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub screen: Option<Level>,
    pub keyboard: Option<Level>,
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

pub fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        if let Some(list) = output(&["brightnessctl", "-m", "-l"]).await {
            set.set(parse(&list));
        }
    });
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
}
