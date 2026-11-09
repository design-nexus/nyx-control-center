//! Plugin tiles without GTK: the `control-center` parts of Nyx plugins, hosted here and
//! drawn by the interface (Nyx.PluginView). The same lifecycle as the GTK panel's
//! `plugin_tiles`: a plugin starts when one of its tiles or detail views shows, hears
//! `visible` as the panel opens and closes, and stops two minutes after; what it last
//! showed stays, so the next open shows it at once.
//!
//! Published as topic `plugins`: `{kinds, off, tiles: {"plugin:<id>/<part>": {item,
//! popover, state}}}`, with view paths resolved (`/plugin/…`, `/storage/…`) and files a
//! third-party plugin may not show left out.

use gio::prelude::*;
use nyx_plugin::{Kind, Plugin, State, discovery, paths};
use nyx_plugin_host::{self as host, Handle, Report, Spec, limits::Limits};
use nyx_plugin_protocol::{API_VERSION, Event, EventKind, Hello, HostMsg, Item, Node, PluginMsg, Widget};
use nyx_widgets::layout::{Page, Size};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

pub const PREFIX: &str = "plugin:";
const STOP_AFTER: Duration = Duration::from_secs(120);
const CELL: (i32, i32) = (87, 48);
const GAP: i32 = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct PluginKind {
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
    settings: Value,
    generation: u64,
    state: RunState,
    widgets: Vec<Widget>,
    visible: bool,
    warned: HashSet<String>,
}

#[derive(Default)]
struct Shown {
    items: Vec<Item>,
    popovers: HashMap<String, Node>,
}

type Publish = Rc<dyn Fn(Value)>;

#[derive(Default)]
pub struct Host {
    test: bool,
    kinds: RefCell<Vec<PluginKind>>,
    off: RefCell<Vec<(String, String)>>,
    plugins: RefCell<BTreeMap<String, Running>>,
    shown: RefCell<HashMap<String, Shown>>,
    /// Plugins whose tiles or detail view are on screen now.
    on_screen: RefCell<HashSet<String>>,
    stop_timers: RefCell<HashMap<String, glib::SourceId>>,
    pages: RefCell<Vec<Page>>,
    settings: RefCell<BTreeMap<String, toml::Table>>,
    generation: Cell<u64>,
    publish: RefCell<Option<Publish>>,
    refresh_pending: Cell<bool>,
    publish_pending: Cell<bool>,
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

/// Start: find plugin tiles and watch for changes. `publish` gets the `plugins` topic.
pub fn start(test: bool, publish: Rc<dyn Fn(Value)>) {
    let h = Rc::new(Host { test, ..Default::default() });
    HOST.with(|cell| *cell.borrow_mut() = Some(h.clone()));
    *h.publish.borrow_mut() = Some(publish);
    host::reap_stale(host_name(&h));
    refresh_registry(&h);
    let mut monitors = vec![];
    let watch = |path: &Path, dir: bool| -> Option<gio::FileMonitor> {
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
    std::mem::forget(monitors);
}

/// The theme changed: tell running plugins.
pub fn theme_changed() {
    let h = host();
    let theme = crate::theme::info();
    for r in h.plugins.borrow().values() {
        r.handle.send(HostMsg::Theme { theme: theme.clone() });
    }
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
    *h.kinds.borrow_mut() = kinds;
    *h.off.borrow_mut() = off;
    let on: Vec<String> = h.on_screen.borrow().iter().cloned().collect();
    for id in on {
        ensure_running(h, &id);
    }
    publish(h);
}

/// The layout and the plugins' settings changed: tell plugins about their tiles.
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

fn settings_for(h: &Host, id: &str, defaults: &toml::Table) -> Value {
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

fn kind(id: &str) -> Option<PluginKind> {
    host().kinds.borrow().iter().find(|k| k.id == id).cloned()
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
        out.push(Widget {
            item: part.to_string(),
            monitor: String::new(),
            size: size.to_string(),
            width,
            height,
            options: Value::Null,
        });
    }
    out
}

fn ensure_running(h: &Rc<Host>, id: &str) {
    if h.plugins.borrow().contains_key(id) || !h.kinds.borrow().iter().any(|k| k.plugin == id) {
        return;
    }
    let Some(p) = discovery::discover().plugins.into_iter().find(|p| p.manifest.id == id) else { return };
    let state = State::load();
    let parts = state.enabled_parts(&p.manifest, Kind::ControlCenter);
    let settings = settings_for(h, id, &p.manifest.settings);
    let mut limits = Limits::resolve(
        p.trust.is_trusted(),
        &p.manifest.limits,
        h.settings.borrow().get(id).and_then(|t| t.get("limits")).and_then(|l| l.as_table()),
    );
    limits.fps = limits.fps.min(30);
    let widgets = widgets_for(h, id, &parts);
    let hello = Hello {
        api_version: API_VERSION,
        kind: Kind::ControlCenter.name().into(),
        plugin: id.to_string(),
        items: parts.clone(),
        settings: settings.clone(),
        monitors: vec![],
        theme: crate::theme::info(),
        permissions: vec![],
        frame_socket: None,
        bar_height: None,
        widgets: widgets.clone(),
    };
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
    publish(h);
}

fn stop(h: &Host, id: &str) {
    if let Some(t) = h.stop_timers.borrow_mut().remove(id) {
        t.remove();
    }
    h.plugins.borrow_mut().remove(id);
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
                if !r.visible {
                    r.handle.send(HostMsg::Visible { visible: false, item: None });
                }
            }
            publish(h);
        }
        Report::Message(m) => on_message(h, id, m),
        Report::Frames(_) => {}
        Report::Exited { status, stderr } => {
            eprintln!(
                "nyx-control-center: {id} stopped ({status}){}",
                if stderr.is_empty() { String::new() } else { format!(": {stderr}") }
            );
            if let Some(r) = h.plugins.borrow_mut().get_mut(id) {
                r.state = RunState::Crashed(status);
            }
            publish(h);
        }
        Report::Suspended { reason, crashed } => {
            eprintln!("nyx-control-center: {id} is off: {reason}");
            if let Some(r) = h.plugins.borrow_mut().get_mut(id) {
                r.state = RunState::Off(reason.clone());
            }
            h.shown.borrow_mut().remove(id);
            if crashed && let Ok(mut e) = nyx_plugin::state::Editor::open() {
                e.suspend(id, &reason, &nyx_plugin::state::now_utc());
                let _ = e.save();
            }
            publish(h);
        }
        Report::Warning(w) => warn_once(h, id, w),
    }
}

fn simple(item: &Item) -> Node {
    use nyx_plugin_protocol::{Align, NodeKind, Orientation};
    let mut children = vec![];
    if let Some(i) = &item.icon {
        children.push(Node::new(NodeKind::Icon { name: i.clone(), size: None }).id("icon"));
    }
    if let Some(t) = &item.text {
        children.push(Node::new(NodeKind::Label { text: t.clone(), markup: false }).id("text"));
    }
    if let Some(b) = &item.badge {
        children.push(Node::new(NodeKind::Badge { text: b.clone() }).id("badge"));
    }
    if let Some(p) = item.progress {
        children.push(Node::new(NodeKind::Progress { value: p }).id("progress"));
    }
    let mut n = Node::new(NodeKind::Box { orientation: Orientation::Horizontal, spacing: 6, align: Align::Center, children });
    n.class = item.class.clone();
    n
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
            let mut shown = h.shown.borrow_mut();
            let s = shown.entry(id.to_string()).or_default();
            match s.items.iter_mut().find(|i| i.id == item.id) {
                Some(old) => *old = item,
                None => s.items.push(item),
            }
        }
        PluginMsg::Patch(p) => {
            let mut shown = h.shown.borrow_mut();
            let Some(s) = shown.get_mut(id) else { return };
            for item in s.items.iter_mut().filter(|i| i.id == p.item) {
                let has_view = item.view.is_some();
                let mut v = item.view.take().unwrap_or_else(|| simple(item));
                for n in &p.nodes {
                    v.apply_patch(&n.id, &n.set);
                }
                // An item without a view keeps its native shape; its fields follow the
                // patch of the simple view's nodes.
                if has_view {
                    item.view = Some(v);
                } else {
                    for n in &p.nodes {
                        let text = n.set.get("text").and_then(Value::as_str).map(String::from);
                        match n.id.as_str() {
                            "text" if text.is_some() => item.text = text,
                            "badge" if text.is_some() => item.badge = text,
                            "icon" => {
                                if let Some(i) = n.set.get("name").or(n.set.get("icon")).and_then(Value::as_str) {
                                    item.icon = Some(i.into());
                                }
                            }
                            "progress" => item.progress = n.set.get("value").and_then(Value::as_f64).or(item.progress),
                            _ => {}
                        }
                        if let Some(c) = n.set.get("class").and_then(Value::as_array) {
                            item.class = c.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                        }
                    }
                }
            }
            if let Some(node) = s.popovers.get_mut(&p.item) {
                for n in &p.nodes {
                    node.apply_patch(&n.id, &n.set);
                }
            }
        }
        PluginMsg::Popover(p) => {
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
        PluginMsg::Remove { item, .. } => {
            if let Some(s) = h.shown.borrow_mut().get_mut(id) {
                s.items.retain(|i| i.id != item);
            }
        }
        PluginMsg::Log { level, message } => {
            eprintln!("[{id}] {level:?}: {message}");
            return;
        }
        PluginMsg::FrameReady { .. } | PluginMsg::Menu(_) | PluginMsg::FrameRequest { .. } => return,
    }
    publish(h);
}

/// Coalesce: plugins may send many messages in a row.
fn publish(h: &Rc<Host>) {
    if h.publish_pending.replace(true) {
        return;
    }
    glib::idle_add_local_once(|| {
        let h = host();
        h.publish_pending.set(false);
        let v = state_json(&h);
        let f = h.publish.borrow().clone();
        if let Some(f) = f {
            f(v);
        }
    });
}

/// A view's file paths as files on this side of the sandbox; anything a third-party
/// plugin may not show is dropped.
fn resolve_paths(v: &mut Value, plugin_dir: &Path, storage_dir: &Path, trusted: bool) {
    let resolve = |p: &str| -> Option<PathBuf> {
        if p.contains("..") {
            return None;
        }
        if let Some(rest) = p.strip_prefix("/plugin/") {
            return Some(plugin_dir.join(rest));
        }
        if let Some(rest) = p.strip_prefix("/storage/") {
            return Some(storage_dir.join(rest));
        }
        if Path::new(p).is_absolute() {
            return trusted.then(|| PathBuf::from(p));
        }
        Some(plugin_dir.join(p))
    };
    match v {
        Value::Object(m) => {
            let node = m.get("node").and_then(Value::as_str).unwrap_or("").to_string();
            if node == "image"
                && let Some(Value::String(p)) = m.get("path").cloned()
            {
                match resolve(&p) {
                    Some(r) => m.insert("path".into(), Value::String(r.display().to_string())),
                    None => m.remove("path"),
                };
            }
            if node == "icon"
                && let Some(Value::String(n)) = m.get("name").cloned()
                && n.contains('/')
            {
                match resolve(&n) {
                    Some(r) => m.insert("name".into(), Value::String(r.display().to_string())),
                    None => m.insert("name".into(), Value::String("image-missing-symbolic".into())),
                };
            }
            for (_, c) in m.iter_mut() {
                resolve_paths(c, plugin_dir, storage_dir, trusted);
            }
        }
        Value::Array(a) => {
            for c in a {
                resolve_paths(c, plugin_dir, storage_dir, trusted);
            }
        }
        _ => {}
    }
}

fn state_json(h: &Host) -> Value {
    let kinds: Vec<Value> = h
        .kinds
        .borrow()
        .iter()
        .map(|k| {
            json!({"id": k.id, "plugin": k.plugin, "part": k.part, "name": k.name, "icon": k.icon,
                   "sizes": k.sizes.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "swatch": k.swatch, "detail": k.detail})
        })
        .collect();
    let off: Vec<Value> = h.off.borrow().iter().map(|(id, name)| json!({"id": id, "name": name})).collect();
    let mut tiles = serde_json::Map::new();
    let plugins = h.plugins.borrow();
    let shown = h.shown.borrow();
    for k in h.kinds.borrow().iter() {
        let p = plugins.get(&k.plugin).map(|r| r.plugin.clone()).or_else(|| discovery::discover().get(&k.plugin).cloned());
        let state = match plugins.get(&k.plugin).map(|r| &r.state) {
            Some(RunState::Starting) => json!({"state": "starting"}),
            Some(RunState::Running) => json!({"state": "running"}),
            Some(RunState::Crashed(why)) => json!({"state": "crashed", "reason": why}),
            Some(RunState::Off(why)) => json!({"state": "off", "reason": why}),
            None => json!({"state": "stopped"}),
        };
        let mut entry = json!({"run": state, "item": null, "popover": null});
        if let (Some(s), Some(p)) = (shown.get(&k.plugin), p.as_ref()) {
            let item =
                s.items.iter().find(|i| p.manifest.part_of(Kind::ControlCenter, &i.id).as_deref() == Some(k.part.as_str()));
            if let Some(item) = item {
                let (dir, storage, trusted) = (p.dir.clone(), paths::storage_dir(&k.plugin), p.trust.is_trusted());
                let mut iv = serde_json::to_value(item).unwrap_or(Value::Null);
                resolve_paths(&mut iv, &dir, &storage, trusted);
                entry["item"] = iv;
                if let Some(pop) = s.popovers.get(&item.id) {
                    let mut pv = serde_json::to_value(pop).unwrap_or(Value::Null);
                    resolve_paths(&mut pv, &dir, &storage, trusted);
                    entry["popover"] = pv;
                }
            }
        }
        tiles.insert(k.id.clone(), entry);
    }
    json!({"kinds": kinds, "off": off, "tiles": tiles})
}

fn send(h: &Host, plugin: &str, msg: HostMsg) {
    if let Some(r) = h.plugins.borrow().get(plugin) {
        r.handle.send(msg);
    }
}

/// An event from a tile or detail view (`kind`: click, action, toggle, change, submit,
/// popover_open, popover_close).
pub fn event(tile: &str, item: &str, kind: &str, control: Option<String>, value: Option<Value>) -> Result<(), String> {
    let (plugin, _) = split_id(tile).ok_or("not a plugin tile")?;
    let kind: EventKind = serde_json::from_value(Value::String(kind.into())).map_err(|_| format!("unknown event kind {kind}"))?;
    let e = Event { item: item.into(), kind, monitor: None, button: Some(1), direction: None, control, value, anchor: None };
    send(&host(), plugin, HostMsg::Event(e));
    Ok(())
}

/// A click on a tile whose item has `on_click`: run it if the plugin may.
pub fn run_click(tile: &str) -> Result<(), String> {
    let h = host();
    let (plugin, part) = split_id(tile).ok_or("not a plugin tile")?;
    let p = h
        .plugins
        .borrow()
        .get(plugin)
        .map(|r| r.plugin.clone())
        .or_else(|| discovery::discover().get(plugin).cloned())
        .ok_or("no such plugin")?;
    let argv = h
        .shown
        .borrow()
        .get(plugin)
        .and_then(|s| {
            s.items
                .iter()
                .find(|i| p.manifest.part_of(Kind::ControlCenter, &i.id).as_deref() == Some(part))
                .and_then(|i| i.on_click.clone())
        })
        .ok_or("nothing to run")?;
    if !host::allowed_command(&p, &argv) {
        warn_once(&h, plugin, format!("it asked to run {:?}, which isn't in its host_commands", argv.join(" ")));
        return Err("not allowed".into());
    }
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    crate::launch(&args);
    Ok(())
}

/// The plugins whose tiles or detail views are on screen now.
pub fn set_on_screen(ids: HashSet<String>) {
    let h = host();
    let before = h.on_screen.replace(ids.clone());
    for id in ids.difference(&before) {
        if let Some(t) = h.stop_timers.borrow_mut().remove(id) {
            t.remove();
        }
        ensure_running(&h, id);
        let mut plugins = h.plugins.borrow_mut();
        if let Some(r) = plugins.get_mut(id)
            && !r.visible
        {
            r.visible = true;
            r.handle.send(HostMsg::Visible { visible: true, item: None });
        }
    }
    for id in before.difference(&ids) {
        let keep_alive = {
            let mut plugins = h.plugins.borrow_mut();
            let Some(r) = plugins.get_mut(id) else { continue };
            if r.visible {
                r.visible = false;
                r.handle.send(HostMsg::Visible { visible: false, item: None });
            }
            r.plugin.trust.is_trusted() && r.plugin.manifest.section(Kind::ControlCenter).keep_alive
        };
        if keep_alive {
            continue;
        }
        let id2 = id.clone();
        let source = glib::timeout_add_local_once(STOP_AFTER, move || {
            let h = host();
            h.stop_timers.borrow_mut().remove(&id2);
            if !h.on_screen.borrow().contains(&id2) {
                stop(&h, &id2);
                publish(&h);
            }
        });
        if let Some(old) = h.stop_timers.borrow_mut().insert(id.clone(), source) {
            old.remove();
        }
    }
}

pub fn list() -> Value {
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
        out.push(json!({
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
            "limits": r.map(|r| json!({
                "memory_mb": r.handle.limits.memory_mb,
                "cpu_percent": r.handle.limits.cpu_percent,
                "tasks": r.handle.limits.tasks,
                "messages_per_sec": r.handle.limits.messages_per_sec,
            })),
        }));
    }
    Value::Array(out)
}

pub fn restart(id: &str) -> bool {
    let h = host();
    let running = h.plugins.borrow().contains_key(id);
    if running {
        stop(&h, id);
        if h.on_screen.borrow().contains(id) {
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

/// `ControlCenterPlugins1`, as the GTK panel serves it.
pub fn serve(conn: &gio::DBusConnection) {
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
    if let Err(e) = registered {
        eprintln!("nyx-control-center: couldn't serve {DBUS_INTERFACE}: {e}");
    }
}
