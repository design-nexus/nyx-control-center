//! Detail views (they replace the dashboard inside the panel) and the volume popover.

use crate::tiles::{Detail, app_icon, output_icon};
use crate::ui::{self, act, bind, icon, icon_button, label, set_class};
use gtk::glib;
use gtk::prelude::*;
use nyx_providers::{audio, bluetooth, network};
use std::rc::Rc;

/// Build a detail view. `back` returns to the dashboard.
pub fn view(which: Detail, back: Rc<dyn Fn()>) -> gtk::Widget {
    match which {
        Detail::Wifi => wifi(back),
        Detail::Bluetooth => bluetooth_view(back),
        Detail::Vpn => crate::route_tiles::vpn_view(back),
        Detail::Tailscale => crate::route_tiles::tailscale_view(back),
        Detail::Display => display_view(back),
    }
}

/// Back, title, and the feature's switch.
pub fn head(title: &str, back: Rc<dyn Fn()>) -> (gtk::Box, gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.add_css_class("cc-detail-head");
    let b = icon_button("go-previous-symbolic", "Back (Esc)");
    b.connect_clicked(move |_| back());
    let t = label(title, "cc-detail-title");
    t.set_hexpand(true);
    let sw = gtk::Switch::new();
    sw.set_valign(gtk::Align::Center);
    row.append(&b);
    row.append(&t);
    row.append(&sw);
    (row, sw)
}

pub fn group(title: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&title.to_uppercase()));
    l.add_css_class("cc-group");
    l.set_xalign(0.0);
    l
}

pub fn card() -> gtk::Box {
    let c = gtk::Box::new(gtk::Orientation::Vertical, 0);
    c.add_css_class("cc-card");
    c
}

pub fn row(icon_name: &str, title: &str, sub: Option<&str>) -> gtk::Box {
    let r = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    r.add_css_class("cc-row");
    r.append(&icon(icon_name, 16));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&label(title, "cc-row-title"));
    if let Some(s) = sub {
        text.append(&label(s, "cc-row-sub"));
    }
    r.append(&text);
    r
}

pub fn on_click(w: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
    let c = gtk::GestureClick::new();
    c.connect_released(move |_, _, _, _| f());
    w.add_controller(c);
    w.add_css_class("cc-clickable");
}

pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let s = gtk::ScrolledWindow::new();
    s.set_hscrollbar_policy(gtk::PolicyType::Never);
    s.set_vexpand(true);
    s.set_propagate_natural_height(true);
    s.set_child(Some(child));
    s
}

pub fn foot(buttons: &[gtk::Button]) -> gtk::Box {
    let f = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    f.add_css_class("cc-detail-foot");
    f.set_halign(gtk::Align::End);
    for b in buttons {
        f.append(b);
    }
    f
}

pub fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

/// A switch that reports user flips only (not the ones made to show the state).
pub fn guarded_switch(sw: &gtk::Switch, f: impl Fn(bool) + 'static) -> Rc<std::cell::Cell<bool>> {
    let quiet = Rc::new(std::cell::Cell::new(false));
    let q = quiet.clone();
    sw.connect_state_set(move |_, on| {
        if !q.get() {
            f(on);
        }
        glib::Propagation::Proceed
    });
    quiet
}

pub fn show_switch(sw: &gtk::Switch, quiet: &std::cell::Cell<bool>, on: bool) {
    quiet.set(true);
    sw.set_active(on);
    quiet.set(false);
}

// ---------- Wi-Fi ----------

fn bars_icon(signal: u8) -> &'static str {
    match network::bars(signal) {
        1 => "network-wireless-signal-weak-symbolic",
        2 => "network-wireless-signal-ok-symbolic",
        3 => "network-wireless-signal-good-symbolic",
        _ => "network-wireless-signal-excellent-symbolic",
    }
}

fn wifi(back: Rc<dyn Fn()>) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-detail");
    let (h, sw) = head("Wi-Fi", back);
    root.append(&h);
    let quiet = guarded_switch(&sw, |on| act(network::set_wifi(on)));
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let extras = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let conn = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&body);
    outer.append(&conn);
    outer.append(&extras);
    let cb = conn.clone();
    bind(&root, nyx_providers::dns::details(), move |d| connection_details(&cb, d.as_ref()));
    root.append(&scrolled(&outer));
    let rescan = icon_button("view-refresh-symbolic", "Look for networks again");
    rescan.connect_clicked(|_| act(network::rescan()));
    let speed = icon_button("speedometer-symbolic", "Speed test: measure download and upload");
    speed.connect_clicked(|_| nyx_providers::dns::speed_test());
    let qr = icon_button("qr-code-symbolic", "Share this network: a QR code a phone can scan to join");
    qr.connect_clicked(|_| nyx_providers::dns::share_qr());
    let settings = icon_button("emblem-system-symbolic", "Wi-Fi settings");
    settings.connect_clicked(|_| ui::open_settings("wifi"));
    root.append(&foot(&[rescan, speed, qr, settings]));
    let e = extras.clone();
    bind(&root, nyx_providers::dns::live(), move |st| wifi_extras(&e, st));

    // The network whose password is being typed. While it's set, system updates don't
    // redraw the list (a scan would rebuild the field and lose what's typed).
    let asking: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    type Draw = Rc<dyn Fn(&network::State)>;
    let slot: Rc<std::cell::RefCell<Option<Draw>>> = Rc::default();
    let redraw = {
        let slot = slot.clone();
        move || {
            let d = slot.borrow().clone();
            if let Some(d) = d {
                d(&network::live().get());
            }
        }
    };
    let draw: Draw = {
        let (b, asking, sw, quiet) = (body.clone(), asking.clone(), sw.clone(), quiet.clone());
        let redraw = Rc::new(redraw.clone());
        Rc::new(move |st: &network::State| {
            show_switch(&sw, &quiet, st.wifi_on);
            clear(&b);
            if !st.wifi_on {
                let off = label("Wi-Fi is off", "cc-empty");
                off.set_xalign(0.5);
                b.append(&off);
                return;
            }
            if let Some(cur) = st.networks.iter().find(|n| n.active) {
                b.append(&current_card(cur, st));
            }
            for (title, known) in [("Known networks", true), ("Other networks", false)] {
                let list: Vec<&network::Network> = st.networks.iter().filter(|n| !n.active && n.known == known).collect();
                if list.is_empty() {
                    continue;
                }
                b.append(&group(title));
                let c = card();
                for n in list.into_iter().take(12) {
                    c.append(&network_row(n));
                    let wants_password = !n.known && n.secure;
                    if wants_password && asking.borrow().as_deref() == Some(n.ssid.as_str()) {
                        c.append(&password_row(n, asking.clone(), redraw.clone()));
                    }
                    let last = c.last_child().expect("row");
                    // The row (not the password field) joins or asks for the password.
                    let r = if last.has_css_class("cc-password") { last.prev_sibling().expect("row") } else { last };
                    let (net, a, rd) = (n.clone(), asking.clone(), redraw.clone());
                    on_click(&r, move || {
                        if wants_password {
                            *a.borrow_mut() = Some(net.ssid.clone());
                            rd();
                        } else {
                            let n = net.clone();
                            act(async move { network::connect(&n, None).await });
                        }
                    });
                }
                b.append(&c);
            }
        })
    };
    *slot.borrow_mut() = Some(draw.clone());
    let a = asking.clone();
    bind(&root, network::live(), move |st| {
        if a.borrow().is_none() {
            draw(st);
        }
    });
    root.upcast()
}

/// The active connection: address, gateway, link, ping and throughput.
fn connection_details(b: &gtk::Box, d: Option<&nyx_providers::dns::Details>) {
    use nyx_providers::dns::rate;
    clear(b);
    let Some(d) = d else { return };
    b.append(&group("Connection"));
    let grid = gtk::Grid::new();
    grid.add_css_class("cc-card");
    grid.add_css_class("cc-facts");
    grid.set_column_homogeneous(true);
    grid.set_row_spacing(4);
    grid.set_column_spacing(10);
    let ms = |v: Option<f64>| v.map(|m| format!("{m:.0} ms")).unwrap_or_else(|| "—".into());
    let band = d.freq.map(|f| {
        if f < 3000.0 {
            "2.4 GHz"
        } else if f < 5900.0 {
            "5 GHz"
        } else {
            "6 GHz"
        }
    });
    let mut facts: Vec<(&str, String)> = vec![
        ("Address", if d.prefix.is_empty() { d.ip.clone() } else { format!("{}/{}", d.ip, d.prefix) }),
        ("Router", d.gateway.clone()),
        ("Router ping", ms(d.router_ping_ms)),
        ("Internet ping", ms(d.internet_ping_ms)),
        ("Down", d.down.map(rate).unwrap_or_else(|| "…".into())),
        ("Up", d.up.map(rate).unwrap_or_else(|| "…".into())),
    ];
    if d.kind == "wifi" {
        facts.push(("Signal", d.signal_dbm.map(|s| format!("{s} dBm")).unwrap_or_else(|| "—".into())));
        facts.push((
            "Link",
            match (band, d.bitrate.is_empty()) {
                (Some(bd), false) => format!("{bd} · {}", d.bitrate.replace("MBit/s", "Mbit/s")),
                (Some(bd), true) => bd.to_string(),
                (None, _) => d.bitrate.clone(),
            },
        ));
    }
    for (i, (k, v)) in facts.iter().enumerate() {
        let cell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        cell.append(&label(k, "cc-fact-name"));
        let val = label(v, "cc-fact-value");
        val.set_selectable(true);
        cell.append(&val);
        grid.attach(&cell, (i % 2) as i32, (i / 2) as i32, 1, 1);
    }
    b.append(&grid);
}

/// DNS provider, Wi-Fi band and a sign-in page: Omarchy's network panel's extras.
fn wifi_extras(b: &gtk::Box, st: &nyx_providers::dns::State) {
    use nyx_providers::dns;
    clear(b);
    if st.portal {
        let c = card();
        let r = row("dialog-password-symbolic", "This network wants you to sign in", Some("Opens its sign-in page"));
        on_click(&r, dns::sign_in);
        c.append(&r);
        b.append(&c);
    }
    if !st.dns.is_empty() {
        b.append(&group("DNS"));
        let c = card();
        for (id, name, desc) in dns::PROVIDERS {
            let current = st.dns == *id;
            let r = row("network-server-symbolic", name, Some(desc));
            set_class(&r, "selected", current);
            if current {
                r.append(&icon("object-select-symbolic", 14));
            }
            let id = id.to_string();
            on_click(&r, move || {
                let id = id.clone();
                act(async move { dns::set_dns(&id).await });
            });
            c.append(&r);
        }
        b.append(&c);
    }
    if let Some(band) = &st.band
        && (band.available.len() > 1 || band.selected != "auto")
    {
        b.append(&group("Band"));
        let c = card();
        let mut options = vec![("auto".to_string(), format!("Automatic (now {} GHz)", band.current))];
        options.extend(band.available.iter().map(|a| (a.clone(), format!("{a} GHz only"))));
        for (id, text) in options {
            let current = band.selected == id;
            let r = row("network-wireless-symbolic", &text, None);
            set_class(&r, "selected", current);
            if current {
                r.append(&icon("object-select-symbolic", 14));
            }
            on_click(&r, move || {
                let id = id.clone();
                act(async move { dns::set_band(&id).await });
            });
            c.append(&r);
        }
        b.append(&c);
    }
}

fn current_card(cur: &network::Network, st: &network::State) -> gtk::Box {
    let c = card();
    let r = row(bars_icon(cur.signal), cur.ssid.trim(), Some("Connected"));
    r.add_css_class("selected");
    if let Some(uuid) = st.wifi_uuid.clone() {
        let off = icon_button("network-offline-symbolic", "Disconnect");
        off.connect_clicked(move |_| {
            let u = uuid.clone();
            act(async move { network::disconnect(&u).await });
        });
        r.append(&off);
    }
    c.append(&r);
    let m = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    m.add_css_class("cc-option");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text.set_hexpand(true);
    text.append(&label("Metered connection", "cc-row-title"));
    let desc = label("Pause large downloads and updates on this network", "cc-row-sub");
    desc.set_ellipsize(gtk::pango::EllipsizeMode::None);
    desc.set_wrap(true);
    text.append(&desc);
    let msw = gtk::Switch::new();
    msw.set_valign(gtk::Align::Center);
    msw.set_active(st.metered);
    let uuid = st.wifi_uuid.clone();
    msw.connect_state_set(move |_, on| {
        if let Some(u) = uuid.clone() {
            act(async move { network::set_metered(&u, on).await });
        }
        glib::Propagation::Proceed
    });
    m.append(&text);
    m.append(&msw);
    c.append(&m);
    c
}

fn network_row(n: &network::Network) -> gtk::Box {
    let r = row(bars_icon(n.signal), n.ssid.trim(), None);
    if n.secure {
        let lock = icon("system-lock-screen-symbolic", 13);
        lock.add_css_class("cc-dim");
        r.append(&lock);
    }
    r
}

/// The password field under a new secured network; Enter or the arrow joins, Esc cancels.
fn password_row(n: &network::Network, asking: Rc<std::cell::RefCell<Option<String>>>, redraw: Rc<dyn Fn()>) -> gtk::Box {
    let entry_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    entry_box.add_css_class("cc-password");
    let entry = gtk::PasswordEntry::new();
    entry.set_show_peek_icon(true);
    entry.set_hexpand(true);
    entry.set_placeholder_text(Some("Password"));
    let join = icon_button("go-next-symbolic", "Join (Enter)");
    let (net, e, a, rd) = (n.clone(), entry.clone(), asking.clone(), redraw.clone());
    let go = Rc::new(move || {
        let (net, pw) = (net.clone(), e.text().to_string());
        a.borrow_mut().take();
        rd();
        act(async move { network::connect(&net, Some(&pw)).await });
    });
    let g = go.clone();
    entry.connect_activate(move |_| g());
    join.connect_clicked(move |_| go());
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            asking.borrow_mut().take();
            redraw();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    entry.add_controller(keys);
    entry_box.append(&entry);
    entry_box.append(&join);
    glib::idle_add_local_once(move || {
        entry.grab_focus();
    });
    entry_box
}

// ---------- Bluetooth ----------

fn bluetooth_view(back: Rc<dyn Fn()>) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-detail");
    let (h, sw) = head("Bluetooth", back);
    root.append(&h);
    let quiet = guarded_switch(&sw, |on| act(bluetooth::set_powered(on)));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    let look = gtk::ToggleButton::new();
    look.set_icon_name("system-search-symbolic");
    look.add_css_class("cc-icon-button");
    look.set_tooltip_text(Some("Look for devices to pair (stops when you leave this view)"));
    look.connect_toggled(|t| act(bluetooth::discover(t.is_active())));
    let settings = icon_button("emblem-system-symbolic", "Bluetooth settings");
    settings.connect_clicked(|_| ui::open_settings("bluetooth"));
    let f = foot(&[settings]);
    f.prepend(&look);
    root.append(&f);
    // Stop looking when the view goes.
    let l2 = look.clone();
    root.connect_unmap(move |_| {
        if l2.is_active() {
            l2.set_active(false);
        }
    });
    let b = body.clone();
    // The device being connected, disconnected, paired or forgotten.
    let busy: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    bind(&root, bluetooth::live(), move |st| {
        show_switch(&sw, &quiet, st.powered);
        sw.set_sensitive(st.available);
        look.set_sensitive(st.powered);
        clear(&b);
        if !st.available || !st.powered {
            let l = label(if st.available { "Bluetooth is off" } else { "No Bluetooth adapter" }, "cc-empty");
            l.set_xalign(0.5);
            b.append(&l);
            return;
        }
        let is_busy = |path: &str| busy.borrow().as_deref() == Some(path);
        let run = {
            let busy = busy.clone();
            move |path: String, fut: std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>>>>| {
                *busy.borrow_mut() = Some(path);
                let b2 = busy.clone();
                glib::MainContext::default().spawn_local(async move {
                    if let Err(e) = fut.await {
                        eprintln!("nyx-control-center: {e}");
                    }
                    b2.borrow_mut().take();
                });
            }
        };
        if st.devices.is_empty() {
            let l = label("No paired devices. Look for one with the search button below.", "cc-empty");
            l.set_wrap(true);
            l.set_xalign(0.5);
            b.append(&l);
        } else {
            b.append(&group("Devices"));
            let c = card();
            for d in &st.devices {
                let sub = match (d.connected, d.battery, is_busy(&d.path)) {
                    (_, _, true) => Some("Working…".to_string()),
                    (true, Some(p), _) => Some(format!("Connected · battery {p}%")),
                    (true, None, _) => Some("Connected".into()),
                    (false, Some(p), _) => Some(format!("Battery {p}%")),
                    (false, None, _) => None,
                };
                let r = row(&format!("{}-symbolic", d.icon), &d.name, sub.as_deref());
                set_class(&r, "selected", d.connected);
                let forget =
                    icon_button("user-trash-symbolic", &format!("Forget {}: it has to be paired again to connect", d.name));
                forget.add_css_class("cc-row-button");
                forget.set_valign(gtk::Align::Center);
                let (addr, path, run2) = (d.address.clone(), d.path.clone(), run.clone());
                forget.connect_clicked(move |_| {
                    let a = addr.clone();
                    run2(path.clone(), Box::pin(async move { bluetooth::forget(&a).await }));
                });
                r.append(&forget);
                let (path, connected, run2) = (d.path.clone(), d.connected, run.clone());
                on_click(&r, move || {
                    let p = path.clone();
                    run2(path.clone(), Box::pin(async move { bluetooth::set_connected(&p, !connected).await }));
                });
                r.set_tooltip_text(Some(&if d.connected {
                    format!("Disconnect {}", d.name)
                } else {
                    format!("Connect {}", d.name)
                }));
                c.append(&r);
            }
            b.append(&c);
        }
        if st.discovering || !st.nearby.is_empty() {
            b.append(&group(if st.discovering { "Nearby (looking…)" } else { "Nearby" }));
            let c = card();
            if st.nearby.is_empty() {
                c.append(&row("system-search-symbolic", "Looking…", Some("Put the device in pairing mode")));
            }
            for d in st.nearby.iter().take(12) {
                let sub = if is_busy(&d.path) { Some("Pairing…") } else { None };
                let r = row(&format!("{}-symbolic", d.icon), &d.name, sub);
                r.set_tooltip_text(Some(&format!("Pair with {} and connect", d.name)));
                let (addr, path, run2) = (d.address.clone(), d.path.clone(), run.clone());
                on_click(&r, move || {
                    let a = addr.clone();
                    run2(path.clone(), Box::pin(async move { bluetooth::pair(&a).await }));
                });
                c.append(&r);
            }
            b.append(&c);
        }
    });
    root.upcast()
}

// ---------- Display ----------

fn display_view(back: Rc<dyn Fn()>) -> gtk::Widget {
    use nyx_providers::display;
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-detail");
    let (h, sw) = head("Screens", back);
    sw.set_visible(false);
    root.append(&h);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    let settings = icon_button("emblem-system-symbolic", "Display settings: resolution, arrangement, refresh rate");
    settings.connect_clicked(|_| ui::open_settings("displays"));
    root.append(&foot(&[settings]));

    // Brightness sliders are built once per screen (a redraw mid-drag would drop them);
    // the rest is redrawn.
    let sliders = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let rest = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&sliders);
    body.append(&rest);
    let shown: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
    let pushing = Rc::new(std::cell::Cell::new(false));
    let scales: Rc<std::cell::RefCell<Vec<(String, gtk::Scale, gtk::Label)>>> = Rc::default();
    bind(&root, display::live(), move |st| {
        let names: Vec<String> = st.screens.iter().filter(|s| s.brightness.is_some()).map(|s| s.name.clone()).collect();
        if *shown.borrow() != names {
            clear(&sliders);
            scales.borrow_mut().clear();
            if !names.is_empty() {
                sliders.append(&group("Brightness"));
            }
            let c = card();
            for s in st.screens.iter().filter(|s| s.brightness.is_some()) {
                let title =
                    if s.internal { "Laptop screen".to_string() } else { format!("{} ({}×{})", s.name, s.width, s.height) };
                let (r, _, scale, value) = ui::slider("display-brightness-symbolic", &title);
                r.add_css_class("tint-amber");
                r.set_tooltip_text(Some(&title));
                scale.set_range(0.01, 1.0);
                let (name, p) = (s.name.clone(), pushing.clone());
                let debounce = nyx_providers::Debounce::new(80);
                scale.connect_value_changed(move |sc| {
                    if !p.get() {
                        let (n, v) = (name.clone(), sc.value());
                        debounce.call(move || act(async move { display::set_brightness(&n, v).await }));
                    }
                });
                let cap = label(&title, "cc-row-sub");
                cap.add_css_class("cc-slider-caption");
                c.append(&cap);
                c.append(&r);
                scales.borrow_mut().push((s.name.clone(), scale, value));
            }
            if !names.is_empty() {
                sliders.append(&c);
            }
            *shown.borrow_mut() = names;
        }
        for (name, scale, value) in scales.borrow().iter() {
            if let Some(v) = st.screens.iter().find(|s| &s.name == name).and_then(|s| s.brightness) {
                pushing.set(true);
                scale.set_value(v.max(0.01));
                pushing.set(false);
                value.set_text(&format!("{}%", (v * 100.0).round()));
            }
        }

        clear(&rest);
        // Scale of the focused screen.
        let focused = st.screens.iter().find(|s| s.focused).map(|s| s.name.clone()).unwrap_or_default();
        rest.append(&group(&format!(
            "Scale of {}",
            if display::is_internal(&focused) { "the laptop screen".to_string() } else { focused }
        )));
        let c = card();
        let seg = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        seg.add_css_class("cc-segmented");
        seg.set_homogeneous(true);
        let mut first: Option<gtk::ToggleButton> = None;
        for &sc in display::SCALES {
            let text = if sc.fract() == 0.0 { format!("{sc:.0}×") } else { format!("{sc}×") };
            let b = gtk::ToggleButton::with_label(&text);
            b.set_tooltip_text(Some(&format!("Everything on this screen at {}% of its size", (sc * 100.0).round())));
            b.set_active((st.scale - sc).abs() < 0.01);
            if let Some(f) = &first {
                b.set_group(Some(f));
            } else {
                first = Some(b.clone());
            }
            b.connect_toggled(move |b| {
                if b.is_active() && (display::live().get().scale - sc).abs() > 0.01 {
                    act(display::set_scale(sc));
                }
            });
            seg.append(&b);
        }
        seg.add_css_class("cc-scale-choice");
        c.append(&seg);
        rest.append(&c);

        // Text size everywhere.
        rest.append(&group("Text size"));
        let c = card();
        let (r, b, scale, value) = ui::slider("font-x-generic-symbolic", "Back to the standard size (12 px)");
        scale.set_range(display::TEXT_MIN as f64, display::TEXT_MAX as f64);
        scale.set_increments(1.0, 1.0);
        scale.set_round_digits(0);
        scale.set_value(st.text_px as f64);
        value.set_text(&format!("{} px", st.text_px));
        r.set_tooltip_text(Some("The bar, menus, apps and terminals together"));
        let debounce = nyx_providers::Debounce::new(700);
        let v2 = value.clone();
        scale.connect_value_changed(move |s| {
            let px = s.value().round() as u32;
            v2.set_text(&format!("{px} px"));
            debounce.call(move || act(display::set_text_size(px)));
        });
        b.connect_clicked(|_| act(display::set_text_size(display::TEXT_DEFAULT)));
        c.append(&r);
        rest.append(&c);

        // The laptop's own screen, while another is plugged in.
        if let Some(lap) = st.internal() {
            if st.external_active() || !lap.enabled {
                let c = card();
                let r = row(
                    "computer-symbolic",
                    "Laptop screen",
                    Some(if lap.enabled { "On" } else { "Off: the other screen shows everything" }),
                );
                let s = gtk::Switch::new();
                s.set_valign(gtk::Align::Center);
                s.set_active(lap.enabled);
                s.connect_state_set(|_, on| {
                    act(display::set_internal(on));
                    glib::Propagation::Proceed
                });
                r.append(&s);
                c.append(&r);
                rest.append(&c);
            }
        }
    });
    root.upcast()
}

// ---------- Volume popover ----------

pub fn volume_popover(anchor: &gtk::Button) {
    let pop = gtk::Popover::new();
    pop.add_css_class("cc-popover");
    let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
    pop.set_child(Some(&col));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    col.append(&body);
    // The microphone: built once (a redraw mid-drag would drop the slider).
    col.append(&group("Microphone"));
    let (mrow, mbutton, mscale, mvalue) = ui::slider("audio-input-microphone-symbolic", "Mute the microphone");
    mrow.add_css_class("tint-coral");
    mscale.set_range(0.0, 1.5);
    mscale.add_mark(1.0, gtk::PositionType::Bottom, None);
    col.append(&mrow);
    let inputs = gtk::Box::new(gtk::Orientation::Vertical, 0);
    col.append(&inputs);
    let pushing = Rc::new(std::cell::Cell::new(false));
    let p = pushing.clone();
    let debounce = nyx_providers::Debounce::new(40);
    mscale.connect_value_changed(move |s| {
        if !p.get() {
            let v = s.value();
            debounce.call(move || act(audio::set_mic_volume(v)));
        }
    });
    mbutton.connect_clicked(|_| act(audio::set_mic_muted(!audio::live().get().mic_muted)));
    let settings = icon_button("emblem-system-symbolic", "Sound settings");
    settings.connect_clicked(|_| ui::open_settings("audio"));
    col.append(&foot(&[settings]));
    let (mr, ins) = (mrow.clone(), inputs.clone());
    bind(&col, audio::live(), move |st| {
        mr.set_visible(!st.inputs.is_empty());
        pushing.set(true);
        mscale.set_value(st.mic_volume);
        pushing.set(false);
        mvalue.set_text(&if st.mic_muted { "Off".into() } else { format!("{}%", (st.mic_volume * 100.0).round()) });
        mbutton.set_icon_name(if st.mic_muted {
            "microphone-sensitivity-muted-symbolic"
        } else {
            "audio-input-microphone-symbolic"
        });
        mbutton.set_tooltip_text(Some(if st.mic_muted {
            "The microphone is muted. Click to unmute"
        } else {
            "Mute the microphone"
        }));
        set_class(&mbutton, "on", st.mic_muted);
        clear(&ins);
        if st.inputs.len() > 1 {
            for i in &st.inputs {
                let r = row(
                    if i.kind == "headset" { "audio-headset-symbolic" } else { "audio-input-microphone-symbolic" },
                    &i.description,
                    None,
                );
                if i.default {
                    r.add_css_class("selected");
                    r.append(&icon("object-select-symbolic", 14));
                }
                let name = i.name.clone();
                on_click(&r, move || {
                    let n = name.clone();
                    act(async move { audio::set_input(&n).await });
                });
                ins.append(&r);
            }
        }
        if !st.recording.is_empty() {
            let l = label(&format!("Recording now: {}", st.recording.join(", ")), "cc-row-sub");
            l.add_css_class("cc-recording");
            ins.append(&l);
        }
    });
    let b = body.clone();
    bind(&col, audio::live(), move |st| {
        clear(&b);
        b.append(&group("Output"));
        for o in &st.outputs {
            let r = row(output_icon(&o.kind), &o.description, None);
            if o.default {
                r.add_css_class("selected");
                r.append(&icon("object-select-symbolic", 14));
            }
            let name = o.name.clone();
            on_click(&r, move || {
                let n = name.clone();
                act(async move { audio::set_output(&n).await });
            });
            b.append(&r);
        }
        if !st.streams.is_empty() {
            b.append(&group("Apps"));
            for s in &st.streams {
                let r = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                r.add_css_class("cc-app-volume");
                let i = gtk::Image::from_gicon(&app_icon(&s.binary, s.icon.as_deref()));
                i.set_pixel_size(20);
                i.set_tooltip_text(Some(&s.app));
                let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.5, 0.01);
                scale.set_draw_value(false);
                scale.set_hexpand(true);
                scale.set_value(s.volume);
                let v = label(&format!("{}%", (s.volume * 100.0).round()), "cc-value");
                v.set_width_chars(4);
                v.set_xalign(1.0);
                let (index, vl) = (s.index, v.clone());
                let debounce = nyx_providers::Debounce::new(40);
                scale.connect_value_changed(move |sc| {
                    let val = sc.value();
                    vl.set_text(&format!("{}%", (val * 100.0).round()));
                    debounce.call(move || act(audio::set_stream_volume(index, val)));
                });
                r.append(&i);
                r.append(&scale);
                r.append(&v);
                b.append(&r);
            }
        }
    });
    pop.set_parent(anchor);
    pop.set_position(gtk::PositionType::Bottom);
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}
