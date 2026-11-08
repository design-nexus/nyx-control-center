//! Plugin tiles: the `control-center` parts of Nyx plugins (see nyx-bar's
//! `docs/plugins.md`), as tiles and detail views beside the native ones.
//!
//! A tile's id in the layout is `plugin:<plugin>/<part>`. Its plugin starts the first
//! time one of its tiles is shown, hears `visible` as the panel opens and closes, and
//! stops after two minutes hidden; its last items stay, so the next open shows them at
//! once while it starts again. An item without a view is drawn as a native toggle (or
//! a split tile when the part has a detail view); the part's popover is its detail view.

use crate::tiles::{self, Ctx, Detail};
use crate::ui::{self, label};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use nyx_plugin::{Kind, Plugin, State, discovery, paths};
use nyx_plugin_host::{self as host, Handle, Report, Spec, frames::Buffers, limits::Limits};
use nyx_plugin_protocol::{API_VERSION, Event, EventKind, Hello, HostMsg, Item, Node, PluginMsg, ThemeInfo, Widget};
use nyx_plugin_view::{self as view, Rendered, ViewEvent};
use nyx_widgets::layout::{Page, Size};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::time::Duration;

pub const PREFIX: &str = "plugin:";

/// How long a plugin keeps running after its last tile is hidden.
const STOP_AFTER: Duration = Duration::from_secs(120);

/// One grid cell: (400 px panel − 2 × 14 px padding − 3 × 8 px gaps) / 4, and a row.
const CELL: (i32, i32) = (87, 48);
const GAP: i32 = 8;

/// A plugin's tile as the dashboard knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginKind {
    /// `plugin:<plugin>/<part>`.
    pub id: String,
    pub plugin: String,
    pub part: String,
    pub name: String,
    pub icon: String,
    pub sizes: Vec<Size>,
    pub swatch: Option<String>,
    pub detail: bool,
}

pub fn tile_id(plugin: &str, part: &str) -> String {
    format!("{PREFIX}{plugin}/{part}")
}

/// `plugin:<plugin>/<part>` → (plugin, part).
pub fn split_id(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix(PREFIX)?;
    Some(rest.split_once('/').unwrap_or((rest, nyx_plugin::manifest::WHOLE)))
}

pub fn px(size: Size) -> (i32, i32) {
    let (c, r) = (size.cols as i32, size.rows as i32);
    (c * CELL.0 + (c - 1) * GAP, r * CELL.1 + (r - 1) * GAP)
}

fn kind_of(p: &Plugin, part: &nyx_plugin::Part) -> PluginKind {
    let sizes: Vec<Size> = part
        .extra
        .get("sizes")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str().and_then(Size::parse)).filter(|s| s.cols <= 4 && s.rows <= 3).collect())
        .unwrap_or_default();
    let text = |k: &str| part.extra.get(k).and_then(|v| v.as_str()).map(String::from);
    PluginKind {
        id: tile_id(&p.manifest.id, &part.id),
        plugin: p.manifest.id.clone(),
        part: part.id.clone(),
        name: if part.id == nyx_plugin::manifest::WHOLE { p.manifest.name.clone() } else { part.name.clone() },
        icon: text("icon").unwrap_or_else(|| "application-x-addon-symbolic".into()),
        sizes: if sizes.is_empty() { vec![Size::S2X1] } else { sizes },
        swatch: text("swatch").filter(|s| s.chars().all(|c| c.is_ascii_lowercase())),
        detail: part.extra.get("detail").and_then(|d| d.as_bool()).unwrap_or(false),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RunState {
    Starting,
    Running,
    Crashed(String),
    Off(String),
}

struct Running {
    plugin: Plugin,
    handle: Handle,
    parts: Vec<String>,
    settings: serde_json::Value,
    generation: u64,
    state: RunState,
    widgets: Vec<Widget>,
    visible: bool,
    frames: HashMap<(String, String), Buffers>,
    warned: HashSet<String>,
}

/// What a plugin last showed, kept while it's stopped.
#[derive(Default)]
struct Shown {
    items: Vec<Item>,
    popovers: HashMap<String, Node>,
}

struct TileView {
    plugin: String,
    part: String,
    kind: PluginKind,
    size: Size,
    /// Weak: the root holds this view (as data), not the other way round.
    root: glib::WeakRef<gtk::Box>,
    ctx: Ctx,
    rendered: RefCell<Option<Rc<Rendered>>>,
    last: RefCell<Option<(Item, bool)>>,
}

struct DetailView {
    plugin: String,
    part: String,
    body: gtk::ScrolledWindow,
    rendered: RefCell<Option<Rc<Rendered>>>,
}

#[derive(Default)]
struct Host {
    test: bool,
    kinds: RefCell<Vec<PluginKind>>,
    /// Plugins with Control Center parts that aren't turned on (or approved).
    off: RefCell<Vec<(String, String)>>,
    plugins: RefCell<BTreeMap<String, Running>>,
    shown: RefCell<HashMap<String, Shown>>,
    tiles: RefCell<Vec<Weak<TileView>>>,
    details: RefCell<Vec<Weak<DetailView>>>,
    /// Tiles and detail views on screen, per plugin.
    mapped: RefCell<HashMap<String, usize>>,
    stop_timers: RefCell<HashMap<String, glib::SourceId>>,
    pages: RefCell<Vec<Page>>,
    settings: RefCell<BTreeMap<String, toml::Table>>,
    generation: Cell<u64>,
    on_change: RefCell<Option<Rc<dyn Fn()>>>,
    monitors: RefCell<Vec<gio::FileMonitor>>,
    refresh_pending: Cell<bool>,
    interned: RefCell<Vec<String>>,
}

thread_local! {
    static HOST: RefCell<Option<Rc<Host>>> = const { RefCell::new(None) };
}

fn host() -> Rc<Host> {
    HOST.with(|h| h.borrow_mut().get_or_insert_with(|| Rc::new(Host::default())).clone())
}

fn host_name(h: &Host) -> &'static str {
    if h.test { "control-center-test" } else { "control-center" }
}

/// Start: find plugin tiles, and watch for changes. `changed` redraws the dashboard
/// when the set of plugin tiles changes.
pub fn start(test: bool, changed: Rc<dyn Fn()>) {
    let h = Rc::new(Host { test, ..Default::default() });
    HOST.with(|cell| *cell.borrow_mut() = Some(h.clone()));
    *h.on_change.borrow_mut() = Some(changed);
    host::reap_stale(host_name(&h));
    refresh_registry(&h);
    let mut monitors = vec![];
    let watch = |path: &std::path::Path, dir: bool| -> Option<gio::FileMonitor> {
        let f = gio::File::for_path(path);
        let m = if dir {
            let _ = std::fs::create_dir_all(path);
            f.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE).ok()?
        } else {
            f.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE).ok()?
        };
        m.connect_changed(|_, file, _, _| {
            if file.basename().is_some_and(|b| b.to_string_lossy().starts_with('.')) {
                return;
            }
            schedule_refresh();
        });
        Some(m)
    };
    monitors.extend(watch(&paths::state_file(), false));
    for (dir, _) in discovery::search_dirs() {
        monitors.extend(watch(&dir, true));
    }
    *h.monitors.borrow_mut() = monitors;
    nyx_theme::on_change(|| {
        let h = host();
        let theme = theme_info();
        for r in h.plugins.borrow().values() {
            r.handle.send(HostMsg::Theme { theme: theme.clone() });
        }
    });
}

fn schedule_refresh() {
    let h = host();
    if h.refresh_pending.replace(true) {
        return;
    }
    glib::timeout_add_local_once(Duration::from_millis(300), || {
        let h = host();
        h.refresh_pending.set(false);
        refresh_registry(&h);
    });
}

/// Find the turned-on Control Center parts; restart plugins whose folder, manifest or
/// parts changed; redraw the dashboard if the tiles changed.
fn refresh_registry(h: &Rc<Host>) {
    let found = discovery::discover();
    let state = State::load();
    let mut kinds = vec![];
    let mut off = vec![];
    for p in found.plugins.iter().filter(|p| p.manifest.has_kind(Kind::ControlCenter)) {
        let enabled = state.enabled_parts(&p.manifest, Kind::ControlCenter);
        let runs = state.runs(p, Kind::ControlCenter) && p.missing_needs().is_empty();
        for part in p.manifest.parts(Kind::ControlCenter) {
            if runs && enabled.contains(&part.id) {
                kinds.push(kind_of(p, &part));
            } else if !off.iter().any(|(id, _): &(String, String)| *id == p.manifest.id) {
                off.push((p.manifest.id.clone(), p.manifest.name.clone()));
            }
        }
    }
    // A plugin that changed starts again, from its new folder, on its next showing.
    let stale: Vec<String> = h
        .plugins
        .borrow()
        .iter()
        .filter(|(id, r)| {
            let now = found.get(id);
            now.is_none_or(|p| p.manifest != r.plugin.manifest || p.dir != r.plugin.dir || p.trust != r.plugin.trust)
                || r.parts != state.enabled_parts(&r.plugin.manifest, Kind::ControlCenter)
                || !kinds.iter().any(|k| k.plugin == **id)
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in &stale {
        stop(h, id);
    }
    let changed = *h.kinds.borrow() != kinds || *h.off.borrow() != off;
    *h.kinds.borrow_mut() = kinds;
    *h.off.borrow_mut() = off;
    if changed || !stale.is_empty() {
        let cb = h.on_change.borrow().clone();
        if let Some(cb) = cb {
            cb();
        }
    }
    // Restarted plugins whose tiles are on screen start again now.
    let mapped: Vec<String> = h.mapped.borrow().iter().filter(|(_, n)| **n > 0).map(|(id, _)| id.clone()).collect();
    for id in mapped {
        ensure_running(h, &id);
    }
}

/// The plugin tiles turned on, for the "Add widgets" tray.
pub fn kinds() -> Vec<PluginKind> {
    host().kinds.borrow().clone()
}

pub fn kind(id: &str) -> Option<PluginKind> {
    host().kinds.borrow().iter().find(|k| k.id == id).cloned()
}

/// Installed plugins with tiles that aren't turned on: (id, name).
pub fn off() -> Vec<(String, String)> {
    host().off.borrow().clone()
}

/// A name for a tile whose plugin isn't available.
pub fn missing_name(id: &str) -> String {
    let Some((plugin, _)) = split_id(id) else { return id.to_string() };
    discovery::discover().get(plugin).map(|p| p.manifest.name.clone()).unwrap_or_else(|| plugin.to_string())
}

/// The layout and the plugins' settings changed (`[plugins."<id>"]` in
/// control-center.toml): tell plugins about their tiles and settings.
pub fn set_layout(pages: &[Page], settings: &BTreeMap<String, toml::Table>) {
    let h = host();
    *h.pages.borrow_mut() = pages.to_vec();
    *h.settings.borrow_mut() = settings.clone();
    let ids: Vec<String> = h.plugins.borrow().keys().cloned().collect();
    for id in ids {
        let (manifest, parts) = {
            let plugins = h.plugins.borrow();
            let r = &plugins[&id];
            (r.plugin.manifest.clone(), r.parts.clone())
        };
        let widgets = widgets_for(&h, &id, &parts);
        let settings = settings_for(&h, &id, &manifest.settings);
        let mut plugins = h.plugins.borrow_mut();
        let Some(r) = plugins.get_mut(&id) else { continue };
        if r.widgets != widgets {
            r.handle.send(HostMsg::Widgets { widgets: widgets.clone() });
            r.widgets = widgets;
        }
        if r.settings != settings {
            r.handle.send(HostMsg::Settings { settings: settings.clone() });
            r.settings = settings;
        }
    }
}

fn settings_for(h: &Host, id: &str, defaults: &toml::Table) -> serde_json::Value {
    let mut t = defaults.clone();
    if let Some(user) = h.settings.borrow().get(id) {
        for (k, v) in user {
            if k != "limits" {
                t.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::to_value(t).unwrap_or_default()
}

fn widgets_for(h: &Host, plugin: &str, parts: &[String]) -> Vec<Widget> {
    let mut out = vec![];
    for t in h.pages.borrow().iter().flat_map(|p| p.tiles.iter()) {
        let Some((p, part)) = split_id(&t.widget) else { continue };
        if p != plugin || !parts.iter().any(|x| x == part) {
            continue;
        }
        let size = kind(&t.widget).filter(|k| !k.sizes.contains(&t.size)).map(|k| k.sizes[0]).unwrap_or(t.size);
        let (width, height) = px(size);
        out.push(Widget { item: part.to_string(), monitor: String::new(), size: size.to_string(), width, height, options: serde_json::Value::Null });
    }
    out
}

fn theme_info() -> ThemeInfo {
    let r = nyx_theme::resolved();
    ThemeInfo { light: r.theme.palette.light, name: r.meta.id, generation: nyx_theme::engine::snapshot::generation() }
}

// ---------- Running plugins ----------

fn ensure_running(h: &Rc<Host>, id: &str) {
    if h.plugins.borrow().contains_key(id) {
        return;
    }
    if !h.kinds.borrow().iter().any(|k| k.plugin == id) {
        return;
    }
    let Some(p) = discovery::discover().plugins.into_iter().find(|p| p.manifest.id == id) else { return };
    let state = State::load();
    let parts = state.enabled_parts(&p.manifest, Kind::ControlCenter);
    let settings = settings_for(h, id, &p.manifest.settings);
    let mut limits = Limits::resolve(p.trust.is_trusted(), &p.manifest.limits, h.settings.borrow().get(id).and_then(|t| t.get("limits")).and_then(|l| l.as_table()));
    // Tiles are small and the panel is a glance: 30 frames a second is plenty.
    limits.fps = limits.fps.min(30);
    let widgets = widgets_for(h, id, &parts);
    let hello = Hello {
        api_version: API_VERSION,
        kind: Kind::ControlCenter.name().into(),
        plugin: id.to_string(),
        items: parts.clone(),
        settings: settings.clone(),
        monitors: vec![],
        theme: theme_info(),
        permissions: vec![],
        frame_socket: None,
        bar_height: None,
        widgets: widgets.clone(),
    };
    load_css(&p, &limits);
    let spec = Spec { plugin: p.clone(), kind: Kind::ControlCenter, host: host_name(h).into(), hello, limits };
    let (tx, rx) = async_channel::unbounded();
    let handle = host::spawn(spec, tx);
    let generation = h.generation.get() + 1;
    h.generation.set(generation);
    h.plugins.borrow_mut().insert(
        id.to_string(),
        Running {
            plugin: p,
            handle,
            parts,
            settings,
            generation,
            state: RunState::Starting,
            widgets,
            visible: true,
            frames: HashMap::new(),
            warned: HashSet::new(),
        },
    );
    let id = id.to_string();
    glib::spawn_future_local(async move {
        while let Ok(report) = rx.recv().await {
            let h = host();
            if !h.plugins.borrow().get(&id).is_some_and(|r| r.generation == generation) {
                return;
            }
            on_report(&h, &id, report);
        }
    });
}

fn stop(h: &Host, id: &str) {
    if let Some(t) = h.stop_timers.borrow_mut().remove(id) {
        t.remove();
    }
    // Dropping the handle stops the process; what it showed stays in `shown`.
    h.plugins.borrow_mut().remove(id);
}

thread_local! {
    static CSS: RefCell<HashMap<String, gtk::CssProvider>> = RefCell::default();
}

fn load_css(p: &Plugin, limits: &Limits) {
    CSS.with(|css| {
        let mut css = css.borrow_mut();
        let display = gdk::Display::default();
        if let (Some(old), Some(d)) = (css.remove(&p.manifest.id), display.as_ref()) {
            gtk::style_context_remove_provider_for_display(d, &old);
        }
        let Ok(text) = std::fs::read_to_string(p.dir.join("style.css")) else { return };
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&view::css::scope(&text, &p.manifest.id, limits.infinite_animations));
        if let Some(d) = display {
            gtk::style_context_add_provider_for_display(&d, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
        }
        css.insert(p.manifest.id.clone(), provider);
    });
}

fn warn_once(h: &Host, id: &str, what: String) {
    if let Some(r) = h.plugins.borrow_mut().get_mut(id)
        && r.warned.insert(what.clone())
    {
        eprintln!("nyx-control-center: {id}: {what}");
    }
}

fn on_report(h: &Rc<Host>, id: &str, report: Report) {
    match report {
        Report::Started { .. } => {
            if let Some(r) = h.plugins.borrow_mut().get_mut(id) {
                r.state = RunState::Running;
                // It starts visible; say so if it isn't any more.
                if !r.visible {
                    r.handle.send(HostMsg::Visible { visible: false, item: None });
                }
            }
        }
        Report::Message(m) => on_message(h, id, m),
        Report::Frames(b) => {
            if let Some(r) = h.plugins.borrow_mut().get_mut(id) {
                r.frames.insert((b.item.clone(), b.node.clone()), b);
            }
        }
        Report::Exited { status, stderr } => {
            eprintln!(
                "nyx-control-center: {id} stopped ({status}){}",
                if stderr.is_empty() { String::new() } else { format!(": {stderr}") }
            );
            if let Some(r) = h.plugins.borrow_mut().get_mut(id) {
                r.state = RunState::Crashed(status);
                r.frames.clear();
            }
        }
        Report::Suspended { reason, crashed } => {
            eprintln!("nyx-control-center: {id} is off: {reason}");
            if let Some(r) = h.plugins.borrow_mut().get_mut(id) {
                r.state = RunState::Off(reason.clone());
            }
            h.shown.borrow_mut().remove(id);
            refresh_plugin(id);
            if crashed && let Ok(mut e) = nyx_plugin::state::Editor::open() {
                e.suspend(id, &reason, &nyx_plugin::state::now_utc());
                let _ = e.save();
            }
        }
        Report::Warning(w) => warn_once(h, id, w),
    }
}

fn on_message(h: &Rc<Host>, id: &str, msg: PluginMsg) {
    let part_of = |item: &str| -> Option<String> {
        let plugins = h.plugins.borrow();
        let r = plugins.get(id)?;
        r.plugin.manifest.part_of(Kind::ControlCenter, item).filter(|p| r.parts.contains(p))
    };
    match msg {
        PluginMsg::Item(item) => {
            if part_of(&item.id).is_none() {
                warn_once(h, id, format!("item {} isn't one of the parts turned on", item.id));
                return;
            }
            {
                let mut shown = h.shown.borrow_mut();
                let s = shown.entry(id.to_string()).or_default();
                match s.items.iter_mut().find(|i| i.id == item.id) {
                    Some(old) => *old = item,
                    None => s.items.push(item),
                }
            }
            refresh_plugin(id);
        }
        PluginMsg::Patch(p) => {
            {
                let mut shown = h.shown.borrow_mut();
                let Some(s) = shown.get_mut(id) else { return };
                for item in s.items.iter_mut().filter(|i| i.id == p.item) {
                    let mut v = item.view.take().unwrap_or_else(|| view::simple(item));
                    for n in &p.nodes {
                        v.apply_patch(&n.id, &n.set);
                    }
                    item.view = Some(v);
                }
                if let Some(node) = s.popovers.get_mut(&p.item) {
                    for n in &p.nodes {
                        node.apply_patch(&n.id, &n.set);
                    }
                }
            }
            patch_views(h, id, &p);
        }
        PluginMsg::Popover(p) => {
            {
                let mut shown = h.shown.borrow_mut();
                let s = shown.entry(id.to_string()).or_default();
                match p.view {
                    Some(v) => {
                        s.popovers.insert(p.item.clone(), v);
                    }
                    None => {
                        s.popovers.remove(&p.item);
                    }
                }
            }
            refresh_plugin(id);
        }
        PluginMsg::Remove { item, .. } => {
            if let Some(s) = h.shown.borrow_mut().get_mut(id) {
                s.items.retain(|i| i.id != item);
            }
            refresh_plugin(id);
        }
        PluginMsg::Log { level, message } => eprintln!("[{id}] {level:?}: {message}"),
        PluginMsg::FrameReady { item, node, buffer } => {
            // Frames only while the panel shows them.
            let Some(b) = h.plugins.borrow().get(id).filter(|r| r.visible).and_then(|r| r.frames.get(&(item.clone(), node.clone())).cloned())
            else {
                return;
            };
            let Some(map) = b.maps.get(buffer as usize) else { return };
            for t in live_tiles(h).iter().filter(|t| t.plugin == id) {
                if let Some(r) = t.rendered.borrow().as_ref()
                    && let Some(pic) = r.frame(&node)
                {
                    view::show_frame(&pic, map.bytes(), b.width, b.height, b.stride);
                }
            }
        }
        PluginMsg::Menu(_) | PluginMsg::FrameRequest { .. } => {}
    }
}

fn send(h: &Host, plugin: &str, msg: HostMsg) {
    if let Some(r) = h.plugins.borrow().get(plugin) {
        r.handle.send(msg);
    }
}

fn event(h: &Host, plugin: &str, item: &str, kind: EventKind, control: Option<String>, value: Option<serde_json::Value>) {
    let e = Event { item: item.into(), kind, monitor: None, button: Some(1), direction: None, control, value, anchor: None };
    send(h, plugin, HostMsg::Event(e));
}

fn run_for(h: &Host, plugin: &str, argv: &[String]) {
    let p = h.plugins.borrow().get(plugin).map(|r| r.plugin.clone());
    let Some(p) = p.or_else(|| discovery::discover().plugins.into_iter().find(|p| p.manifest.id == plugin)) else { return };
    if !host::allowed_command(&p, argv) {
        warn_once(h, plugin, format!("it asked to run {:?}, which isn't in its host_commands", argv.join(" ")));
        return;
    }
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    ui::launch(&args);
}

/// A tile or detail view of `plugin` came on screen, or left it.
fn mapped(plugin: &str, on: bool) {
    let h = host();
    let n = {
        let mut m = h.mapped.borrow_mut();
        let n = m.entry(plugin.to_string()).or_default();
        *n = if on { *n + 1 } else { n.saturating_sub(1) };
        *n
    };
    if on {
        if let Some(t) = h.stop_timers.borrow_mut().remove(plugin) {
            t.remove();
        }
        ensure_running(&h, plugin);
        let mut plugins = h.plugins.borrow_mut();
        if let Some(r) = plugins.get_mut(plugin)
            && !r.visible
        {
            r.visible = true;
            r.handle.send(HostMsg::Visible { visible: true, item: None });
        }
        return;
    }
    if n > 0 {
        return;
    }
    let keep_alive = {
        let mut plugins = h.plugins.borrow_mut();
        let Some(r) = plugins.get_mut(plugin) else { return };
        if r.visible {
            r.visible = false;
            r.handle.send(HostMsg::Visible { visible: false, item: None });
        }
        r.plugin.trust.is_trusted() && r.plugin.manifest.section(Kind::ControlCenter).keep_alive
    };
    if keep_alive {
        return;
    }
    let id = plugin.to_string();
    let source = glib::timeout_add_local_once(STOP_AFTER, move || {
        let h = host();
        h.stop_timers.borrow_mut().remove(&id);
        if h.mapped.borrow().get(&id).copied().unwrap_or(0) == 0 {
            stop(&h, &id);
        }
    });
    h.stop_timers.borrow_mut().insert(plugin.to_string(), source);
}

fn track_mapping(w: &impl IsA<gtk::Widget>, plugin: &str) {
    let p = plugin.to_string();
    w.connect_map(move |_| mapped(&p, true));
    let p = plugin.to_string();
    w.connect_unmap(move |_| mapped(&p, false));
}

// ---------- Tiles ----------

fn live_tiles(h: &Host) -> Vec<Rc<TileView>> {
    let mut tiles = h.tiles.borrow_mut();
    tiles.retain(|t| t.strong_count() > 0);
    tiles.iter().filter_map(Weak::upgrade).collect()
}

fn live_details(h: &Host) -> Vec<Rc<DetailView>> {
    let mut d = h.details.borrow_mut();
    d.retain(|t| t.strong_count() > 0);
    d.iter().filter_map(Weak::upgrade).collect()
}

/// The item a tile shows, and whether the part has a detail view now.
fn current(h: &Host, plugin: &str, part: &str) -> Option<(Item, bool)> {
    let shown = h.shown.borrow();
    let s = shown.get(plugin)?;
    let manifest = discovery_manifest(h, plugin)?;
    let item = s.items.iter().find(|i| manifest.part_of(Kind::ControlCenter, &i.id).as_deref() == Some(part))?.clone();
    let detail = s.popovers.contains_key(&item.id);
    Some((item, detail))
}

fn discovery_manifest(h: &Host, plugin: &str) -> Option<nyx_plugin::Manifest> {
    if let Some(r) = h.plugins.borrow().get(plugin) {
        return Some(r.plugin.manifest.clone());
    }
    discovery::discover().get(plugin).map(|p| p.manifest.clone())
}

fn view_ctx(plugin: &str, item: &str) -> Option<view::Ctx> {
    let h = host();
    let p = h.plugins.borrow().get(plugin).map(|r| r.plugin.clone()).or_else(|| discovery::discover().get(plugin).cloned())?;
    let (pid, iid) = (plugin.to_string(), item.to_string());
    Some(view::Ctx {
        plugin: plugin.to_string(),
        plugin_dir: p.dir.clone(),
        storage_dir: paths::storage_dir(plugin),
        trusted: p.trust.is_trusted(),
        max_height: None,
        on_event: Rc::new(move |e: ViewEvent| event(&host(), &pid, &iid, e.kind, e.control, e.value)),
    })
}

/// Build a plugin tile for the dashboard.
pub fn build(kind: &PluginKind, ctx: &Ctx, size: Size) -> gtk::Widget {
    let root = tiles::frame(size);
    root.add_css_class("cc-plugin");
    root.add_css_class(&view::css::plugin_class(&kind.plugin));
    if let Some(s) = &kind.swatch {
        tiles::tint(&root, s);
    }
    let t = Rc::new(TileView {
        plugin: kind.plugin.clone(),
        part: kind.part.clone(),
        kind: kind.clone(),
        size,
        root: root.downgrade(),
        ctx: ctx.clone(),
        rendered: RefCell::default(),
        last: RefCell::default(),
    });
    let h = host();
    h.tiles.borrow_mut().push(Rc::downgrade(&t));
    track_mapping(&root, &kind.plugin);
    // The tile owns its view; the host keeps only a weak reference.
    unsafe { root.set_data("nyx-plugin-tile", t.clone()) };
    refresh_tile(&h, &t);
    root.upcast()
}

fn refresh_plugin(plugin: &str) {
    let h = host();
    for t in live_tiles(&h).iter().filter(|t| t.plugin == plugin) {
        refresh_tile(&h, t);
    }
    for d in live_details(&h).iter().filter(|d| d.plugin == plugin) {
        refresh_detail(&h, d);
    }
}

fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

fn refresh_tile(h: &Rc<Host>, t: &Rc<TileView>) {
    let Some(root) = t.root.upgrade() else { return };
    let now = current(h, &t.plugin, &t.part);
    if *t.last.borrow() == now && root.first_child().is_some() {
        return;
    }
    clear(&root);
    for c in ["cc-toggle", "cc-split", "cc-plugin-view", "on", "cc-clickable"] {
        root.remove_css_class(c);
    }
    t.rendered.borrow_mut().take();
    let Some((item, has_detail)) = now.clone() else {
        // Nothing yet: the native shape with the part's icon, until the plugin speaks.
        template(h, t, None, false);
        *t.last.borrow_mut() = now;
        return;
    };
    root.set_tooltip_text(item.tooltip.as_deref());
    root.set_visible(item.visible);
    match &item.view {
        Some(node) => {
            root.add_css_class("cc-plugin-view");
            if let Some(ctx) = view_ctx(&t.plugin, &item.id) {
                let r = Rc::new(view::render(node, &ctx));
                r.root.set_vexpand(true);
                r.attach_states(root.upcast_ref(), &ctx);
                root.append(&r.root);
                *t.rendered.borrow_mut() = Some(r);
            }
            clickable(h, t, &item, has_detail);
        }
        None => template(h, t, Some(&item), has_detail),
    }
    *t.last.borrow_mut() = now;
}

/// Clicks on a tile with its own view: its command, else its detail view, else an event.
fn clickable(h: &Rc<Host>, t: &Rc<TileView>, item: &Item, has_detail: bool) {
    let Some(root) = t.root.upgrade() else { return };
    if item.on_click.is_none() && !has_detail {
        // Its own controls get the events.
        return;
    }
    root.add_css_class("cc-clickable");
    let click = gtk::GestureClick::new();
    let (wt, cmd, item_id) = (Rc::downgrade(t), item.on_click.clone(), item.id.clone());
    let _ = h;
    click.connect_released(move |g, _, _, _| {
        let Some(t) = wt.upgrade() else { return };
        g.set_state(gtk::EventSequenceState::Claimed);
        let h = host();
        match &cmd {
            Some(c) => {
                (t.ctx.hide)();
                run_for(&h, &t.plugin, c);
            }
            None => (t.ctx.open)(Detail::Plugin(intern(&t.plugin, &item_id))),
        }
    });
    root.add_controller(click);
}

/// The native toggle (1×1: the icon; 2×1 and up: icon, name and status), or a split
/// tile when the part has a detail view.
fn template(h: &Rc<Host>, t: &Rc<TileView>, item: Option<&Item>, has_detail: bool) {
    let Some(root) = t.root.upgrade() else { return };
    let icon = item.and_then(|i| i.icon.clone()).unwrap_or_else(|| t.kind.icon.clone());
    let on = item.is_some_and(|i| i.class.iter().any(|c| c == "on" || c == "active"));
    let (chip, _) = ui::chip(&icon);
    ui::set_class(&chip, "on", on);
    ui::set_class(&root, "on", on);
    let status = item.and_then(|i| i.text.clone()).unwrap_or_default();
    let item_id = item.map(|i| i.id.clone()).unwrap_or_else(|| t.part.clone());
    let on_main = {
        let (wt, cmd, id) = (Rc::downgrade(t), item.and_then(|i| i.on_click.clone()), item_id.clone());
        move || {
            let Some(t) = wt.upgrade() else { return };
            let h = host();
            match &cmd {
                Some(c) => {
                    (t.ctx.hide)();
                    run_for(&h, &t.plugin, c);
                }
                None => event(&h, &t.plugin, &id, EventKind::Toggle, None, Some((!on).into())),
            }
        }
    };
    let _ = h;
    if t.size.cols == 1 {
        root.add_css_class("cc-toggle");
        chip.set_halign(gtk::Align::Center);
        chip.set_valign(gtk::Align::Center);
        chip.set_vexpand(true);
        root.append(&chip);
        click_area(&root, on_main);
        if root.tooltip_text().is_none() {
            let tip = if status.is_empty() { t.kind.name.clone() } else { format!("{}: {status}", t.kind.name) };
            root.set_tooltip_text(Some(&tip));
        }
        return;
    }
    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text.set_valign(gtk::Align::Center);
    text.append(&label(&t.kind.name, "cc-title"));
    let s = label(&status, "cc-status");
    s.set_visible(!status.is_empty());
    text.append(&s);
    if has_detail && t.kind.detail {
        root.add_css_class("cc-split");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.set_vexpand(true);
        let main = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        main.add_css_class("cc-split-main");
        main.set_hexpand(true);
        chip.set_valign(gtk::Align::Center);
        main.append(&chip);
        main.append(&text);
        click_area(&main, on_main);
        let more = gtk::Button::from_icon_name("go-next-symbolic");
        more.add_css_class("cc-split-more");
        more.set_tooltip_text(Some(&format!("{}: more", t.kind.name)));
        let (wt, id) = (Rc::downgrade(t), item_id);
        more.connect_clicked(move |_| {
            if let Some(t) = wt.upgrade() {
                (t.ctx.open)(Detail::Plugin(intern(&t.plugin, &id)));
            }
        });
        row.append(&main);
        row.append(&more);
        root.append(&row);
    } else {
        root.add_css_class("cc-toggle");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_vexpand(true);
        chip.set_valign(gtk::Align::Center);
        row.append(&chip);
        row.append(&text);
        root.append(&row);
        if has_detail {
            let (wt, id) = (Rc::downgrade(t), item_id);
            click_area(&root, move || {
                if let Some(t) = wt.upgrade() {
                    (t.ctx.open)(Detail::Plugin(intern(&t.plugin, &id)));
                }
            });
        } else {
            click_area(&root, on_main);
        }
    }
}

fn click_area(w: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
    w.add_css_class("cc-clickable");
    let click = gtk::GestureClick::new();
    click.connect_released(move |g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        f();
    });
    w.add_controller(click);
}

fn patch_views(h: &Rc<Host>, plugin: &str, p: &nyx_plugin_protocol::Patch) {
    for t in live_tiles(h).iter().filter(|t| t.plugin == plugin) {
        let showing = t.last.borrow().as_ref().is_some_and(|(i, _)| i.id == p.item);
        if !showing {
            continue;
        }
        match (t.rendered.borrow().as_ref(), view_ctx(plugin, &p.item)) {
            (Some(r), Some(ctx)) => {
                for n in &p.nodes {
                    r.patch(n, &ctx);
                }
            }
            // A template tile: draw it again from the patched item.
            _ => {
                t.last.borrow_mut().take();
            }
        }
        if t.rendered.borrow().is_none() {
            refresh_tile(h, t);
        } else {
            *t.last.borrow_mut() = current(h, plugin, &t.part);
        }
    }
    for d in live_details(h).iter().filter(|d| d.plugin == plugin && d.part == p.item) {
        if let (Some(r), Some(ctx)) = (d.rendered.borrow().as_ref(), view_ctx(plugin, &p.item)) {
            for n in &p.nodes {
                r.patch(n, &ctx);
            }
        }
    }
}

// ---------- Detail views ----------

/// Detail views are named by a small number, so `Detail` stays `Copy`.
pub fn intern(plugin: &str, item: &str) -> u16 {
    let h = host();
    let key = format!("{plugin}/{item}");
    let mut i = h.interned.borrow_mut();
    match i.iter().position(|k| *k == key) {
        Some(n) => n as u16,
        None => {
            i.push(key);
            (i.len() - 1) as u16
        }
    }
}

/// `plugin:<plugin>/<item>`, for `--toggle` and `NYX_CC_TEST_DETAIL`.
pub fn parse_detail(s: &str) -> Option<u16> {
    let (plugin, item) = split_id(s)?;
    Some(intern(plugin, item))
}

pub fn detail_view(n: u16, back: Rc<dyn Fn()>) -> gtk::Widget {
    let h = host();
    let key = h.interned.borrow().get(n as usize).cloned().unwrap_or_default();
    let (plugin, item) = key.split_once('/').unwrap_or((&key, ""));
    let title = h
        .kinds
        .borrow()
        .iter()
        .find(|k| k.plugin == plugin && discovery_manifest(&h, plugin).and_then(|m| m.part_of(Kind::ControlCenter, item)).as_deref() == Some(k.part.as_str()))
        .map(|k| k.name.clone())
        .unwrap_or_else(|| missing_name(&tile_id(plugin, item)));
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("cc-detail");
    let (head, switch) = crate::details::head(&title, back);
    switch.set_visible(false);
    page.append(&head);
    let body = gtk::ScrolledWindow::new();
    body.set_hscrollbar_policy(gtk::PolicyType::Never);
    body.set_vexpand(true);
    body.add_css_class("cc-plugin-detail");
    page.append(&body);
    let d = Rc::new(DetailView { plugin: plugin.to_string(), part: item.to_string(), body, rendered: RefCell::default() });
    h.details.borrow_mut().push(Rc::downgrade(&d));
    unsafe { page.set_data("nyx-plugin-detail", d.clone()) };
    track_mapping(&page, plugin);
    let (p1, i1) = (plugin.to_string(), item.to_string());
    page.connect_map(move |_| event(&host(), &p1, &i1, EventKind::PopoverOpen, None, None));
    let (p2, i2) = (plugin.to_string(), item.to_string());
    page.connect_unmap(move |_| event(&host(), &p2, &i2, EventKind::PopoverClose, None, None));
    refresh_detail(&h, &d);
    page.upcast()
}

fn refresh_detail(h: &Host, d: &DetailView) {
    let node = h.shown.borrow().get(&d.plugin).and_then(|s| s.popovers.get(&d.part).cloned());
    let child: gtk::Widget = match node.and_then(|n| view_ctx(&d.plugin, &d.part).map(|c| (n, c))) {
        Some((node, ctx)) => {
            let r = Rc::new(view::render(&node, &ctx));
            let w = r.root.clone();
            *d.rendered.borrow_mut() = Some(r);
            w
        }
        None => {
            d.rendered.borrow_mut().take();
            let l = label("Nothing to show yet", "cc-status");
            l.set_xalign(0.5);
            l.set_margin_top(24);
            l.upcast()
        }
    };
    d.body.set_child(Some(&child));
}

// ---------- Edit mode, D-Bus ----------

/// A tile whose plugin is missing, off or stopped, in edit mode.
pub fn placeholder(id: &str, size: Size) -> gtk::Widget {
    let tile = tiles::frame(size);
    tile.add_css_class("cc-placeholder");
    let l = label(&format!("{} isn't available", missing_name(id)), "cc-status");
    l.set_xalign(0.5);
    l.set_valign(gtk::Align::Center);
    l.set_vexpand(true);
    tile.append(&l);
    tile.upcast()
}

/// For `ControlCenterPlugins1.ListPlugins`.
pub fn list() -> serde_json::Value {
    let h = host();
    let plugins = h.plugins.borrow();
    let state = State::load();
    let mut out = vec![];
    for p in discovery::discover().plugins.into_iter().filter(|p| p.manifest.has_kind(Kind::ControlCenter)) {
        let id = p.manifest.id.clone();
        let r = plugins.get(&id);
        let (memory, cpu) = r.and_then(|r| r.handle.usage()).map(|(m, c)| (Some(m), c)).unwrap_or((None, None));
        let status = match r.map(|r| &r.state) {
            Some(RunState::Running) if r.is_some_and(|r| r.visible) => "running".to_string(),
            Some(RunState::Running) => "paused (panel hidden)".to_string(),
            Some(RunState::Starting) => "starting".to_string(),
            Some(RunState::Crashed(why)) => format!("restarting ({why})"),
            Some(RunState::Off(why)) => format!("off: {why}"),
            None if state.suspended.contains_key(&id) => format!("off: {}", state.suspended[&id].reason),
            None if !nyx_plugin::authorized(&p, &state) => "needs approval".to_string(),
            None if state.enabled_parts(&p.manifest, Kind::ControlCenter).is_empty() => "off".to_string(),
            None => "stopped (starts when its tile shows)".to_string(),
        };
        out.push(serde_json::json!({
            "id": id,
            "status": status,
            "pid": r.and_then(|r| r.handle.pid()),
            "sandboxed": r.map(|r| r.handle.sandboxed).unwrap_or(!p.trust.is_trusted()),
            "scoped": r.map(|r| r.handle.scoped).unwrap_or(false),
            "restarts": r.map(|r| r.handle.restarts()).unwrap_or(0),
            "memory_bytes": memory,
            "cpu_percent": cpu,
            "parts": state.enabled_parts(&p.manifest, Kind::ControlCenter),
            "permissions": p.manifest.permissions.names(),
            "limits": r.map(|r| serde_json::json!({
                "memory_mb": r.handle.limits.memory_mb,
                "cpu_percent": r.handle.limits.cpu_percent,
                "tasks": r.handle.limits.tasks,
                "messages_per_sec": r.handle.limits.messages_per_sec,
            })),
        }));
    }
    serde_json::Value::Array(out)
}

pub fn restart(id: &str) -> bool {
    let h = host();
    let running = h.plugins.borrow().contains_key(id);
    if running {
        stop(&h, id);
        if h.mapped.borrow().get(id).copied().unwrap_or(0) > 0 {
            ensure_running(&h, id);
        }
    }
    running
}

pub fn reload() {
    refresh_registry(&host());
}

const DBUS_PATH: &str = "/io/github/design_nexus/nyx/ControlCenter/Plugins";
const DBUS_INTERFACE: &str = "io.github.design_nexus.nyx.ControlCenterPlugins1";
const DBUS_XML: &str = r#"<node>
  <interface name="io.github.design_nexus.nyx.ControlCenterPlugins1">
    <method name="ListPlugins"><arg type="s" name="json" direction="out"/></method>
    <method name="RestartPlugin"><arg type="s" name="id" direction="in"/><arg type="b" name="found" direction="out"/></method>
    <method name="Reload"/>
  </interface>
</node>"#;

/// `ControlCenterPlugins1` on the session bus, for `nyx plugin list` and tests. A test
/// copy also takes `<app id>.Test`, since it doesn't own the app's name.
pub fn serve(conn: &gio::DBusConnection, test: bool) {
    let Ok(info) = gio::DBusNodeInfo::for_xml(DBUS_XML) else { return };
    let Some(iface) = info.lookup_interface(DBUS_INTERFACE) else { return };
    let registered = conn
        .register_object(DBUS_PATH, &iface)
        .method_call(|_, _, _, _, method, params, invocation| match method {
            "ListPlugins" => invocation.return_value(Some(&(list().to_string(),).to_variant())),
            "RestartPlugin" => {
                let id = params.get::<(String,)>().map(|(i,)| i).unwrap_or_default();
                invocation.return_value(Some(&(restart(&id),).to_variant()));
            }
            "Reload" => {
                reload();
                invocation.return_value(None);
            }
            _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
        })
        .build();
    // Registered for the life of the app.
    if let Err(e) = registered {
        eprintln!("nyx-control-center: couldn't serve {DBUS_INTERFACE}: {e}");
    }
    if test {
        gio::bus_own_name_on_connection(
            conn,
            &format!("{}.Test", crate::APP_ID),
            gio::BusNameOwnerFlags::NONE,
            |_, _| {},
            |_, name| eprintln!("nyx-control-center: {name} is taken; this test copy has no {DBUS_INTERFACE}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyx_widgets::layout::{self, Tile};

    #[test]
    fn ids() {
        assert_eq!(tile_id("nyx.weather", "tile"), "plugin:nyx.weather/tile");
        assert_eq!(split_id("plugin:nyx.weather/tile"), Some(("nyx.weather", "tile")));
        assert_eq!(split_id("plugin:acme.whole"), Some(("acme.whole", "main")));
        assert_eq!(split_id("wifi"), None);
    }

    #[test]
    fn pixel_sizes_match_the_grid() {
        assert_eq!(px(Size::S1X1), (87, 48));
        assert_eq!(px(Size::S2X1), (182, 48));
        assert_eq!(px(Size::S4X3), (372, 160));
    }

    #[test]
    fn kinds_from_the_manifest() {
        let text = r#"
id = "acme.ping"
name = "Ping"
api_version = 1
kinds = ["control-center"]
exec = ["true"]
[[control-center.items]]
id = "ping"
name = "Latency"
sizes = ["2x1", "9x9", "4x3", "4x4", "1x1"]
icon = "network-transmit-receive-symbolic"
swatch = "teal"
detail = true
[[control-center.items]]
id = "plain"
name = "Plain"
swatch = "Not A Swatch"
"#;
        let manifest: nyx_plugin::Manifest = toml::from_str(text).unwrap();
        let p = Plugin { manifest, dir: "/tmp".into(), trust: nyx_plugin::Trust::ThirdParty, origin: None, linked: false };
        let parts = p.manifest.parts(Kind::ControlCenter);
        let k = kind_of(&p, &parts[0]);
        assert_eq!(k.id, "plugin:acme.ping/ping");
        // Bad sizes and anything over 4×3 are dropped.
        assert_eq!(k.sizes, vec![Size::S2X1, Size::S4X3, Size::S1X1]);
        assert_eq!((k.swatch.as_deref(), k.detail), (Some("teal"), true));
        let plain = kind_of(&p, &parts[1]);
        assert_eq!((plain.sizes.clone(), plain.swatch, plain.detail), (vec![Size::S2X1], None, false));
        assert_eq!(plain.icon, "application-x-addon-symbolic");
        // Plugin sizes go through the same resizing and packing as native ones.
        assert_eq!(layout::next_size(&k.sizes, Size::S2X1), Size::S4X3);
        assert_eq!(layout::pack(&[Size::S2X1, Size::S2X1, Size::S1X1]).len(), 3);
    }

    #[test]
    fn a_missing_plugin_tile_survives() {
        let mut c = crate::panel::Config {
            pages: vec![Page { tiles: vec![Tile::new("wifi", Size::S2X1), Tile::new("plugin:acme.gone/meter", Size::S2X1)] }],
            remember_page: true,
            version: 3,
            plugins: Default::default(),
        };
        crate::panel::migrate(&mut c);
        layout::tidy(&mut c.pages);
        let text = toml::to_string_pretty(&c).unwrap();
        let back: crate::panel::Config = toml::from_str(&text).unwrap();
        assert!(back.pages[0].tiles.iter().any(|t| t.widget == "plugin:acme.gone/meter"), "{text}");
    }
}
