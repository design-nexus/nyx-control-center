//! Print what every provider reports, then stop. Read-only.
//! `cargo run -p nyx-providers --example dump`
use nyx_providers::*;

fn main() {
    let main = glib::MainLoop::new(None, false);
    let _subs = (
        network::live().subscribe(|s| println!("network: {s:?}")),
        bluetooth::live().subscribe(|s| println!("bluetooth: {s:?}")),
        audio::live().subscribe(|s| println!("audio: {s:?}")),
        power::live().subscribe(|s| println!("power: {s:?}")),
        brightness::live().subscribe(|s| println!("brightness: {s:?}")),
        media::live().subscribe(|s| println!("media: {s:?}")),
        omarchy::live().subscribe(|s| println!("omarchy: {s:?}")),
    );
    let m = main.clone();
    glib::timeout_add_seconds_local_once(2, move || m.quit());
    main.run();
}
