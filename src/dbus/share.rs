//! Plugin capability checks and share URL calls.

use anyhow::{Context, Result};
use zbus::Connection;

use super::{BASE_PATH, DEST};
use crate::dbus::DeviceState;

/// Send a URL to a device via the `kdeconnect_share` plugin.
pub(crate) async fn share_url(conn: &Connection, device_id: &str, url: &str) -> Result<()> {
    let share_path = format!("{BASE_PATH}/devices/{device_id}/share");
    conn.call_method(
        Some(DEST),
        share_path.as_str(),
        Some("org.kde.kdeconnect.device.share"),
        "shareUrl",
        &(url),
    )
    .await
    .with_context(|| format!("shareUrl({url}) to {device_id}"))?;

    Ok(())
}

/// Check whether a device currently supports a plugin, is reachable, and is
/// trusted — matching the Go `Device.supported()` logic.
pub(crate) fn check_capability(state: &DeviceState, plugin: &str) -> Result<()> {
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
