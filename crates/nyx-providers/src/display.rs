//! Screens, as Omarchy's Display panel offers them: each screen's brightness (laptop
//! backlight or DDC), the focused screen's scale, the text size everywhere, and
//! turning the laptop's own screen off while another is plugged in. Everything goes
//! through Omarchy's commands, so its keys and OSD agree. Read when a view appears
//! and after each change (external monitors answer slowly, so nothing polls).

use crate::{output, run};
use nyx_widgets::live::{Live, Setter, Stop};

/// The scales Omarchy steps through.
pub const SCALES: &[f64] = &[1.0, 1.25, 1.6, 2.0, 3.0, 4.0];
/// Text size limits (px), and the default.
pub const TEXT_MIN: u32 = 9;
pub const TEXT_MAX: u32 = 20;
pub const TEXT_DEFAULT: u32 = 12;

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Screen {
    pub name: String,
    pub focused: bool,
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
    /// 0.0–1.0, when it can be set.
    pub brightness: Option<f64>,
    /// The laptop's own panel.
    pub internal: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    pub screens: Vec<Screen>,
    /// The focused screen's scale.
    pub scale: f64,
    pub text_px: u32,
}

impl State {
    /// A screen other than the laptop's is on: the laptop's can be turned off.
    pub fn external_active(&self) -> bool {
        self.screens.iter().any(|s| !s.internal && s.enabled)
    }

    pub fn internal(&self) -> Option<&Screen> {
        self.screens.iter().find(|s| s.internal)
    }
}

pub fn is_internal(name: &str) -> bool {
    ["eDP-", "LVDS-", "DSI-"].iter().any(|p| name.starts_with(p))
}

/// The JSON line of `omarchy-monitor-state`: the screens (test outputs left out).
pub fn parse_screens(state: &str) -> Vec<Screen> {
    let Some(json) = state.lines().rev().find(|l| l.trim_start().starts_with('[')) else { return Vec::new() };
    let v: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    v.iter()
        .filter_map(|m| {
            let name = m["name"].as_str()?.to_string();
            if name.starts_with("HEADLESS-") {
                return None;
            }
            Some(Screen {
                internal: is_internal(&name),
                focused: m["focused"].as_bool().unwrap_or(false),
                enabled: m["enabled"].as_bool().unwrap_or(true),
                width: m["width"].as_u64().unwrap_or(0) as u32,
                height: m["height"].as_u64().unwrap_or(0) as u32,
                brightness: None,
                name,
            })
        })
        .collect()
}

/// `hyprctl monitors all -j`: the screens and the focused one's scale (without Omarchy).
pub fn parse_hypr_monitors(json: &str) -> (Vec<Screen>, f64) {
    let v: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    let scale = v.iter().find(|m| m["focused"].as_bool() == Some(true)).and_then(|m| m["scale"].as_f64()).unwrap_or(1.0);
    let screens = v
        .iter()
        .filter_map(|m| {
            let name = m["name"].as_str()?.to_string();
            if name.starts_with("HEADLESS-") {
                return None;
            }
            Some(Screen {
                internal: is_internal(&name),
                focused: m["focused"].as_bool().unwrap_or(false),
                enabled: !m["disabled"].as_bool().unwrap_or(false),
                width: m["width"].as_u64().unwrap_or(0) as u32,
                height: m["height"].as_u64().unwrap_or(0) as u32,
                brightness: None,
                name,
            })
        })
        .collect();
    (screens, scale)
}

/// The laptop panel's backlight from `brightnessctl -m`: `device,backlight,now,50%,max`.
fn parse_backlight(line: &str) -> Option<f64> {
    let pct = line.split(',').nth(3)?.trim_end_matches('%').parse::<f64>().ok()?;
    Some((pct / 100.0).clamp(0.0, 1.0))
}

/// `text size: 12 px` (or `12 (default)`).
pub fn parse_text_size(text: &str) -> Option<u32> {
    let line = text.lines().find(|l| l.starts_with("text size:"))?;
    line.trim_start_matches("text size:").split_whitespace().next()?.parse().ok()
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set);
    Box::new(crate::TimerStop(None))
}

pub fn refresh(set: Setter<State>) {
    if !crate::omarchy::here() {
        // Hyprland's screens; only the laptop panel's brightness can be set.
        glib::MainContext::default().spawn_local(async move {
            let (mut screens, scale) =
                output(&["hyprctl", "monitors", "all", "-j"]).await.map(|t| parse_hypr_monitors(&t)).unwrap_or_default();
            let backlight = output(&["brightnessctl", "-m", "-c", "backlight", "info"]).await.and_then(|l| parse_backlight(&l));
            for s in screens.iter_mut().filter(|s| s.internal && s.enabled) {
                s.brightness = backlight;
            }
            set.set(State { screens, scale, text_px: TEXT_DEFAULT });
        });
        return;
    }
    glib::MainContext::default().spawn_local(async move {
        let mut screens = output(&["omarchy-monitor-state"]).await.map(|t| parse_screens(&t)).unwrap_or_default();
        for s in screens.iter_mut().filter(|s| s.enabled) {
            s.brightness = output(&["omarchy-brightness-display", "--monitor", &s.name])
                .await
                .and_then(|o| o.trim().parse::<f64>().ok())
                .map(|p| (p / 100.0).clamp(0.0, 1.0));
        }
        let scale = output(&["omarchy-hyprland-monitor-scaling"]).await.and_then(|o| o.trim().parse().ok()).unwrap_or(1.0);
        let text_px = output(&["omarchy-display-text-size"]).await.and_then(|t| parse_text_size(&t)).unwrap_or(TEXT_DEFAULT);
        set.set(State { screens, scale, text_px });
    });
}

async fn then_refresh(r: Result<(), String>) -> Result<(), String> {
    refresh(live().setter());
    r
}

pub async fn set_brightness(screen: &str, v: f64) -> Result<(), String> {
    let pct = format!("{}%", (v.clamp(0.01, 1.0) * 100.0).round().max(1.0) as u32);
    let r = if crate::omarchy::here() {
        run(&["omarchy-brightness-display", "--monitor", screen, "--no-osd", &pct], None).await
    } else if is_internal(screen) {
        run(&["brightnessctl", "-q", "-c", "backlight", "set", &pct], None).await
    } else {
        Err("this screen's brightness can't be set here".into())
    };
    // The sound tile's slider shows the focused screen's.
    crate::brightness::refresh(crate::brightness::live().setter());
    r
}

/// Scale the focused screen.
pub async fn set_scale(scale: f64) -> Result<(), String> {
    let s = if scale.fract() == 0.0 { format!("{scale:.0}") } else { format!("{scale}") };
    then_refresh(run(&["omarchy-hyprland-monitor-scaling", &s], None).await).await
}

/// Text size everywhere (the shell, GTK apps, terminals).
pub async fn set_text_size(px: u32) -> Result<(), String> {
    then_refresh(run(&["omarchy-display-text-size", &px.clamp(TEXT_MIN, TEXT_MAX).to_string()], None).await).await
}

/// Turn the laptop's own screen on or off (off only while another screen is on).
pub async fn set_internal(on: bool) -> Result<(), String> {
    then_refresh(run(&["omarchy-hyprland-monitor-internal", if on { "on" } else { "off" }], None).await).await
}

#[cfg(test)]
mod hypr_tests {
    use super::*;

    #[test]
    fn screens_from_hyprland() {
        let json = r#"[{"name":"eDP-1","focused":true,"disabled":false,"width":2560,"height":1600,"scale":1.6},
            {"name":"DP-2","focused":false,"disabled":true,"width":3840,"height":2160,"scale":1.0},
            {"name":"HEADLESS-1","focused":false,"disabled":false,"width":800,"height":600,"scale":1.0}]"#;
        let (screens, scale) = parse_hypr_monitors(json);
        assert_eq!(scale, 1.6);
        assert_eq!(screens.len(), 2);
        assert!(screens[0].internal && screens[0].focused && screens[0].enabled);
        assert!(!screens[1].internal && !screens[1].enabled);
        assert_eq!(parse_backlight("intel_backlight,backlight,12000,50%,24000"), Some(0.5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_state() {
        let s = parse_screens(concat!(
            "25\neDP-2\n1.6\n",
            r#"[{"name":"eDP-2","enabled":true,"focused":true,"width":2560,"height":1600},"#,
            r#"{"name":"DP-1","enabled":true,"focused":false,"width":3840,"height":2160},{"name":"HEADLESS-20","enabled":true}]"#
        ));
        assert_eq!(s.len(), 2, "test outputs left out");
        assert!(s[0].internal && !s[1].internal);
        let st = State { screens: s, scale: 1.6, text_px: 12 };
        assert!(st.external_active());
        assert_eq!(parse_text_size("text size: 14 px\ngtk text-scaling-factor: 1.1\n"), Some(14));
        assert_eq!(parse_text_size("text size: 12 (default) px\n"), Some(12));
    }
}
