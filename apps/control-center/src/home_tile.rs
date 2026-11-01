//! The Home tile: a room's lights and devices from Nyx Home, with its scenes. Click a
//! device to switch it; right-click a light for brightness and colour. The room is
//! picked from the header and remembered in the layout file.

use crate::tiles::{Ctx, frame};
use crate::ui::{self, act, bind, icon, icon_button, label, set_class};
use gtk::glib;
use gtk::prelude::*;
use nyx_providers::home;
use nyx_widgets::layout::{Size, Tile};
use std::cell::RefCell;
use std::rc::Rc;

/// A light's colour as a tint: a style class per colour, its rules added once.
fn tint_class(color: Option<[u8; 3]>) -> String {
    use std::collections::BTreeSet;
    thread_local!(static DONE: RefCell<(Option<gtk::CssProvider>, BTreeSet<String>)> = RefCell::default());
    let Some([r, g, b]) = color else { return "cc-tint-warm".into() };
    let class = format!("cc-tint-{r:02x}{g:02x}{b:02x}");
    DONE.with(|d| {
        let mut d = d.borrow_mut();
        if !d.1.insert(class.clone()) {
            return;
        }
        let rules: String =
            d.1.iter()
                .map(|c| {
                    let hex = &c["cc-tint-".len()..];
                    format!(
                        "window.nyx-control-center .cc-device.on.{c} {{ background: mix(@theme_card, #{hex}, 0.2); }}\n\
                     window.nyx-control-center .cc-device.on.{c} image {{ color: #{hex}; }}\n\
                     window.nyx-control-center button.cc-swatch.{c} {{ background: #{hex}; }}\n"
                    )
                })
                .collect();
        let provider = d.0.get_or_insert_with(|| {
            let p = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(&display, &p, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
            }
            p
        });
        provider.load_from_string(&rules);
    });
    class
}

fn device_icon(d: &home::Device) -> &'static str {
    match d.domain.as_str() {
        "light" => "display-brightness-symbolic",
        "fan" => "weather-windy-symbolic",
        "climate" => "weather-few-clouds-symbolic",
        _ => "system-shutdown-symbolic",
    }
}

fn status(d: &home::Device) -> String {
    if !d.available {
        return "Unavailable".into();
    }
    if d.domain == "climate" {
        return match (d.target, d.current) {
            (Some(t), Some(c)) => format!("{t:.1}° · now {c:.1}°"),
            (Some(t), None) => format!("{t:.1}°"),
            _ => if d.on { "On" } else { "Off" }.into(),
        };
    }
    match (d.on, d.brightness) {
        (false, _) => "Off".into(),
        (true, Some(b)) => format!("{}%", (b * 100.0).round()),
        (true, None) => "On".into(),
    }
}

/// Brightness and colour for a light.
fn light_popover(anchor: &gtk::Box, d: &home::Device) {
    let pop = gtk::Popover::new();
    pop.add_css_class("cc-popover");
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.add_css_class("cc-light-popover");
    col.append(&label(&d.name, "cc-title"));
    if d.dimmable {
        let (row, _, scale, value) = ui::slider("display-brightness-symbolic", "Brightness");
        scale.set_range(1.0, 100.0);
        scale.set_value(d.brightness.map_or(100.0, |b| (b * 100.0).max(1.0)));
        value.set_text(&format!("{}%", scale.value().round()));
        let id = d.id.clone();
        let debounce = nyx_providers::Debounce::new(150);
        scale.connect_value_changed(move |s| {
            let v = s.value().round();
            value.set_text(&format!("{v}%"));
            let id = id.clone();
            debounce
                .call(move || act(async move { home::call(&id, "turn_on", serde_json::json!({ "brightness_pct": v })).await }));
        });
        col.append(&row);
    }
    if d.colorable {
        let swatches = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        swatches.set_halign(gtk::Align::Center);
        for (name, rgb) in [
            ("Warm", [255u8, 180, 107]),
            ("White", [255, 244, 229]),
            ("Blue", [91, 156, 255]),
            ("Violet", [157, 123, 255]),
            ("Pink", [255, 111, 174]),
            ("Green", [70, 208, 138]),
        ] {
            let b = gtk::Button::new();
            b.add_css_class("cc-swatch");
            b.add_css_class(&tint_class(Some(rgb)));
            b.set_tooltip_text(Some(name));
            let id = d.id.clone();
            b.connect_clicked(move |_| {
                let id = id.clone();
                act(async move { home::call(&id, "turn_on", serde_json::json!({ "rgb_color": rgb })).await });
            });
            swatches.append(&b);
        }
        col.append(&swatches);
    }
    pop.set_child(Some(&col));
    pop.set_parent(anchor);
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    pop.popup();
}

pub fn build(ctx: &Ctx, size: Size, tile: &Tile) -> gtk::Widget {
    let root = frame(size);
    root.add_css_class("cc-home");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    head.add_css_class("cc-home-head");
    let house = icon("go-home-symbolic", 16);
    house.add_css_class("cc-home-icon");
    head.append(&house);
    let room_button = gtk::MenuButton::new();
    room_button.add_css_class("cc-room");
    room_button.set_hexpand(true);
    room_button.set_halign(gtk::Align::Start);
    room_button.set_tooltip_text(Some("Room"));
    let room_label = label("Home", "cc-room-label");
    room_label.set_hexpand(false);
    room_label.set_max_width_chars(18);
    room_button.set_child(Some(&room_label));
    head.append(&room_button);
    let open = icon_button("go-next-symbolic", "Open Home");
    let hide = ctx.hide.clone();
    open.connect_clicked(move |_| {
        hide();
        ui::launch(&["gtk-launch", "io.github.design_nexus.nyx.Home.desktop"]);
    });
    head.append(&open);
    root.append(&head);
    let devices = gtk::Grid::new();
    devices.set_column_homogeneous(true);
    devices.set_column_spacing(6);
    devices.set_row_spacing(6);
    devices.set_vexpand(true);
    devices.set_valign(gtk::Align::Start);
    root.append(&devices);
    let scenes = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    scenes.add_css_class("cc-scenes");
    root.append(&scenes);
    let note = label("", "cc-status");
    note.set_vexpand(true);
    note.set_wrap(true);
    note.set_valign(gtk::Align::Center);
    root.append(&note);

    let room: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(tile.options.get("room").cloned()));
    let columns = if size.cols >= 4 { 4 } else { 2 };
    type Draw = Rc<dyn Fn(&home::State)>;
    let slot: Rc<RefCell<Option<Draw>>> = Rc::default();
    let set_option = ctx.set_option.clone();
    let draw: Draw = {
        let slot = slot.clone();
        Rc::new(move |st: &home::State| {
            while let Some(c) = devices.first_child() {
                devices.remove(&c);
            }
            while let Some(c) = scenes.first_child() {
                scenes.remove(&c);
            }
            note.set_visible(!st.available);
            devices.set_visible(st.available);
            scenes.set_visible(st.available);
            if !st.available {
                room_label.set_text("Home");
                note.set_text(st.problem.as_deref().unwrap_or("Connect Home Assistant in Home"));
                return;
            }
            // The saved room, else the first one.
            let current = room.borrow().clone().filter(|r| st.rooms.contains(r)).or_else(|| st.rooms.first().cloned());
            room_label.set_text(current.as_deref().unwrap_or("Home"));
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
            for r in &st.rooms {
                let b = gtk::Button::with_label(r);
                b.add_css_class("flat");
                let (room, r2, set_option, slot, rb) =
                    (room.clone(), r.clone(), set_option.clone(), slot.clone(), room_button.clone());
                b.connect_clicked(move |_| {
                    *room.borrow_mut() = Some(r2.clone());
                    set_option("home", "room", &r2);
                    rb.popdown();
                    let d = slot.borrow().clone();
                    if let Some(d) = d {
                        d(&home::live().get());
                    }
                });
                menu.append(&b);
            }
            let pop = gtk::Popover::new();
            pop.add_css_class("cc-popover");
            pop.set_child(Some(&menu));
            room_button.set_popover(Some(&pop));

            for (i, d) in st.in_room(current.as_deref()).into_iter().take(columns * 2).enumerate() {
                let cell = gtk::Box::new(gtk::Orientation::Vertical, 0);
                cell.add_css_class("cc-device");
                cell.add_css_class(&tint_class(d.color));
                set_class(&cell, "on", d.on);
                cell.set_sensitive(d.available);
                let i_ = icon(device_icon(d), 15);
                i_.set_halign(gtk::Align::Start);
                cell.append(&i_);
                cell.append(&label(&d.name, "cc-device-name"));
                cell.append(&label(&status(d), "cc-device-status"));
                cell.set_tooltip_text(Some(&if d.dimmable || d.colorable {
                    format!("{} (right-click for brightness)", d.name)
                } else {
                    d.name.clone()
                }));
                let dev = d.clone();
                let click = gtk::GestureClick::new();
                click.set_button(0);
                let cw = cell.downgrade();
                click.connect_released(move |g, _, _, _| {
                    let Some(cell) = cw.upgrade() else { return };
                    if g.current_button() == 3 && (dev.dimmable || dev.colorable) {
                        light_popover(&cell, &dev);
                    } else if g.current_button() == 1 && dev.domain != "climate" {
                        let d = dev.clone();
                        act(async move { home::toggle(&d).await });
                    }
                });
                cell.add_controller(click);
                devices.attach(&cell, (i % columns) as i32, (i / columns) as i32, 1, 1);
            }
            for sc in st.scenes_in(current.as_deref()).into_iter().take(if columns == 4 { 5 } else { 2 }) {
                let b = gtk::Button::new();
                b.set_child(Some(&label(&sc.name, "cc-scene-label")));
                b.set_hexpand(true);
                b.add_css_class("cc-scene");
                b.set_tooltip_text(Some(&format!("Run {}", sc.name)));
                let id = sc.id.clone();
                b.connect_clicked(move |_| {
                    let id = id.clone();
                    act(async move { home::call(&id, "turn_on", serde_json::Value::Null).await });
                });
                scenes.append(&b);
            }
        })
    };
    *slot.borrow_mut() = Some(draw.clone());
    bind(&root, home::live(), move |st| draw(st));
    root.upcast()
}
