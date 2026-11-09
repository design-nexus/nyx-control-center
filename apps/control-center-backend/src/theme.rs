//! Keeping the theme snapshot current for Quickshell parts where no theme service runs
//! (Omarchy): what the GTK panel did through `nyx_theme::use_shell_scope()`, with
//! nyx-theme-core and file monitors instead of GTK.

use gio::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

/// Publish the system theme now and whenever anything it depends on changes. Plugins
/// hear about a new generation through `on_change`.
pub fn follow(on_change: impl Fn() + 'static) {
    nyx_theme_core::notify_changed("system", "shell");
    let pending = Rc::new(Cell::new(false));
    let on_change = Rc::new(on_change);
    let mut monitors = Vec::new();
    for dir in nyx_theme_core::paths::watched_dirs() {
        let Ok(m) = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        else {
            continue;
        };
        let (pending, on_change) = (pending.clone(), on_change.clone());
        m.connect_changed(move |_, file, _, _| {
            if file.basename().is_some_and(|b| b.to_string_lossy().ends_with(".tmp")) || pending.replace(true) {
                return;
            }
            let (pending, on_change) = (pending.clone(), on_change.clone());
            glib::timeout_add_local_once(Duration::from_millis(120), move || {
                pending.set(false);
                let before = nyx_theme_core::snapshot::generation();
                if nyx_theme_core::notify_changed("system", "shell") != before {
                    on_change();
                }
            });
        });
        monitors.push(m);
    }
    std::mem::forget(monitors);
}

/// What plugins are told about the theme.
pub fn info() -> nyx_plugin_protocol::ThemeInfo {
    let s = nyx_theme_core::snapshot::read();
    nyx_plugin_protocol::ThemeInfo {
        light: s.as_ref().is_some_and(|s| s.legacy.get("light").is_some_and(|v| v == "true")),
        name: s.as_ref().map(|s| s.theme.id.clone()).unwrap_or_default(),
        generation: nyx_theme_core::snapshot::generation(),
    }
}
