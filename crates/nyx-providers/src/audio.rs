//! Sound through PipeWire's PulseAudio service (`pactl`): the default output's volume,
//! the outputs to choose from and each app's own volume; the microphone's level and
//! mute, the inputs to choose from, and which apps are recording. `pactl subscribe`
//! says when something changed.

use crate::{Debounce, Watch, output, run};
use nyx_widgets::live::{Live, Setter, Stop};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Output {
    pub name: String,
    pub description: String,
    /// `headphones`, `speaker`, `hdmi`, `bluetooth`, …: what to draw.
    pub kind: String,
    pub default: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Stream {
    pub index: u32,
    pub app: String,
    /// The app's program (`firefox`), to find its icon.
    pub binary: String,
    pub icon: Option<String>,
    pub volume: f64,
    pub muted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    pub available: bool,
    /// 0.0–1.5 (150 % is allowed, as in Omarchy).
    pub volume: f64,
    pub muted: bool,
    pub outputs: Vec<Output>,
    pub streams: Vec<Stream>,
    /// The default microphone's level, 0.0–1.5.
    pub mic_volume: f64,
    pub mic_muted: bool,
    /// Microphones and other inputs (not the monitors of outputs).
    pub inputs: Vec<Output>,
    /// Apps recording from a microphone now.
    pub recording: Vec<String>,
}

impl State {
    pub fn current(&self) -> Option<&Output> {
        self.outputs.iter().find(|o| o.default)
    }

    pub fn current_input(&self) -> Option<&Output> {
        self.inputs.iter().find(|o| o.default)
    }
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    let debounce = Debounce::new(60);
    Box::new(Watch::lines(&["pactl", "subscribe"], move |line| {
        // Sinks, sources, their streams and the server (default changes).
        if line.contains("sink") || line.contains("source") || line.contains("server") {
            let s = set.clone();
            debounce.call(move || refresh(s));
        }
    }))
}

pub fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        let default = output(&["pactl", "get-default-sink"]).await.unwrap_or_default();
        let sinks = output(&["pactl", "-f", "json", "list", "sinks"]).await;
        let inputs = output(&["pactl", "-f", "json", "list", "sink-inputs"]).await.unwrap_or_default();
        let Some(sinks) = sinks else {
            set.set(State::default());
            return;
        };
        let mut st = parse(default.trim(), &sinks, &inputs);
        let default_source = output(&["pactl", "get-default-source"]).await.unwrap_or_default();
        let sources = output(&["pactl", "-f", "json", "list", "sources"]).await.unwrap_or_default();
        let recording = output(&["pactl", "-f", "json", "list", "source-outputs"]).await.unwrap_or_default();
        parse_inputs(&mut st, default_source.trim(), &sources, &recording);
        set.set(st);
    });
}

fn percent(v: &Value) -> f64 {
    // `{"front-left": {"value_percent": "40%", …}, …}`: the loudest channel.
    v.as_object()
        .map(|chans| {
            chans
                .values()
                .filter_map(|c| c.get("value_percent")?.as_str()?.trim_end_matches('%').parse::<f64>().ok())
                .fold(0.0, f64::max)
        })
        .unwrap_or(0.0)
        / 100.0
}

fn kind(props: &Value) -> String {
    let get = |k: &str| props.get(k).and_then(Value::as_str).unwrap_or("");
    let ff = get("device.form_factor");
    if get("device.bus") == "bluetooth" {
        return if ff == "headset" || ff == "headphone" { "headphones".into() } else { "bluetooth".into() };
    }
    match ff {
        "headphone" | "headset" => "headphones".into(),
        "speaker" | "internal" => "speaker".into(),
        _ if get("device.icon_name").contains("hdmi") || get("device.description").contains("HDMI") => "hdmi".into(),
        _ => "speaker".into(),
    }
}

/// Build the state from `pactl -f json list sinks` and `… sink-inputs`.
pub fn parse(default: &str, sinks: &str, inputs: &str) -> State {
    let mut st = State { available: true, ..Default::default() };
    let sinks: Vec<Value> = serde_json::from_str(sinks).unwrap_or_default();
    for s in &sinks {
        let name = s.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let props = s.get("properties").cloned().unwrap_or(Value::Null);
        let is_default = name == default;
        if is_default {
            st.volume = s.get("volume").map(percent).unwrap_or(0.0);
            st.muted = s.get("mute").and_then(Value::as_bool).unwrap_or(false);
        }
        st.outputs.push(Output {
            description: s.get("description").and_then(Value::as_str).unwrap_or(&name).to_string(),
            kind: kind(&props),
            default: is_default,
            name,
        });
    }
    let inputs: Vec<Value> = serde_json::from_str(inputs).unwrap_or_default();
    for i in &inputs {
        let props = i.get("properties").cloned().unwrap_or(Value::Null);
        let get = |k: &str| props.get(k).and_then(Value::as_str).map(str::to_string);
        // Event sounds and the like aren't apps to turn down.
        if get("media.role").as_deref() == Some("event") {
            continue;
        }
        st.streams.push(Stream {
            index: i.get("index").and_then(Value::as_u64).unwrap_or(0) as u32,
            app: get("application.name").unwrap_or_else(|| "App".into()),
            binary: get("application.process.binary").unwrap_or_default(),
            icon: get("application.icon_name"),
            volume: i.get("volume").map(percent).unwrap_or(0.0),
            muted: i.get("mute").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    st
}

/// Add the microphones from `pactl -f json list sources` and the apps recording from
/// `… source-outputs`.
pub fn parse_inputs(st: &mut State, default: &str, sources: &str, outputs: &str) {
    let sources: Vec<Value> = serde_json::from_str(sources).unwrap_or_default();
    for s in &sources {
        let props = s.get("properties").cloned().unwrap_or(Value::Null);
        // Every output has a "monitor" source; those aren't microphones.
        if props.get("device.class").and_then(Value::as_str) == Some("monitor") {
            continue;
        }
        let name = s.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let is_default = name == default;
        if is_default {
            st.mic_volume = s.get("volume").map(percent).unwrap_or(0.0);
            st.mic_muted = s.get("mute").and_then(Value::as_bool).unwrap_or(false);
        }
        let bt = props.get("device.bus").and_then(Value::as_str) == Some("bluetooth");
        st.inputs.push(Output {
            description: s.get("description").and_then(Value::as_str).unwrap_or(&name).to_string(),
            kind: if bt { "headset".into() } else { "microphone".into() },
            default: is_default,
            name,
        });
    }
    let outs: Vec<Value> = serde_json::from_str(outputs).unwrap_or_default();
    for o in &outs {
        let props = o.get("properties").cloned().unwrap_or(Value::Null);
        let get = |k: &str| props.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        // Peak meters (pavucontrol, the shell's level bars) aren't recording.
        if get("stream.monitor") == "true" || get("application.id") == "org.PulseAudio.pavucontrol" {
            continue;
        }
        let app = get("application.name");
        if !app.is_empty() && !st.recording.contains(&app) {
            st.recording.push(app);
        }
    }
}

pub async fn set_mic_volume(v: f64) -> Result<(), String> {
    run(&["pactl", "set-source-volume", "@DEFAULT_SOURCE@", &pct(v)], None).await
}

pub async fn set_mic_muted(muted: bool) -> Result<(), String> {
    run(&["pactl", "set-source-mute", "@DEFAULT_SOURCE@", if muted { "1" } else { "0" }], None).await
}

pub async fn set_input(name: &str) -> Result<(), String> {
    run(&["pactl", "set-default-source", name], None).await
}

fn pct(v: f64) -> String {
    format!("{}%", (v.clamp(0.0, 1.5) * 100.0).round() as u32)
}

pub async fn set_volume(v: f64) -> Result<(), String> {
    run(&["pactl", "set-sink-volume", "@DEFAULT_SINK@", &pct(v)], None).await
}

pub async fn set_muted(muted: bool) -> Result<(), String> {
    run(&["pactl", "set-sink-mute", "@DEFAULT_SINK@", if muted { "1" } else { "0" }], None).await
}

pub async fn set_output(name: &str) -> Result<(), String> {
    run(&["pactl", "set-default-sink", name], None).await
}

pub async fn set_stream_volume(index: u32, v: f64) -> Result<(), String> {
    run(&["pactl", "set-sink-input-volume", &index.to_string(), &pct(v)], None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINKS: &str = r#"[
      {"name":"alsa_output.analog","description":"Built-in Audio","mute":false,
       "volume":{"front-left":{"value_percent":"40%"},"front-right":{"value_percent":"42%"}},
       "properties":{"device.form_factor":"internal","device.bus":"pci"}},
      {"name":"bluez_output.XX","description":"WH-1000XM5","mute":false,
       "volume":{"front-left":{"value_percent":"70%"}},
       "properties":{"device.form_factor":"headphone","device.bus":"bluetooth"}}
    ]"#;
    const INPUTS: &str = r#"[
      {"index":7,"mute":false,"volume":{"front-left":{"value_percent":"100%"}},
       "properties":{"application.name":"Firefox","application.process.binary":"firefox"}},
      {"index":8,"mute":false,"volume":{"front-left":{"value_percent":"100%"}},
       "properties":{"application.name":"libcanberra","media.role":"event"}}
    ]"#;

    #[test]
    fn reads_outputs_and_apps() {
        let st = parse("bluez_output.XX", SINKS, INPUTS);
        assert!((st.volume - 0.7).abs() < 1e-9);
        assert_eq!(st.current().unwrap().kind, "headphones");
        assert_eq!(st.outputs[0].kind, "speaker");
        assert!((parse("alsa_output.analog", SINKS, "[]").volume - 0.42).abs() < 1e-9, "the loudest channel");
        assert_eq!(st.streams.len(), 1, "event sounds left out");
        assert_eq!(st.streams[0].binary, "firefox");
    }

    #[test]
    fn reads_microphones() {
        let mut st = State::default();
        parse_inputs(
            &mut st,
            "alsa_input.analog",
            r#"[{"name":"alsa_output.analog.monitor","description":"Monitor of Built-in","properties":{"device.class":"monitor"}},
                {"name":"alsa_input.analog","description":"Built-in Mic","mute":true,"volume":{"mono":{"value_percent":"55%"}},
                 "properties":{"device.class":"sound","device.bus":"pci"}}]"#,
            r#"[{"properties":{"application.name":"Firefox"}},{"properties":{"application.name":"Shell","stream.monitor":"true"}}]"#,
        );
        assert_eq!(st.inputs.len(), 1, "monitors aren't microphones");
        assert!(st.mic_muted && (st.mic_volume - 0.55).abs() < 1e-9);
        assert_eq!(st.recording, vec!["Firefox"], "level meters aren't recording");
    }

    #[test]
    fn percentages() {
        assert_eq!(pct(0.644), "64%");
        assert_eq!(pct(2.0), "150%");
    }
}
