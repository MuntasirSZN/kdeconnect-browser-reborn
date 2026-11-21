//! Device discovery and the cached device list.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use tracing::warn;
use zbus::Connection;

use super::props::{fetch_all_props, variant_to_json};
use super::{BASE_PATH, DEST, DeviceState, SharedDevice};
use crate::types::Device;

/// Fetch **all** device properties and build a `Device` struct.
async fn fetch_device(conn: &Connection, id: &str) -> Result<Device> {
    let obj_path = format!("{BASE_PATH}/devices/{id}");
    let raw = fetch_all_props(conn, &obj_path).await?;
    let props: HashMap<String, serde_json::Value> = raw
        .iter()
        .map(|(key, value)| (key.clone(), variant_to_json(value)))
        .collect();

    Ok(Device::from_props(id, &props))
}

/// Fetch currently reachable, paired device IDs visible to KDE Connect.
pub(crate) async fn list_device_ids(conn: &Connection) -> Result<Vec<String>> {
    let reply = conn
        .call_method(
            Some(DEST),
            BASE_PATH,
            Some("org.kde.kdeconnect.daemon"),
            "devices",
            &(true, true),
        )
        .await
        .context("call devices()")?;

    let ids: Vec<String> = reply
        .body()
        .deserialize()
        .context("deserialize device id list")?;
    Ok(ids)
}

/// Enumerate visible reachable devices and fetch their current properties.
pub(crate) async fn discover_devices(conn: &Connection) -> Result<Vec<SharedDevice>> {
    let ids = list_device_ids(conn).await?;
    let mut devices = Vec::with_capacity(ids.len());

    for id in &ids {
        match fetch_device(conn, id).await {
            Ok(device) => {
                let obj_path = format!("{BASE_PATH}/devices/{id}");
                devices.push(Arc::new(Mutex::new(DeviceState {
                    device,
                    object_path: obj_path,
                })));
            }
            Err(e) => {
                warn!(device = %id, error = %e, "failed to fetch device properties, skipping");
            }
        }
    }

    Ok(devices)
}

/// Serialise the current device cache as the `typeDevices` payload,
/// keyed by device id.
pub(crate) async fn devices_snapshot(devices: &[SharedDevice]) -> serde_json::Value {
    let mut map = BTreeMap::new();
    for shared in devices {
        let state = shared.lock().await;
        map.insert(state.device.id.clone(), state.device.clone());
    }

    serde_json::to_value(map)
        .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::default()))
}

/// Re-read every serializable property on a device and update in place.
pub(crate) async fn refresh_device(conn: &Connection, state: &DeviceState) -> Result<Device> {
    let dev = fetch_device(conn, &state.device.id).await?;
    Ok(dev)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[expect(
        clippy::zero_sized_map_values,
        reason = "test helper builds the map like Device::supported_plugins"
    )]
    fn device(id: &str, name: &str) -> Device {
        Device {
            id: id.into(),
            kind: "phone".into(),
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

    #[tokio::test]
    async fn snapshot_is_keyed_by_device_id() {
        let devices = vec![shared(device("a", "Phone")), shared(device("b", "Laptop"))];
        let json = devices_snapshot(&devices).await;
        assert_eq!(json["a"]["name"], "Phone");
        assert_eq!(json["b"]["name"], "Laptop");
        assert_eq!(json.as_object().expect("object").len(), 2);
    }

    #[tokio::test]
    async fn snapshot_waits_for_locked_device_instead_of_dropping_it() {
        let locked = shared(device("locked", "Phone"));
        let devices = vec![locked.clone()];
        let guard = locked.lock().await;

        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = started_tx.send(());
            devices_snapshot(&devices).await
        });
        started_rx.await.expect("snapshot task started");
        tokio::task::yield_now().await;
        assert!(
            !task.is_finished(),
            "snapshot should wait for the held device lock"
        );

        drop(guard);
        let json = task.await.expect("snapshot task");
        assert_eq!(json["locked"]["name"], "Phone");
    }
}
