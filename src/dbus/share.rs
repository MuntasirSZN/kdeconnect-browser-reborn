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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::PLUGIN_SHARE;
    use crate::types::{Device, Empty};
    use std::collections::HashMap;

    #[expect(
        clippy::zero_sized_map_values,
        reason = "mirrors the wire format: supported plugins are a set of names"
    )]
    fn state(plugin: Option<&str>, reachable: bool, trusted: bool) -> DeviceState {
        DeviceState {
            device: Device {
                id: "dev1".to_owned(),
                kind: "phone".to_owned(),
                name: "Pixel".to_owned(),
                icon_name: String::new(),
                status_icon_name: String::new(),
                is_reachable: reachable,
                is_trusted: trusted,
                supported_plugins: plugin
                    .map(|name| (name.to_owned(), Empty))
                    .into_iter()
                    .collect::<HashMap<_, _>>(),
            },
            object_path: "/modules/kdeconnect/devices/dev1".to_owned(),
        }
    }

    #[test]
    fn supported_reachable_and_trusted_devices_pass() {
        assert!(check_capability(&state(Some(PLUGIN_SHARE), true, true), PLUGIN_SHARE).is_ok());
    }

    #[test]
    fn a_missing_plugin_is_rejected() {
        let error = check_capability(&state(Some("kdeconnect_ping"), true, true), PLUGIN_SHARE)
            .expect_err("share plugin missing");
        assert_eq!(
            error.to_string(),
            "device dev1 does not support plugin kdeconnect_share"
        );
    }

    #[test]
    fn reachability_is_checked_before_trust() {
        let unreachable = check_capability(&state(Some(PLUGIN_SHARE), false, false), PLUGIN_SHARE)
            .expect_err("unreachable");
        assert_eq!(unreachable.to_string(), "device dev1 is not reachable");

        let untrusted = check_capability(&state(Some(PLUGIN_SHARE), true, false), PLUGIN_SHARE)
            .expect_err("untrusted");
        assert_eq!(untrusted.to_string(), "device dev1 is not trusted");
    }
}
