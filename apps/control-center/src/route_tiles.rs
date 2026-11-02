//! Tailscale and VPN: their tiles and detail views.
//!
//! - **Tailscale** (what Omarchy's Tailscale panel offers): on or off, this machine's
//!   address, the tailnet's machines (copy an address or name, send files), the exit
//!   node (Mullvad's by country), offering this machine as an exit node, accounts.
//! - **VPN** (what the Privacy Route plugin offers): direct, NordVPN or Tor, only one
//!   carrying traffic at a time; NordVPN's country and settings, Tor's mode, exit country
//!   and a new circuit; and NetworkManager's own VPN connections.

use crate::details::{card, clear, group, guarded_switch, head, on_click, row, scrolled, show_switch};
use crate::tiles::{Ctx, Detail, split, tint, toggle};
use crate::ui::{self, act, bind, icon, icon_button, label, set_class};
use gtk::prelude::*;
use gtk::{gdk, glib};
use nyx_providers::routes::{self, Route, TorMode};
use nyx_providers::{network, tailscale};
use nyx_widgets::layout::{Size, Tile};
use std::cell::RefCell;
use std::rc::Rc;

fn copy(text: &str) {
    if let Some(d) = gdk::Display::default() {
        d.clipboard().set_text(text);
    }
}

/// A small icon button that sits at the end of a row.
fn row_button(icon_name: &str, tip: &str, f: impl Fn() + 'static) -> gtk::Button {
    let b = icon_button(icon_name, tip);
    b.add_css_class("cc-row-button");
    b.set_valign(gtk::Align::Center);
    b.connect_clicked(move |_| f());
    b
}

/// A row that's picked (a check at its end when it's the current one).
fn choice_row(icon_name: &str, title: &str, sub: Option<&str>, current: bool, f: impl Fn() + 'static) -> gtk::Box {
    let r = row(icon_name, title, sub);
    set_class(&r, "selected", current);
    if current {
        r.append(&icon("object-select-symbolic", 14));
    }
    on_click(&r, f);
    r
}

// ---------------------------------------------------------------- Tailscale

pub fn tailscale(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let flip = || {
        let st = tailscale::live().get();
        act(tailscale::set_up(!st.running()));
    };
    let (tile, chip_box, _, status) = if size.cols >= 2 {
        let o = ctx.open.clone();
        split("network-vpn-symbolic", "Tailscale", flip, move || o(Detail::Tailscale))
    } else {
        toggle(size, "network-vpn-symbolic", "Tailscale", flip)
    };
    tint(&tile, "violet");
    let t = tile.clone();
    bind(&tile, tailscale::live(), move |st| {
        let on = st.running();
        set_class(&t, "on", on);
        set_class(&chip_box, "on", on);
        t.set_sensitive(st.installed);
        let online = st.peers.iter().filter(|p| p.online).count();
        status.set_text(&if !st.installed {
            "Not installed".into()
        } else if st.backend == "NeedsLogin" {
            "Needs sign-in".into()
        } else if !on {
            "Off".into()
        } else if let Some(x) = st.exit_node() {
            format!("Exit: {}", x.location.clone().unwrap_or_else(|| x.name.clone()))
        } else {
            format!("{online} online")
        });
        t.set_tooltip_text(Some(&if !st.installed {
            "Tailscale isn't installed".to_string()
        } else if on {
            format!("Tailscale is on in {} ({online} machines online). Click to turn it off", st.tailnet)
        } else {
            "Tailscale is off. Click to connect to your tailnet".to_string()
        }));
    });
    tile.upcast()
}

pub fn tailscale_view(back: Rc<dyn Fn()>) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-detail");
    let (h, sw) = head("Tailscale", back);
    root.append(&h);
    let quiet = guarded_switch(&sw, |on| act(tailscale::set_up(on)));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    let b = body.clone();
    bind(&root, tailscale::live(), move |st| {
        show_switch(&sw, &quiet, st.running());
        sw.set_sensitive(st.installed);
        clear(&b);
        if !st.installed {
            let l =
                label("Tailscale isn't installed. Install it from the Omarchy menu: Install › Service › Tailscale.", "cc-empty");
            l.set_wrap(true);
            b.append(&l);
            return;
        }
        if st.backend == "NeedsLogin" {
            let c = card();
            let r = row("dialog-password-symbolic", "Sign in to Tailscale", Some("Opens the sign-in page in your browser"));
            r.append(&row_button("go-next-symbolic", "Sign in", || ui::launch(&["tailscale", "login"])));
            c.append(&r);
            b.append(&c);
            return;
        }
        if !st.running() {
            return;
        }

        // This machine.
        let c = card();
        let ip = st.me.ips.first().cloned().unwrap_or_default();
        let r = row("computer-symbolic", &st.me.name, Some(&format!("{ip} · {}", st.tailnet)));
        set_class(&r, "selected", true);
        let (ip2, dns) = (ip.clone(), st.me.dns.clone());
        r.append(&row_button("edit-copy-symbolic", "Copy this machine's Tailscale address", move || copy(&ip2)));
        r.append(&row_button("insert-link-symbolic", "Copy this machine's Tailscale name", move || copy(&dns)));
        c.append(&r);
        b.append(&c);

        // Exit node.
        let exits = st.exit_nodes();
        b.append(&group("Exit node"));
        let c = card();
        c.append(&choice_row(
            "network-wired-symbolic",
            "None",
            Some("Internet traffic goes out directly"),
            st.exit_node().is_none(),
            || act(tailscale::set_exit(None)),
        ));
        // Located ones (Mullvad) once per country, the best-placed first; others by name.
        let mut seen_countries: Vec<String> = Vec::new();
        for x in exits.iter().filter(|x| x.online || x.exit_active) {
            if let Some(country) = &x.country {
                if seen_countries.contains(country) && !x.exit_active {
                    continue;
                }
                seen_countries.push(country.clone());
            }
            let title = x.location.clone().unwrap_or_else(|| x.name.clone());
            let ip = x.ips.first().cloned().unwrap_or_default();
            c.append(&choice_row("network-vpn-symbolic", &title, Some(&x.name), x.exit_active, move || {
                let ip = ip.clone();
                act(async move { tailscale::set_exit(Some(&ip)).await });
            }));
        }
        if exits.is_empty() {
            c.append(&row("dialog-information-symbolic", "No exit nodes in this tailnet", None));
        }
        b.append(&c);

        // Machines.
        b.append(&group("Machines"));
        let c = card();
        for p in st.peers.iter().filter(|p| !p.exit_option || !p.location.is_some()) {
            let icon_name = match p.os.as_str() {
                "iOS" | "android" => "phone-symbolic",
                _ => "computer-symbolic",
            };
            let ip = p.ips.first().cloned().unwrap_or_default();
            let sub = format!("{} · {}", if p.online { "online" } else { "offline" }, ip);
            let r = row(icon_name, &p.name, Some(&sub));
            set_class(&r, "cc-dim", !p.online);
            let (ip2, dns, name) = (ip.clone(), p.dns.clone(), p.dns.clone());
            r.append(&row_button("edit-copy-symbolic", &format!("Copy {}'s address ({ip})", p.name), move || copy(&ip2)));
            r.append(&row_button("insert-link-symbolic", &format!("Copy {}'s name ({})", p.name, p.dns), move || copy(&dns)));
            if p.online {
                r.append(&row_button("document-send-symbolic", &format!("Send files to {}", p.name), move || {
                    ui::launch(&["omarchy-tailscale-send", &name]);
                }));
            }
            c.append(&r);
        }
        b.append(&c);

        // Accounts.
        if st.accounts.len() > 1 {
            b.append(&group("Accounts"));
            let c = card();
            for a in &st.accounts {
                let id = a.id.clone();
                c.append(&choice_row("avatar-default-symbolic", &a.tailnet, Some(&a.account), a.active, move || {
                    let id = id.clone();
                    act(async move { tailscale::switch(&id).await });
                }));
            }
            b.append(&c);
        }
    });
    root.upcast()
}

// ---------------------------------------------------------------- VPN

/// NordVPN's switches, as `(name, on)`.
type NordSettings = Vec<(String, bool)>;

fn route_icon(r: Route) -> &'static str {
    match r {
        Route::Direct => "security-low-symbolic",
        Route::Nord => "security-high-symbolic",
        Route::Tor => "security-medium-symbolic",
        Route::Both => "dialog-warning-symbolic",
    }
}

pub fn vpn(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    // The main part goes direct when a route is on, else back to NordVPN.
    let flip = || {
        let st = routes::live().get();
        let nm_vpn = network::live().get().vpn_active().filter(|v| v.id != "tailscale").cloned();
        if let Some(v) = nm_vpn {
            act(async move { network::set_vpn(&v, false).await });
        } else if st.route() == Route::Direct {
            if st.nord.installed {
                act(routes::nord(None));
            } else if st.tor.installed {
                act(routes::tor(TorMode::Lan));
            }
        } else {
            act(routes::direct());
        }
    };
    let (tile, chip_box, chip_icon, status) = if size.cols >= 2 {
        let o = ctx.open.clone();
        split("security-high-symbolic", "VPN", flip, move || o(Detail::Vpn))
    } else {
        toggle(size, "security-high-symbolic", "VPN", flip)
    };
    tint(&tile, "green");
    let t = tile.clone();
    let draw: Rc<dyn Fn()> = Rc::new(move || {
        let st = routes::live().get();
        let net = network::live().get();
        let nm = net.vpn_active().filter(|v| v.id != "tailscale").cloned();
        let route = st.route();
        let on = route != Route::Direct || nm.is_some();
        set_class(&t, "on", on);
        set_class(&chip_box, "on", on);
        set_class(&t, "tint-red", route == Route::Both);
        chip_icon.set_icon_name(Some(route_icon(route)));
        let (text, tip) = match (route, &nm) {
            (Route::Both, _) => ("NordVPN and Tor".to_string(), "NordVPN and Tor are both on. Click to go direct".to_string()),
            (Route::Nord, _) => {
                let place = if st.nord.city.is_empty() {
                    st.nord.country.clone()
                } else {
                    format!("{}, {}", st.nord.country, st.nord.city)
                };
                (format!("NordVPN · {}", st.nord.country), format!("Traffic goes through NordVPN ({place}). Click to go direct"))
            }
            (Route::Tor, _) => {
                let exit = if st.tor.exit_country.is_empty() {
                    "Tor".to_string()
                } else {
                    format!("Tor · {}", st.tor.exit_country.to_uppercase())
                };
                (exit, "Traffic goes through Tor. Click to go direct".to_string())
            }
            (Route::Direct, Some(v)) => (v.name.clone(), format!("{} is connected. Click to disconnect", v.name)),
            (Route::Direct, None) => (
                "Direct".to_string(),
                if st.nord.installed {
                    "Traffic goes out directly. Click to connect NordVPN".to_string()
                } else {
                    "Traffic goes out directly".to_string()
                },
            ),
        };
        status.set_text(&text);
        t.set_tooltip_text(Some(&tip));
    });
    let d = draw.clone();
    bind(&tile, routes::live(), move |_| d());
    bind(&tile, network::live(), move |_| draw());
    tile.upcast()
}

pub fn vpn_view(back: Rc<dyn Fn()>) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.add_css_class("cc-detail");
    let (h, sw) = head("VPN", back);
    sw.set_tooltip_text(Some("Off: go direct (NordVPN and Tor off)"));
    root.append(&h);
    let quiet = guarded_switch(&sw, |on| {
        if on {
            act(routes::nord(None));
        } else {
            act(routes::direct());
        }
    });
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    // NordVPN's countries and settings are slow to ask for: once per view.
    let countries: Rc<RefCell<Option<Vec<String>>>> = Rc::default();
    let settings: Rc<RefCell<Option<NordSettings>>> = Rc::default();
    let show_countries = Rc::new(std::cell::Cell::new(false));
    type Draw = Rc<dyn Fn()>;
    let slot: Rc<RefCell<Option<Draw>>> = Rc::default();
    let b = body.clone();
    let (cs, ss, sc, sl) = (countries.clone(), settings.clone(), show_countries.clone(), slot.clone());
    let draw: Draw = Rc::new(move || {
        let st = routes::live().get();
        let net = network::live().get();
        show_switch(&sw, &quiet, st.route() != Route::Direct);
        sw.set_sensitive(st.nord.installed || st.tor.installed);
        clear(&b);
        let redraw = {
            let sl = sl.clone();
            move || {
                let d = sl.borrow().clone();
                if let Some(d) = d {
                    d();
                }
            }
        };

        if st.route() == Route::Both {
            let c = card();
            let r = row("dialog-warning-symbolic", "NordVPN and Tor are both on", Some("Pick one to keep"));
            r.append(&row_button("security-high-symbolic", "Keep NordVPN", || act(routes::tor_off())));
            r.append(&row_button("security-medium-symbolic", "Keep Tor", || act(routes::nord_off())));
            c.append(&r);
            b.append(&c);
        }

        // Routes.
        let c = card();
        let route = st.route();
        c.append(&choice_row(
            route_icon(Route::Direct),
            "Direct",
            Some("No VPN: traffic goes straight out"),
            route == Route::Direct,
            || act(routes::direct()),
        ));
        if st.nord.installed {
            let sub = if st.nord.connected() {
                format!("{} {} · {}", st.nord.country, st.nord.city, st.nord.server)
            } else {
                st.nord.status.clone()
            };
            c.append(&choice_row(route_icon(Route::Nord), "NordVPN", Some(&sub), st.nord.connected(), || {
                act(routes::nord(None))
            }));
        }
        if st.tor.installed {
            let sub = if st.tor.connected {
                let mode = st.tor.mode.map(|m| m.label()).unwrap_or("On");
                if st.tor.exit_country.is_empty() {
                    mode.to_string()
                } else {
                    format!("{mode} · exit {}", st.tor.exit_country.to_uppercase())
                }
            } else if st.tor.running {
                format!("Starting… {}%", st.tor.bootstrap)
            } else {
                "Off".to_string()
            };
            c.append(&choice_row(route_icon(Route::Tor), "Tor", Some(&sub), st.tor.connected, || act(routes::tor(TorMode::Lan))));
        }
        b.append(&c);
        if !st.nord.installed && !st.tor.installed && net.vpns.iter().all(|v| v.id == "tailscale") {
            let l = label("No VPN set up. NordVPN, Tor (tormarchy) and NetworkManager VPNs show here.", "cc-empty");
            l.set_wrap(true);
            b.append(&l);
        }

        // NordVPN: where, and its settings.
        if st.nord.installed {
            b.append(&group("NordVPN"));
            let c = card();
            let r =
                row("find-location-symbolic", "Location", Some(if st.nord.connected() { &st.nord.country } else { "Fastest" }));
            let sc2 = sc.clone();
            let cs2 = cs.clone();
            let rd = redraw.clone();
            r.append(&row_button(if sc.get() { "pan-up-symbolic" } else { "pan-down-symbolic" }, "Pick a country", move || {
                sc2.set(!sc2.get());
                if cs2.borrow().is_none() {
                    let (cs3, rd2) = (cs2.clone(), rd.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *cs3.borrow_mut() = Some(routes::nord_countries().await);
                        rd2();
                    });
                }
                rd();
            }));
            c.append(&r);
            if sc.get() {
                let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
                list.add_css_class("cc-country-list");
                match cs.borrow().as_ref() {
                    None => list.append(&label("Loading…", "cc-row-sub")),
                    Some(all) => {
                        for country in all {
                            let shown = country.replace('_', " ");
                            let current = st.nord.connected() && st.nord.country == shown;
                            let (cc, sc3) = (country.clone(), sc.clone());
                            list.append(&choice_row("mark-location-symbolic", &shown, None, current, move || {
                                sc3.set(false);
                                let cc = cc.clone();
                                act(async move { routes::nord(Some(&cc)).await });
                            }));
                        }
                    }
                }
                let s = gtk::ScrolledWindow::new();
                s.set_hscrollbar_policy(gtk::PolicyType::Never);
                s.set_min_content_height(200);
                s.set_max_content_height(200);
                s.set_child(Some(&list));
                c.append(&s);
            }
            if ss.borrow().is_none() {
                let (ss2, rd2) = (ss.clone(), redraw.clone());
                *ss.borrow_mut() = Some(Vec::new());
                glib::MainContext::default().spawn_local(async move {
                    *ss2.borrow_mut() = Some(routes::nord_settings().await);
                    rd2();
                });
            }
            for (key, name, desc) in [
                ("Kill Switch", "killswitch", "Block the internet whenever the VPN drops"),
                ("Auto-connect", "autoconnect", "Connect when the computer starts"),
                ("Threat Protection Lite", "threatprotectionlite", "Block ads and malicious sites"),
            ] {
                let on = ss.borrow().as_ref().and_then(|l| l.iter().find(|(k, _)| k == key).map(|(_, v)| *v));
                let Some(on) = on else { continue };
                let r = row("emblem-system-symbolic", key, Some(desc));
                let s = gtk::Switch::new();
                s.set_valign(gtk::Align::Center);
                s.set_active(on);
                let ss2 = ss.clone();
                s.connect_state_set(move |_, v| {
                    if let Some(l) = ss2.borrow_mut().as_mut() {
                        for (k, x) in l.iter_mut() {
                            if k == key {
                                *x = v;
                            }
                        }
                    }
                    act(routes::nord_set(name, v));
                    glib::Propagation::Proceed
                });
                r.append(&s);
                c.append(&r);
            }
            b.append(&c);
        }

        // Tor: how much goes through it, the exit, a new circuit.
        if st.tor.installed {
            b.append(&group("Tor"));
            let c = card();
            for (mode, desc) in [
                (TorMode::Strict, "Everything, the local network cut off"),
                (TorMode::Lan, "Internet traffic; the local network still works"),
                (TorMode::Socks, "Only apps set to use it; works beside NordVPN"),
            ] {
                let current = st.tor.connected && st.tor.mode == Some(mode);
                c.append(&choice_row("security-medium-symbolic", mode.label(), Some(desc), current, move || {
                    act(routes::tor(mode))
                }));
            }
            if st.tor.connected {
                let exit = if st.tor.requested_exit.is_empty() {
                    "Tor chooses".to_string()
                } else {
                    st.tor.requested_exit.to_uppercase()
                };
                let r = row("find-location-symbolic", "Exit", Some(&format!("{exit} · {}", st.tor.exit_ip)));
                r.append(&row_button("view-refresh-symbolic", "New circuit: a new route and exit", || {
                    act(routes::tor_new_circuit())
                }));
                r.append(&row_button("edit-clear-symbolic", "Let Tor choose the exit country", || act(routes::tor_exit("auto"))));
                c.append(&r);
            }
            b.append(&c);
        }

        // NetworkManager's own VPN connections (WireGuard, OpenVPN…).
        let nm: Vec<&network::Vpn> = net.vpns.iter().filter(|v| v.id != "tailscale").collect();
        if !nm.is_empty() {
            b.append(&group("Connections"));
            let c = card();
            for v in nm {
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
        }
    });
    *slot.borrow_mut() = Some(draw.clone());
    let d = draw.clone();
    bind(&root, routes::live(), move |_| d());
    bind(&root, network::live(), move |_| draw());
    root.upcast()
}
