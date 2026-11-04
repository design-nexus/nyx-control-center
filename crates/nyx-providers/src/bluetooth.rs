//! Bluetooth through BlueZ on the system bus: the adapter's power, the paired devices,
//! and (while looking) devices nearby to pair. Pairing and forgetting go through
//! Omarchy's `omarchy-bluetooth-device`, which handles trust and the pairing agent.

use crate::{Debounce, SignalStop, Stops, system_bus};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Device {
    pub path: String,
    pub name: String,
    /// BlueZ's icon name (`audio-headphones`, `input-mouse`, …).
    pub icon: String,
    pub connected: bool,
    pub battery: Option<u8>,
    /// `AA:BB:CC:DD:EE:FF`.
    pub address: String,
    /// Signal strength while looking (dBm), for nearby devices.
    pub rssi: Option<i16>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// There's an adapter.
    pub available: bool,
    pub powered: bool,
    pub adapter: String,
    /// Paired devices, connected first.
    pub devices: Vec<Device>,
    /// Looking for devices now.
    pub discovering: bool,
    /// Devices nearby that aren't paired (named ones, strongest first).
    pub nearby: Vec<Device>,
}

impl State {
    pub fn connected(&self) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.connected).collect()
    }
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    let Some(bus) = system_bus() else { return Box::new(()) };
    set.set(read(&bus));
    let debounce = Debounce::new(150);
    let mut stops: Vec<Box<dyn Stop>> = Vec::new();
    for (iface, member) in [
        ("org.freedesktop.DBus.ObjectManager", "InterfacesAdded"),
        ("org.freedesktop.DBus.ObjectManager", "InterfacesRemoved"),
        ("org.freedesktop.DBus.Properties", "PropertiesChanged"),
    ] {
        let (set, bus2, debounce) = (set.clone(), bus.clone(), debounce.clone());
        let sub = bus.subscribe_to_signal(
            Some("org.bluez"),
            Some(iface),
            Some(member),
            None,
            None,
            gio::DBusSignalFlags::NONE,
            move |_| {
                let (set, bus) = (set.clone(), bus2.clone());
                debounce.call(move || set.set(read(&bus)));
            },
        );
        stops.push(Box::new(SignalStop(sub)));
    }
    Box::new(Stops(stops))
}

fn read(bus: &gio::DBusConnection) -> State {
    let mut st = State::default();
    let Ok(reply) = bus.call_sync(
        Some("org.bluez"),
        "/",
        "org.freedesktop.DBus.ObjectManager",
        "GetManagedObjects",
        None,
        None,
        gio::DBusCallFlags::NONE,
        1000,
        gio::Cancellable::NONE,
    ) else {
        return st;
    };
    // a{oa{sa{sv}}}
    let objects = reply.child_value(0);
    for i in 0..objects.n_children() {
        let entry = objects.child_value(i);
        let path = entry.child_value(0).str().unwrap_or_default().to_string();
        let ifaces = entry.child_value(1);
        let lookup = |name: &str| -> Option<glib::VariantDict> {
            (0..ifaces.n_children())
                .map(|j| ifaces.child_value(j))
                .find(|e| e.child_value(0).str() == Some(name))
                .map(|e| glib::VariantDict::new(Some(&e.child_value(1))))
        };
        if let Some(a) = lookup("org.bluez.Adapter1")
            && !st.available
        {
            st.available = true;
            st.powered = a.lookup::<bool>("Powered").ok().flatten().unwrap_or(false);
            st.discovering = a.lookup::<bool>("Discovering").ok().flatten().unwrap_or(false);
            st.adapter = path.clone();
        }
        if let Some(d) = lookup("org.bluez.Device1") {
            let paired = d.lookup::<bool>("Paired").ok().flatten().unwrap_or(false);
            // A nearby device is worth showing only with a real name.
            let real_name = d.lookup::<String>("Name").ok().flatten();
            if !paired && real_name.is_none() {
                continue;
            }
            let name = d.lookup::<String>("Alias").ok().flatten().or(real_name).unwrap_or_else(|| "Device".into());
            let dev = Device {
                path: path.clone(),
                name,
                icon: d.lookup::<String>("Icon").ok().flatten().unwrap_or_else(|| "bluetooth".into()),
                connected: d.lookup::<bool>("Connected").ok().flatten().unwrap_or(false),
                battery: lookup("org.bluez.Battery1").and_then(|b| b.lookup::<u8>("Percentage").ok().flatten()),
                address: d.lookup::<String>("Address").ok().flatten().unwrap_or_default(),
                rssi: d.lookup::<i16>("RSSI").ok().flatten(),
            };
            if paired {
                st.devices.push(dev);
            } else if dev.rssi.is_some() {
                // BlueZ keeps devices it saw a while ago; only those in range now have RSSI.
                st.nearby.push(dev);
            }
        }
    }
    st.devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    st.nearby.sort_by_key(|d| std::cmp::Reverse(d.rssi));
    st
}

pub async fn set_powered(on: bool) -> Result<(), String> {
    let bus = system_bus().ok_or("no system bus")?;
    let adapter = live().get().adapter;
    if adapter.is_empty() {
        return Err("no Bluetooth adapter".into());
    }
    bus.call_future(
        Some("org.bluez"),
        &adapter,
        "org.freedesktop.DBus.Properties",
        "Set",
        Some(&("org.bluez.Adapter1", "Powered", on.to_variant()).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
    )
    .await
    .map(|_| ())
    .map_err(|e| e.message().to_string())
}

/// Connect or disconnect a paired device.
pub async fn set_connected(path: &str, on: bool) -> Result<(), String> {
    let bus = system_bus().ok_or("no system bus")?;
    bus.call_future(
        Some("org.bluez"),
        path,
        "org.bluez.Device1",
        if on { "Connect" } else { "Disconnect" },
        None,
        None,
        gio::DBusCallFlags::NONE,
        20_000,
    )
    .await
    .map(|_| ())
    .map_err(|e| e.message().to_string())
}

/// Look for devices to pair (or stop). BlueZ stops on its own when this program goes.
pub async fn discover(on: bool) -> Result<(), String> {
    let bus = system_bus().ok_or("no system bus")?;
    let adapter = live().get().adapter;
    if adapter.is_empty() {
        return Err("no Bluetooth adapter".into());
    }
    let r = bus
        .call_future(
            Some("org.bluez"),
            &adapter,
            "org.bluez.Adapter1",
            if on { "StartDiscovery" } else { "StopDiscovery" },
            None,
            None,
            gio::DBusCallFlags::NONE,
            5000,
        )
        .await;
    match r {
        Ok(_) => Ok(()),
        // Already looking, or already stopped (someone else's session): fine.
        Err(e) if e.message().contains("InProgress") || e.message().contains("NotReady") || e.message().contains("Failed") => {
            Ok(())
        }
        Err(e) => Err(e.message().to_string()),
    }
}

/// Pair with a nearby device, trust it and connect (Omarchy's sequence).
pub async fn pair(address: &str) -> Result<(), String> {
    if crate::omarchy::here() {
        return crate::run(&["omarchy-bluetooth-device", "pair", address], None).await;
    }
    crate::run(&["bluetoothctl", "--timeout", "20", "pair", address], None).await?;
    crate::run(&["bluetoothctl", "trust", address], None).await?;
    crate::run(&["bluetoothctl", "--timeout", "10", "connect", address], None).await
}

/// Unpair a device: it has to be paired again to connect.
pub async fn forget(address: &str) -> Result<(), String> {
    if crate::omarchy::here() {
        return crate::run(&["omarchy-bluetooth-device", "forget", address], None).await;
    }
    crate::run(&["bluetoothctl", "remove", address], None).await
}
