//! Backup's documented grant-checked Core Control contract. Polls only while visible.
use crate::{TimerStop, session_bus};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};
use std::time::Duration;
const BUS: &str = "io.github.design_nexus.nyx.Backup1";
const PATH: &str = "/io/github/design_nexus/nyx/Backup1";
const IFACE: &str = "io.github.design_nexus.nyx.Control";
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct State {
    pub grant: Option<String>,
    pub name: String,
    pub health: String,
    pub last_success: Option<i64>,
    pub schedule: Option<String>,
    pub next_schedule: Option<i64>,
    pub active: Option<String>,
    pub fraction: Option<f64>,
    pub can_backup: bool,
    pub can_cancel: bool,
    pub problem: Option<String>,
}
thread_local! {static LIVE:Live<State>=Live::new(State::default(),start);}
pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}
async fn request(method: &str, args: Option<&glib::Variant>) -> Result<serde_json::Value, String> {
    let bus = session_bus().ok_or("No session bus")?;
    let v = bus
        .call_future(
            Some(BUS),
            PATH,
            IFACE,
            method,
            args,
            Some(glib::VariantTy::new("(s)").unwrap()),
            gio::DBusCallFlags::NO_AUTO_START,
            3000,
        )
        .await
        .map_err(|_| "Backup is unavailable; open Backup to start its service".to_string())?;
    serde_json::from_str(v.child_value(0).str().ok_or("Invalid Backup reply")?).map_err(|_| "Invalid Backup reply".into())
}
fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(set.clone());
    Box::new(TimerStop(Some(glib::timeout_add_local(Duration::from_secs(5), move || {
        refresh(set.clone());
        glib::ControlFlow::Continue
    }))))
}
fn refresh(set: Setter<State>) {
    glib::MainContext::default().spawn_local(async move {
        let result = async {
            let d = request("Describe", None).await?;
            let g = d
                .as_array()
                .and_then(|a| a.iter().find(|t| t["name"] == "backup.status"))
                .and_then(|t| t["inputSchema"]["properties"]["grant"]["enum"].as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .ok_or("Allow Control Center access in Backup Settings")?;
            let v = request("Call", Some(&("backup.status", serde_json::json!({"grant":g}).to_string()).to_variant())).await?;
            parse(g, &v)
        }
        .await;
        set.set(match result {
            Ok(s) => s,
            Err(e) => State { problem: Some(e), ..Default::default() },
        });
    });
}
pub fn parse(grant: &str, value: &serde_json::Value) -> Result<State, String> {
    if let Some(e) = value["error"].as_str() {
        return Err(e.into());
    }
    let v = &value["ok"];
    if v["version"] != 1 {
        return Err("Unsupported Backup status".into());
    }
    let actions = v["available_actions"].as_array();
    let allowed = |name: &str| actions.is_some_and(|a| a.iter().any(|v| v == name));
    Ok(State {
        grant: Some(grant.into()),
        name: v["name"].as_str().unwrap_or("Backup").into(),
        health: v["health"].as_str().unwrap_or("unknown").into(),
        last_success: v["last_success"].as_i64(),
        schedule: v["schedule"].as_str().map(Into::into),
        next_schedule: v["next_schedule_us"].as_u64().filter(|v| *v > 0).map(|v| (v / 1_000_000) as i64),
        active: v["active"]["id"].as_str().map(Into::into),
        fraction: v["active"]["progress"].as_f64().filter(|x| x.is_finite()).map(|x| x.clamp(0., 1.)),
        can_backup: allowed("backup"),
        can_cancel: allowed("cancel"),
        problem: None,
    })
}
pub async fn action(grant: &str, name: &str, job: Option<&str>) -> Result<(), String> {
    let mut a = serde_json::json!({"grant":grant});
    if let Some(job) = job {
        a["job"] = serde_json::Value::String(job.into())
    }
    let v = request("Call", Some(&(name, a.to_string()).to_variant())).await?;
    if let Some(e) = v["error"].as_str() {
        return Err(e.into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn status_is_versioned_and_grant_scoped() {
        let s=super::parse("g",&serde_json::json!({"ok":{"version":1,"name":"Profile","health":"working","active":{"id":"j","progress":1.5},"available_actions":["status","cancel"]}})).unwrap();
        assert_eq!(s.fraction, Some(1.));
        assert!(s.can_cancel && !s.can_backup);
        assert!(super::parse("g", &serde_json::json!({"error":"revoked"})).is_err());
    }
}
