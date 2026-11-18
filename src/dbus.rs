//! KDE Connect D-Bus integration.
//!
//! Talks to `org.kde.kdeconnect` on the session bus — enumerates devices,
//! reads properties, listens for state-change signals, and invokes `shareUrl`.

use std::collections::HashMap;
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

/// Fetch **all** device properties and build a `Device` struct.
async fn fetch_device(conn: &Connection, id: &str) -> Result<Device> {
    let obj_path = format!("{}/devices/{}", BASE_PATH, id);

    macro_rules! prop {
        ($prop:literal) => {
            get_prop_raw(conn, &obj_path, $prop).await
        };
    }

    // String properties
    let name: String = owned_value_to_json(&prop!("name")?)?;
    let device_type: String = owned_value_to_json(&prop!("type")?)?;
    let icon_name: String = owned_value_to_json(&prop!("iconName")?)?;
    let status_icon_name: String = owned_value_to_json(&prop!("statusIconName")?)?;

    // Bool properties
    let is_reachable: bool = owned_value_to_json(&prop!("isReachable")?)?;
    let is_trusted: bool = owned_value_to_json(&prop!("isPaired")?)?;

    // Array-of-strings property – convert to HashMap<String, Empty> for wire compat.
    let plugin_names: Vec<String> = owned_value_to_json(&prop!("supportedPlugins")?)?;
    let supported_plugins: HashMap<String, Empty> =
        plugin_names.into_iter().map(|p| (p, Empty)).collect();

    Ok(Device {
        id: id.to_owned(),
        device_type,
        name,
        icon_name,
        status_icon_name,
        is_reachable,
        is_trusted,
        supported_plugins,
    })
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

// Signals emitted by `org.kde.kdeconnect.device` as seen from D-Bus
// introspection.  Only signals that affect our serialised properties are
// watched; the rest (pairStateChanged, linksChanged, …) trigger a full
// property refresh via the default branch.
const SIGNAL_MEMBERS: &[&str] = &[
    "reachableChanged",
    "nameChanged",
    "typeChanged",
    "statusIconNameChanged",
    "pluginsChanged",
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

// ---------------------------------------------------------------------------
// Signal listener
// ---------------------------------------------------------------------------

/// Spawn a task that listens for device property-change signals and pushes
/// `DeviceUpdate` messages onto `output_tx` whenever a device changes.
pub fn spawn_signal_listener(
    conn: Connection,
    devices: Arc<Mutex<Vec<SharedDevice>>>,
    output_tx: tokio::sync::mpsc::UnboundedSender<Message>,
) {
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

            // We only care about device interface signals
            if msg.message_type() != zbus::message::Type::Signal {
                continue;
            }
            let hdr = msg.header();
            let iface = hdr.interface().map(|i| i.as_str());
            if iface != Some(DEVICE_IFACE) {
                continue;
            }
            let path_str = match hdr.path() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let member_str = match hdr.member() {
                Some(m) => m.to_string(),
                None => continue,
            };

            // Map the signal member to the action we need
            if let Err(e) = handle_signal(&conn, &devices, &path_str, &member_str, &output_tx).await
            {
                warn!(signal = %member_str, path = %path_str, error = %e, "signal handler");
            }
        }
    });
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
