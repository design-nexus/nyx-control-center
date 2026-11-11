//! Power mode (power-profiles-daemon) and battery (UPower), both on the system bus.

use crate::{SignalStop, Stops, system_bus};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
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

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Battery {
    /// 0–100
    pub percent: f64,
    pub charging: bool,
    pub full: bool,
    /// Seconds to empty (discharging) or full (charging), when UPower knows.
    pub time: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
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

// ---------------------------------------------------------------- Battery details

/// What Omarchy's power panel shows beyond the percentage (`omarchy-battery-status
/// --shell`): power draw, capacity, charge cycles and the charge limit.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Details {
    pub state: String,
    /// "13.1W".
    pub rate: String,
    /// "87Wh".
    pub size: String,
    pub time: String,
    pub cycles: Option<u32>,
    /// "75-80%": charging stops in this range to spare the battery.
    pub threshold: String,
}

/// `key<TAB>value` lines.
pub fn parse_details(text: &str) -> Details {
    let mut d = Details::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('\t') else { continue };
        let v = v.trim().to_string();
        match k {
            "state" => d.state = v,
            "rate" => d.rate = v,
            "size" => d.size = v,
            "time" => d.time = v,
            "cycles" => d.cycles = v.parse().ok().filter(|c| *c > 0),
            "threshold" => d.threshold = v,
            _ => {}
        }
    }
    d
}

thread_local! {
    static DETAILS: Live<Option<Details>> = Live::new(None, start_details);
}

pub fn details() -> Live<Option<Details>> {
    DETAILS.with(|l| l.clone())
}

fn start_details(set: Setter<Option<Details>>) -> Box<dyn Stop> {
    read_details(set.clone());
    let id = glib::timeout_add_local(std::time::Duration::from_secs(5), move || {
        read_details(set.clone());
        glib::ControlFlow::Continue
    });
    Box::new(crate::TimerStop(Some(id)))
}

fn read_details(set: Setter<Option<Details>>) {
    glib::MainContext::default().spawn_local(async move {
        set.set(crate::output(&["omarchy-battery-status", "--shell"]).await.map(|t| parse_details(&t)));
    });
}

// ---------------------------------------------------------------- Battery history

/// The battery's charge over the last day (UPower's history, which the summary
/// "DisplayDevice" doesn't keep), and the apps using the processor most now, the
/// likeliest drains: an estimate from their share of the processor, not a measurement.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct History {
    /// Oldest first. UPower records a point when the charge or state changes, so a
    /// flat stretch has none: each point holds until the next.
    pub points: Vec<Point>,
    /// The apps using the most processor time, most first.
    pub energy: Vec<Hog>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Point {
    /// Unix seconds.
    pub time: i64,
    /// 0–100
    pub percent: f64,
    pub charging: bool,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Hog {
    pub app: String,
    /// Percent of one processor core, averaged over the last minute or so.
    pub cpu: f64,
}

const DAY: u32 = 24 * 3600;

/// The first battery that powers the computer (UPower type 2, `PowerSupply`).
fn battery_path(bus: &gio::DBusConnection) -> Option<String> {
    let reply = bus
        .call_sync(
            Some("org.freedesktop.UPower"),
            "/org/freedesktop/UPower",
            "org.freedesktop.UPower",
            "EnumerateDevices",
            None,
            None,
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let list = reply.child_value(0);
    (0..list.n_children()).filter_map(|i| list.child_value(i).str().map(String::from)).find(|path| {
        props(bus, ("org.freedesktop.UPower", path, UPOWER.2)).is_some_and(|p| {
            p.lookup::<u32>("Type").ok().flatten() == Some(2) && p.lookup::<bool>("PowerSupply").ok().flatten().unwrap_or(false)
        })
    })
}

/// UPower's `(time, value, state)` records, newest first, to points oldest first.
/// Unknown states (0) are dropped; states 1 and 5 (pending charge) count as charging.
pub fn points_of(records: &[(u32, f64, u32)], since: i64) -> Vec<Point> {
    let mut out: Vec<Point> = records
        .iter()
        .filter(|(t, _, s)| *s != 0 && *t as i64 >= since)
        .map(|&(t, v, s)| Point { time: t as i64, percent: v.clamp(0.0, 100.0), charging: s == 1 || s == 5 })
        .collect();
    out.sort_by_key(|p| p.time);
    out.dedup_by_key(|p| p.time);
    out
}

fn read_points(bus: &gio::DBusConnection) -> Vec<Point> {
    let Some(path) = battery_path(bus) else { return vec![] };
    let Ok(reply) = bus.call_sync(
        Some("org.freedesktop.UPower"),
        &path,
        UPOWER.2,
        "GetHistory",
        Some(&("charge", DAY, 200u32).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        2000,
        gio::Cancellable::NONE,
    ) else {
        return vec![];
    };
    let list = reply.child_value(0);
    let records: Vec<(u32, f64, u32)> = (0..list.n_children()).filter_map(|i| list.child_value(i).get::<(u32, f64, u32)>()).collect();
    let now = glib::real_time() / 1_000_000;
    points_of(&records, now - DAY as i64)
}

/// Processor ticks used so far by each program (its executable's name), from /proc.
/// Kernel threads (no executable) are left out.
fn cpu_ticks() -> std::collections::HashMap<String, u64> {
    let mut out = std::collections::HashMap::new();
    let Ok(dir) = std::fs::read_dir("/proc") else { return out };
    for e in dir.flatten() {
        let name = e.file_name();
        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(exe) = std::fs::read_link(e.path().join("exe")) else { continue };
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else { continue };
        let Some(ticks) = stat_ticks(&stat) else { continue };
        let app = exe.file_name().map(|n| n.to_string_lossy().trim_end_matches(" (deleted)").to_string()).unwrap_or_default();
        if !app.is_empty() {
            *out.entry(app).or_insert(0) += ticks;
        }
    }
    out
}

/// utime + stime from a /proc/PID/stat line (fields 14 and 15, after the name in
/// parentheses, which may hold spaces).
pub fn stat_ticks(stat: &str) -> Option<u64> {
    let rest = &stat[stat.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    Some(f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?)
}

/// The busiest programs between two samples `secs` apart, smoothed into `avg`.
pub fn hogs(
    before: &std::collections::HashMap<String, u64>,
    after: &std::collections::HashMap<String, u64>,
    secs: f64,
    avg: &mut std::collections::HashMap<String, f64>,
) -> Vec<Hog> {
    // USER_HZ, 100 on every Linux the desktop runs on.
    let hz = 100.0;
    for (app, t) in after {
        let used = t.saturating_sub(*before.get(app).unwrap_or(t)) as f64 / (secs * hz) * 100.0;
        let a = avg.entry(app.clone()).or_insert(used);
        *a = *a * 0.6 + used * 0.4;
    }
    avg.retain(|app, a| after.contains_key(app) && *a >= 0.5);
    let mut list: Vec<Hog> = avg.iter().filter(|(_, a)| **a >= 5.0).map(|(app, a)| Hog { app: pretty(app), cpu: (*a * 10.0).round() / 10.0 }).collect();
    list.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    list.truncate(4);
    list
}

fn pretty(program: &str) -> String {
    let mut c = program.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

thread_local! {
    static HISTORY: Live<History> = Live::new(History::default(), start_history);
}

pub fn history() -> Live<History> {
    HISTORY.with(|l| l.clone())
}

fn start_history(set: Setter<History>) -> Box<dyn Stop> {
    use std::cell::RefCell;
    use std::rc::Rc;
    let bus = system_bus();
    let points = bus.as_ref().map(read_points).unwrap_or_default();
    set.set(History { points: points.clone(), energy: vec![] });
    // Processor use: a first sample now and every 10 s; the charge again every minute.
    let state = Rc::new(RefCell::new((cpu_ticks(), std::time::Instant::now(), std::collections::HashMap::new(), points, 0u32)));
    let id = glib::timeout_add_local(std::time::Duration::from_secs(10), move || {
        let mut s = state.borrow_mut();
        let now_ticks = cpu_ticks();
        let secs = s.1.elapsed().as_secs_f64().max(1.0);
        let (before, _, avg, points, n) = &mut *s;
        let energy = hogs(before, &now_ticks, secs, avg);
        *n += 1;
        if *n % 6 == 0
            && let Some(b) = bus.as_ref()
        {
            *points = read_points(b);
        }
        let points = points.clone();
        s.0 = now_ticks;
        s.1 = std::time::Instant::now();
        set.set(History { points, energy });
        glib::ControlFlow::Continue
    });
    Box::new(crate::TimerStop(Some(id)))
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn upower_records_to_points() {
        let p = points_of(&[(300, 80.0, 2), (200, 0.0, 0), (100, 95.0, 1), (50, 99.0, 4)], 60);
        assert_eq!(
            p,
            vec![Point { time: 100, percent: 95.0, charging: true }, Point { time: 300, percent: 80.0, charging: false }]
        );
    }

    #[test]
    fn processor_ticks_and_hogs() {
        assert_eq!(stat_ticks("1234 (Web Content) S 1 2 3 4 5 6 7 8 9 10 250 50 0 0"), Some(300));
        let before = HashMap::from([("firefox".to_string(), 1000u64), ("bash".to_string(), 10)]);
        let after = HashMap::from([("firefox".to_string(), 1500u64), ("bash".to_string(), 10)]);
        let mut avg = HashMap::new();
        let h = hogs(&before, &after, 10.0, &mut avg);
        assert_eq!(h, vec![Hog { app: "Firefox".into(), cpu: 50.0 }]);
    }
}

#[cfg(test)]
mod detail_tests {
    #[test]
    fn reads_details() {
        let d = super::parse_details(
            "percentage\t100%\nstate\tfully-charged\nrate\t0W\nsize\t87Wh\ntime\t\ncycles\t0\nthreshold\t75-80%\n",
        );
        assert_eq!((d.size.as_str(), d.threshold.as_str(), d.cycles), ("87Wh", "75-80%", None));
    }
}
