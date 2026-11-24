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

/// The `AddMatch` rule for one property-change signal on a device.
fn device_match_rule(object_path: &str, member: &str) -> String {
    format!("type='signal',path='{object_path}',interface='{DEVICE_IFACE}',member='{member}'")
}

/// The `AddMatch` rule for every signal emitted by the KDE Connect daemon
/// (`deviceAdded`, `deviceRemoved`, `deviceVisibilityChanged`, …).
fn daemon_match_rule() -> String {
    format!("type='signal',path='{BASE_PATH}',interface='{DAEMON_IFACE}'")
}

/// Register D-Bus match rules for every property-change signal on a device,
/// matching the Go `Device.addMatchSignal` calls for each member.
pub(crate) async fn add_device_match_rules(conn: &Connection, object_path: &str) -> Result<()> {
    for member in SIGNAL_MEMBERS {
        let rule = device_match_rule(object_path, member);
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

/// Register a match rule for every signal from the KDE Connect daemon.
pub(crate) async fn add_daemon_match_rules(conn: &Connection) -> Result<()> {
    let rule = daemon_match_rule();
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

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE_PATH: &str = "/modules/kdeconnect/devices/abc123";

    #[test]
    fn device_rules_name_the_device_interface_and_signal() {
        for member in SIGNAL_MEMBERS {
            let rule = device_match_rule(DEVICE_PATH, member);
            assert_eq!(
                rule,
                format!(
                    "type='signal',path='{DEVICE_PATH}',interface='org.kde.kdeconnect.device',member='{member}'"
                )
            );
        }
    }

    #[test]
    fn daemon_rule_matches_every_daemon_signal() {
        assert_eq!(
            daemon_match_rule(),
            "type='signal',path='/modules/kdeconnect',interface='org.kde.kdeconnect.daemon'"
        );
    }
}
