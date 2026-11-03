//! Headless contract/subscription proof against a separately isolated Backup fixture.
use gio::prelude::*;
use std::time::{Duration, Instant};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() == 3, "Pass fixture-only backupctl path and profile ID");
    let name = "io.github.design_nexus.nyx.BackupProviderFixture";
    let app = gio::Application::new(Some(name), gio::ApplicationFlags::FLAGS_NONE);
    app.register(gio::Cancellable::NONE).unwrap();
    let grant = std::process::Command::new(&args[1]).args(["integrations", "grant", name, &args[2], "--json"]).output().unwrap();
    assert!(grant.status.success(), "{}", String::from_utf8_lossy(&grant.stdout));
    let grant: serde_json::Value = serde_json::from_slice(&grant.stdout).unwrap();
    let live = nyx_providers::backup::live();
    let subscription = live.subscribe(|_| {});
    let context = glib::MainContext::default();
    let start = Instant::now();
    while live.get().grant.is_none() && start.elapsed() < Duration::from_secs(8) {
        context.iteration(false);
        std::thread::sleep(Duration::from_millis(10));
    }
    let state = live.get();
    assert!(state.problem.is_none(), "{:?}", state.problem);
    assert!(state.grant.is_some() && !state.can_backup);
    assert!(live.running());
    drop(subscription);
    assert!(!live.running(), "A hidden tile must not retain a polling timer");
    let revoke =
        std::process::Command::new(&args[1]).args(["integrations", "revoke", grant["id"].as_str().unwrap()]).output().unwrap();
    assert!(revoke.status.success());
    println!("{}", serde_json::json!({"provider_status":true,"read_only_grant":true,"unsubscribed_provider_stopped":true}));
}
