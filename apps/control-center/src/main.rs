//! Nyx Control Center: quick settings for Omarchy and Hyprland.
//!
//! `nyx-control-center` starts it (hidden; normally as a user service).
//! `nyx-control-center --toggle | --show | --hide` opens or closes the running one;
//! `--toggle wifi | bluetooth | vpn` opens it on that list.
//! `--only <output>` keeps it on one output (for trying it on a test output).

mod backup_tile;
mod details;
mod home_tile;
mod panel;
mod plugin_tiles;
mod route_tiles;
mod theme;
mod tiles;
mod ui;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

pub const APP_ID: &str = "io.github.design_nexus.nyx.ControlCenter";

/// The Quickshell interface's pieces, for `--ui`.
const COMPONENT: nyx_ui_link::switch::Component = nyx_ui_link::switch::Component {
    app: "nyx-control-center",
    gtk_unit: "nyx-control-center.service",
    backend_units: &["nyx-control-center-backend.service"],
};

/// `--ui qml|gtk`, and the command line's requests when the Quickshell interface runs.
fn quickshell(args: &[String]) -> Option<std::process::ExitCode> {
    if let Some(i) = args.iter().position(|a| a == "--ui") {
        let Some(ui) = args.get(i + 1).and_then(|a| nyx_ui_link::switch::Ui::parse(a)) else {
            let now = nyx_ui_link::switch::current(&COMPONENT);
            println!("{}", if now == nyx_ui_link::switch::Ui::Qml { "qml" } else { "gtk" });
            return Some(std::process::ExitCode::SUCCESS);
        };
        return Some(match nyx_ui_link::switch::set(&COMPONENT, ui) {
            Ok(msg) => {
                println!("{msg}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("nyx-control-center: {e}");
                std::process::ExitCode::FAILURE
            }
        });
    }
    if args.iter().any(|a| a == "--only") {
        return None;
    }
    let (op, body) = if let Some(i) = args.iter().position(|a| a == "--toggle") {
        ("Toggle", serde_json::json!({"detail": args.get(i + 1).filter(|a| !a.starts_with("--"))}))
    } else if args.iter().any(|a| a == "--show") {
        ("Show", serde_json::json!({}))
    } else if args.iter().any(|a| a == "--hide") {
        ("Hide", serde_json::json!({}))
    } else {
        return None;
    };
    match nyx_ui_link::switch::forward("nyx-control-center", op, body)? {
        Ok(_) => Some(std::process::ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("nyx-control-center: {e}");
            Some(std::process::ExitCode::FAILURE)
        }
    }
}

fn main() -> std::process::ExitCode {
    let cli: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = quickshell(&cli) {
        return code;
    }
    // A test copy (`--only <output>`) is separate: its own id, never the running panel.
    let test = std::env::args().any(|a| a == "--only");
    let (id, mut flags) = if test {
        (format!("{APP_ID}.Test"), gio::ApplicationFlags::NON_UNIQUE)
    } else {
        (APP_ID.to_string(), gio::ApplicationFlags::empty())
    };
    flags |= gio::ApplicationFlags::HANDLES_COMMAND_LINE;
    let app = gtk::Application::builder().application_id(id).flags(flags).build();
    let panel: Rc<RefCell<Option<Rc<panel::Panel>>>> = Rc::default();
    app.connect_startup(|app| {
        theme::install();
        // Resident: closing the panel doesn't quit.
        std::mem::forget(app.hold());
    });
    let p = panel.clone();
    app.connect_command_line(move |app, cmd| {
        let args: Vec<String> = cmd.arguments().iter().skip(1).map(|a| a.to_string_lossy().to_string()).collect();
        if args.iter().any(|a| a == "-h" || a == "--help") {
            println!(
                "Usage: nyx-control-center [--toggle [PAGE] | --show | --hide] [--only <output>]\n       nyx-control-center --ui [qml|gtk]\n\n\
                 With no option, start (hidden). The options open or close the running one;\n\
                 --toggle with a page's name opens it on that page: wifi, bluetooth, vpn,\n\
                 tailscale, devices, display, battery, or sound (the Quickshell interface).\n\
                 --ui switches between the GTK interface and the Quickshell one (or says\n\
                 which is on)."
            );
            return glib::ExitCode::SUCCESS;
        }
        let only = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1)).cloned();
        let first = p.borrow().is_none();
        if first {
            *p.borrow_mut() = Some(panel::build(app, only));
        }
        let panel = p.borrow().clone().expect("built");
        if let Some(i) = args.iter().position(|a| a == "--toggle") {
            match args.get(i + 1).and_then(|a| tiles::Detail::parse(a)) {
                Some(which) => panel.toggle_detail(which),
                None => panel.toggle(),
            }
        } else if args.iter().any(|a| a == "--show") {
            panel.show();
        } else if args.iter().any(|a| a == "--hide") {
            panel.hide();
        }
        glib::ExitCode::SUCCESS
    });
    let code = app.run();
    std::process::ExitCode::from(code.get())
}
