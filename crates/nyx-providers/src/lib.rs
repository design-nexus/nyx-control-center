//! System data for Nyx widgets. Each provider is a [`nyx_widgets::live::Live`] value
//! shared by every view in the process: it starts listening when the first view
//! subscribes and stops when the last one goes, so a hidden panel costs nothing.
//!
//! | Provider | Source | Changes from |
//! | --- | --- | --- |
//! | [`network`] | NetworkManager (`nmcli`), Tailscale | `nmcli monitor` |
//! | [`bluetooth`] | BlueZ (D-Bus) | D-Bus signals |
//! | [`audio`] | PipeWire through `pactl` | `pactl subscribe` |
//! | [`power`] | power-profiles-daemon, UPower (D-Bus) | D-Bus signals |
//! | [`brightness`] | `brightnessctl` | read while shown, every 2 s |
//! | [`home`] | Nyx Home's service (`Home.Devices1`) | read every 3 s while shown |
//! | [`media`] | MPRIS players (D-Bus) | D-Bus signals |
//! | [`omarchy`] | Omarchy's toggles and state files | file monitors |
//! | [`tailscale`] | `tailscale status --json` | read every 5 s while shown |
//! | [`routes`] | NordVPN and Tor (`nordvpn`, `tormarchy`) | read every 4 s while shown |
//! | [`dns`] | Omarchy's DNS provider (`omarchy-dns`) | read when shown |

pub mod audio;
pub mod bluetooth;
pub mod brightness;
pub mod display;
pub mod dns;
pub mod home;
pub mod media;
pub mod network;
pub mod omarchy;
pub mod power;
pub mod routes;
pub mod tailscale;

use gio::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// Run a command and return what it printed, or `None` if it failed.
pub async fn output(args: &[&str]) -> Option<String> {
    let argv: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
    let p = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE).ok()?;
    let (out, _) = p.communicate_utf8_future(None).await.ok()?;
    p.is_successful().then(|| out.map(|s| s.to_string()).unwrap_or_default())
}

/// Run a command for its effect; `true` if it succeeded. `stdin` is written to it
/// (a password for `nmcli --ask`, say) so it never shows in a process list.
pub async fn run(args: &[&str], stdin: Option<&str>) -> Result<(), String> {
    let argv: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
    let mut flags = gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_PIPE;
    if stdin.is_some() {
        flags |= gio::SubprocessFlags::STDIN_PIPE;
    }
    let p = gio::Subprocess::newv(&argv, flags).map_err(|e| e.to_string())?;
    let (_, err) = p.communicate_utf8_future(stdin.map(String::from)).await.map_err(|e| e.to_string())?;
    if p.is_successful() {
        Ok(())
    } else {
        Err(err.map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).unwrap_or_else(|| format!("{} failed", args[0])))
    }
}

/// A program is on PATH.
pub fn present(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

/// Fire and forget.
pub fn spawn(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    glib::MainContext::default().spawn_local(async move {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Err(e) = run(&refs, None).await {
            eprintln!("nyx: {e}");
        }
    });
}

/// A long-running command whose output lines mean "something changed"
/// (`nmcli monitor`, `pactl subscribe`). Stops the process when dropped.
pub struct Watch(Option<gio::Subprocess>);

impl Watch {
    pub fn lines(args: &[&str], on_line: impl Fn(&str) + 'static) -> Watch {
        let argv: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        let Ok(p) = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE) else {
            return Watch(None);
        };
        if let Some(out) = p.stdout_pipe() {
            let data = gio::DataInputStream::new(&out);
            glib::MainContext::default().spawn_local(async move {
                while let Ok(Some(line)) = data.read_line_utf8_future(glib::Priority::DEFAULT).await {
                    on_line(&line);
                }
            });
        }
        Watch(Some(p))
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            p.force_exit();
        }
    }
}

impl nyx_widgets::live::Stop for Watch {}

/// Collapse a burst of calls into one, `delay` after the last.
#[derive(Clone)]
pub struct Debounce {
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    delay: Duration,
}

impl Debounce {
    pub fn new(ms: u64) -> Debounce {
        Debounce { pending: Rc::default(), delay: Duration::from_millis(ms) }
    }

    pub fn call(&self, f: impl FnOnce() + 'static) {
        if let Some(id) = self.pending.borrow_mut().take() {
            id.remove();
        }
        let pending = self.pending.clone();
        let id = glib::timeout_add_local_once(self.delay, move || {
            pending.borrow_mut().take();
            f();
        });
        *self.pending.borrow_mut() = Some(id);
    }
}

/// Several things that stop together.
pub struct Stops(pub Vec<Box<dyn nyx_widgets::live::Stop>>);

impl nyx_widgets::live::Stop for Stops {
    fn stop(self: Box<Self>) {
        for s in self.0 {
            s.stop();
        }
    }
}

/// A D-Bus signal subscription as something that stops.
pub struct SignalStop(pub gio::SignalSubscription);
impl nyx_widgets::live::Stop for SignalStop {}

/// A file or folder monitor as something that stops.
pub struct MonitorStop(pub gio::FileMonitor);
impl nyx_widgets::live::Stop for MonitorStop {
    fn stop(self: Box<Self>) {
        self.0.cancel();
    }
}

/// A repeating timer as something that stops.
pub struct TimerStop(pub Option<glib::SourceId>);
impl nyx_widgets::live::Stop for TimerStop {
    fn stop(mut self: Box<Self>) {
        if let Some(id) = self.0.take() {
            id.remove();
        }
    }
}

pub fn session_bus() -> Option<gio::DBusConnection> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()
}

pub fn system_bus() -> Option<gio::DBusConnection> {
    gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE).ok()
}

/// Split one line of `nmcli -t` output: fields are separated by `:`, and a `:` or `\`
/// inside a field is escaped with `\`. Spaces are kept (SSIDs can end in one).
pub fn terse_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    fields.last_mut().unwrap().push(n);
                }
            }
            ':' => fields.push(String::new()),
            _ => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nmcli_fields() {
        assert_eq!(terse_fields("*:Pandora :83:WPA2"), vec!["*", "Pandora ", "83", "WPA2"]);
        assert_eq!(terse_fields(" :Cafe\\: Lumen:40:"), vec![" ", "Cafe: Lumen", "40", ""]);
        assert_eq!(terse_fields("a\\\\b:c"), vec!["a\\b", "c"]);
    }
}
