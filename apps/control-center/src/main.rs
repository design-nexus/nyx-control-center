//! Nyx Control Center: quick settings for Omarchy and Hyprland.
//!
//! `nyx-control-center` starts it (hidden; normally as a user service).
//! `nyx-control-center --toggle | --show | --hide` opens or closes the running one.
//! `--only <output>` keeps it on one output (for trying it on a test output).

mod details;
mod panel;
mod theme;
mod tiles;
mod ui;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::rc::Rc;

pub const APP_ID: &str = "io.github.design_nexus.nyx.ControlCenter";

fn main() -> std::process::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE).build();
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
                "Usage: nyx-control-center [--toggle | --show | --hide] [--only <output>]\n\n\
                 With no option, start (hidden). The options open or close the running one."
            );
            return glib::ExitCode::SUCCESS;
        }
        let only = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1)).cloned();
        let first = p.borrow().is_none();
        if first {
            *p.borrow_mut() = Some(panel::build(app, only));
        }
        let panel = p.borrow().clone().expect("built");
        if args.iter().any(|a| a == "--toggle") {
            panel.toggle();
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
