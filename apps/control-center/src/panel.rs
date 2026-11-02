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
    config: RefCell<Config>,
    ctx: RefCell<Option<Ctx>>,
    editing: Cell<bool>,
    /// The header's normal and edit-mode buttons.
    head: RefCell<Option<(gtk::Box, gtk::Box)>>,
    /// Widgets to add, shown in edit mode.
    tray: gtk::Box,
    me: RefCell<std::rc::Weak<Panel>>,
    panel1: RefCell<Option<nyx_desktop::panel::Panel>>,
    /// The detail view last opened.
    detail: Cell<Option<Detail>>,
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
        // For screenshots: edit mode, or a detail view, straight away.
        if std::env::var_os("NYX_CC_TEST_EDIT").is_some() {
            self.set_editing(true);
        }
        // For screenshots: open a detail view straight away.
        if let Some(which) = std::env::var("NYX_CC_TEST_DETAIL").ok().as_deref().and_then(Detail::parse) {
            self.show_detail(which);
        }
    }

    /// Open on one list (Wi-Fi, Bluetooth, VPN); close if it's already showing.
    pub fn toggle_detail(&self, which: Detail) {
        if self.is_open() && self.detail.get() == Some(which) && self.content.visible_child_name().as_deref() == Some("detail") {
            self.hide();
            return;
        }
        self.show();
        self.show_detail(which);
    }

    fn show_detail(&self, which: Detail) {
        let open = self.ctx.borrow().as_ref().map(|c| c.open.clone());
        if let Some(open) = open {
            open(which);
        }
    }

    pub fn hide(&self) {
        if self.editing.get() {
            self.set_editing(false);
        }
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

    /// Draw the pages again from the layout (after an edit).
    fn rebuild(&self) {
        let Some(ctx) = self.ctx.borrow().clone() else { return };
        while let Some(c) = self.pages.first_child() {
            self.pages.remove(&c);
        }
        while let Some(c) = self.dots.first_child() {
            self.dots.remove(&c);
        }
        let editing = self.editing.get();
        let pages = self.config.borrow().pages.clone();
        for (i, page) in pages.iter().enumerate() {
            self.pages.add_named(&page_grid(&ctx, page, i, editing, self.me.borrow().clone()), Some(&format!("page-{i}")));
            let dot = gtk::Button::new();
            dot.add_css_class("cc-dot");
            dot.set_valign(gtk::Align::Center);
            dot.set_tooltip_text(Some(&format!("Page {}", i + 1)));
            let w = self.me.borrow().clone();
            dot.connect_clicked(move |_| {
                if let Some(p) = w.upgrade() {
                    p.go_to(i);
                }
            });
            // Drop a tile on a dot to move it to that page.
            if editing {
                self.drop_target(&dot, Some(i), None);
            }
            self.dots.append(&dot);
        }
        if editing {
            let add = ui::icon_button("list-add-symbolic", "Add a page");
            add.add_css_class("cc-dot-add");
            let w = self.me.borrow().clone();
            add.connect_clicked(move |_| {
                if let Some(p) = w.upgrade() {
                    p.config.borrow_mut().pages.push(Page::default());
                    let last = p.config.borrow().pages.len() - 1;
                    p.save();
                    p.rebuild();
                    p.go_to(last);
                }
            });
            self.dots.append(&add);
        }
        self.dots.set_visible(editing || pages.len() > 1);
        self.fill_tray();
        self.go_to(self.page.get());
    }

    fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("toml.tmp");
        let text = toml::to_string_pretty(&*self.config.borrow()).unwrap_or_default();
        if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
            eprintln!("nyx-control-center: couldn't save the layout to {}", path.display());
        }
    }

    fn set_editing(&self, on: bool) {
        self.editing.set(on);
        if let Some((normal, edit)) = self.head.borrow().as_ref() {
            normal.set_visible(!on);
            edit.set_visible(on);
        }
        self.tray.set_visible(on);
        if !on {
            let mut c = self.config.borrow_mut();
            layout::tidy(&mut c.pages);
            drop(c);
            self.save();
        }
        self.rebuild();
    }

    /// Change the layout, save it and redraw.
    fn edit(&self, f: impl FnOnce(&mut Vec<Page>)) {
        f(&mut self.config.borrow_mut().pages);
        self.save();
        self.rebuild();
    }

    /// Accept a dragged tile: onto a page dot (`page`, at its end) or before a tile
    /// (`page` and `before`).
    fn drop_target(&self, w: &impl IsA<gtk::Widget>, page: Option<usize>, before: Option<usize>) {
        let target = gtk::DropTarget::new(glib::BoxedAnyObject::static_type(), gdk::DragAction::MOVE);
        let me = self.me.borrow().clone();
        target.connect_drop(move |_, value, _, _| {
            let (Some(p), Ok(obj)) = (me.upgrade(), value.get::<glib::BoxedAnyObject>()) else { return false };
            let Ok(from) = obj.try_borrow::<TileDrag>() else { return false };
            let (fp, fi) = (from.0, from.1);
            let to_page = page.unwrap_or(fp);
            let mut to_index = before.unwrap_or(usize::MAX);
            // Moving later in the same page: the tile's own place closes up first.
            if to_page == fp && to_index != usize::MAX && fi < to_index {
                to_index -= 1;
            }
            p.edit(|pages| {
                layout::move_tile(pages, (fp, fi), to_page, to_index);
            });
            true
        });
        w.add_controller(target);
    }

    /// The widgets that aren't on any page yet, to add to this one.
    fn fill_tray(&self) {
        while let Some(c) = self.tray.first_child() {
            self.tray.remove(&c);
        }
        if !self.editing.get() {
            return;
        }
        let used: Vec<String> =
            self.config.borrow().pages.iter().flat_map(|p| p.tiles.iter().map(|t| t.widget.clone())).collect();
        let free: Vec<tiles::Kind> = tiles::kinds().into_iter().filter(|k| !used.iter().any(|u| u == k.id)).collect();
        let head = gtk::Label::new(Some(if free.is_empty() { "EVERY WIDGET IS ON A PAGE" } else { "ADD WIDGETS" }));
        head.add_css_class("cc-group");
        head.set_xalign(0.0);
        self.tray.append(&head);
        let grid = gtk::FlowBox::new();
        grid.add_css_class("cc-tray");
        grid.set_selection_mode(gtk::SelectionMode::None);
        grid.set_min_children_per_line(3);
        grid.set_max_children_per_line(3);
        grid.set_homogeneous(true);
        grid.set_column_spacing(8);
        grid.set_row_spacing(8);
        for k in free {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            card.add_css_class("cc-tray-card");
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            let (chip, _) = ui::chip(k.icon);
            top.append(&chip);
            let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            spacer.set_hexpand(true);
            top.append(&spacer);
            let add = ui::icon_button("list-add-symbolic", &format!("Add {}", k.name));
            add.add_css_class("cc-tray-add");
            top.append(&add);
            card.append(&top);
            card.append(&label(k.name, "cc-title"));
            let sizes: Vec<String> = k.sizes.iter().map(|s| s.to_string().replace('x', "×")).collect();
            card.append(&label(&sizes.join(" · "), "cc-status"));
            let (me, id, size) = (self.me.borrow().clone(), k.id, k.sizes[0]);
            add.connect_clicked(move |_| {
                if let Some(p) = me.upgrade() {
                    let page = p.page.get();
                    p.edit(|pages| {
                        if let Some(pg) = pages.get_mut(page) {
                            pg.tiles.push(layout::Tile::new(id, size));
                        }
                    });
                }
            });
            grid.insert(&card, -1);
            if let Some(child) = grid.last_child() {
                child.set_focusable(false);
            }
        }
        self.tray.append(&grid);
    }

    fn open_detail(&self, which: Detail, back: Rc<dyn Fn()>) {
        if let Some(d) = self.content.child_by_name("detail") {
            self.content.remove(&d);
        }
        self.detail.set(Some(which));
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
    // On the panel's own layer (mapped first, so the panel stays above it) and over the
    // bar and the dock: they're on the layer below, and the bar's surface spans the
    // whole screen, so a catcher under them never sees the click.
    catcher.set_layer(Layer::Overlay);
    catcher.set_keyboard_mode(KeyboardMode::None);
    for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        catcher.set_anchor(e, true);
    }
    catcher.set_exclusive_zone(-1);
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
        config: RefCell::new(config),
        ctx: RefCell::default(),
        editing: Cell::new(false),
        head: RefCell::default(),
        tray: gtk::Box::new(gtk::Orientation::Vertical, 0),
        me: RefCell::default(),
        panel1: RefCell::default(),
        detail: Cell::new(None),
    });
    *panel.me.borrow_mut() = Rc::downgrade(&panel);

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
        set_option: {
            let weak = weak.clone();
            Rc::new(move |widget: &str, key: &str, value: &str| {
                let Some(p) = weak.upgrade() else { return };
                let mut c = p.config.borrow_mut();
                if let Some(t) = c.pages.iter_mut().flat_map(|pg| pg.tiles.iter_mut()).find(|t| t.widget == widget) {
                    t.options.insert(key.into(), value.into());
                }
                drop(c);
                p.save();
            })
        },
    };

    // Header: who's here, battery, and the panel's own actions.
    let (head, normal, edit) = header(&ctx, weak.clone());
    root.append(&head);
    *panel.head.borrow_mut() = Some((normal, edit));
    *panel.ctx.borrow_mut() = Some(ctx.clone());

    // Dashboard: pages of tiles, swiped or picked by their dots.
    let dash = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dash.add_css_class("cc-dashboard");
    dash.append(&pages);
    panel.tray.add_css_class("cc-tray-area");
    panel.tray.set_visible(false);
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    // In edit mode the tray can be long: the dashboard scrolls.
    let scroll_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    scroll_area.append(&dash);
    scroll_area.append(&panel.tray);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_propagate_natural_height(true);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&scroll_area));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&scroller);
    column.append(&dots);
    content.add_named(&column, Some("dashboard"));
    root.append(&content);
    panel.rebuild();
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
            } else if p.editing.get() {
                p.set_editing(false);
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

fn header(ctx: &Ctx, panel: std::rc::Weak<Panel>) -> (gtk::Widget, gtk::Box, gtk::Box) {
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
    let normal = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let edit = icon_button("document-edit-symbolic", "Edit the dashboard");
    let p = panel.clone();
    edit.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.set_editing(true);
        }
    });
    normal.append(&edit);
    normal.append(&settings);
    normal.append(&power_button);
    row.append(&normal);

    let editing = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    editing.set_visible(false);
    // Restoring the default layout throws edits away: the first click asks, the second does.
    let restore = icon_button("edit-undo-symbolic", "Restore the default layout");
    let p = panel.clone();
    restore.connect_clicked(move |b| {
        if b.has_css_class("armed") {
            if let Some(p) = p.upgrade() {
                p.edit(|pages| *pages = tiles::default_pages());
            }
            b.remove_css_class("armed");
            b.set_tooltip_text(Some("Restore the default layout"));
        } else {
            b.add_css_class("armed");
            b.set_tooltip_text(Some("Click again to restore the default layout"));
            let bw = b.downgrade();
            glib::timeout_add_local_once(std::time::Duration::from_secs(4), move || {
                if let Some(b) = bw.upgrade() {
                    b.remove_css_class("armed");
                    b.set_tooltip_text(Some("Restore the default layout"));
                }
            });
        }
    });
    let done = icon_button("object-select-symbolic", "Done (Esc)");
    done.add_css_class("primary");
    done.connect_clicked(move |_| {
        if let Some(p) = panel.upgrade() {
            p.set_editing(false);
        }
    });
    editing.append(&restore);
    editing.append(&done);
    row.append(&editing);
    (row.upcast(), normal, editing)
}

/// A tile being dragged in edit mode: its page and place.
struct TileDrag(usize, usize);

fn page_grid(ctx: &Ctx, page: &Page, page_index: usize, editing: bool, panel: std::rc::Weak<Panel>) -> gtk::Widget {
    let grid = gtk::Grid::new();
    grid.add_css_class("cc-grid");
    grid.set_column_homogeneous(true);
    grid.set_row_homogeneous(true);
    grid.set_column_spacing(10);
    grid.set_row_spacing(10);
    grid.set_valign(gtk::Align::Start);
    // Unknown widgets (from a newer version, or a typo) are skipped, not fatal.
    let known: Vec<(usize, &layout::Tile, tiles::Kind)> =
        page.tiles.iter().enumerate().filter_map(|(i, t)| tiles::kind(&t.widget).map(|k| (i, t, k))).collect();
    let sizes: Vec<layout::Size> =
        known.iter().map(|(_, t, k)| if k.sizes.contains(&t.size) { t.size } else { k.sizes[0] }).collect();
    for ((index, t, k), at) in known.iter().zip(layout::pack(&sizes)) {
        let tile = (k.build)(ctx, at.size, t);
        let w: gtk::Widget = if editing { edit_cell(tile, *index, page_index, k, at.size, &panel) } else { tile };
        grid.attach(&w, at.col as i32, at.row as i32, at.size.cols as i32, at.size.rows as i32);
    }
    if editing && known.is_empty() {
        let empty = label("Add widgets from below, or drag one here", "cc-status");
        empty.set_xalign(0.5);
        empty.add_css_class("cc-empty-page");
        grid.attach(&empty, 0, 0, 4, 1);
        if let Some(p) = panel.upgrade() {
            p.drop_target(&empty, Some(page_index), None);
        }
    }
    grid.upcast()
}

/// A tile in edit mode: not clickable itself, with remove and resize buttons, dragged
/// to move (before another tile, or onto a page dot).
fn edit_cell(
    tile: gtk::Widget,
    index: usize,
    page: usize,
    kind: &tiles::Kind,
    size: layout::Size,
    panel: &std::rc::Weak<Panel>,
) -> gtk::Widget {
    tile.set_can_target(false);
    let cell = gtk::Overlay::new();
    cell.add_css_class("cc-edit-cell");
    cell.set_child(Some(&tile));
    let remove = icon_button("window-close-symbolic", &format!("Remove {}", kind.name));
    remove.add_css_class("cc-edit-remove");
    remove.set_halign(gtk::Align::Start);
    remove.set_valign(gtk::Align::Start);
    let p = panel.clone();
    remove.connect_clicked(move |_| {
        if let Some(p) = p.upgrade() {
            p.edit(|pages| {
                layout::remove_tile(pages, page, index);
            });
        }
    });
    cell.add_overlay(&remove);
    if kind.sizes.len() > 1 {
        let next = layout::next_size(kind.sizes, size);
        let resize = icon_button("view-fullscreen-symbolic", &format!("Size {size} (click for {next})").replace('x', "×"));
        resize.add_css_class("cc-edit-resize");
        resize.set_halign(gtk::Align::End);
        resize.set_valign(gtk::Align::End);
        let p = panel.clone();
        resize.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.edit(|pages| {
                    if let Some(t) = pages.get_mut(page).and_then(|pg| pg.tiles.get_mut(index)) {
                        t.size = next;
                    }
                });
            }
        });
        cell.add_overlay(&resize);
    }
    let drag = gtk::DragSource::new();
    drag.set_actions(gdk::DragAction::MOVE);
    drag.connect_prepare(move |_, _, _| {
        Some(gdk::ContentProvider::for_value(&glib::BoxedAnyObject::new(TileDrag(page, index)).to_value()))
    });
    let tw = tile.downgrade();
    drag.connect_drag_begin(move |src, _| {
        if let Some(t) = tw.upgrade() {
            src.set_icon(Some(&gtk::WidgetPaintable::new(Some(&t))), t.width() / 2, t.height() / 2);
        }
    });
    cell.add_controller(drag);
    if let Some(p) = panel.upgrade() {
        p.drop_target(&cell, Some(page), Some(index));
    }
    cell.upcast()
}
