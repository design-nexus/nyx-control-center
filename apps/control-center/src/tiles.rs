//! The tiles: views of the providers at Control Center sizes. Each kind says which sizes
//! it comes in; the dashboard places them.

use crate::ui::{self, act, bind, chip, icon, icon_button, label, set_class};
use gtk::prelude::*;
use gtk::{gio, glib};
use nyx_providers::{audio, bluetooth, brightness, media, network, omarchy, power};
use nyx_widgets::layout::{Page, Size, Tile};
use nyx_widgets::live::Live;
use std::rc::Rc;

/// What a tile can ask of the panel.
#[derive(Clone)]
pub struct Ctx {
    /// Open a detail view in place of the dashboard.
    pub open: Rc<dyn Fn(Detail)>,
    /// Close the panel (before an action that takes the screen, like a screenshot).
    pub hide: Rc<dyn Fn()>,
    /// The active workspace's Omarchy mode.
    pub mode: Live<Mode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    Wifi,
    Bluetooth,
    Vpn,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mode {
    pub workspace: i64,
    pub mode: Option<nyx_desktop::omarchy::Mode>,
}

pub struct Kind {
    pub id: &'static str,
    // Shown when adding widgets in edit mode.
    #[allow(dead_code)]
    pub name: &'static str,
    #[allow(dead_code)]
    pub icon: &'static str,
    pub sizes: &'static [Size],
    pub build: fn(&Ctx, Size, &Tile) -> gtk::Widget,
}

pub fn kinds() -> Vec<Kind> {
    use Size as S;
    vec![
        Kind { id: "wifi", name: "Wi-Fi", icon: "network-wireless-symbolic", sizes: &[S::S2X1, S::S1X1], build: wifi },
        Kind {
            id: "bluetooth",
            name: "Bluetooth",
            icon: "bluetooth-symbolic",
            sizes: &[S::S2X1, S::S1X1],
            build: bluetooth_tile,
        },
        Kind { id: "vpn", name: "VPN", icon: "network-vpn-symbolic", sizes: &[S::S2X1, S::S1X1], build: vpn },
        Kind {
            id: "dnd",
            name: "Do not disturb",
            icon: "notifications-disabled-symbolic",
            sizes: &[S::S1X1, S::S2X1],
            build: dnd,
        },
        Kind { id: "mode", name: "Workspace mode", icon: "view-grid-symbolic", sizes: &[S::S1X1, S::S2X1], build: mode },
        Kind {
            id: "sound",
            name: "Sound & brightness",
            icon: "audio-volume-high-symbolic",
            sizes: &[S::S4X1, S::S2X1],
            build: sound,
        },
        Kind {
            id: "media",
            name: "Now playing",
            icon: "audio-x-generic-symbolic",
            sizes: &[S::S2X1, S::S4X1],
            build: media_tile,
        },
        Kind {
            id: "power",
            name: "Power mode",
            icon: "power-profile-balanced-symbolic",
            sizes: &[S::S2X1, S::S4X1],
            build: power_tile,
        },
        Kind { id: "battery", name: "Battery", icon: "battery-good-symbolic", sizes: &[S::S1X1, S::S2X1], build: battery },
        Kind {
            id: "night-light",
            name: "Night light",
            icon: "night-light-symbolic",
            sizes: &[S::S1X1, S::S2X1],
            build: night_light,
        },
        Kind {
            id: "keep-awake",
            name: "Keep awake",
            icon: "preferences-desktop-screensaver-symbolic",
            sizes: &[S::S1X1, S::S2X1],
            build: keep_awake,
        },
        Kind {
            id: "keyboard-light",
            name: "Key light",
            icon: "keyboard-brightness-symbolic",
            sizes: &[S::S1X1, S::S2X1],
            build: keyboard_light,
        },
        Kind { id: "capture", name: "Capture", icon: "camera-photo-symbolic", sizes: &[S::S1X1, S::S2X1], build: capture },
    ]
}

pub fn kind(id: &str) -> Option<Kind> {
    kinds().into_iter().find(|k| k.id == id)
}

/// The first-start layout (the approved mockup, without Home until it has an API).
pub fn default_pages() -> Vec<Page> {
    use Size as S;
    let t = Tile::new;
    vec![
        Page {
            tiles: vec![
                t("wifi", S::S2X1),
                t("bluetooth", S::S2X1),
                t("vpn", S::S2X1),
                t("dnd", S::S1X1),
                t("mode", S::S1X1),
                t("sound", S::S4X1),
                t("media", S::S2X1),
                t("power", S::S2X1),
                t("night-light", S::S1X1),
                t("keep-awake", S::S1X1),
                t("keyboard-light", S::S1X1),
                t("capture", S::S1X1),
            ],
        },
        Page { tiles: vec![t("battery", S::S1X1)] },
    ]
}

// ---------- Tile shapes ----------

fn frame(size: Size) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.add_css_class("cc-tile");
    b.add_css_class(&format!("cc-{}x{}", size.cols, size.rows));
    b
}

/// A toggle: chip and name (1×1: stacked; 2×1: side by side with a status line).
/// Returns the tile, its chip and the status label.
fn toggle(
    size: Size,
    icon_name: &str,
    name: &str,
    on_click: impl Fn() + 'static,
) -> (gtk::Box, gtk::Box, gtk::Image, gtk::Label) {
    let tile = frame(size);
    tile.add_css_class("cc-toggle");
    let (chip_box, chip_icon) = chip(icon_name);
    let title = label(name, "cc-title");
    let status = label("", "cc-status");
    if size.cols == 1 {
        title.set_wrap(true);
        title.set_lines(2);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_valign(gtk::Align::End);
        title.set_vexpand(true);
        tile.append(&chip_box);
        tile.append(&title);
        status.set_visible(false);
    } else {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_vexpand(true);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        text.set_valign(gtk::Align::Center);
        text.append(&title);
        text.append(&status);
        row.append(&chip_box);
        row.append(&text);
        tile.append(&row);
    }
    clickable(&tile, on_click);
    tile.set_tooltip_text(Some(name));
    (tile, chip_box, chip_icon, status)
}

/// A split tile (2×1): the main part toggles, the arrow opens the detail view.
fn split(
    icon_name: &str,
    name: &str,
    on_toggle: impl Fn() + 'static,
    on_more: impl Fn() + 'static,
) -> (gtk::Box, gtk::Box, gtk::Image, gtk::Label) {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tile.add_css_class("cc-tile");
    tile.add_css_class("cc-split");
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    main.add_css_class("cc-split-main");
    main.set_hexpand(true);
    let (chip_box, chip_icon) = chip(icon_name);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    let title = label(name, "cc-title");
    let status = label("", "cc-status");
    text.append(&title);
    text.append(&status);
    main.append(&chip_box);
    main.append(&text);
    clickable(&main, on_toggle);
    let more = gtk::Button::from_icon_name("go-next-symbolic");
    more.add_css_class("cc-split-more");
    more.set_tooltip_text(Some(&format!("{name} details")));
    more.connect_clicked(move |_| on_more());
    tile.append(&main);
    tile.append(&more);
    (tile, chip_box, chip_icon, status)
}

fn clickable(w: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
    let click = gtk::GestureClick::new();
    click.connect_released(move |g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        f();
    });
    w.add_controller(click);
    w.add_css_class("cc-clickable");
}

fn on(w: &impl IsA<gtk::Widget>, state: bool) {
    set_class(w, "on", state);
}

// ---------- Network ----------

fn wifi(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let toggle_wifi = || {
        let st = network::live().get();
        act(network::set_wifi(!st.wifi_on));
    };
    let (tile, chip_box, chip_icon, status) = if size.cols >= 2 {
        let o = ctx.open.clone();
        split("network-wireless-symbolic", "Wi-Fi", toggle_wifi, move || o(Detail::Wifi))
    } else {
        toggle(size, "network-wireless-symbolic", "Wi-Fi", toggle_wifi)
    };
    let t = tile.clone();
    bind(&tile, network::live(), move |st| {
        on(&t, st.wifi_on && st.has_wifi);
        on(&chip_box, st.wifi_on);
        chip_icon.set_icon_name(Some(match (st.wifi_on, st.ssid.as_ref().map(|_| network::bars(st.signal))) {
            (false, _) => "network-wireless-offline-symbolic",
            (true, None) => "network-wireless-no-route-symbolic",
            (true, Some(1)) => "network-wireless-signal-weak-symbolic",
            (true, Some(2)) => "network-wireless-signal-ok-symbolic",
            (true, Some(3)) => "network-wireless-signal-good-symbolic",
            (true, Some(_)) => "network-wireless-signal-excellent-symbolic",
        }));
        status.set_text(&if !st.has_wifi {
            "No Wi-Fi".into()
        } else if !st.wifi_on {
            "Off".into()
        } else {
            st.ssid.clone().map(|s| s.trim().to_string()).unwrap_or_else(|| "Not connected".into())
        });
    });
    tile.upcast()
}

fn bluetooth_tile(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let toggle_bt = || act(bluetooth::set_powered(!bluetooth::live().get().powered));
    let (tile, chip_box, _, status) = if size.cols >= 2 {
        let o = ctx.open.clone();
        split("bluetooth-symbolic", "Bluetooth", toggle_bt, move || o(Detail::Bluetooth))
    } else {
        toggle(size, "bluetooth-symbolic", "Bluetooth", toggle_bt)
    };
    let t = tile.clone();
    bind(&tile, bluetooth::live(), move |st| {
        on(&t, st.powered);
        on(&chip_box, st.powered);
        t.set_sensitive(st.available);
        let connected = st.connected();
        status.set_text(&if !st.available {
            "No adapter".into()
        } else if !st.powered {
            "Off".into()
        } else if connected.is_empty() {
            "On".into()
        } else if connected.len() == 1 {
            connected[0].name.clone()
        } else {
            format!("{} devices", connected.len())
        });
    });
    tile.upcast()
}

fn vpn(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    // The main part turns the active VPN off, or the first one on.
    let toggle_vpn = || {
        let st = network::live().get();
        if let Some(v) = st.vpn_active().cloned() {
            act(async move { network::set_vpn(&v, false).await });
        } else if let Some(v) = st.vpns.first().cloned() {
            act(async move { network::set_vpn(&v, true).await });
        }
    };
    let (tile, chip_box, _, status) = if size.cols >= 2 {
        let o = ctx.open.clone();
        split("network-vpn-symbolic", "VPN", toggle_vpn, move || o(Detail::Vpn))
    } else {
        toggle(size, "network-vpn-symbolic", "VPN", toggle_vpn)
    };
    let t = tile.clone();
    bind(&tile, network::live(), move |st| {
        let active = st.vpn_active();
        on(&t, active.is_some());
        on(&chip_box, active.is_some());
        status.set_text(&match (active, st.vpns.is_empty()) {
            (Some(v), _) => v.name.clone(),
            (None, true) => "None set up".into(),
            (None, false) => "Off".into(),
        });
    });
    tile.upcast()
}

// ---------- Omarchy toggles ----------

fn dnd(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) =
        toggle(size, "notifications-disabled-symbolic", "Do not disturb", || act(omarchy::toggle_dnd()));
    let t = tile.clone();
    bind(&tile, omarchy::live(), move |st| {
        on(&t, st.dnd);
        on(&chip_box, st.dnd);
        status.set_text(if st.dnd { "On" } else { "Off" });
    });
    tile.upcast()
}

fn night_light(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) = toggle(size, "night-light-symbolic", "Night light", || act(omarchy::toggle_night_light()));
    let t = tile.clone();
    bind(&tile, omarchy::live(), move |st| {
        on(&t, st.night_light);
        on(&chip_box, st.night_light);
        status.set_text(if st.night_light { "On" } else { "Off" });
    });
    tile.upcast()
}

fn keep_awake(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) =
        toggle(size, "preferences-desktop-screensaver-symbolic", "Keep awake", || act(omarchy::toggle_keep_awake()));
    let t = tile.clone();
    bind(&tile, omarchy::live(), move |st| {
        on(&t, st.keep_awake);
        on(&chip_box, st.keep_awake);
        status.set_text(if st.keep_awake { "Screen stays on" } else { "Off" });
    });
    tile.upcast()
}

fn mode(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    use nyx_desktop::omarchy::Mode as M;
    let (tile, chip_box, chip_icon, status) = toggle(size, "view-grid-symbolic", "Tiling", || {
        if let Err(e) = nyx_desktop::omarchy::cycle(false) {
            eprintln!("nyx-control-center: {e}");
        }
    });
    on(&chip_box, true);
    let title = find_title(&tile);
    bind(&tile, ctx.mode.clone(), move |m| {
        let mode = m.mode.unwrap_or(M::Tiling);
        if let Some(t) = &title {
            t.set_text(mode.label());
        }
        chip_icon.set_icon_name(Some(match mode {
            M::Tiling => "view-grid-symbolic",
            M::Scrolling => "view-continuous-symbolic",
            M::Floating => "window-new-symbolic",
        }));
        status.set_text(&format!("Workspace {}", m.workspace));
    });
    tile.set_tooltip_text(Some("Workspace mode: click for the next"));
    tile.upcast()
}

/// The title label inside a toggle tile.
fn find_title(tile: &gtk::Box) -> Option<gtk::Label> {
    fn walk(w: &gtk::Widget) -> Option<gtk::Label> {
        if let Some(l) = w.downcast_ref::<gtk::Label>()
            && l.has_css_class("cc-title")
        {
            return Some(l.clone());
        }
        let mut c = w.first_child();
        while let Some(x) = c {
            if let Some(found) = walk(&x) {
                return Some(found);
            }
            c = x.next_sibling();
        }
        None
    }
    walk(tile.upcast_ref())
}

fn capture(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let hide = ctx.hide.clone();
    let (tile, _, _, status) = toggle(size, "camera-photo-symbolic", "Capture", move || {
        hide();
        // Let the panel get out of the picture first.
        glib::timeout_add_local_once(std::time::Duration::from_millis(250), || ui::launch(&["omarchy-capture-screenshot"]));
    });
    status.set_text("Screenshot");
    tile.upcast()
}

fn keyboard_light(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) = toggle(size, "keyboard-brightness-symbolic", "Key light", || {
        // Each click steps up a level, wrapping to off.
        if let Some(k) = brightness::live().get().keyboard {
            let step = (k.value * k.max as f64).round() as u32;
            let next = if step >= k.max { 0 } else { step + 1 };
            let device = k.device.clone();
            act(async move { brightness::set(&device, next as f64 / k.max as f64).await });
        }
    });
    let t = tile.clone();
    bind(&tile, brightness::live(), move |st| {
        t.set_visible(st.keyboard.is_some() || st.screen.is_none());
        let level = st.keyboard.as_ref().map_or(0.0, |k| k.value);
        on(&t, level > 0.0);
        on(&chip_box, level > 0.0);
        status.set_text(&if level > 0.0 { format!("{}%", (level * 100.0).round()) } else { "Off".into() });
    });
    tile.upcast()
}

// ---------- Sound and brightness ----------

fn sound(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let tile = frame(size);
    tile.add_css_class("cc-sliders");
    let (vrow, vbutton, vscale, vvalue) = ui::slider("audio-volume-high-symbolic", "Sound outputs and apps");
    vscale.set_range(0.0, 1.5);
    vscale.add_mark(1.0, gtk::PositionType::Bottom, None);
    let out = icon_button("audio-speakers-symbolic", "Mute");
    vrow.append(&out);
    tile.append(&vrow);
    let (brow, bbutton, bscale, bvalue) = ui::slider("display-brightness-symbolic", "Display settings");
    brow.append(&icon_button("video-display-symbolic", "Displays"));
    tile.append(&brow);

    // Dragging sets the value; a value coming back from the system mustn't fight it.
    let pushing = Rc::new(std::cell::Cell::new(false));
    let p = pushing.clone();
    let debounce = nyx_providers::Debounce::new(40);
    vscale.connect_value_changed(move |s| {
        if p.get() {
            return;
        }
        let v = s.value();
        debounce.call(move || act(audio::set_volume(v)));
    });
    let p = pushing.clone();
    let debounce = nyx_providers::Debounce::new(60);
    bscale.connect_value_changed(move |s| {
        if p.get() {
            return;
        }
        let v = s.value();
        let device = brightness::live().get().screen.map(|l| l.device).unwrap_or_default();
        debounce.call(move || act(async move { brightness::set(&device, v).await }));
    });
    out.connect_clicked(|_| act(audio::set_muted(!audio::live().get().muted)));
    vbutton.connect_clicked(crate::details::volume_popover);
    bbutton.connect_clicked(|_| ui::open_settings("displays"));

    let (p, vs, vv, o, vb) = (pushing.clone(), vscale.clone(), vvalue.clone(), out.clone(), vbutton.clone());
    bind(&tile, audio::live(), move |st| {
        p.set(true);
        vs.set_value(st.volume);
        p.set(false);
        vv.set_text(&if st.muted { "Off".into() } else { format!("{}%", (st.volume * 100.0).round()) });
        vb.set_icon_name(match (st.muted, st.volume) {
            (true, _) => "audio-volume-muted-symbolic",
            (_, v) if v < 0.34 => "audio-volume-low-symbolic",
            (_, v) if v < 0.67 => "audio-volume-medium-symbolic",
            _ => "audio-volume-high-symbolic",
        });
        let kind = st.current().map(|o| o.kind.as_str()).unwrap_or("speaker");
        o.set_icon_name(match kind {
            "headphones" => "audio-headphones-symbolic",
            "hdmi" => "video-display-symbolic",
            "bluetooth" => "bluetooth-symbolic",
            _ => "audio-speakers-symbolic",
        });
        o.set_tooltip_text(Some(&format!(
            "{} ({})",
            if st.muted { "Unmute" } else { "Mute" },
            st.current().map(|o| o.description.as_str()).unwrap_or("output")
        )));
        set_class(&o, "on", st.muted);
    });
    let (p, bs) = (pushing, bscale.clone());
    let br = brow.clone();
    bind(&tile, brightness::live(), move |st| {
        br.set_visible(st.screen.is_some());
        if let Some(s) = &st.screen {
            p.set(true);
            bs.set_value(s.value);
            p.set(false);
            bvalue.set_text(&format!("{}%", (s.value * 100.0).round()));
        }
    });
    tile.upcast()
}

// ---------- Media, power, battery ----------

fn media_tile(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let tile = frame(size);
    tile.add_css_class("cc-media");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_vexpand(true);
    let art = gtk::Image::new();
    art.set_pixel_size(52);
    art.add_css_class("cc-art");
    art.set_valign(gtk::Align::Center);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let title = label("", "cc-title");
    let artist = label("", "cc-status");
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    buttons.add_css_class("cc-media-buttons");
    let prev = icon_button("media-skip-backward-symbolic", "Previous");
    let play = icon_button("media-playback-start-symbolic", "Play");
    let next = icon_button("media-skip-forward-symbolic", "Next");
    prev.connect_clicked(|_| media::command("Previous"));
    play.connect_clicked(|_| media::command("PlayPause"));
    next.connect_clicked(|_| media::command("Next"));
    for b in [&prev, &play, &next] {
        buttons.append(b);
    }
    text.append(&title);
    text.append(&artist);
    text.append(&buttons);
    row.append(&art);
    row.append(&text);
    tile.append(&row);
    let shown_art: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    bind(&tile, media::live(), move |st| {
        let nothing = st.player.is_empty();
        title.set_text(if nothing {
            "Nothing playing"
        } else if st.title.is_empty() {
            "Unknown"
        } else {
            &st.title
        });
        artist.set_text(&st.artist);
        artist.set_visible(!st.artist.is_empty());
        buttons.set_sensitive(!nothing);
        prev.set_sensitive(st.can_previous);
        next.set_sensitive(st.can_next);
        play.set_icon_name(if st.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" });
        play.set_tooltip_text(Some(if st.playing { "Pause" } else { "Play" }));
        if *shown_art.borrow() != st.art || st.art.is_none() {
            match &st.art {
                Some(uri) => art.set_from_gicon(&ui::file_icon(uri)),
                None => art.set_icon_name(Some("audio-x-generic-symbolic")),
            }
            *shown_art.borrow_mut() = st.art.clone();
        }
    });
    tile.upcast()
}

fn power_tile(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    use power::Profile as P;
    let tile = frame(size);
    tile.add_css_class("cc-power");
    let title = label("Power mode", "cc-title");
    title.set_vexpand(true);
    title.set_valign(gtk::Align::Start);
    tile.append(&title);
    let seg = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    seg.add_css_class("cc-segmented");
    seg.set_homogeneous(true);
    let mut buttons: Vec<(P, gtk::ToggleButton)> = Vec::new();
    for (p, icon_name) in [
        (P::PowerSaver, "power-profile-power-saver-symbolic"),
        (P::Balanced, "power-profile-balanced-symbolic"),
        (P::Performance, "power-profile-performance-symbolic"),
    ] {
        let b = gtk::ToggleButton::new();
        b.set_child(Some(&icon(icon_name, 16)));
        b.set_tooltip_text(Some(p.label()));
        if let Some((_, first)) = buttons.first() {
            b.set_group(Some(first));
        }
        b.connect_toggled(move |b| {
            if b.is_active() && power::live().get().profile != Some(p) {
                act(power::set_profile(p));
            }
        });
        seg.append(&b);
        buttons.push((p, b));
    }
    tile.append(&seg);
    bind(&tile, power::live(), move |st| {
        title.set_text(st.profile.map(|p| p.label()).unwrap_or("Power mode"));
        for (p, b) in &buttons {
            b.set_visible(st.profiles.is_empty() || st.profiles.contains(p));
            if st.profile == Some(*p) && !b.is_active() {
                b.set_active(true);
            }
        }
    });
    tile.upcast()
}

fn battery(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, _, chip_icon, status) = toggle(size, "battery-good-symbolic", "Battery", || ui::open_settings("power"));
    let title = find_title(&tile);
    bind(&tile, power::live(), move |st| {
        let Some(b) = &st.battery else {
            if let Some(t) = &title {
                t.set_text("No battery");
            }
            return;
        };
        if let Some(t) = &title {
            t.set_text(&format!("{}%", b.percent.round()));
        }
        let level = match b.percent as u32 {
            0..=10 => "empty",
            11..=30 => "caution",
            31..=60 => "low",
            61..=90 => "good",
            _ => "full",
        };
        chip_icon.set_icon_name(Some(&format!("battery-{level}{}-symbolic", if b.charging { "-charging" } else { "" })));
        status.set_text(&match (b.charging, b.full, b.time) {
            (_, true, _) => "Full".into(),
            (true, _, Some(t)) => format!("{} to full", power::duration(t)),
            (true, _, None) => "Charging".into(),
            (false, _, Some(t)) => format!("{} left", power::duration(t)),
            (false, _, None) => "On battery".into(),
        });
    });
    tile.upcast()
}

/// The icon for a kind of sound output.
pub fn output_icon(kind: &str) -> &'static str {
    match kind {
        "headphones" => "audio-headphones-symbolic",
        "hdmi" => "video-display-symbolic",
        "bluetooth" => "bluetooth-symbolic",
        _ => "audio-speakers-symbolic",
    }
}

/// An app's icon by its program name, if the icon theme has it.
pub fn app_icon(binary: &str, icon_name: Option<&str>) -> gio::Icon {
    let name = icon_name.map(str::to_string).unwrap_or_else(|| binary.to_lowercase());
    gio::ThemedIcon::with_default_fallbacks(&name).upcast()
}
