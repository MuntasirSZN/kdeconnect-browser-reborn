//! D-Bus signal match-rule registration.

use anyhow::{Context, Result};
use zbus::Connection;

use super::{BASE_PATH, DAEMON_IFACE, DEVICE_IFACE};

/// Signals emitted by `org.kde.kdeconnect.device`. Unlisted signals never
/// reach us: D-Bus only delivers members with a matching `AddMatch` rule.
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
pub(crate) async fn add_device_match_rules(conn: &Connection, object_path: &str) -> Result<()> {
    for member in SIGNAL_MEMBERS {
        let rule = format!(
            "type='signal',path='{object_path}',interface='{DEVICE_IFACE}',member='{member}'"
        );
        conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "AddMatch",
            &(rule.as_str()),
        )
        .await
        .with_context(|| format!("AddMatch({member})"))?;
    }
    Ok(())
}

/// Register a match rule for every signal from the KDE Connect daemon
/// (`deviceAdded`, `deviceRemoved`, `deviceVisibilityChanged`, …).
pub(crate) async fn add_daemon_match_rules(conn: &Connection) -> Result<()> {
    let rule = format!("type='signal',path='{BASE_PATH}',interface='{DAEMON_IFACE}'");
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
