//! KDE Connect D-Bus integration.
//!
//! Talks to `org.kde.kdeconnect` on the session bus — enumerates devices,
//! reads properties, listens for state-change signals, and invokes `shareUrl`.

mod devices;
mod matchers;
mod props;
mod share;
mod signals;

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use zbus::Connection;

use crate::types::Device;

pub(crate) use devices::{devices_snapshot, discover_devices};
pub(crate) use matchers::{add_daemon_match_rules, add_device_match_rules};
pub(crate) use share::{check_capability, share_url};
pub(crate) use signals::spawn_signal_listener;

pub(crate) const DEST: &str = "org.kde.kdeconnect";
pub(crate) const BASE_PATH: &str = "/modules/kdeconnect";
pub(crate) const PLUGIN_SHARE: &str = "kdeconnect_share";
pub(crate) const DAEMON_IFACE: &str = "org.kde.kdeconnect.daemon";

pub(crate) const DEVICE_IFACE: &str = "org.kde.kdeconnect.device";

/// Thread-safe handle to a device's mutable state.
pub(crate) type SharedDevice = Arc<Mutex<DeviceState>>;

/// Mutable device state kept in sync with D-Bus properties.
pub(crate) struct DeviceState {
    pub device: Device,
    pub object_path: String,
}

/// Open a session-bus connection.
pub(crate) async fn open_connection() -> Result<Connection> {
    Connection::session().await.context("open session bus")
}
