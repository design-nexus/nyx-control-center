//! Small building blocks shared by tiles, detail views and popovers.

use gtk::prelude::*;
use gtk::{gio, glib};
use nyx_widgets::live::{Live, Subscription};
use std::cell::RefCell;
use std::future::Future;

/// Keep `w` drawn from `live` while it's on screen: subscribe when it's mapped, drop
/// the subscription when it's unmapped. A hidden panel therefore runs no providers.
pub fn bind<T: Clone + PartialEq + 'static, W: IsA<gtk::Widget>>(w: &W, live: Live<T>, draw: impl Fn(&T) + Clone + 'static) {
    let holder: std::rc::Rc<RefCell<Option<Subscription>>> = Default::default();
    let (h, l, d) = (holder.clone(), live.clone(), draw.clone());
    w.connect_map(move |_| {
        *h.borrow_mut() = Some(l.subscribe(d.clone()));
    });
    let h = holder;
    w.connect_unmap(move |_| {
        h.borrow_mut().take();
    });
    // Draw once now with what's known, so a tile never shows empty.
    draw(&live.get());
}

/// Run an action in the background; report a failure on stderr (and to `on_error`).
pub fn act(fut: impl Future<Output = Result<(), String>> + 'static) {
    glib::MainContext::default().spawn_local(async move {
        if let Err(e) = fut.await {
            eprintln!("nyx-control-center: {e}");
        }
    });
}

pub fn icon(name: &str, size: i32) -> gtk::Image {
    let i = gtk::Image::from_icon_name(name);
    i.set_pixel_size(size);
    i
}

pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class(class);
    l.set_xalign(0.0);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    // Ask for no width of its own: the panel's width decides, and long text ellipsizes.
    l.set_max_width_chars(1);
    l.set_hexpand(true);
    l
}

/// An icon button with a tooltip (STYLE.md: actions are icons, named in the tooltip).
pub fn icon_button(name: &str, tip: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name(name);
    b.set_tooltip_text(Some(tip));
    b.add_css_class("cc-icon-button");
    b
}

pub fn set_class(w: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        w.add_css_class(class);
    } else {
        w.remove_css_class(class);
    }
}

/// The 32 px rounded square holding a tile's icon; solid accent when on.
pub fn chip(icon_name: &str) -> (gtk::Box, gtk::Image) {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    b.add_css_class("cc-chip");
    b.set_valign(gtk::Align::Center);
    b.set_halign(gtk::Align::Start);
    let i = icon(icon_name, 17);
    i.set_hexpand(true);
    b.append(&i);
    // The icon centres inside the chip; the chip itself doesn't take spare width.
    b.set_hexpand(false);
    (b, i)
}

/// A slider row: icon (a button), the bar, the value.
pub fn slider(icon_name: &str, tip: &str) -> (gtk::Box, gtk::Button, gtk::Scale, gtk::Label) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("cc-slider");
    let button = icon_button(icon_name, tip);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    let value = gtk::Label::new(None);
    value.add_css_class("cc-value");
    value.set_width_chars(4);
    value.set_xalign(1.0);
    row.append(&button);
    row.append(&scale);
    row.append(&value);
    (row, button, scale, value)
}

/// Start a program the way Omarchy's launcher does, detached from the panel.
pub fn launch(args: &[&str]) {
    let mut cmd = std::process::Command::new("setsid");
    cmd.arg("-f");
    let uwsm = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("uwsm-app").is_file()));
    if uwsm {
        cmd.args(["uwsm-app", "--"]);
    }
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    match cmd.spawn() {
        Ok(mut c) => {
            std::thread::spawn(move || c.wait());
        }
        Err(e) => eprintln!("nyx-control-center: couldn't start {}: {e}", args.join(" ")),
    }
}

/// Open a Nyx Settings page (or Settings itself when `page` is empty).
pub fn open_settings(page: &str) {
    if page.is_empty() {
        launch(&["gtk-launch", "io.github.design_nexus.nyx.Settings.desktop"]);
    } else {
        launch(&["nyx-settings", "--section", page]);
    }
}

pub fn file_icon(uri: &str) -> gio::Icon {
    gio::FileIcon::new(&gio::File::for_uri(uri)).upcast()
}

/// A CSS class drawing an event in its calendar's colour, as Nyx Calendar's week view
/// does: the colour at a fifth over the card, a faint edge in it, the title tinted
/// toward it. Rules are added once per colour.
pub fn event_class(hex: &str) -> String {
    use std::collections::BTreeSet;
    thread_local!(static DONE: RefCell<(Option<gtk::CssProvider>, BTreeSet<String>)> = RefCell::default());
    let clean: String = hex.trim_start_matches('#').chars().filter(|c| c.is_ascii_hexdigit()).take(6).collect();
    if clean.len() != 6 {
        return "cc-ev-plain".into();
    }
    let class = format!("cc-ev-{clean}");
    DONE.with(|d| {
        let mut d = d.borrow_mut();
        if !d.1.insert(clean.clone()) {
            return;
        }
        let rules: String = d
            .1
            .iter()
            .map(|h| {
                format!(
                    "window.nyx-control-center .cc-event.cc-ev-{h} {{ background: mix(@theme_card, #{h}, 0.20); border: 1px solid alpha(#{h}, 0.28); }}\n\
                     window.nyx-control-center .cc-event.cc-ev-{h} .cc-event-title {{ color: mix(@theme_text, #{h}, 0.45); }}\n\
                     window.nyx-control-center .cc-event.cc-ev-{h} .cc-event-when {{ color: alpha(mix(@theme_text, #{h}, 0.45), 0.8); }}\n"
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
