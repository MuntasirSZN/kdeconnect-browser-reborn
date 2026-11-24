//! Signal stream handling.

use futures_util::StreamExt;
use tokio::sync::Mutex;
use zbus::MessageStream;
use zbus::message::Type;

use anyhow::Result;
use tracing::{error, warn};
use zbus::Connection;

use super::SharedDevice;
use super::devices::{devices_snapshot, discover_devices, refresh_device};
use super::matchers::add_device_match_rules;
use super::props::owned_value_to_json;
use crate::types::{Empty, Message, MessageType};

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
    if iface == Some(crate::dbus::DAEMON_IFACE) {
        SignalAction::Resync
    } else if iface == Some(crate::dbus::DEVICE_IFACE) {
        if member == Some("pairStateChanged") || member == Some("reachableChanged") {
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
async fn resync_devices<D: super::Dbus + ?Sized>(
    conn: &D,
    devices: &std::sync::Arc<Mutex<Vec<SharedDevice>>>,
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

            let snapshot_devices = fresh.clone();
            {
                let mut guard = devices.lock().await;
                *guard = fresh;
            }
            let snapshot = devices_snapshot(&snapshot_devices).await;
            drop(output_tx.send(Message::result(MessageType::Devices, snapshot)));
        }
        Err(e) => {
            warn!(error = %e, "device re-discovery failed");
            drop(output_tx.send(Message::result(
                MessageType::Error,
                serde_json::Value::String(format!("KDE Connect re-discovery failed: {e:#}")),
            )));
        }
    }
}

/// Spawn a task that listens for daemon and device signals and pushes
/// `DeviceUpdate` / `Devices` / `Error` messages onto `output_tx`.
pub(crate) fn spawn_signal_listener(
    conn: Connection,
    devices: std::sync::Arc<Mutex<Vec<SharedDevice>>>,
    output_tx: tokio::sync::mpsc::UnboundedSender<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stream = MessageStream::from(&conn);
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

            if msg.message_type() != Type::Signal {
                continue;
            }

            // Copy everything we need out of the borrowed header before any
            // await: no header borrow may cross a suspension point.
            let (iface, member, path) = {
                let hdr = msg.header();
                (
                    hdr.interface().map(ToString::to_string),
                    hdr.member().map(ToString::to_string),
                    hdr.path().map(ToString::to_string),
                )
            };

            match signal_action(iface.as_deref(), member.as_deref()) {
                SignalAction::Ignore => {}
                SignalAction::Resync => resync_devices(&conn, &devices, &output_tx).await,
                SignalAction::Refresh => {
                    let (Some(path), Some(member)) = (path, member) else {
                        continue;
                    };
                    if let Err(e) = handle_signal(&conn, &devices, &path, &member, &output_tx).await
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
    devices: &std::sync::Arc<Mutex<Vec<SharedDevice>>>,
    path: &str,
    member: &str,
    output_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<()> {
    // Find the device by object path
    let candidates = devices.lock().await.clone();
    let mut device_arc = None;
    for candidate in candidates {
        let matches = candidate.lock().await.object_path == path;
        if matches {
            device_arc = Some(candidate);
            break;
        }
    }

    let Some(dev_arc) = device_arc else {
        return Ok(()); // not one of our watched devices
    };

    let mut state = dev_arc.lock().await;

    // Determine which fields to refresh based on signal type.
    match member {
        "nameChanged" => {
            let v = super::props::get_prop_raw(conn, &state.object_path, "name").await?;
            state.device.name = owned_value_to_json(&v)?;
        }
        "typeChanged" => {
            let v = super::props::get_prop_raw(conn, &state.object_path, "type").await?;
            state.device.kind = owned_value_to_json(&v)?;
        }
        "statusIconNameChanged" => {
            let v = super::props::get_prop_raw(conn, &state.object_path, "statusIconName").await?;
            state.device.status_icon_name = owned_value_to_json(&v)?;
        }
        "pluginsChanged" => {
            let v =
                super::props::get_prop_raw(conn, &state.object_path, "supportedPlugins").await?;
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
    drop(output_tx.send(update));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::MockDbus;
    use zbus::OwnedVariant;
    use zbus::wire::Str;

    #[tokio::test]
    async fn resync_replaces_the_device_list_and_publishes_a_snapshot() {
        let mut mock = MockDbus::new();
        mock.expect_list_device_ids()
            .times(1)
            .returning(|| Ok(vec!["dev-a".to_owned()]));
        mock.expect_fetch_all_props().times(1).returning(|_: &str| {
            Ok(std::collections::HashMap::from([(
                "name".to_owned(),
                OwnedVariant::from(Str::from("Pixel")),
            )]))
        });
        mock.expect_add_match().returning(|_rule| Ok(()));

        let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel();
        let devices = std::sync::Arc::new(Mutex::new(Vec::new()));

        resync_devices(&mock, &devices, &output_tx).await;

        let message = output_rx.try_recv().expect("snapshot published");
        assert_eq!(message.msg_type, MessageType::Devices);
        let data = message.data.expect("snapshot data");
        assert_eq!(data["dev-a"]["name"], "Pixel");
        assert_eq!(devices.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn resync_reports_discovery_failures_to_the_extension() {
        let mut mock = MockDbus::new();
        mock.expect_list_device_ids()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("lost the bus")));

        let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel();
        let devices = std::sync::Arc::new(Mutex::new(Vec::new()));

        resync_devices(&mock, &devices, &output_tx).await;

        let message = output_rx.try_recv().expect("error published");
        assert_eq!(message.msg_type, MessageType::Error);
        let text = message.data.expect("error data");
        assert!(
            text.as_str().is_some_and(|s| s.contains("lost the bus")),
            "unexpected message: {text}"
        );
        assert!(devices.lock().await.is_empty());
    }

    #[test]
    fn daemon_signals_resync() {
        assert_eq!(
            signal_action(Some(crate::dbus::DAEMON_IFACE), Some("deviceAdded")),
            SignalAction::Resync
        );
        assert_eq!(
            signal_action(Some(crate::dbus::DAEMON_IFACE), Some("deviceRemoved")),
            SignalAction::Resync
        );
        assert_eq!(
            signal_action(
                Some(crate::dbus::DAEMON_IFACE),
                Some("deviceVisibilityChanged")
            ),
            SignalAction::Resync
        );
    }

    #[test]
    fn pair_state_changes_resync() {
        assert_eq!(
            signal_action(Some(crate::dbus::DEVICE_IFACE), Some("pairStateChanged")),
            SignalAction::Resync
        );
    }

    #[test]
    fn reachable_changes_resync_visible_devices() {
        assert_eq!(
            signal_action(Some(crate::dbus::DEVICE_IFACE), Some("reachableChanged")),
            SignalAction::Resync
        );
    }

    #[test]
    fn ordinary_device_signals_refresh() {
        assert_eq!(
            signal_action(Some(crate::dbus::DEVICE_IFACE), Some("nameChanged")),
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
