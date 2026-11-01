//! The panel: a full-height layer-shell surface on the right edge of the focused screen,
//! under the bar, with a transparent catcher behind it that closes it on a click
//! anywhere else. It stays in memory while hidden (so it opens at once) but its tiles
//! unsubscribe, so nothing runs.

use crate::details;
use crate::tiles::{self, Ctx, Detail, Mode};
use crate::ui::{self, bind, icon_button, label};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use nyx_providers::power;
use nyx_widgets::layout::{self, Page};
use nyx_widgets::live::{Live, Setter, Stop};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

pub const WIDTH: i32 = 400;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub pages: Vec<Page>,
    /// Open on the page shown last (else always the first).
    #[serde(default = "yes")]
    pub remember_page: bool,
}

fn yes() -> bool {
    true
}

pub fn config_path() -> PathBuf {
    glib::user_config_dir().join("nyx/control-center.toml")
}

pub fn load_config() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path).map(|t| toml::from_str::<Config>(&t)) {
        Ok(Ok(c)) if !c.pages.is_empty() => c,
        Ok(Err(e)) => {
            eprintln!("nyx-control-center: {}: {e}; using the default layout", path.display());
            Config { pages: tiles::default_pages(), remember_page: true }
        }
        _ => {
            let c = Config { pages: tiles::default_pages(), remember_page: true };
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, toml::to_string_pretty(&c).unwrap_or_default());
            c
        }
    }
}

pub struct Panel {
    window: gtk::Window,
    catcher: gtk::Window,
    content: gtk::Stack,
    pages: gtk::Stack,
    dots: gtk::Box,
    page: Cell<usize>,
    only: Option<String>,
    remember: bool,
    panel1: RefCell<Option<nyx_desktop::panel::Panel>>,
}

impl Panel {
    pub fn is_open(&self) -> bool {
        self.window.is_visible()
    }

    pub fn show(&self) {
        if self.is_open() {
            return;
        }
        if let Some(m) = self.target_monitor() {
            self.window.set_monitor(Some(&m));
            self.catcher.set_monitor(Some(&m));
        }
        if !self.remember {
            self.go_to(0);
        }
        self.content.set_visible_child_name("dashboard");
        self.catcher.present();
        self.window.present();
        if let Some(p) = self.panel1.borrow().as_ref() {
            p.announce();
        }
        // For screenshots: open a detail view straight away.
        if let Some(which) = std::env::var("NYX_CC_TEST_DETAIL").ok().and_then(|d| match d.as_str() {
            "wifi" => Some(Detail::Wifi),
            "bluetooth" => Some(Detail::Bluetooth),
            "vpn" => Some(Detail::Vpn),
            _ => None,
        }) {
            self.open_detail(which, Rc::new(|| {}));
        }
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
        self.catcher.set_visible(false);
        // Leave detail views behind; the next open starts on the dashboard.
        if let Some(d) = self.content.child_by_name("detail") {
            self.content.remove(&d);
        }
        self.content.set_visible_child_name("dashboard");
    }

    pub fn toggle(&self) {
        if self.is_open() { self.hide() } else { self.show() }
    }

    /// The focused screen (or `--only`'s).
    fn target_monitor(&self) -> Option<gdk::Monitor> {
        let name = match &self.only {
            Some(o) => o.clone(),
            None => nyx_desktop::hypr::Instance::find()
                .ok()
                .and_then(|i| i.json::<Vec<serde_json::Value>>("monitors").ok())
                .and_then(|ms| ms.into_iter().find(|m| m["focused"].as_bool() == Some(true)))
                .and_then(|m| m["name"].as_str().map(str::to_string))?,
        };
        let display = gdk::Display::default()?;
        display.monitors().iter::<gdk::Monitor>().flatten().find(|m| m.connector().as_deref() == Some(name.as_str()))
    }

    fn go_to(&self, i: usize) {
        let n = self.page_count();
        if n == 0 {
            return;
        }
        let i = i.min(n - 1);
        self.page.set(i);
        self.pages.set_visible_child_name(&format!("page-{i}"));
        let mut c = self.dots.first_child();
        let mut k = 0;
        while let Some(w) = c {
            ui::set_class(&w, "on", k == i);
            c = w.next_sibling();
            k += 1;
        }
    }

    fn page_count(&self) -> usize {
        let mut n = 0;
        let mut c = self.pages.first_child();
        while let Some(w) = c {
            n += 1;
            c = w.next_sibling();
        }
        n
    }

    fn open_detail(&self, which: Detail, back: Rc<dyn Fn()>) {
        if let Some(d) = self.content.child_by_name("detail") {
            self.content.remove(&d);
        }
        let view = details::view(which, back);
        self.content.add_named(&view, Some("detail"));
        self.content.set_visible_child_name("detail");
    }

    fn back(&self) {
        self.content.set_visible_child_name("dashboard");
        let content = self.content.clone();
        // Drop the detail view once the slide is over (its providers stop with it).
        glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            if content.visible_child_name().as_deref() == Some("dashboard")
                && let Some(d) = content.child_by_name("detail")
            {
                content.remove(&d);
            }
        });
    }
}

/// The active workspace and its Omarchy mode, kept current while a tile shows it.
fn mode_live() -> Live<Mode> {
    // Held only to keep the watcher and the file monitor running.
    #[allow(dead_code)]
    struct Running(nyx_desktop::watch::Watcher, Option<gio::FileMonitor>);
    impl Stop for Running {}
    Live::new(Mode::default(), |set: Setter<Mode>| {
        let s = set.clone();
        let watcher = nyx_desktop::watch::watch_local(move |u| {
            if !u.changes.active && !u.changes.workspaces && !u.changes.monitors {
                return;
            }
            let Some(m) = u.desktop.focused_monitor() else { return };
            let layout = u.desktop.workspaces.iter().find(|w| w.id == m.active_workspace).map(|w| w.tiled_layout.clone());
            s.set(Mode {
                workspace: m.active_workspace,
                mode: Some(nyx_desktop::omarchy::workspace_mode(m.active_workspace, layout.as_deref().unwrap_or(""))),
            });
        });
        // Omarchy writes the mode files when the mode changes.
        let dir = nyx_desktop::omarchy::layouts_dir();
        let _ = std::fs::create_dir_all(&dir);
        let monitor = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE).ok();
        if let Some(fm) = &monitor {
            let s = set.clone();
            fm.connect_changed(move |_, _, _, _| {
                if let Some(cur) = s.get() {
                    s.set(Mode { mode: Some(nyx_desktop::omarchy::workspace_mode(cur.workspace, "")), ..cur });
                }
            });
        }
        Box::new(Running(watcher, monitor))
    })
}

pub fn build(app: &gtk::Application, only: Option<String>) -> Rc<Panel> {
    let config = load_config();

    let window = gtk::Window::new();
    window.set_application(Some(app));
    window.add_css_class("nyx-control-center");
    window.add_css_class("cc-surface");
    window.init_layer_shell();
    window.set_namespace(Some("nyx-control-center"));
    window.set_layer(Layer::Overlay);
    // Keys (Esc, typing a Wi-Fi password) when clicked. NYX_CC_NO_KEYBOARD keeps it out
    // of keyboard focus altogether, for trying it on a test output while someone types.
    window.set_keyboard_mode(if std::env::var_os("NYX_CC_NO_KEYBOARD").is_some() {
        KeyboardMode::None
    } else {
        KeyboardMode::OnDemand
    });
    for e in [Edge::Top, Edge::Bottom, Edge::Right] {
        window.set_anchor(e, true);
    }
    window.set_exclusive_zone(0);
    window.set_default_size(WIDTH, -1);

    let catcher = gtk::Window::new();
    catcher.set_application(Some(app));
    catcher.add_css_class("cc-catcher");
    catcher.init_layer_shell();
    catcher.set_namespace(Some("nyx-control-center-catcher"));
    catcher.set_layer(Layer::Top);
    catcher.set_keyboard_mode(KeyboardMode::None);
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        catcher.set_anchor(e, true);
    }
    catcher.set_exclusive_zone(0);
    catcher.set_child(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-panel");
    root.set_size_request(WIDTH, -1);
    window.set_child(Some(&root));

    let content = gtk::Stack::new();
    content.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    content.set_transition_duration(200);
    content.set_vexpand(true);
    let pages = gtk::Stack::new();
    pages.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    pages.set_transition_duration(200);
    pages.set_vhomogeneous(false);
    let dots = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    dots.add_css_class("cc-dots");
    dots.set_halign(gtk::Align::Center);

    let panel = Rc::new(Panel {
        window: window.clone(),
        catcher: catcher.clone(),
        content: content.clone(),
        pages: pages.clone(),
        dots: dots.clone(),
        page: Cell::new(0),
        only,
        remember: config.remember_page,
        panel1: RefCell::default(),
    });

    let weak = Rc::downgrade(&panel);
    let ctx = Ctx {
        open: {
            let weak = weak.clone();
            Rc::new(move |which| {
                let Some(p) = weak.upgrade() else { return };
                let w2 = weak.clone();
                p.open_detail(
                    which,
                    Rc::new(move || {
                        if let Some(p) = w2.upgrade() {
                            p.back();
                        }
                    }),
                );
            })
        },
        hide: {
            let weak = weak.clone();
            Rc::new(move || {
                if let Some(p) = weak.upgrade() {
                    p.hide();
                }
            })
        },
        mode: mode_live(),
    };

    // Header: who's here, battery, and the panel's own actions.
    root.append(&header(&ctx));

    // Dashboard: pages of tiles, swiped or picked by their dots.
    let dash = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dash.add_css_class("cc-dashboard");
    for (i, page) in config.pages.iter().enumerate() {
        pages.add_named(&page_grid(&ctx, page), Some(&format!("page-{i}")));
        let dot = gtk::Button::new();
        dot.add_css_class("cc-dot");
        dot.set_tooltip_text(Some(&format!("Page {}", i + 1)));
        let w = weak.clone();
        dot.connect_clicked(move |_| {
            if let Some(p) = w.upgrade() {
                p.go_to(i);
            }
        });
        dots.append(&dot);
    }
    dots.set_visible(config.pages.len() > 1);
    dash.append(&pages);
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    dash.append(&spacer);
    dash.append(&dots);
    content.add_named(&dash, Some("dashboard"));
    root.append(&content);
    panel.go_to(0);

    // Swipe or scroll sideways to change page.
    let swipe = gtk::GestureSwipe::new();
    let w = weak.clone();
    swipe.connect_swipe(move |_, vx, vy| {
        if vx.abs() > 400.0
            && vx.abs() > vy.abs() * 1.5
            && let Some(p) = w.upgrade()
        {
            let cur = p.page.get();
            p.go_to(if vx < 0.0 { cur + 1 } else { cur.saturating_sub(1) });
        }
    });
    pages.add_controller(swipe);
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::HORIZONTAL);
    let acc = Rc::new(Cell::new(0.0f64));
    let w = weak.clone();
    scroll.connect_scroll(move |_, dx, _| {
        acc.set(acc.get() + dx);
        if acc.get().abs() > 40.0 {
            if let Some(p) = w.upgrade() {
                let cur = p.page.get();
                p.go_to(if acc.get() > 0.0 { cur + 1 } else { cur.saturating_sub(1) });
            }
            acc.set(0.0);
        }
        glib::Propagation::Stop
    });
    scroll.connect_scroll_end({
        let acc = Rc::new(Cell::new(0.0f64));
        move |_| acc.set(0.0)
    });
    pages.add_controller(scroll);

    // Esc: back from a detail view, else close.
    let keys = gtk::EventControllerKey::new();
    let w = weak.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(p) = w.upgrade() {
            if p.content.visible_child_name().as_deref() == Some("detail") {
                p.back();
            } else {
                p.hide();
            }
        }
        glib::Propagation::Stop
    });
    window.add_controller(keys);

    // A click outside closes it.
    let click = gtk::GestureClick::new();
    let w = weak.clone();
    click.connect_pressed(move |_, _, _, _| {
        if let Some(p) = w.upgrade() {
            p.hide();
        }
    });
    catcher.add_controller(click);

    // Never with the Notification Center: opening one closes the other.
    if let Some(conn) = app.dbus_connection() {
        let (w1, w2, w3) = (weak.clone(), weak.clone(), weak.clone());
        match nyx_desktop::panel::Panel::serve(
            &conn,
            "control-center",
            nyx_desktop::panel::Handlers {
                show: Box::new(move || {
                    if let Some(p) = w1.upgrade() {
                        p.show();
                    }
                }),
                hide: Box::new(move || {
                    if let Some(p) = w2.upgrade() {
                        p.hide();
                    }
                }),
                is_open: Box::new(move || w3.upgrade().is_some_and(|p| p.is_open())),
            },
        ) {
            Ok(p) => *panel.panel1.borrow_mut() = Some(p),
            Err(e) => eprintln!("nyx-control-center: panel coordination: {e}"),
        }
    }

    window.set_visible(false);
    catcher.set_visible(false);
    panel
}

fn header(ctx: &Ctx) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("cc-header");
    let real = glib::real_name().to_string_lossy().to_string();
    let user = glib::user_name().to_string_lossy().to_string();
    let name = if real.is_empty() || real == "Unknown" { user } else { real };
    let avatar = gtk::Label::new(Some(&name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()));
    avatar.add_css_class("cc-avatar");
    avatar.set_valign(gtk::Align::Center);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&label(&name, "cc-name"));
    let sub = label("", "cc-sub");
    text.append(&sub);
    bind(&sub, power::live(), {
        let sub = sub.clone();
        move |st| {
            let line = match &st.battery {
                Some(b) if b.full => "Battery full".to_string(),
                Some(b) => match b.time {
                    Some(t) if b.charging => format!("Battery {}% · {} to full", b.percent.round(), power::duration(t)),
                    Some(t) => format!("Battery {}% · {} left", b.percent.round(), power::duration(t)),
                    None => format!("Battery {}%", b.percent.round()),
                },
                None => String::new(),
            };
            sub.set_visible(!line.is_empty());
            sub.set_text(&line);
        }
    });
    row.append(&avatar);
    row.append(&text);
    let settings = icon_button("emblem-system-symbolic", "Settings");
    let hide = ctx.hide.clone();
    settings.connect_clicked(move |_| {
        hide();
        ui::open_settings("");
    });
    let power_button = icon_button("system-shutdown-symbolic", "Lock, suspend, restart, shut down");
    let hide = ctx.hide.clone();
    power_button.connect_clicked(move |_| {
        hide();
        ui::launch(&["omarchy-menu", "toggle", "system"]);
    });
    row.append(&settings);
    row.append(&power_button);
    row.upcast()
}

fn page_grid(ctx: &Ctx, page: &Page) -> gtk::Widget {
    let grid = gtk::Grid::new();
    grid.add_css_class("cc-grid");
    grid.set_column_homogeneous(true);
    grid.set_row_homogeneous(true);
    grid.set_column_spacing(10);
    grid.set_row_spacing(10);
    grid.set_valign(gtk::Align::Start);
    // Unknown widgets (from a newer version, or a typo) are skipped, not fatal.
    let known: Vec<(&layout::Tile, tiles::Kind)> =
        page.tiles.iter().filter_map(|t| tiles::kind(&t.widget).map(|k| (t, k))).collect();
    let sizes: Vec<layout::Size> =
        known.iter().map(|(t, k)| if k.sizes.contains(&t.size) { t.size } else { k.sizes[0] }).collect();
    for ((t, k), at) in known.iter().zip(layout::pack(&sizes)) {
        let w = (k.build)(ctx, at.size, t);
        grid.attach(&w, at.col as i32, at.row as i32, at.size.cols as i32, at.size.rows as i32);
    }
    grid.upcast()
}
