//! KDE Connect D-Bus integration.
//!
//! Talks to `org.kde.kdeconnect` on the session bus — enumerates devices,
//! reads properties, listens for state-change signals, and invokes `shareUrl`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use tokio::sync::Mutex;
use tracing::{error, warn};
use zbus::Connection;
use zbus::zvariant::OwnedVariant;

use crate::types::{Device, Empty, Message, MessageType};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const DEST: &str = "org.kde.kdeconnect";
pub const BASE_PATH: &str = "/modules/kdeconnect";
pub const PLUGIN_SHARE: &str = "kdeconnect_share";
pub const DAEMON_IFACE: &str = "org.kde.kdeconnect.daemon";

const DEVICE_IFACE: &str = "org.kde.kdeconnect.device";

// ---------------------------------------------------------------------------
// Shared device
// ---------------------------------------------------------------------------

/// Thread-safe handle to a device's mutable state.
pub type SharedDevice = Arc<Mutex<DeviceState>>;

/// Mutable device state kept in sync with D-Bus properties.
pub struct DeviceState {
    pub device: Device,
    pub object_path: String,
}

// ---------------------------------------------------------------------------
// Connection management
// ---------------------------------------------------------------------------

/// Open a session-bus connection.
pub async fn open_connection() -> Result<Connection> {
    Connection::session().await.context("open session bus")
}

// ---------------------------------------------------------------------------
// Device enumeration
// ---------------------------------------------------------------------------

/// Fetch all known (including unreachable) paired device IDs.
pub async fn list_device_ids(conn: &Connection) -> Result<Vec<String>> {
    let reply = conn
        .call_method(
            Some(DEST),
            BASE_PATH,
            Some("org.kde.kdeconnect.daemon"),
            "devices",
            &(false, true),
        )
        .await
        .context("call devices()")?;

    let ids: Vec<String> = reply
        .body()
        .deserialize()
        .context("deserialize device id list")?;
    Ok(ids)
}

// ---------------------------------------------------------------------------
// D-Bus property helpers
// ---------------------------------------------------------------------------

/// Deserialize a D-Bus property via the value's JSON representation.
///
/// `zvariant::OwnedVariant` serialises as `{"signature": "…", "value": …}`;
/// we extract the `value` field before deserialising to the concrete Rust
/// type.  This avoids depending on `DynamicDeserialize` bounds.
fn owned_value_to_json<T: serde::de::DeserializeOwned>(v: &OwnedVariant) -> Result<T> {
    let json = serde_json::to_value(v).context("OwnedVariant -> json")?;
    let inner = json.get("value").cloned().unwrap_or(json);
    serde_json::from_value(inner).context("json -> target type")
}

/// Call Properties.Get and return the result as `OwnedVariant`.
async fn get_prop_raw(conn: &Connection, path: &str, prop: &str) -> Result<OwnedVariant> {
    let reply = conn
        .call_method(
            Some(DEST),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(DEVICE_IFACE, prop),
        )
        .await
        .with_context(|| format!("Properties.Get({})", prop))?;

    let value: OwnedVariant = reply
        .body()
        .deserialize()
        .with_context(|| format!("deserialize property '{}'", prop))?;
    Ok(value)
}

/// Call `Properties.GetAll` and return every property as an `OwnedVariant`.
async fn fetch_all_props(
    conn: &Connection,
    path: &str,
) -> Result<HashMap<String, OwnedVariant>> {
    let reply = conn
        .call_method(
            Some(DEST),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &(DEVICE_IFACE,),
        )
        .await
        .with_context(|| format!("Properties.GetAll({})", path))?;

    reply
        .body()
        .deserialize()
        .context("deserialize property map")
}

/// Convert an `OwnedVariant` to the JSON value it carries.
fn variant_to_json(v: &OwnedVariant) -> serde_json::Value {
    let json = serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
    json.get("value").cloned().unwrap_or(json)
}

/// Fetch **all** device properties and build a `Device` struct.
async fn fetch_device(conn: &Connection, id: &str) -> Result<Device> {
    let obj_path = format!("{}/devices/{}", BASE_PATH, id);
    let raw = fetch_all_props(conn, &obj_path).await?;
    let props: HashMap<String, serde_json::Value> = raw
        .iter()
        .map(|(key, value)| (key.clone(), variant_to_json(value)))
        .collect();

    Ok(Device::from_props(id, &props))
}

// ---------------------------------------------------------------------------
// Device list builder
// ---------------------------------------------------------------------------

/// Enumerate known devices and fetch their current properties.
pub async fn discover_devices(conn: &Connection) -> Result<Vec<SharedDevice>> {
    let ids = list_device_ids(conn).await?;
    let mut devices = Vec::with_capacity(ids.len());

    for id in &ids {
        match fetch_device(conn, id).await {
            Ok(device) => {
                let obj_path = format!("{}/devices/{}", BASE_PATH, id);
                devices.push(Arc::new(Mutex::new(DeviceState {
                    device,
                    object_path: obj_path,
                })));
            }
            Err(e) => {
                warn!(device = %id, error = %e, "failed to fetch device properties, skipping");
                continue;
            }
        }
    }

    Ok(devices)
}

/// Serialise the current device cache as the `typeDevices` payload,
/// keyed by device id.
pub fn devices_snapshot(devices: &[SharedDevice]) -> serde_json::Value {
    let map: BTreeMap<String, Device> = devices
        .iter()
        .filter_map(|shared| {
            let state = shared.try_lock().ok()?;
            Some((state.device.id.clone(), state.device.clone()))
        })
        .collect();

    serde_json::to_value(map).unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
}

// ---------------------------------------------------------------------------
// Device property refresh
// ---------------------------------------------------------------------------

/// Re-read every serializable property on a device and update in place.
pub async fn refresh_device(conn: &Connection, state: &DeviceState) -> Result<Device> {
    let dev = fetch_device(conn, &state.device.id).await?;
    Ok(dev)
}

// ---------------------------------------------------------------------------
// Share a URL
// ---------------------------------------------------------------------------

/// Send a URL to a device via the `kdeconnect_share` plugin.
///
/// This replicates the Go call chain:
///    1. Check plugin support (caller should have validated).
///    2. Check reachable & trusted (caller should have validated).
///    3. Call `shareUrl` on `/modules/kdeconnect/devices/{id}/share`.
pub async fn share_url(conn: &Connection, device_id: &str, url: &str) -> Result<()> {
    let share_path = format!("{}/devices/{}/share", BASE_PATH, device_id);
    conn.call_method(
        Some(DEST),
        share_path.as_str(),
        Some("org.kde.kdeconnect.device.share"),
        "shareUrl",
        &(url),
    )
    .await
    .with_context(|| format!("shareUrl({}) to {}", url, device_id))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Device capability check
// ---------------------------------------------------------------------------

/// Check whether a device currently supports a plugin, is reachable, and is
/// trusted — matching the Go `Device.supported()` logic.
pub fn check_capability(state: &DeviceState, plugin: &str) -> Result<()> {
    let dev = &state.device;
    if !dev.supported_plugins.contains_key(plugin) {
        anyhow::bail!("device {} does not support plugin {}", dev.id, plugin);
    }
    if !dev.is_reachable {
        anyhow::bail!("device {} is not reachable", dev.id);
    }
    if !dev.is_trusted {
        anyhow::bail!("device {} is not trusted", dev.id);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// D-Bus match rules for device signals
// ---------------------------------------------------------------------------

// Signals emitted by `org.kde.kdeconnect.device`.  Unlisted signals never
// reach us: D-Bus only delivers members with a matching AddMatch rule.
const SIGNAL_MEMBERS: &[&str] = &[
    "reachableChanged",
    "nameChanged",
    "typeChanged",
    "statusIconNameChanged",
    "pluginsChanged",
    "pairStateChanged",
];

/// Register D-Bus match rules for every property-change signal on a device,
/// matching the Go `Device.addMatchSignal` calls for each member.
pub async fn add_device_match_rules(conn: &Connection, object_path: &str) -> Result<()> {
    for member in SIGNAL_MEMBERS {
        let rule = format!(
            "type='signal',path='{}',interface='{}',member='{}'",
            object_path, DEVICE_IFACE, member
        );
        conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "AddMatch",
            &(rule.as_str()),
        )
        .await
        .with_context(|| format!("AddMatch({})", member))?;
    }
    Ok(())
}

/// Register a match rule for every signal from the KDE Connect daemon
/// (`deviceAdded`, `deviceRemoved`, `deviceVisibilityChanged`, …).
pub async fn add_daemon_match_rules(conn: &Connection) -> Result<()> {
    let rule = format!(
        "type='signal',path='{}',interface='{}'",
        BASE_PATH, DAEMON_IFACE
    );
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "AddMatch",
        &(rule.as_str()),
    )
    .await
    .context("AddMatch(daemon signals)")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Signal listener
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum SignalAction {
    /// Re-read one device's properties.
    Refresh,
    /// Re-enumerate every device; the device set itself changed.
    Resync,
    /// Not a signal we subscribe to.
    Ignore,
}

fn signal_action(iface: Option<&str>, member: Option<&str>) -> SignalAction {
    if iface == Some(DAEMON_IFACE) {
        SignalAction::Resync
    } else if iface == Some(DEVICE_IFACE) {
        if member == Some("pairStateChanged") {
            SignalAction::Resync
        } else {
            SignalAction::Refresh
        }
    } else {
        SignalAction::Ignore
    }
}

/// Re-enumerate devices after a daemon or pairing signal and publish the new
/// snapshot.  On failure the previous device list is kept and the extension is
/// told why.
async fn resync_devices(
    conn: &Connection,
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    output_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) {
    match discover_devices(conn).await {
        Ok(fresh) => {
            for state in &fresh {
                let obj_path = state.lock().await.object_path.clone();
                if let Err(e) = add_device_match_rules(conn, &obj_path).await {
                    warn!(path = %obj_path, error = %e, "failed to add match rules");
                }
            }

            let guard = {
                let mut guard = devices.lock().await;
                *guard = fresh;
                guard
            };
            let _ = output_tx.send(Message::result(
                MessageType::Devices,
                devices_snapshot(guard.as_slice()),
            ));
        }
        Err(e) => {
            warn!(error = %e, "device re-discovery failed");
            let _ = output_tx.send(Message::result(
                MessageType::Error,
                serde_json::Value::String(format!("KDE Connect re-discovery failed: {e:#}")),
            ));
        }
    }
}

/// Spawn a task that listens for daemon and device signals and pushes
/// `DeviceUpdate` / `Devices` / `Error` messages onto `output_tx`.
pub fn spawn_signal_listener(
    conn: Connection,
    devices: Arc<Mutex<Vec<SharedDevice>>>,
    output_tx: tokio::sync::mpsc::UnboundedSender<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stream = zbus::MessageStream::from(&conn);
        loop {
            let msg = match stream.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => {
                    error!(error = %e, "D-Bus signal stream error");
                    continue;
                }
                None => {
                    warn!("D-Bus signal stream ended");
                    break;
                }
            };

            if msg.message_type() != zbus::message::Type::Signal {
                continue;
            }

            // Copy everything we need out of the borrowed header before any
            // await: no header borrow may cross a suspension point.
            let (iface, member, path) = {
                let hdr = msg.header();
                (
                    hdr.interface().map(|i| i.to_string()),
                    hdr.member().map(|m| m.to_string()),
                    hdr.path().map(|p| p.to_string()),
                )
            };

            match signal_action(iface.as_deref(), member.as_deref()) {
                SignalAction::Ignore => continue,
                SignalAction::Resync => resync_devices(&conn, &devices, &output_tx).await,
                SignalAction::Refresh => {
                    let (Some(path), Some(member)) = (path, member) else {
                        continue;
                    };
                    if let Err(e) =
                        handle_signal(&conn, &devices, &path, &member, &output_tx).await
                    {
                        warn!(signal = %member, path = %path, error = %e, "signal handler");
                    }
                }
            }
        }
    })
}

/// Process a single D-Bus signal for a given device path.
async fn handle_signal(
    conn: &Connection,
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    path: &str,
    member: &str,
    output_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<()> {
    // Find the device by object path
    let device_arc = {
        let guard = devices.lock().await;
        guard
            .iter()
            .find(|d| d.try_lock().map(|s| s.object_path == path).unwrap_or(false))
            .cloned()
    };

    let dev_arc = match device_arc {
        Some(d) => d,
        None => return Ok(()), // not one of our watched devices
    };

    let mut state = dev_arc.lock().await;

    // Determine which fields to refresh based on signal type.
    match member {
        "reachableChanged" => {
            let v = get_prop_raw(conn, &state.object_path, "isReachable").await?;
            state.device.is_reachable = owned_value_to_json(&v)?;
        }
        "nameChanged" => {
            let v = get_prop_raw(conn, &state.object_path, "name").await?;
            state.device.name = owned_value_to_json(&v)?;
        }
        "typeChanged" => {
            let v = get_prop_raw(conn, &state.object_path, "type").await?;
            state.device.device_type = owned_value_to_json(&v)?;
        }
        "statusIconNameChanged" => {
            let v = get_prop_raw(conn, &state.object_path, "statusIconName").await?;
            state.device.status_icon_name = owned_value_to_json(&v)?;
        }
        "pluginsChanged" => {
            let v = get_prop_raw(conn, &state.object_path, "supportedPlugins").await?;
            let plugins: Vec<String> = owned_value_to_json(&v)?;
            state.device.supported_plugins = plugins.into_iter().map(|p| (p, Empty)).collect();
        }
        // Other signals (pairStateChanged, linksChanged, …) → full refresh
        _ => {
            state.device = refresh_device(conn, &state).await?;
        }
    }

    // Push device-update notification to the output channel
    let update = Message::result(
        MessageType::DeviceUpdate,
        serde_json::to_value(&state.device)?,
    );
    let _ = output_tx.send(update);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str) -> Device {
        Device {
            id: id.into(),
            device_type: "phone".into(),
            name: name.into(),
            icon_name: String::new(),
            status_icon_name: String::new(),
            is_reachable: true,
            is_trusted: true,
            supported_plugins: HashMap::new(),
        }
    }

    fn shared(d: Device) -> SharedDevice {
        Arc::new(Mutex::new(DeviceState {
            object_path: format!("/modules/kdeconnect/devices/{}", d.id),
            device: d,
        }))
    }

    #[test]
    fn snapshot_is_keyed_by_device_id() {
        let devices = vec![shared(device("a", "Phone")), shared(device("b", "Laptop"))];
        let json = devices_snapshot(&devices);
        assert_eq!(json["a"]["name"], "Phone");
        assert_eq!(json["b"]["name"], "Laptop");
        assert_eq!(json.as_object().expect("object").len(), 2);
    }

    #[test]
    fn daemon_signals_resync() {
        assert_eq!(
            signal_action(Some(DAEMON_IFACE), Some("deviceAdded")),
            SignalAction::Resync
        );
        assert_eq!(
            signal_action(Some(DAEMON_IFACE), Some("deviceRemoved")),
            SignalAction::Resync
        );
        assert_eq!(
            signal_action(Some(DAEMON_IFACE), Some("deviceVisibilityChanged")),
            SignalAction::Resync
        );
    }

    #[test]
    fn pair_state_changes_resync() {
        assert_eq!(
            signal_action(Some(DEVICE_IFACE), Some("pairStateChanged")),
            SignalAction::Resync
        );
    }

    #[test]
    fn ordinary_device_signals_refresh() {
        assert_eq!(
            signal_action(Some(DEVICE_IFACE), Some("reachableChanged")),
            SignalAction::Refresh
        );
    }

    #[test]
    fn unrelated_signals_are_ignored() {
        assert_eq!(
            signal_action(Some("org.example.Other"), Some("changed")),
            SignalAction::Ignore
        );
    }
}
