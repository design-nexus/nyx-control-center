//! `nyx-control-center-backend`: the Control Center without GTK, for its Quickshell
//! interface (`qml/ControlCenter` in this repository).
//!
//! It owns `io.github.design_nexus.nyx.ControlCenter` and serves `Panel1` and
//! `ControlCenterPlugins1` there as the GTK panel did, so keybinds, bar widgets, Shell
//! Core and the other panels see no difference. The interface talks to it over
//! `$XDG_RUNTIME_DIR/nyx-control-center/ui.sock` (nyx-ui-link):
//!
//! - topic `panel`: `{open, detail, serial}`, what the panel should show;
//! - topic `layout`: the pages, the native kinds, `remember_page`;
//! - topic `env`: what this computer has (Omarchy, programs, the user's name);
//! - one topic per provider (`audio`, `network`, `bluetooth`, …), running only while
//!   the interface subscribes to it, as GTK tiles subscribed only while mapped;
//! - topic `plugins`: plugin tiles (see `plugins.rs`);
//! - operations: `Show`, `Hide`, `Toggle`, the providers' actions by name
//!   (`audio.set_volume`, …), layout edits, launches.

mod ops;
mod plugins;
mod theme;
mod topics;

use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

pub const APP_ID: &str = "io.github.design_nexus.nyx.ControlCenter";

mod layout;

/// What the panel shows, decided here so `Panel1`, the command line and the interface
/// agree.
#[derive(Default)]
pub struct PanelState {
    pub open: bool,
    pub detail: Option<String>,
    pub serial: u64,
    /// The opening the interface last said is on screen (`Shown`).
    pub shown: u64,
}

pub struct App {
    pub link: nyx_ui_link::Link,
    pub panel: RefCell<PanelState>,
    pub panel1: RefCell<Option<nyx_desktop::panel::Panel>>,
    pub config: RefCell<layout::Config>,
    pub topics: topics::Topics,
    pub test: bool,
}

impl App {
    pub fn publish_panel(&self) {
        let p = self.panel.borrow();
        self.link.publish("panel", json!({"open": p.open, "detail": p.detail, "serial": p.serial}));
        nyx_providers::brightness::set_polling(p.open);
        if p.open {
            nyx_providers::brightness::refresh(nyx_providers::brightness::live().setter());
        }
    }

    pub fn show(&self, detail: Option<String>) {
        {
            let mut p = self.panel.borrow_mut();
            p.open = true;
            p.detail = detail;
            p.serial += 1;
        }
        self.publish_panel();
        if let Some(p) = self.panel1.borrow().as_ref() {
            p.announce();
        }
    }

    pub fn hide(&self) {
        let was = std::mem::replace(&mut self.panel.borrow_mut().open, false);
        if was {
            self.panel.borrow_mut().detail = None;
            self.publish_panel();
        }
    }

    pub fn toggle(&self, detail: Option<String>) {
        let (open, current) = {
            let p = self.panel.borrow();
            (p.open, p.detail.clone())
        };
        // Open but not on screen yet (the interface is still drawing it, or never
        // could): a press shows it again rather than closing what nobody saw.
        let seen = {
            let p = self.panel.borrow();
            p.shown == p.serial
        };
        if open && seen && (detail.is_none() || current == detail) {
            self.hide();
        } else {
            self.show(detail);
        }
    }

    /// The interface has opening `serial` on screen.
    pub fn shown(&self, serial: u64) {
        let mut p = self.panel.borrow_mut();
        if p.open && p.serial == serial {
            p.shown = serial;
        }
    }

    pub fn publish_layout(&self) {
        let c = self.config.borrow();
        self.link.publish("layout", json!({"pages": c.pages, "remember_page": c.remember_page, "kinds": layout::kinds_json()}));
        plugins::set_layout(&c.pages, &c.plugins);
    }
}

/// Start a program the way Omarchy's launcher does, detached from the backend.
pub fn launch(args: &[&str]) {
    let mut cmd = std::process::Command::new("setsid");
    cmd.arg("-f");
    if nyx_providers::present("uwsm-app") {
        cmd.args(["uwsm-app", "--"]);
    }
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    match cmd.spawn() {
        Ok(mut c) => {
            std::thread::spawn(move || c.wait());
        }
        Err(e) => eprintln!("nyx-control-center: couldn't start {}: {e}", args.join(" ")),
    }
}

fn env_json() -> Value {
    let has = |p: &str| nyx_providers::present(p);
    let programs = [
        "nordvpn",
        "tormarchy",
        "tailscale",
        "nyx-settings",
        "nyx-capture",
        "nyx-backup",
        "omarchy-menu",
        "brightnessctl",
        "wl-copy",
        "omarchy-tailscale-send",
        "uwsm",
    ];
    let mut have = serde_json::Map::new();
    for p in programs {
        have.insert(p.into(), Value::Bool(has(p)));
    }
    let name = glib::real_name().to_string_lossy().trim().to_string();
    let name = if name.is_empty() || name == "Unknown" { glib::user_name().to_string_lossy().to_string() } else { name };
    json!({"omarchy": nyx_providers::omarchy::here(), "name": name, "have": have})
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!(
            "Usage: nyx-control-center-backend\n\nThe Control Center's services for its Quickshell interface (started by nyx-control-center.service in QML mode)."
        );
        return std::process::ExitCode::SUCCESS;
    }
    // A test copy (its own socket from NYX_CONTROL_CENTER_UI_SOCKET) doesn't take the
    // bus name, so it never meets the running panel.
    let test = std::env::var_os("NYX_CONTROL_CENTER_UI_SOCKET").is_some();
    let ctx = glib::MainContext::default();
    let _guard = ctx.acquire().expect("main context");

    let (req_tx, req_rx) = async_channel::unbounded::<nyx_ui_link::Request>();
    let (watch_tx, watch_rx) = async_channel::unbounded::<(String, usize)>();
    let mut options = nyx_ui_link::Options::new("nyx-control-center");
    options.on_watch = Some(std::sync::Arc::new(move |t: &str, n| {
        let _ = watch_tx.send_blocking((t.to_string(), n));
    }));
    let link = match nyx_ui_link::serve(options, move |r| {
        let _ = req_tx.send_blocking(r);
    }) {
        Ok(l) => l,
        Err(nyx_ui_link::Error::AlreadyRunning(p)) => {
            eprintln!("nyx-control-center-backend: already running at {}", p.display());
            return std::process::ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("nyx-control-center-backend: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let app = Rc::new(App {
        link: link.clone(),
        panel: RefCell::default(),
        panel1: RefCell::default(),
        config: RefCell::new(layout::load()),
        topics: topics::Topics::default(),
        test,
    });
    app.publish_panel();
    app.link.publish("env", env_json());

    let l = link.clone();
    plugins::start(test, Rc::new(move |v| l.publish("plugins", v)));
    app.publish_layout();
    theme::follow(plugins::theme_changed);

    // The layout changed in any layer: by hand, by Nyx Settings or `nyx plugin`, or
    // NyxOS's defaults in an update.
    let a = Rc::downgrade(&app);
    let (tx, rx) = async_channel::unbounded::<()>();
    if let Some(w) = nyx_widgets::settings::watch(move || {
        let _ = tx.send_blocking(());
    }) {
        std::mem::forget(w);
    }
    glib::spawn_future_local(async move {
        while rx.recv().await.is_ok() {
            glib::timeout_future(std::time::Duration::from_millis(150)).await;
            while rx.try_recv().is_ok() {}
            let Some(app) = a.upgrade() else { return };
            let fresh = layout::load();
            if *app.config.borrow() != fresh {
                *app.config.borrow_mut() = fresh;
                app.publish_layout();
            }
        }
    });

    if !test {
        let a = Rc::downgrade(&app);
        match gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
            Ok(conn) => {
                plugins::serve(&conn);
                let (s, h, o) = (a.clone(), a.clone(), a.clone());
                let handlers = nyx_desktop::panel::Handlers {
                    show: Box::new(move || {
                        if let Some(app) = s.upgrade() {
                            // Panel1 announces itself after `show`.
                            let mut p = app.panel.borrow_mut();
                            p.open = true;
                            p.detail = None;
                            p.serial += 1;
                            drop(p);
                            app.publish_panel();
                        }
                    }),
                    hide: Box::new(move || {
                        if let Some(app) = h.upgrade() {
                            app.hide();
                        }
                    }),
                    is_open: Box::new(move || o.upgrade().is_some_and(|app| app.panel.borrow().open)),
                };
                match nyx_desktop::panel::Panel::serve(&conn, "control-center", handlers) {
                    Ok(p) => *app.panel1.borrow_mut() = Some(p),
                    Err(e) => eprintln!("nyx-control-center-backend: Panel1: {e}"),
                }
                gio::bus_own_name_on_connection(
                    &conn,
                    APP_ID,
                    gio::BusNameOwnerFlags::REPLACE | gio::BusNameOwnerFlags::ALLOW_REPLACEMENT,
                    |_, _| {},
                    |_, name| eprintln!("nyx-control-center-backend: {name} is taken (is the GTK panel running?)"),
                );
                std::mem::forget(conn);
            }
            Err(e) => eprintln!("nyx-control-center-backend: no session bus: {e}"),
        }
    }

    let a = app.clone();
    ctx.spawn_local(async move {
        while let Ok((topic, n)) = watch_rx.recv().await {
            topics::watched(&a, &topic, n);
        }
    });
    let a = app.clone();
    ctx.spawn_local(async move {
        while let Ok(req) = req_rx.recv().await {
            ops::handle(&a, req);
        }
    });

    let main = glib::MainLoop::new(Some(&ctx), false);
    main.run();
    std::process::ExitCode::SUCCESS
}
