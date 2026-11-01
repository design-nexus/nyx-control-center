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
        Detail::Vpn => vpn(back),
    }
}

/// Back, title, and the feature's switch.
fn head(title: &str, back: Rc<dyn Fn()>) -> (gtk::Box, gtk::Switch) {
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

fn group(title: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(&title.to_uppercase()));
    l.add_css_class("cc-group");
    l.set_xalign(0.0);
    l
}

fn card() -> gtk::Box {
    let c = gtk::Box::new(gtk::Orientation::Vertical, 0);
    c.add_css_class("cc-card");
    c
}

fn row(icon_name: &str, title: &str, sub: Option<&str>) -> gtk::Box {
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

fn on_click(w: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
    let c = gtk::GestureClick::new();
    c.connect_released(move |_, _, _, _| f());
    w.add_controller(c);
    w.add_css_class("cc-clickable");
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let s = gtk::ScrolledWindow::new();
    s.set_hscrollbar_policy(gtk::PolicyType::Never);
    s.set_vexpand(true);
    s.set_propagate_natural_height(true);
    s.set_child(Some(child));
    s
}

fn foot(buttons: &[gtk::Button]) -> gtk::Box {
    let f = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    f.add_css_class("cc-detail-foot");
    f.set_halign(gtk::Align::End);
    for b in buttons {
        f.append(b);
    }
    f
}

fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

/// A switch that reports user flips only (not the ones made to show the state).
fn guarded_switch(sw: &gtk::Switch, f: impl Fn(bool) + 'static) -> Rc<std::cell::Cell<bool>> {
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

fn show_switch(sw: &gtk::Switch, quiet: &std::cell::Cell<bool>, on: bool) {
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
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    let rescan = icon_button("view-refresh-symbolic", "Look for networks");
    rescan.connect_clicked(|_| act(network::rescan()));
    let settings = icon_button("emblem-system-symbolic", "Wi-Fi settings");
    settings.connect_clicked(|_| ui::open_settings("wifi"));
    root.append(&foot(&[rescan, settings]));

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
    let settings = icon_button("list-add-symbolic", "Pair a device (Settings)");
    settings.connect_clicked(|_| ui::open_settings("bluetooth"));
    root.append(&foot(&[settings]));
    let b = body.clone();
    let busy: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    bind(&root, bluetooth::live(), move |st| {
        show_switch(&sw, &quiet, st.powered);
        sw.set_sensitive(st.available);
        clear(&b);
        if !st.available || !st.powered || st.devices.is_empty() {
            let text = if !st.available {
                "No Bluetooth adapter"
            } else if !st.powered {
                "Bluetooth is off"
            } else {
                "No paired devices"
            };
            let l = label(text, "cc-empty");
            l.set_xalign(0.5);
            b.append(&l);
            return;
        }
        b.append(&group("Devices"));
        let c = card();
        for d in &st.devices {
            let sub = match (d.connected, d.battery, busy.borrow().as_deref() == Some(d.path.as_str())) {
                (_, _, true) => Some(if d.connected { "Disconnecting…".to_string() } else { "Connecting…".to_string() }),
                (true, Some(p), _) => Some(format!("Connected · {p}%")),
                (true, None, _) => Some("Connected".into()),
                (false, _, _) => None,
            };
            let r = row(&format!("{}-symbolic", d.icon), &d.name, sub.as_deref());
            set_class(&r, "selected", d.connected);
            let (path, connected, busy) = (d.path.clone(), d.connected, busy.clone());
            on_click(&r, move || {
                *busy.borrow_mut() = Some(path.clone());
                let (p, b2) = (path.clone(), busy.clone());
                glib::MainContext::default().spawn_local(async move {
                    if let Err(e) = bluetooth::set_connected(&p, !connected).await {
                        eprintln!("nyx-control-center: {e}");
                    }
                    b2.borrow_mut().take();
                });
            });
            c.append(&r);
        }
        b.append(&c);
    });
    root.upcast()
}

// ---------- VPN ----------

fn vpn(back: Rc<dyn Fn()>) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-detail");
    let (h, sw) = head("VPN", back);
    sw.set_visible(false);
    root.append(&h);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    let settings = icon_button("emblem-system-symbolic", "Network settings");
    settings.connect_clicked(|_| ui::open_settings("network"));
    root.append(&foot(&[settings]));
    let b = body.clone();
    bind(&root, network::live(), move |st| {
        clear(&b);
        if st.vpns.is_empty() {
            let l = label("No VPNs set up", "cc-empty");
            l.set_xalign(0.5);
            b.append(&l);
            return;
        }
        let c = card();
        for v in &st.vpns {
            let r = row("network-vpn-symbolic", &v.name, Some(if v.active { "Connected" } else { "Off" }));
            set_class(&r, "selected", v.active);
            let s = gtk::Switch::new();
            s.set_valign(gtk::Align::Center);
            s.set_active(v.active);
            let vv = v.clone();
            s.connect_state_set(move |_, on| {
                let v = vv.clone();
                act(async move { network::set_vpn(&v, on).await });
                glib::Propagation::Proceed
            });
            r.append(&s);
            c.append(&r);
        }
        b.append(&c);
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
    let settings = icon_button("emblem-system-symbolic", "Sound settings");
    settings.connect_clicked(|_| ui::open_settings("audio"));
    col.append(&foot(&[settings]));
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
