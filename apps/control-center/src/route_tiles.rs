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
use nyx_providers::{network, tailscale, vpn};
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

/// A star at the end of a server's row: add it to (or take it off) the favourites.
fn star_button(svc_id: &str, place: vpn::Favorite, starred: bool, redraw: impl Fn() + 'static) -> gtk::Button {
    let tip =
        if starred { format!("Remove {} from favourites", place.name) } else { format!("Add {} to favourites", place.name) };
    let b = row_button(if starred { "starred-symbolic" } else { "non-starred-symbolic" }, &tip, {
        let id = svc_id.to_string();
        move || {
            vpn::set_favorite(&id, &place, !starred);
            redraw();
        }
    });
    b.add_css_class("cc-star");
    set_class(&b, "on", starred);
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
    // Built once (a redraw mid-typing would drop it): filters the machines.
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Find a machine"));
    search.add_css_class("cc-search");
    root.append(&search);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&scrolled(&body));
    // The exit-node country whose cities are open.
    let open_country: Rc<RefCell<Option<String>>> = Rc::default();
    type Draw = Rc<dyn Fn(&tailscale::State)>;
    let slot: Rc<RefCell<Option<Draw>>> = Rc::default();
    let redraw = {
        let slot = slot.clone();
        move || {
            let d = slot.borrow().clone();
            if let Some(d) = d {
                d(&tailscale::live().get());
            }
        }
    };
    let rd = redraw.clone();
    search.connect_search_changed(move |_| rd());
    let (b, q, oc) = (body.clone(), search.clone(), open_country.clone());
    let draw: Draw = Rc::new(move |st: &tailscale::State| {
        let sw = sw.clone();
        let quiet = quiet.clone();
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
        // Files other machines send here (Omarchy's Taildrop receiver).
        let r = row("document-save-symbolic", "Receive files", Some("Files sent to this machine are saved to Downloads"));
        r.append(&row_button("folder-download-symbolic", "Open Downloads", || {
            let dir =
                glib::user_special_dir(glib::UserDirectory::Downloads).unwrap_or_else(|| glib::home_dir().join("Downloads"));
            ui::launch(&["xdg-open", &dir.to_string_lossy()]);
        }));
        let s = gtk::Switch::new();
        s.set_valign(gtk::Align::Center);
        s.set_active(st.receiving);
        s.set_tooltip_text(Some("Save files sent here (Taildrop), now and at login"));
        s.connect_state_set(|_, on| {
            act(tailscale::set_receiving(on));
            glib::Propagation::Proceed
        });
        r.append(&s);
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
        let pick = |x: &tailscale::Machine| {
            let ip = x.ips.first().cloned().unwrap_or_default();
            move || {
                let ip = ip.clone();
                act(async move { tailscale::set_exit(Some(&ip)).await });
            }
        };
        // Your own exit nodes by name; located ones (Mullvad) by country, opening to cities.
        for x in exits.iter().filter(|x| x.country.is_none() && (x.online || x.exit_active)) {
            c.append(&choice_row("computer-symbolic", &x.name, Some("Your exit node"), x.exit_active, pick(x)));
        }
        let mut countries: Vec<String> = exits.iter().filter_map(|x| x.country.clone()).collect();
        countries.sort();
        countries.dedup();
        for country in countries {
            let here: Vec<&&tailscale::Machine> =
                exits.iter().filter(|x| x.country.as_ref() == Some(&country) && x.online).collect();
            let active = exits.iter().any(|x| x.country.as_ref() == Some(&country) && x.exit_active);
            let open = oc.borrow().as_deref() == Some(country.as_str());
            let r = row("mark-location-symbolic", &country, Some(&format!("{} servers", here.len())));
            set_class(&r, "selected", active);
            r.append(&icon(if open { "pan-up-symbolic" } else { "pan-down-symbolic" }, 14));
            let (oc2, rd2, cc) = (oc.clone(), redraw.clone(), country.clone());
            on_click(&r, move || {
                let mut o = oc2.borrow_mut();
                *o = if o.as_deref() == Some(cc.as_str()) { None } else { Some(cc.clone()) };
                drop(o);
                rd2();
            });
            c.append(&r);
            if open {
                // One server per city: the first online one.
                let mut cities: Vec<String> = Vec::new();
                for x in &here {
                    let city = x.location.clone().unwrap_or_default();
                    if cities.contains(&city) {
                        continue;
                    }
                    cities.push(city.clone());
                    let r = choice_row(
                        "mark-location-symbolic",
                        city.split(", ").nth(1).unwrap_or(&city),
                        Some(&x.name),
                        x.exit_active,
                        pick(x),
                    );
                    r.add_css_class("cc-indent");
                    c.append(&r);
                }
            }
        }
        if exits.is_empty() {
            c.append(&row("dialog-information-symbolic", "No exit nodes in this tailnet", None));
        }
        b.append(&c);

        // Machines.
        b.append(&group("Machines"));
        let c = card();
        let query = q.text().to_lowercase();
        for p in st.peers.iter().filter(|p| !p.exit_option || p.location.is_none()).filter(|p| {
            query.is_empty()
                || p.name.to_lowercase().contains(&query)
                || p.dns.contains(&query)
                || p.ips.iter().any(|i| i.contains(&query))
        }) {
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
    *slot.borrow_mut() = Some(draw.clone());
    bind(&root, tailscale::live(), move |st| draw(st));
    root.upcast()
}

// ---------------------------------------------------------------- VPN

/// NordVPN's switches, as `(name, on)`.
type NordSettings = Vec<(String, bool)>;
/// Tor's exit countries, as `(code, relays)`.
type ExitCountries = Vec<(String, usize)>;

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
            (Route::Both, _) => {
                (format!("{} and Tor", st.nord.name), format!("{} and Tor are both on. Click to go direct", st.nord.name))
            }
            (Route::Nord, _) => {
                let place = if st.nord.city.is_empty() {
                    st.nord.country.clone()
                } else {
                    format!("{}, {}", st.nord.country, st.nord.city)
                };
                let name = &st.nord.name;
                let text = if st.nord.country.is_empty() { name.clone() } else { format!("{name} · {}", st.nord.country) };
                (text, format!("Traffic goes through {name} ({place}). Click to go direct"))
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
                    format!("Traffic goes out directly. Click to connect {}", st.nord.name)
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
    sw.set_tooltip_text(Some("Off: go direct (the VPN and Tor off)"));
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
    let setup = icon_button("list-add-symbolic", "Set up a VPN, or choose the default one (Settings › Online Accounts)");
    setup.connect_clicked(|_| ui::open_settings("online-accounts"));
    root.append(&crate::details::foot(&[setup]));
    // The default VPN's places and NordVPN's settings are slow to ask for: once per view.
    let countries: Rc<RefCell<Option<Vec<vpn::Place>>>> = Rc::default();
    // Every NordVPN city, for searching (cached a week).
    let all_cities: Rc<RefCell<Option<Vec<vpn::Place>>>> = Rc::default();
    // Built once and moved into the list on each redraw, so typing survives a refresh.
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Find a location"));
    search.add_css_class("cc-search");
    search.add_css_class("cc-search-inline");
    // For screenshots: the location list open, with a search typed.
    let test_search = std::env::var("NYX_CC_TEST_SEARCH").ok();
    let settings: Rc<RefCell<Option<NordSettings>>> = Rc::default();
    let show_countries = Rc::new(std::cell::Cell::new(test_search.is_some()));
    if let Some(q) = &test_search {
        search.set_text(q);
    }
    let open_country: Rc<RefCell<Option<String>>> = Rc::default();
    let cities: Rc<RefCell<std::collections::HashMap<String, Vec<String>>>> = Rc::default();
    let technology: Rc<RefCell<Option<String>>> = Rc::default();
    let protocol: Rc<RefCell<Option<String>>> = Rc::default();
    let tor_exits: Rc<RefCell<Option<ExitCountries>>> = Rc::default();
    let show_exits = Rc::new(std::cell::Cell::new(false));
    let latency: Rc<RefCell<Option<String>>> = Rc::default();
    let bridges: Rc<RefCell<Option<(bool, bool)>>> = Rc::default();
    type Draw = Rc<dyn Fn()>;
    let slot: Rc<RefCell<Option<Draw>>> = Rc::default();
    let b = body.clone();
    let (cs, ss, sc, sl) = (countries.clone(), settings.clone(), show_countries.clone(), slot.clone());
    let srch = search.clone();
    let allc = all_cities.clone();
    {
        // Typing filters the list (the view redraws, the box keeps its text and focus).
        let sl2 = slot.clone();
        search.connect_search_changed(move |e| {
            let d = sl2.borrow().clone();
            if let Some(d) = d {
                d();
            }
            e.grab_focus();
            e.set_position(-1);
        });
    }
    let proto = protocol.clone();
    let (oc, ci, tech, tx, sx, lat, br) = (
        open_country.clone(),
        cities.clone(),
        technology.clone(),
        tor_exits.clone(),
        show_exits.clone(),
        latency.clone(),
        bridges.clone(),
    );
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
            let r = row("dialog-warning-symbolic", &format!("{} and Tor are both on", st.nord.name), Some("Pick one to keep"));
            r.append(&row_button("security-high-symbolic", &format!("Keep {}", st.nord.name), || act(routes::tor_off())));
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
                let place = format!("{} {}", st.nord.country, st.nord.city).trim().to_string();
                if st.nord.server.is_empty() { place } else { format!("{place} · {}", st.nord.server) }
            } else {
                st.nord.status.clone()
            };
            c.append(&choice_row(route_icon(Route::Nord), &st.nord.name, Some(&sub), st.nord.connected(), || {
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
            let l = label("No VPN set up. Add one with the button below (Settings › Online Accounts).", "cc-empty");
            l.set_wrap(true);
            b.append(&l);
        }

        // The default VPN: where, and (NordVPN) its settings.
        if st.nord.installed {
            let kind = st.nord.service.as_ref().map(|s| s.kind.clone());
            let is_nord = kind == Some(vpn::Kind::Nord);
            let has_places = st.nord.service.as_ref().is_some_and(|s| s.has_locations());
            b.append(&group(&st.nord.name));
            let c = card();
            let svc_id = st.nord.service.as_ref().map(|s| s.id.clone()).unwrap_or_default();
            let conf = vpn::service_config(&svc_id);
            let default_name = conf
                .server
                .as_ref()
                .map(|a| {
                    conf.favorites.iter().find(|f| &f.arg == a).map(|f| f.name.clone()).unwrap_or_else(|| a.replace('_', " "))
                })
                .unwrap_or_else(|| "Fastest".to_string());
            let r = row(
                "find-location-symbolic",
                "Location",
                Some(if st.nord.connected() { &st.nord.country } else { &default_name }),
            );
            let sc2 = sc.clone();
            let cs2 = cs.clone();
            let rd = redraw.clone();
            let k2 = kind.clone();
            let allc2 = allc.clone();
            r.append(&row_button(if sc.get() { "pan-up-symbolic" } else { "pan-down-symbolic" }, "Pick a country", move || {
                sc2.set(!sc2.get());
                if cs2.borrow().is_none()
                    && let Some(k) = k2.clone()
                {
                    let (cs3, rd2) = (cs2.clone(), rd.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *cs3.borrow_mut() = Some(vpn::places(&k).await);
                        rd2();
                    });
                }
                // NordVPN's cities, for the search, gathered while you look.
                if k2 == Some(vpn::Kind::Nord) && allc2.borrow().is_none() {
                    *allc2.borrow_mut() = Some(Vec::new());
                    let (a3, rd2) = (allc2.clone(), rd.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *a3.borrow_mut() = Some(vpn::nord_all_cities().await);
                        rd2();
                    });
                }
                rd();
            }));
            r.set_visible(has_places);
            c.append(&r);
            // Favourite servers: connect with a click; the marked one (or Fastest) is
            // where the tile and the switch connect.
            if has_places && !conf.favorites.is_empty() {
                let mut options: Vec<(Option<String>, String)> = vec![(None, "Fastest".to_string())];
                options.extend(conf.favorites.iter().map(|f| (Some(f.arg.clone()), f.name.clone())));
                for (arg, name) in options {
                    let is_default = conf.server == arg;
                    let icon_name = if arg.is_none() { "find-location-symbolic" } else { "starred-symbolic" };
                    let r = row(icon_name, &name, if is_default { Some("Default") } else { None });
                    r.add_css_class("cc-favorite");
                    let mark = row_button(
                        if is_default { "radio-checked-symbolic" } else { "radio-symbolic" },
                        if is_default { "Where the VPN connects by default" } else { "Connect here by default" },
                        {
                            let (id, a, rd2) = (svc_id.clone(), arg.clone(), redraw.clone());
                            move || {
                                vpn::set_server(&id, a.as_deref());
                                rd2();
                            }
                        },
                    );
                    mark.add_css_class("cc-default-mark");
                    set_class(&mark, "on", is_default);
                    r.append(&mark);
                    if let Some(a) = &arg {
                        let fav = vpn::Favorite { arg: a.clone(), name: name.clone() };
                        r.append(&star_button(&svc_id, fav, true, redraw.clone()));
                    }
                    let a = arg.clone();
                    crate::details::on_click(&r, move || {
                        let a = a.clone();
                        // "" is the fastest, whatever the default.
                        act(async move { routes::nord(Some(a.as_deref().unwrap_or(""))).await });
                    });
                    c.append(&r);
                }
            }
            if sc.get() && has_places {
                // Whatever opened the list, its places (and NordVPN's cities) load.
                if cs.borrow().is_none()
                    && let Some(k) = kind.clone()
                {
                    *cs.borrow_mut() = Some(Vec::new());
                    let (cs3, rd2) = (cs.clone(), redraw.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *cs3.borrow_mut() = Some(vpn::places(&k).await);
                        rd2();
                    });
                }
                if is_nord && allc.borrow().is_none() {
                    *allc.borrow_mut() = Some(Vec::new());
                    let (a3, rd2) = (allc.clone(), redraw.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *a3.borrow_mut() = Some(vpn::nord_all_cities().await);
                        rd2();
                    });
                }
                if let Some(p) = srch.parent()
                    && let Ok(pb) = p.downcast::<gtk::Box>()
                {
                    pb.remove(&srch);
                }
                c.append(&srch);
                let query = srch.text().to_lowercase();
                let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
                list.add_css_class("cc-country-list");
                match cs.borrow().as_ref() {
                    None => list.append(&label("Loading…", "cc-row-sub")),
                    Some(all) if all.is_empty() => list.append(&label("Loading…", "cc-row-sub")),
                    Some(all) => {
                        for place in all.iter().filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query)) {
                            let country = &place.arg;
                            let shown = place.name.clone();
                            let current = st.nord.connected() && st.nord.country == shown;
                            if place.city {
                                // Mullvad's cities, under their country.
                                let (cc, sc3) = (place.arg.clone(), sc.clone());
                                let r = choice_row("mark-location-symbolic", &shown, None, current, move || {
                                    sc3.set(false);
                                    let cc = cc.clone();
                                    act(async move { routes::nord(Some(&cc)).await });
                                });
                                r.add_css_class("cc-indent");
                                let starred = conf.favorites.iter().any(|f| f.arg == place.arg);
                                r.append(&star_button(
                                    &svc_id,
                                    vpn::Favorite { arg: place.arg.clone(), name: shown.clone() },
                                    starred,
                                    redraw.clone(),
                                ));
                                list.append(&r);
                                continue;
                            }
                            let (cc, sc3) = (country.clone(), sc.clone());
                            let r = choice_row("mark-location-symbolic", &shown, None, current, move || {
                                sc3.set(false);
                                let cc = cc.clone();
                                act(async move { routes::nord(Some(&cc)).await });
                            });
                            let starred = conf.favorites.iter().any(|f| &f.arg == country);
                            r.append(&star_button(
                                &svc_id,
                                vpn::Favorite { arg: country.clone(), name: shown.clone() },
                                starred,
                                redraw.clone(),
                            ));
                            let open = oc.borrow().as_deref() == Some(country.as_str());
                            let (oc2, ci2, rd2, cc2) = (oc.clone(), ci.clone(), redraw.clone(), country.clone());
                            if !is_nord {
                                list.append(&r);
                                continue;
                            }
                            r.append(&row_button(
                                if open { "pan-up-symbolic" } else { "pan-down-symbolic" },
                                &format!("Cities in {shown}"),
                                move || {
                                    let now_open = oc2.borrow().as_deref() == Some(cc2.as_str());
                                    *oc2.borrow_mut() = if now_open { None } else { Some(cc2.clone()) };
                                    if !now_open && !ci2.borrow().contains_key(&cc2) {
                                        let (ci3, rd3, cc3) = (ci2.clone(), rd2.clone(), cc2.clone());
                                        glib::MainContext::default().spawn_local(async move {
                                            let list = routes::nord_cities(&cc3).await;
                                            ci3.borrow_mut().insert(cc3, list);
                                            rd3();
                                        });
                                    }
                                    rd2();
                                },
                            ));
                            list.append(&r);
                            if open {
                                match ci.borrow().get(country.as_str()) {
                                    None => list.append(&label("Loading…", "cc-row-sub")),
                                    Some(cs) => {
                                        for city in cs {
                                            let name = city.replace('_', " ");
                                            let here = current && st.nord.city == name;
                                            let starred = conf.favorites.iter().any(|f| &f.arg == city);
                                            let fav = vpn::Favorite { arg: city.clone(), name: format!("{name}, {shown}") };
                                            let (city, sc3) = (city.clone(), sc.clone());
                                            let r = choice_row("mark-location-symbolic", &name, None, here, move || {
                                                sc3.set(false);
                                                let c = city.clone();
                                                act(async move { routes::nord(Some(&c)).await });
                                            });
                                            r.add_css_class("cc-indent");
                                            r.append(&star_button(&svc_id, fav, starred, redraw.clone()));
                                            list.append(&r);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Searching NordVPN also finds cities ("Atlanta, United States").
                if is_nord && !query.is_empty() {
                    match allc.borrow().as_ref() {
                        Some(cities) if !cities.is_empty() => {
                            let hits: Vec<&vpn::Place> =
                                cities.iter().filter(|p| p.name.to_lowercase().contains(&query)).take(40).collect();
                            if !hits.is_empty() {
                                let cap = label("Cities", "cc-group");
                                list.append(&cap);
                            }
                            for p in hits {
                                let here = st.nord.connected() && p.name.starts_with(&st.nord.city) && !st.nord.city.is_empty();
                                let starred = conf.favorites.iter().any(|f| f.arg == p.arg);
                                let (arg, sc3) = (p.arg.clone(), sc.clone());
                                let r = choice_row("mark-location-symbolic", &p.name, None, here, move || {
                                    sc3.set(false);
                                    let a = arg.clone();
                                    act(async move { routes::nord(Some(&a)).await });
                                });
                                r.append(&star_button(
                                    &svc_id,
                                    vpn::Favorite { arg: p.arg.clone(), name: p.name.clone() },
                                    starred,
                                    redraw.clone(),
                                ));
                                list.append(&r);
                            }
                        }
                        _ => list.append(&label("Gathering cities to search…", "cc-row-sub")),
                    }
                }
                let s = gtk::ScrolledWindow::new();
                s.set_hscrollbar_policy(gtk::PolicyType::Never);
                s.set_min_content_height(200);
                s.set_max_content_height(200);
                s.set_child(Some(&list));
                c.append(&s);
            }
            if is_nord && ss.borrow().is_none() {
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
                let Some(on) = on.filter(|_| is_nord) else { continue };
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
            // Pause (it comes back by itself), and the technology.
            if is_nord && st.nord.connected() {
                let r = row("media-playback-pause-symbolic", "Pause", Some("Reconnects by itself afterwards"));
                let seg = gtk::Box::new(gtk::Orientation::Horizontal, 2);
                seg.add_css_class("cc-segmented");
                seg.add_css_class("cc-small-choice");
                for d in ["5m", "15m", "1h", "24h"] {
                    let b = gtk::Button::with_label(d);
                    b.set_tooltip_text(Some(&format!("Pause NordVPN for {d}")));
                    b.connect_clicked(move |_| act(routes::nord_pause(d)));
                    seg.append(&b);
                }
                seg.set_valign(gtk::Align::Center);
                r.append(&seg);
                c.append(&r);
            }
            if is_nord && tech.borrow().is_none() {
                *tech.borrow_mut() = Some(String::new());
                let (t2, rd2) = (tech.clone(), redraw.clone());
                glib::MainContext::default().spawn_local(async move {
                    *t2.borrow_mut() = Some(routes::nord_technology().await.unwrap_or_default());
                    rd2();
                });
            }
            let current = tech.borrow().clone().unwrap_or_default();
            if !current.is_empty() {
                let r = row("network-vpn-symbolic", "Technology", Some("NordLynx is the fastest"));
                let seg = gtk::Box::new(gtk::Orientation::Horizontal, 2);
                seg.add_css_class("cc-segmented");
                seg.add_css_class("cc-small-choice");
                for (id, name) in [("NORDLYNX", "NordLynx"), ("OPENVPN", "OpenVPN"), ("NORDWHISPER", "NordWhisper")] {
                    let b = gtk::ToggleButton::with_label(name);
                    b.set_active(current == id);
                    let (t2, rd2) = (tech.clone(), redraw.clone());
                    b.connect_clicked(move |_| {
                        *t2.borrow_mut() = Some(id.to_string());
                        act(routes::nord_set_value("technology", id));
                        rd2();
                    });
                    seg.append(&b);
                }
                seg.set_valign(gtk::Align::Center);
                r.append(&seg);
                c.append(&r);
            }
            // UDP or TCP: OpenVPN only (NordLynx and NordWhisper choose their own).
            if current == "OPENVPN" {
                if proto.borrow().is_none() {
                    *proto.borrow_mut() = Some(String::new());
                    let (p2, rd2) = (proto.clone(), redraw.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *p2.borrow_mut() = Some(routes::nord_protocol().await.unwrap_or_default());
                        rd2();
                    });
                }
                let cur = proto.borrow().clone().unwrap_or_default();
                let r = row(
                    "network-transmit-receive-symbolic",
                    "Protocol",
                    Some("TCP gets through stricter networks; UDP is faster"),
                );
                let seg = gtk::Box::new(gtk::Orientation::Horizontal, 2);
                seg.add_css_class("cc-segmented");
                seg.add_css_class("cc-small-choice");
                for (id, arg) in [("UDP", "udp"), ("TCP", "tcp")] {
                    let b = gtk::ToggleButton::with_label(id);
                    b.set_active(cur == id);
                    let (p2, rd2) = (proto.clone(), redraw.clone());
                    b.connect_clicked(move |_| {
                        *p2.borrow_mut() = Some(id.to_string());
                        act(routes::nord_set_value("protocol", arg));
                        rd2();
                    });
                    seg.append(&b);
                }
                seg.set_valign(gtk::Align::Center);
                r.append(&seg);
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
                // Exit country (a slow list to build: on demand).
                let r = row("mark-location-symbolic", "Exit country", Some("Pick where your traffic leaves Tor"));
                let (sx2, tx2, rd2) = (sx.clone(), tx.clone(), redraw.clone());
                r.append(&row_button(
                    if sx.get() { "pan-up-symbolic" } else { "pan-down-symbolic" },
                    "Choose the exit country",
                    move || {
                        sx2.set(!sx2.get());
                        if sx2.get() && tx2.borrow().is_none() {
                            *tx2.borrow_mut() = Some(Vec::new());
                            let (tx3, rd3) = (tx2.clone(), rd2.clone());
                            glib::MainContext::default().spawn_local(async move {
                                *tx3.borrow_mut() = Some(routes::tor_exit_countries().await);
                                rd3();
                            });
                        }
                        rd2();
                    },
                ));
                c.append(&r);
                if sx.get() {
                    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    let exits = tx.borrow().clone().unwrap_or_default();
                    if exits.is_empty() {
                        list.append(&label("Asking Tor about its exits… (up to a minute)", "cc-row-sub"));
                    }
                    for (cc, n) in exits {
                        let current = st.tor.requested_exit.eq_ignore_ascii_case(&cc);
                        let code = cc.clone();
                        list.append(&choice_row(
                            "mark-location-symbolic",
                            &cc.to_uppercase(),
                            Some(&format!("{n} exit relays")),
                            current,
                            move || {
                                let code = code.clone();
                                act(async move { routes::tor_exit(&code).await });
                            },
                        ));
                    }
                    let s = gtk::ScrolledWindow::new();
                    s.set_hscrollbar_policy(gtk::PolicyType::Never);
                    s.set_min_content_height(180);
                    s.set_max_content_height(180);
                    s.set_child(Some(&list));
                    c.append(&s);
                }
                // Latency through the circuit, measured on request.
                let sub = lat.borrow().clone().unwrap_or_else(|| "Measure how slow the circuit is".into());
                let r = row("network-transmit-receive-symbolic", "Latency", Some(&sub));
                let (lat2, rd2) = (lat.clone(), redraw.clone());
                r.append(&row_button("view-refresh-symbolic", "Measure the latency now", move || {
                    *lat2.borrow_mut() = Some("Measuring…".into());
                    rd2();
                    let (lat3, rd3) = (lat2.clone(), rd2.clone());
                    glib::MainContext::default().spawn_local(async move {
                        *lat3.borrow_mut() = Some(
                            routes::tor_latency()
                                .await
                                .map(|ms| format!("{ms:.0} ms"))
                                .unwrap_or_else(|| "Couldn't measure".into()),
                        );
                        rd3();
                    });
                }));
                c.append(&r);
                let r = row("web-browser-symbolic", "Tor browser", Some("A browser that can reach nothing but Tor"));
                r.append(&row_button("go-next-symbolic", "Open a Tor-only browser", routes::tor_browser));
                c.append(&r);
            }
            // Bridges, for networks that block Tor.
            if br.borrow().is_none() {
                *br.borrow_mut() = Some((false, false));
                let (br2, rd2) = (br.clone(), redraw.clone());
                glib::MainContext::default().spawn_local(async move {
                    *br2.borrow_mut() = Some(routes::tor_bridges().await.unwrap_or((false, false)));
                    rd2();
                });
            }
            let (on, configured) = br.borrow().unwrap_or((false, false));
            let r = row(
                "network-wired-symbolic",
                "Bridges",
                Some(if configured { "For networks that block Tor" } else { "None set up (tormarchy bridge set)" }),
            );
            let s = gtk::Switch::new();
            s.set_valign(gtk::Align::Center);
            s.set_active(on);
            s.set_sensitive(configured);
            let br2 = br.clone();
            s.connect_state_set(move |_, v| {
                *br2.borrow_mut() = Some((v, true));
                act(routes::tor_set_bridges(v));
                glib::Propagation::Proceed
            });
            r.append(&s);
            c.append(&r);
            if st.tor.running {
                let r = row("dialog-warning-symbolic", "Panic", Some("Stuck offline? Remove every Tor rule now"));
                r.append(&row_button("process-stop-symbolic", "Remove every Tor firewall rule now", || act(routes::tor_panic())));
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
