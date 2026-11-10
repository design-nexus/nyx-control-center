//! Operations from the interface: the panel, the providers' actions by name, layout
//! edits and launches. Every action the GTK panel's tiles and detail views took is here,
//! calling the same provider function.

use crate::{App, launch, layout, plugins};
use nyx_providers::{
    audio, backup, bluetooth, brightness, devices, display, dns, home, media, network, omarchy, power, routes, tailscale, vpn,
};
use nyx_ui_link::Request;
use serde_json::{Value, json};
use std::future::Future;
use std::rc::Rc;

fn s(b: &Value, k: &str) -> String {
    b.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}
fn opt_s(b: &Value, k: &str) -> Option<String> {
    b.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from)
}
fn f(b: &Value, k: &str) -> f64 {
    b.get(k).and_then(Value::as_f64).unwrap_or_default()
}
fn on(b: &Value) -> bool {
    b.get("on").and_then(Value::as_bool).unwrap_or_default()
}

/// Run an action and answer when it finishes.
fn act(req: Request, fut: impl Future<Output = Result<(), String>> + 'static) {
    glib::MainContext::default().spawn_local(async move {
        match fut.await {
            Ok(()) => req.ok(json!({})),
            Err(e) => {
                eprintln!("nyx-control-center: {}: {e}", req.op);
                req.err("failed", e)
            }
        }
    });
}

/// Run a query and answer with its result.
fn query<T: serde::Serialize + 'static>(req: Request, fut: impl Future<Output = T> + 'static) {
    glib::MainContext::default().spawn_local(async move {
        let v = fut.await;
        req.ok(serde_json::to_value(v).unwrap_or(Value::Null))
    });
}

fn done(req: Request) {
    req.ok(json!({}));
}

fn open_settings(page: &str) {
    if page.is_empty() {
        launch(&["gtk-launch", "io.github.design_nexus.nyx.Settings.desktop"]);
    } else {
        launch(&["nyx-settings", "--section", page]);
    }
}

pub fn handle(app: &Rc<App>, req: Request) {
    let b = req.body.clone();
    match req.op.as_str() {
        // ---------- Panel ----------
        "Show" => {
            app.show(opt_s(&b, "detail"));
            done(req)
        }
        "Hide" => {
            app.hide();
            done(req)
        }
        "Toggle" => {
            app.toggle(opt_s(&b, "detail"));
            done(req)
        }

        // ---------- Layout ----------
        "layout.save" => {
            let Some(pages) = b.get("pages").and_then(layout::pages_from) else {
                return req.err("invalid", "pages don't parse");
            };
            app.config.borrow_mut().pages = pages;
            if let Err(e) = layout::save(&app.config.borrow()) {
                return req.err("failed", e.to_string());
            }
            app.publish_layout();
            done(req)
        }
        "layout.restore" => {
            app.config.borrow_mut().pages = layout::default_pages();
            let _ = layout::save(&app.config.borrow());
            app.publish_layout();
            done(req)
        }

        // ---------- Plugins ----------
        "plugins.on_screen" => {
            let ids = b
                .get("plugins")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            plugins::set_on_screen(ids);
            done(req)
        }
        "plugins.event" => match plugins::event(
            &s(&b, "tile"),
            &s(&b, "item"),
            &s(&b, "kind"),
            opt_s(&b, "control"),
            b.get("value").cloned().filter(|v| !v.is_null()),
        ) {
            Ok(()) => done(req),
            Err(e) => req.err("invalid", e),
        },
        "plugins.click" => match plugins::run_click(&s(&b, "tile")) {
            Ok(()) => done(req),
            Err(e) => req.err("invalid", e),
        },

        // ---------- Launches ----------
        "open_settings" => {
            open_settings(&s(&b, "page"));
            done(req)
        }
        "open_app" => {
            // Nyx's own apps by desktop id only.
            let id = s(&b, "id");
            if !id.starts_with("io.github.design_nexus.nyx.")
                || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
            {
                return req.err("invalid", "not a Nyx app");
            }
            launch(&["gtk-launch", &format!("{id}.desktop")]);
            done(req)
        }
        "capture" => {
            glib::timeout_add_local_once(std::time::Duration::from_millis(250), || launch(&["nyx-capture", "--capture"]));
            done(req)
        }
        "backup.open" => {
            launch(&["nyx-backup"]);
            done(req)
        }
        "system_menu" => {
            launch(&["omarchy-menu", "toggle", "system"]);
            done(req)
        }
        "power_action" => {
            let argv: &[&str] = match s(&b, "action").as_str() {
                "lock" => &["loginctl", "lock-session"],
                "logout" if nyx_providers::present("uwsm") => &["uwsm", "stop"],
                "logout" => &["hyprctl", "dispatch", "exit"],
                "reboot" => &["systemctl", "reboot"],
                "poweroff" => &["systemctl", "poweroff"],
                _ => return req.err("invalid", "unknown power action"),
            };
            nyx_providers::spawn(argv);
            done(req)
        }
        "open_folder" => {
            let dir = glib::user_special_dir(glib::UserDirectory::Downloads).unwrap_or_else(glib::home_dir);
            launch(&["xdg-open", &dir.display().to_string()]);
            done(req)
        }
        "copy" => {
            let text = s(&b, "text");
            match std::process::Command::new("wl-copy").stdin(std::process::Stdio::piped()).spawn() {
                Ok(mut c) => {
                    if let Some(mut i) = c.stdin.take() {
                        use std::io::Write;
                        let _ = i.write_all(text.as_bytes());
                    }
                    std::thread::spawn(move || c.wait());
                    done(req)
                }
                Err(e) => req.err("failed", e.to_string()),
            }
        }
        "tailscale.login" => {
            launch(&["tailscale", "login"]);
            done(req)
        }
        "tailscale.send" => {
            let dns = s(&b, "dns");
            if dns.is_empty() || dns.starts_with('-') {
                return req.err("invalid", "no machine");
            }
            launch(&["omarchy-tailscale-send", &dns]);
            done(req)
        }

        // ---------- Audio ----------
        "audio.set_volume" => act(req, audio::set_volume(f(&b, "value"))),
        "audio.set_muted" => act(req, audio::set_muted(on(&b))),
        "audio.set_output" => act(req, async move { audio::set_output(&s(&b, "name")).await }),
        "audio.set_stream_volume" => {
            act(req, audio::set_stream_volume(b.get("index").and_then(Value::as_u64).unwrap_or(0) as u32, f(&b, "value")))
        }
        "audio.set_mic_volume" => act(req, audio::set_mic_volume(f(&b, "value"))),
        "audio.set_mic_muted" => act(req, audio::set_mic_muted(on(&b))),
        "audio.set_input" => act(req, async move { audio::set_input(&s(&b, "name")).await }),

        // ---------- Brightness, screens ----------
        // Read back at once, not at the next poll, so the OSD and the sliders follow.
        "brightness.set_screen" => act(req, async move {
            let r = brightness::set_screen(f(&b, "value")).await;
            brightness::refresh(brightness::live().setter());
            r
        }),
        "brightness.set" => act(req, async move {
            let r = brightness::set(&s(&b, "device"), f(&b, "value")).await;
            brightness::refresh(brightness::live().setter());
            r
        }),
        "display.set_brightness" => act(req, async move { display::set_brightness(&s(&b, "screen"), f(&b, "value")).await }),
        "display.set_scale" => act(req, display::set_scale(f(&b, "value"))),
        "display.set_text_size" => act(req, display::set_text_size(f(&b, "value") as u32)),
        "display.set_internal" => act(req, display::set_internal(on(&b))),

        // ---------- Network ----------
        "network.set_wifi" => act(req, network::set_wifi(on(&b))),
        "network.connect" => {
            let ssid = s(&b, "ssid");
            let Some(n) = network::live().get().networks.into_iter().find(|n| n.ssid == ssid) else {
                return req.err("not_found", "no such network");
            };
            let pw = opt_s(&b, "password");
            act(req, async move { network::connect(&n, pw.as_deref()).await })
        }
        "network.disconnect" => act(req, async move { network::disconnect(&s(&b, "uuid")).await }),
        "network.set_metered" => act(req, async move { network::set_metered(&s(&b, "uuid"), on(&b)).await }),
        "network.rescan" => act(req, network::rescan()),
        "network.set_vpn" => {
            let id = s(&b, "id");
            let Some(v) = network::live().get().vpns.into_iter().find(|v| v.id == id) else {
                return req.err("not_found", "no such VPN");
            };
            act(req, async move { network::set_vpn(&v, on(&b)).await })
        }
        "dns.set_dns" => act(req, async move { dns::set_dns(&s(&b, "provider")).await }),
        "dns.set_band" => act(req, async move { dns::set_band(&s(&b, "band")).await }),
        "dns.sign_in" => {
            dns::sign_in();
            done(req)
        }
        "dns.speed_test" => {
            dns::speed_test();
            done(req)
        }
        "dns.share_qr" => {
            dns::share_qr();
            done(req)
        }

        // ---------- Bluetooth ----------
        "bluetooth.set_powered" => act(req, bluetooth::set_powered(on(&b))),
        "bluetooth.set_connected" => act(req, async move { bluetooth::set_connected(&s(&b, "path"), on(&b)).await }),
        "bluetooth.discover" => act(req, bluetooth::discover(on(&b))),
        "bluetooth.pair" => act(req, async move { bluetooth::pair(&s(&b, "address")).await }),
        "bluetooth.forget" => act(req, async move { bluetooth::forget(&s(&b, "address")).await }),
        "devices.ring" => act(req, async move { devices::ring(&s(&b, "id")).await }),
        "devices.send_clipboard" => act(req, async move { devices::send_clipboard(&s(&b, "id")).await }),
        "devices.start" => act(req, devices::start_flux()),

        // ---------- Toggles, power, media, home ----------
        "omarchy.toggle_dnd" => act(req, omarchy::toggle_dnd()),
        "omarchy.toggle_night_light" => act(req, omarchy::toggle_night_light()),
        "omarchy.toggle_keep_awake" => act(req, omarchy::toggle_keep_awake()),
        "mode.cycle" => {
            glib::MainContext::default().spawn_local(async move {
                let r = gio::spawn_blocking(|| nyx_desktop::omarchy::cycle(false).map_err(|e| e.to_string())).await;
                match r {
                    Ok(Ok(())) => req.ok(json!({})),
                    Ok(Err(e)) => req.err("failed", e),
                    Err(_) => req.err("failed", "cycle panicked"),
                }
            });
        }
        "power.set_profile" => match power::Profile::parse(&s(&b, "profile")) {
            Some(p) => act(req, power::set_profile(p)),
            None => req.err("invalid", "unknown profile"),
        },
        "media.command" => {
            let m = s(&b, "method");
            if !["Previous", "PlayPause", "Next"].contains(&m.as_str()) {
                return req.err("invalid", "unknown media command");
            }
            media::command(&m);
            done(req)
        }
        "media.seek" => {
            media::seek(b.get("position").and_then(Value::as_i64).unwrap_or(0));
            done(req)
        }
        "home.toggle" => {
            let id = s(&b, "id");
            let Some(d) = home::live().get().devices.into_iter().find(|d| d.id == id) else {
                return req.err("not_found", "no such device");
            };
            act(req, async move { home::toggle(&d).await })
        }
        "home.call" => act(req, async move {
            home::call(&s(&b, "entity"), &s(&b, "service"), b.get("data").cloned().unwrap_or(Value::Null)).await
        }),
        "backup.action" => {
            let job = opt_s(&b, "job");
            act(req, async move { backup::action(&s(&b, "grant"), &s(&b, "name"), job.as_deref()).await })
        }
        "layout.set_option" => {
            // A tile's own setting (Home's room): page, index, key, value.
            let (page, index) = (
                b.get("page").and_then(Value::as_u64).unwrap_or(0) as usize,
                b.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
            );
            {
                let mut c = app.config.borrow_mut();
                let Some(t) = c.pages.get_mut(page).and_then(|p| p.tiles.get_mut(index)) else {
                    return req.err("not_found", "no such tile");
                };
                t.options.insert(s(&b, "key"), s(&b, "value"));
            }
            let _ = layout::save(&app.config.borrow());
            app.publish_layout();
            done(req)
        }

        // ---------- Tailscale ----------
        "tailscale.set_up" => act(req, tailscale::set_up(on(&b))),
        "tailscale.set_exit" => act(req, async move { tailscale::set_exit(opt_s(&b, "ip").as_deref()).await }),
        "tailscale.set_receiving" => act(req, tailscale::set_receiving(on(&b))),
        "tailscale.switch" => act(req, async move { tailscale::switch(&s(&b, "id")).await }),

        // ---------- Routes and VPNs ----------
        "routes.direct" => act(req, routes::direct()),
        "routes.nord" => act(req, async move { routes::nord(b.get("place").and_then(Value::as_str)).await }),
        "routes.nord_off" => act(req, routes::nord_off()),
        "routes.nord_set" => act(req, async move { routes::nord_set(&s(&b, "name"), on(&b)).await }),
        "routes.nord_pause" => act(req, async move { routes::nord_pause(&s(&b, "duration")).await }),
        "routes.nord_set_value" => act(req, async move { routes::nord_set_value(&s(&b, "name"), &s(&b, "value")).await }),
        "routes.tor" => match routes::TorMode::parse(&s(&b, "mode")) {
            Some(m) => act(req, routes::tor(m)),
            None => req.err("invalid", "unknown Tor mode"),
        },
        "routes.tor_off" => act(req, routes::tor_off()),
        "routes.tor_new_circuit" => act(req, routes::tor_new_circuit()),
        "routes.tor_exit" => act(req, async move { routes::tor_exit(&s(&b, "country")).await }),
        "routes.tor_set_bridges" => act(req, routes::tor_set_bridges(on(&b))),
        "routes.tor_panic" => act(req, routes::tor_panic()),
        "routes.tor_browser" => {
            routes::tor_browser();
            done(req)
        }
        "routes.nord_settings" => query(req, routes::nord_settings()),
        "routes.nord_countries" => query(req, routes::nord_countries()),
        "routes.nord_cities" => query(req, async move { routes::nord_cities(&s(&b, "country")).await }),
        "routes.nord_technology" => query(req, routes::nord_technology()),
        "routes.nord_protocol" => query(req, routes::nord_protocol()),
        "routes.tor_exit_countries" => query(req, routes::tor_exit_countries()),
        "routes.tor_latency" => query(req, routes::tor_latency()),
        "routes.tor_bridges" => query(req, routes::tor_bridges()),
        "vpn.default_service" => query(req, vpn::default_service()),
        "vpn.places" => {
            let id = s(&b, "service");
            let Some(svc) = vpn::live().get().services.into_iter().find(|x| x.id == id) else {
                return query(req, async move {
                    match vpn::default_service().await {
                        Some(svc) if svc.id == id || id.is_empty() => vpn::places(&svc.kind).await,
                        _ => vec![],
                    }
                });
            };
            query(req, async move { vpn::places(&svc.kind).await })
        }
        "vpn.nord_cities" => query(req, async move { vpn::nord_cities(&s(&b, "country")).await }),
        "vpn.nord_all_cities" => query(req, vpn::nord_all_cities()),
        "vpn.config" => req.ok(serde_json::to_value(vpn::load_config()).unwrap_or(Value::Null)),
        "vpn.set_favorite" => {
            let place = vpn::Favorite { arg: s(&b, "arg"), name: s(&b, "name") };
            vpn::set_favorite(&s(&b, "service"), &place, on(&b));
            req.ok(serde_json::to_value(vpn::load_config()).unwrap_or(Value::Null))
        }
        "vpn.set_server" => {
            vpn::set_server(&s(&b, "service"), opt_s(&b, "arg").as_deref());
            req.ok(serde_json::to_value(vpn::load_config()).unwrap_or(Value::Null))
        }
        _ => {
            let op = req.op.clone();
            req.err("unknown_op", op)
        }
    }
}
