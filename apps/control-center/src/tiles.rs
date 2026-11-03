//! The tiles: views of the providers at Control Center sizes. Each kind says which sizes
//! it comes in; the dashboard places them.

use crate::ui::{self, act, bind, chip, icon, icon_button, label, set_class};
use gtk::prelude::*;
use gtk::{gio, glib};
use nyx_providers::{audio, bluetooth, brightness, media, network, omarchy, power};
use nyx_widgets::layout::{Page, Size, Tile};
use nyx_widgets::live::Live;
use std::rc::Rc;

/// Save a tile's setting: (widget, key, value).
pub type SetOption = Rc<dyn Fn(&str, &str, &str)>;

/// What a tile can ask of the panel.
#[derive(Clone)]
pub struct Ctx {
    /// Open a detail view in place of the dashboard.
    pub open: Rc<dyn Fn(Detail)>,
    /// Close the panel (before an action that takes the screen, like a screenshot).
    pub hide: Rc<dyn Fn()>,
    /// The active workspace's Omarchy mode.
    pub mode: Live<Mode>,
    /// Remember a tile's setting in the layout file: (widget, key, value).
    pub set_option: SetOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    Wifi,
    Bluetooth,
    Vpn,
    Tailscale,
    Display,
    Battery,
}

impl Detail {
    pub fn parse(s: &str) -> Option<Detail> {
        match s {
            "wifi" => Some(Detail::Wifi),
            "bluetooth" => Some(Detail::Bluetooth),
            "vpn" => Some(Detail::Vpn),
            "tailscale" => Some(Detail::Tailscale),
            "display" => Some(Detail::Display),
            "battery" => Some(Detail::Battery),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mode {
    pub workspace: i64,
    pub mode: Option<nyx_desktop::omarchy::Mode>,
}

pub struct Kind {
    pub id: &'static str,
    pub name: &'static str,
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
        Kind {
            id: "tailscale",
            name: "Tailscale",
            icon: "network-vpn-symbolic",
            sizes: &[S::S2X1, S::S1X1],
            build: crate::route_tiles::tailscale,
        },
        Kind {
            id: "vpn",
            name: "VPN",
            icon: "security-high-symbolic",
            sizes: &[S::S2X1, S::S1X1],
            build: crate::route_tiles::vpn,
        },
        Kind { id: "dnd", name: "Do not disturb", icon: "notifications-disabled-symbolic", sizes: &[S::S1X1], build: dnd },
        Kind { id: "mode", name: "Workspace mode", icon: "view-grid-symbolic", sizes: &[S::S1X1], build: mode },
        Kind {
            id: "sound",
            name: "Sound & brightness",
            icon: "audio-volume-high-symbolic",
            sizes: &[S::S4X2, S::S2X2],
            build: sound,
        },
        Kind { id: "home", name: "Home", icon: "go-home-symbolic", sizes: &[S::S2X3, S::S4X3], build: crate::home_tile::build },
        Kind { id: "upcoming", name: "Up next", icon: "x-office-calendar-symbolic", sizes: &[S::S4X3, S::S2X3], build: upcoming },
        Kind { id: "media", name: "Now playing", icon: "audio-x-generic-symbolic", sizes: &[S::S4X2], build: media_tile },
        Kind {
            id: "power",
            name: "Power mode",
            icon: "power-profile-balanced-symbolic",
            sizes: &[S::S2X1, S::S4X1],
            build: power_tile,
        },
        Kind { id: "battery", name: "Battery", icon: "battery-good-symbolic", sizes: &[S::S1X1, S::S2X1], build: battery },
        Kind {
            id: "microphone",
            name: "Microphone",
            icon: "audio-input-microphone-symbolic",
            sizes: &[S::S1X1],
            build: microphone,
        },
        Kind { id: "night-light", name: "Night light", icon: "night-light-symbolic", sizes: &[S::S1X1], build: night_light },
        Kind {
            id: "keep-awake",
            name: "Keep awake",
            icon: "preferences-desktop-screensaver-symbolic",
            sizes: &[S::S1X1],
            build: keep_awake,
        },
        Kind {
            id: "keyboard-light",
            name: "Keyboard light",
            icon: "keyboard-brightness-symbolic",
            sizes: &[S::S1X1],
            build: keyboard_light,
        },
        Kind { id: "capture", name: "Screenshot", icon: "camera-photo-symbolic", sizes: &[S::S1X1], build: capture },
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
                t("tailscale", S::S2X1),
                t("dnd", S::S1X1),
                t("mode", S::S1X1),
                t("night-light", S::S1X1),
                t("keep-awake", S::S1X1),
                t("sound", S::S4X2),
                t("media", S::S4X2),
                t("power", S::S2X1),
                t("keyboard-light", S::S1X1),
                t("capture", S::S1X1),
            ],
        },
        Page { tiles: vec![t("upcoming", S::S4X3), t("microphone", S::S1X1), t("battery", S::S1X1), t("home", S::S4X3)] },
    ]
}

// ---------- Tile shapes ----------

pub fn frame(size: Size) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.add_css_class("cc-tile");
    b.add_css_class(&format!("cc-{}x{}", size.cols, size.rows));
    b
}

/// A toggle. 1×1: just its icon, centred (the tooltip names it and says its state).
/// 2×1: the icon with its name and a status line. Returns the tile, its chip, the chip's
/// icon and the status label.
pub fn toggle(
    size: Size,
    icon_name: &str,
    name: &str,
    on_click: impl Fn() + 'static,
) -> (gtk::Box, gtk::Box, gtk::Image, gtk::Label) {
    let tile = frame(size);
    tile.add_css_class("cc-toggle");
    let (chip_box, chip_icon) = chip(icon_name);
    let status = label("", "cc-status");
    if size.cols == 1 {
        chip_box.set_halign(gtk::Align::Center);
        chip_box.set_valign(gtk::Align::Center);
        chip_box.set_vexpand(true);
        tile.append(&chip_box);
        status.set_visible(false);
    } else {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        row.set_vexpand(true);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_valign(gtk::Align::Center);
        text.append(&label(name, "cc-title"));
        text.append(&status);
        chip_box.set_valign(gtk::Align::Center);
        row.append(&chip_box);
        row.append(&text);
        tile.append(&row);
    }
    clickable(&tile, on_click);
    tile.set_tooltip_text(Some(name));
    (tile, chip_box, chip_icon, status)
}

/// A split tile (2×1): the main part toggles, the arrow opens the detail view. 1×1 is a
/// plain toggle.
pub fn split(
    icon_name: &str,
    name: &str,
    on_toggle: impl Fn() + 'static,
    on_more: impl Fn() + 'static,
) -> (gtk::Box, gtk::Box, gtk::Image, gtk::Label) {
    let tile = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tile.add_css_class("cc-tile");
    tile.add_css_class("cc-split");
    let main = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    main.add_css_class("cc-split-main");
    main.set_hexpand(true);
    let (chip_box, chip_icon) = chip(icon_name);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
    more.set_tooltip_text(Some(&format!("{name}: networks, devices and settings")));
    more.connect_clicked(move |_| on_more());
    tile.append(&main);
    tile.append(&more);
    (tile, chip_box, chip_icon, status)
}

/// Give a tile its colour (a theme swatch: blue, violet, pink, red, coral, amber,
/// green, teal).
pub fn tint(w: &impl IsA<gtk::Widget>, swatch: &str) {
    w.add_css_class(&format!("tint-{swatch}"));
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
    tint(&tile, "blue");
    let t = tile.clone();
    bind(&tile, network::live(), move |st| {
        t.set_tooltip_text(Some(&match (st.has_wifi, st.wifi_on, &st.ssid) {
            (false, _, _) => "Wi-Fi: this computer has no Wi-Fi".to_string(),
            (true, false, _) => "Wi-Fi is off. Click to turn it on".to_string(),
            (true, true, Some(s)) => format!("Connected to {} ({}% signal). Click to turn Wi-Fi off", s.trim(), st.signal),
            (true, true, None) => "Wi-Fi is on, not connected. Click to turn it off".to_string(),
        }));
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
    tint(&tile, "teal");
    let t = tile.clone();
    bind(&tile, bluetooth::live(), move |st| {
        let names: Vec<String> = st.connected().iter().map(|d| d.name.clone()).collect();
        t.set_tooltip_text(Some(&if !st.available {
            "Bluetooth: no adapter found".to_string()
        } else if !st.powered {
            "Bluetooth is off. Click to turn it on".to_string()
        } else if names.is_empty() {
            "Bluetooth is on, nothing connected. Click to turn it off".to_string()
        } else {
            format!("Connected: {}. Click to turn Bluetooth off", names.join(", "))
        }));
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

// ---------- Omarchy toggles ----------

fn dnd(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) =
        toggle(size, "notifications-disabled-symbolic", "Do not disturb", || act(omarchy::toggle_dnd()));
    tint(&tile, "violet");
    let t = tile.clone();
    bind(&tile, omarchy::live(), move |st| {
        t.set_tooltip_text(Some(if st.dnd {
            "Do not disturb is on: pop-ups stay quiet. Click to allow them"
        } else {
            "Do not disturb: click to silence pop-ups (they still reach the Notification Center)"
        }));
        on(&t, st.dnd);
        on(&chip_box, st.dnd);
        status.set_text(if st.dnd { "On" } else { "Off" });
    });
    tile.upcast()
}

/// Mute the microphone; red while an app is recording.
fn microphone(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, chip_icon, _) = toggle(size, "audio-input-microphone-symbolic", "Microphone", || {
        act(audio::set_mic_muted(!audio::live().get().mic_muted))
    });
    tint(&tile, "coral");
    let t = tile.clone();
    bind(&tile, audio::live(), move |st| {
        let live = !st.mic_muted && !st.recording.is_empty();
        // On (coloured) while it can hear; muted shows the crossed-out icon.
        on(&chip_box, !st.mic_muted);
        set_class(&t, "tint-red", live);
        set_class(&t, "tint-coral", !live);
        chip_icon.set_icon_name(Some(if st.mic_muted {
            "microphone-sensitivity-muted-symbolic"
        } else {
            "audio-input-microphone-symbolic"
        }));
        t.set_tooltip_text(Some(&match (st.mic_muted, st.recording.is_empty()) {
            (true, _) => "The microphone is muted. Click to unmute".to_string(),
            (false, true) => format!("Microphone on ({}%). Click to mute", (st.mic_volume * 100.0).round()),
            (false, false) => format!("Recording now: {}. Click to mute the microphone", st.recording.join(", ")),
        }));
        // Greyed out, not hidden: a hidden tile is never mapped, so it would miss updates.
        t.set_sensitive(!st.inputs.is_empty() || !st.available);
    });
    tile.upcast()
}

fn night_light(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) = toggle(size, "night-light-symbolic", "Night light", || act(omarchy::toggle_night_light()));
    tint(&tile, "coral");
    let t = tile.clone();
    bind(&tile, omarchy::live(), move |st| {
        t.set_tooltip_text(Some(if st.night_light {
            "Night light is on: warmer screen colours. Click to turn it off"
        } else {
            "Night light: click for warmer screen colours"
        }));
        on(&t, st.night_light);
        on(&chip_box, st.night_light);
        status.set_text(if st.night_light { "On" } else { "Off" });
    });
    tile.upcast()
}

fn keep_awake(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let (tile, chip_box, _, status) =
        toggle(size, "preferences-desktop-screensaver-symbolic", "Keep awake", || act(omarchy::toggle_keep_awake()));
    tint(&tile, "amber");
    let t = tile.clone();
    bind(&tile, omarchy::live(), move |st| {
        t.set_tooltip_text(Some(if st.keep_awake {
            "Keep awake is on: the screen won't dim, lock or sleep. Click to turn it off"
        } else {
            "Keep awake: click to stop the screen dimming, locking or sleeping"
        }));
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
    tint(&tile, "green");
    let title = find_title(&tile);
    let t = tile.clone();
    bind(&tile, ctx.mode.clone(), move |m| {
        let mode = m.mode.unwrap_or(M::Tiling);
        if let Some(t) = &title {
            t.set_text(mode.label());
        }
        t.set_tooltip_text(Some(&format!(
            "Workspace {}: {}. Click for the next mode (tiling, scrolling, floating)",
            m.workspace,
            mode.label().to_lowercase()
        )));
        chip_icon.set_icon_name(Some(match mode {
            M::Tiling => "view-grid-symbolic",
            M::Scrolling => "view-continuous-symbolic",
            M::Floating => "window-new-symbolic",
        }));
        status.set_text(&format!("Workspace {}", m.workspace));
    });
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
    tint(&tile, "pink");
    tile.set_tooltip_text(Some("Screenshot: closes this panel, then pick an area, a window or the screen"));
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
    tint(&tile, "teal");
    // One pip per level the light has, lit up to the current one.
    let pips = gtk::Box::new(gtk::Orientation::Horizontal, 3);
    pips.add_css_class("cc-pips");
    pips.set_halign(gtk::Align::Center);
    tile.append(&pips);
    let t = tile.clone();
    bind(&tile, brightness::live(), move |st| {
        t.set_visible(st.keyboard.is_some() || st.screen.is_none());
        let level = st.keyboard.as_ref().map_or(0.0, |k| k.value);
        let (step, max) = st.keyboard.as_ref().map_or((0, 0), |k| ((k.value * k.max as f64).round() as u32, k.max));
        while let Some(c) = pips.first_child() {
            pips.remove(&c);
        }
        for i in 1..=max.min(5) {
            let p = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            p.add_css_class("cc-pip");
            set_class(&p, "lit", i <= step);
            pips.append(&p);
        }
        t.set_tooltip_text(Some(&if level > 0.0 {
            format!("Keyboard light at level {step} of {max}. Click for the next level")
        } else {
            "Keyboard light is off. Click to turn it on".to_string()
        }));
        on(&t, level > 0.0);
        on(&chip_box, level > 0.0);
        status.set_text(&if level > 0.0 { format!("{}%", (level * 100.0).round()) } else { "Off".into() });
    });
    tile.upcast()
}

// ---------- Sound and brightness ----------

fn sound(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let tile = frame(size);
    tile.add_css_class("cc-sliders");
    // Each slider gets half the tile, so the space above and below them is equal.
    tile.set_homogeneous(true);
    let (vrow, vbutton, vscale, vvalue) = ui::slider("audio-volume-high-symbolic", "Sound: outputs and each app's volume");
    vrow.add_css_class("tint-blue");
    vscale.set_range(0.0, 1.5);
    let out = icon_button("audio-speakers-symbolic", "Mute");
    vrow.append(&out);
    tile.append(&vrow);
    let (brow, bbutton, bscale, bvalue) =
        ui::slider("display-brightness-symbolic", "Screens: brightness of each, scale, text size");
    brow.add_css_class("tint-amber");
    bscale.set_range(0.01, 1.0);
    let screens = icon_button("video-display-symbolic", "Screens: brightness of each, scale, text size");
    brow.append(&screens);
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
        debounce.call(move || act(brightness::set_screen(v)));
    });
    out.connect_clicked(|_| act(audio::set_muted(!audio::live().get().muted)));
    vbutton.connect_clicked(crate::details::volume_popover);
    let o = ctx.open.clone();
    bbutton.connect_clicked(move |_| o(Detail::Display));
    let o = ctx.open.clone();
    screens.connect_clicked(move |_| o(Detail::Display));

    let (p, vs, vv, o, vb, vr) = (pushing.clone(), vscale.clone(), vvalue.clone(), out.clone(), vbutton.clone(), vrow.clone());
    bind(&tile, audio::live(), move |st| {
        p.set(true);
        vs.set_value(st.volume);
        p.set(false);
        let pct = (st.volume * 100.0).round();
        vv.set_text(&if st.muted { "Off".into() } else { format!("{pct}%") });
        // Past 100 % is amplified (it can distort): say so in colour.
        set_class(&vv, "loud", !st.muted && pct > 100.0);
        vb.set_icon_name(match (st.muted, st.volume) {
            (true, _) => "audio-volume-muted-symbolic",
            (_, v) if v < 0.34 => "audio-volume-low-symbolic",
            (_, v) if v < 0.67 => "audio-volume-medium-symbolic",
            _ => "audio-volume-high-symbolic",
        });
        let name = st.current().map(|o| o.description.clone()).unwrap_or_else(|| "the speakers".into());
        let kind = st.current().map(|o| o.kind.as_str()).unwrap_or("speaker");
        o.set_icon_name(output_icon(kind));
        o.set_tooltip_text(Some(&if st.muted { format!("{name} is muted. Click to unmute") } else { format!("Mute {name}") }));
        vr.set_tooltip_text(Some(&format!("Volume {pct}% on {name}")));
        set_class(&o, "on", st.muted);
    });
    let (p, bs, br) = (pushing, bscale.clone(), brow.clone());
    bind(&tile, brightness::live(), move |st| {
        br.set_visible(st.screen.is_some());
        if let Some(s) = &st.screen {
            p.set(true);
            bs.set_value(s.value.max(0.01));
            p.set(false);
            let pct = (s.value * 100.0).round();
            bvalue.set_text(&format!("{pct}%"));
            br.set_tooltip_text(Some(&format!("Brightness of {}: {pct}%", s.device)));
        }
    });
    tile.upcast()
}

// ---------- Media, power, battery ----------

/// "3:07", "1:02:45".
fn clock(us: i64) -> String {
    let s = (us / 1_000_000).max(0);
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// Now playing: the cover, what it is with a progress bar you can drag, and big
/// controls on the right.
fn media_tile(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let tile = frame(size);
    tile.add_css_class("cc-media");
    tint(&tile, "pink");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_vexpand(true);
    // The cover in an 80 px box; without one, a small note in the box.
    let art_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    art_box.add_css_class("cc-art-box");
    art_box.set_size_request(80, 80);
    art_box.set_valign(gtk::Align::Center);
    art_box.set_overflow(gtk::Overflow::Hidden);
    let art = gtk::Image::new();
    art.set_hexpand(true);
    art.set_halign(gtk::Align::Center);
    art.set_valign(gtk::Align::Center);
    art_box.append(&art);
    // The image centres inside; the box itself takes no spare width.
    art_box.set_hexpand(false);
    art_box.set_halign(gtk::Align::Start);
    row.append(&art_box);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let title = label("", "cc-media-title");
    let artist = label("", "cc-media-artist");
    text.append(&title);
    text.append(&artist);
    // Progress: drag to seek (when the player allows it).
    let progress = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    progress.add_css_class("cc-media-progress");
    let elapsed = gtk::Label::new(None);
    elapsed.add_css_class("cc-media-time");
    let bar = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    bar.set_draw_value(false);
    bar.set_hexpand(true);
    let total = gtk::Label::new(None);
    total.add_css_class("cc-media-time");
    progress.append(&elapsed);
    progress.append(&bar);
    progress.append(&total);
    text.append(&progress);
    row.append(&text);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    buttons.add_css_class("cc-media-buttons");
    buttons.set_valign(gtk::Align::Center);
    let prev = icon_button("media-skip-backward-symbolic", "Previous track");
    let play = icon_button("media-playback-start-symbolic", "Play");
    play.add_css_class("cc-play");
    let next = icon_button("media-skip-forward-symbolic", "Next track");
    prev.connect_clicked(|_| media::command("Previous"));
    play.connect_clicked(|_| media::command("PlayPause"));
    next.connect_clicked(|_| media::command("Next"));
    for b in [&prev, &play, &next] {
        b.set_valign(gtk::Align::Center);
        buttons.append(b);
    }
    row.append(&buttons);
    tile.append(&row);

    // Seeking: only a drag by the user moves the player.
    let pushing = Rc::new(std::cell::Cell::new(false));
    let p = pushing.clone();
    let debounce = nyx_providers::Debounce::new(120);
    bar.connect_value_changed(move |b| {
        if p.get() {
            return;
        }
        let st = media::live().get();
        let to = (b.value() * st.length as f64) as i64;
        debounce.call(move || media::seek(to));
    });
    // The bar and times move on their own while playing (players don't announce it).
    let draw_progress = {
        let (bar, elapsed, total, progress, pushing) =
            (bar.clone(), elapsed.clone(), total.clone(), progress.clone(), pushing.clone());
        move |st: &media::State| {
            progress.set_visible(st.length > 0);
            if st.length > 0 {
                let now = st.position_now();
                pushing.set(true);
                bar.set_value(now as f64 / st.length as f64);
                pushing.set(false);
                bar.set_sensitive(st.can_seek);
                elapsed.set_text(&clock(now));
                total.set_text(&clock(st.length));
            }
        }
    };
    let tick: Rc<std::cell::RefCell<Option<glib::SourceId>>> = Rc::default();
    let (dp, tk) = (draw_progress.clone(), tick.clone());
    tile.connect_map(move |_| {
        let dp = dp.clone();
        *tk.borrow_mut() = Some(glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
            let st = media::live().get();
            if st.playing {
                dp(&st);
            }
            glib::ControlFlow::Continue
        }));
    });
    tile.connect_unmap(move |_| {
        if let Some(id) = tick.borrow_mut().take() {
            id.remove();
        }
    });

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
        let who: Vec<String> = [st.artist.clone(), player_name(&st.player)].into_iter().filter(|x| !x.is_empty()).collect();
        artist.set_text(&who.join(" · "));
        artist.set_visible(!who.is_empty());
        buttons.set_sensitive(!nothing);
        prev.set_sensitive(st.can_previous);
        next.set_sensitive(st.can_next);
        play.set_icon_name(if st.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" });
        play.set_tooltip_text(Some(if st.playing { "Pause" } else { "Play" }));
        draw_progress(st);
        if *shown_art.borrow() != st.art || st.art.is_none() {
            match &st.art {
                Some(uri) => {
                    art.set_from_gicon(&ui::file_icon(uri));
                    art.set_pixel_size(80);
                }
                None => {
                    art.set_icon_name(Some("audio-x-generic-symbolic"));
                    art.set_pixel_size(28);
                }
            }
            set_class(&art_box, "empty", st.art.is_none());
            *shown_art.borrow_mut() = st.art.clone();
        }
    });
    tile.upcast()
}

/// A player's name from its bus name: `org.mpris.MediaPlayer2.firefox.instance_1_5` is
/// "Firefox", `…spotify` "Spotify", a Nyx app's "Music".
pub fn player_name(bus: &str) -> String {
    let rest = bus.strip_prefix("org.mpris.MediaPlayer2.").unwrap_or(bus);
    let name = if rest.starts_with("io.github.") || rest.starts_with("org.") || rest.starts_with("com.") {
        rest.split('.').next_back().unwrap_or(rest)
    } else {
        rest.split('.').next().unwrap_or(rest)
    };
    let mut c = name.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// When an event is, said briefly: "in 12 min", "14:00", "Tomorrow 09:00", "Wed",
/// "All day".
pub fn event_when(e: &nyx_providers::calendar::Event, now: i64) -> String {
    let local = |t: i64| glib::DateTime::from_unix_local(t).ok();
    let (Some(start), Some(today)) = (local(e.starts), local(now)) else { return String::new() };
    let days = (start.ymd().0 - today.ymd().0) * 366 + start.day_of_year() - today.day_of_year();
    if e.all_day {
        return match days {
            ..=0 => "All day".into(),
            1 => "Tomorrow".into(),
            2..=6 => start.format("%A").map(|s| s.to_string()).unwrap_or_default(),
            _ => start.format("%b %-d").map(|s| s.to_string()).unwrap_or_default(),
        };
    }
    if e.starts <= now {
        return "Now".into();
    }
    let mins = (e.starts - now) / 60;
    if mins < 60 {
        return format!("in {} min", mins.max(1));
    }
    let time = start.format("%H:%M").map(|s| s.to_string()).unwrap_or_default();
    match days {
        0 => time,
        1 => format!("Tomorrow {time}"),
        2..=6 => format!("{} {time}", start.format("%a").map(|s| s.to_string()).unwrap_or_default()),
        _ => start.format("%b %-d").map(|s| s.to_string()).unwrap_or_default(),
    }
}

/// Up next: the coming events from Nyx Calendar.
fn upcoming(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    use nyx_providers::calendar;
    let tile = frame(size);
    tile.add_css_class("cc-upcoming");
    tint(&tile, "red");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    head.add_css_class("cc-upcoming-head");
    let i = icon("x-office-calendar-symbolic", 14);
    head.append(&i);
    head.append(&label("Up next", "cc-title"));
    tile.append(&head);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    list.set_vexpand(true);
    tile.append(&list);
    let rows = if size.cols >= 4 { 4 } else { 3 };
    clickable(&tile, || ui::launch(&["gtk-launch", "io.github.design_nexus.nyx.Calendar.desktop"]));
    let t = tile.clone();
    bind(&tile, calendar::live(), move |st| {
        while let Some(c) = list.first_child() {
            list.remove(&c);
        }
        let now = glib::real_time() / 1_000_000;
        if !st.available || st.events.is_empty() {
            let l = label(if st.available { "Nothing coming up" } else { "Open Calendar once to see events here" }, "cc-status");
            l.set_vexpand(true);
            l.set_valign(gtk::Align::Center);
            l.set_xalign(0.5);
            list.append(&l);
            t.set_tooltip_text(Some("Open Calendar"));
            return;
        }
        // Each event as Calendar's week view draws it: a block in its calendar's colour,
        // the title, and when (and where) below it.
        for e in st.events.iter().take(rows) {
            // One line: the title, and when on the right.
            let block = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            block.add_css_class("cc-event");
            block.add_css_class(&ui::event_class(e.color.as_deref().unwrap_or("#8a8f98")));
            block.append(&label(&e.title, "cc-event-title"));
            let when = gtk::Label::new(Some(&event_when(e, now)));
            when.add_css_class("cc-event-when");
            when.set_xalign(1.0);
            block.append(&when);
            block.set_tooltip_text(Some(&if e.location.is_empty() {
                format!("{} ({})", e.title, e.calendar)
            } else {
                format!("{} at {} ({})", e.title, e.location, e.calendar)
            }));
            list.append(&block);
        }
        t.set_tooltip_text(Some("Open Calendar"));
    });
    tile.upcast()
}

/// Power mode: just its three icons.
fn power_tile(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    use power::Profile as P;
    let tile = frame(size);
    tile.add_css_class("cc-power");
    let seg = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    seg.add_css_class("cc-segmented");
    seg.set_homogeneous(true);
    seg.set_vexpand(true);
    let mut buttons: Vec<(P, gtk::ToggleButton)> = Vec::new();
    for (p, icon_name, swatch, tip) in [
        (P::PowerSaver, "power-profile-power-saver-symbolic", "green", "Power saver: longer battery life, slower"),
        (P::Balanced, "power-profile-balanced-symbolic", "blue", "Balanced: the everyday setting"),
        (P::Performance, "power-profile-performance-symbolic", "coral", "Performance: fastest, uses more power and runs warmer"),
    ] {
        let b = gtk::ToggleButton::new();
        b.set_child(Some(&icon(icon_name, 18)));
        b.set_tooltip_text(Some(tip));
        tint(&b, swatch);
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
        for (p, b) in &buttons {
            b.set_visible(st.profiles.is_empty() || st.profiles.contains(p));
            if st.profile == Some(*p) && !b.is_active() {
                b.set_active(true);
            }
        }
    });
    tile.upcast()
}

fn battery(ctx: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let tile = frame(size);
    tile.add_css_class("cc-battery");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    row.set_valign(gtk::Align::Center);
    row.set_vexpand(true);
    let img = icon("battery-good-symbolic", 18);
    let pct = gtk::Label::new(None);
    pct.add_css_class("cc-battery-pct");
    row.append(&img);
    row.append(&pct);
    tile.append(&row);
    let o = ctx.open.clone();
    clickable(&tile, move || o(Detail::Battery));
    let t = tile.clone();
    bind(&tile, power::live(), move |st| {
        let Some(b) = &st.battery else {
            pct.set_text("");
            t.set_tooltip_text(Some("No battery"));
            return;
        };
        pct.set_text(&format!("{}%", b.percent.round()));
        let level = match b.percent as u32 {
            0..=10 => "empty",
            11..=30 => "caution",
            31..=60 => "low",
            61..=90 => "good",
            _ => "full",
        };
        img.set_icon_name(Some(&format!("battery-{level}{}-symbolic", if b.charging { "-charging" } else { "" })));
        set_class(&t, "tint-red", b.percent < 15.0 && !b.charging);
        set_class(&t, "tint-green", b.charging || b.full);
        let state = match (b.charging, b.full, b.time) {
            (_, true, _) => "full".to_string(),
            (true, _, Some(time)) => format!("charging, full in {}", power::duration(time)),
            (true, _, None) => "charging".to_string(),
            (false, _, Some(time)) => format!("{} left", power::duration(time)),
            (false, _, None) => "on battery".to_string(),
        };
        t.set_tooltip_text(Some(&format!("Battery {}%, {state}. Click for details", b.percent.round())));
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

#[cfg(test)]
mod tests {
    #[test]
    fn player_names() {
        assert_eq!(super::player_name("org.mpris.MediaPlayer2.firefox.instance_1_511"), "Firefox");
        assert_eq!(super::player_name("org.mpris.MediaPlayer2.spotify"), "Spotify");
        assert_eq!(super::player_name("org.mpris.MediaPlayer2.io.github.design_nexus.nyx.Music"), "Music");
    }
}
