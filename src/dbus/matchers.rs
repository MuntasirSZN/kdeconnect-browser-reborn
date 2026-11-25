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

/// The single `AddMatch` call used for one rule on a real connection.
pub(crate) async fn add_match_rule(conn: &Connection, rule: &str) -> Result<()> {
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "AddMatch",
        &(rule),
    )
    .await
    .context("AddMatch")?;
    Ok(())
}

/// Register D-Bus match rules for every property-change signal on a device,
/// matching the Go `Device.addMatchSignal` calls for each member.
pub(crate) async fn add_device_match_rules<D: super::Dbus + ?Sized>(
    conn: &D,
    object_path: &str,
) -> Result<()> {
    for member in SIGNAL_MEMBERS {
        let rule = device_match_rule(object_path, member);
        conn.add_match(&rule)
            .await
            .with_context(|| format!("AddMatch({member})"))?;
    }
    Ok(())
}

/// Register a match rule for every signal from the KDE Connect daemon
/// (`deviceAdded`, `deviceRemoved`, `deviceVisibilityChanged`, …).
pub(crate) async fn add_daemon_match_rules<D: super::Dbus + ?Sized>(conn: &D) -> Result<()> {
    conn.add_match(&daemon_match_rule())
        .await
        .context("AddMatch(daemon signals)")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::dbus::MockDbus;

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

    /// Mock whose `add_match` pushes every received rule into a shared recording vector.
    fn recording_add_match_mock() -> (MockDbus, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let recorded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mock_record = recorded.clone();
        let mut mock = MockDbus::new();
        mock.expect_add_match().returning(move |rule| {
            mock_record
                .lock()
                .expect("recorded rules")
                .push(rule.to_owned());
            Ok(())
        });
        (mock, recorded)
    }

    #[tokio::test]
    async fn add_device_match_rules_registers_one_rule_per_signal() {
        let (mock, recorded) = recording_add_match_mock();

        add_device_match_rules(&mock, DEVICE_PATH)
            .await
            .expect("register rules");

        let mut actual = recorded.lock().expect("recorded rules").clone();
        actual.sort_unstable();
        let mut expected: Vec<String> = SIGNAL_MEMBERS
            .iter()
            .map(|member| device_match_rule(DEVICE_PATH, member))
            .collect();
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn add_daemon_match_rules_registers_the_daemon_rule() {
        let (mock, recorded) = recording_add_match_mock();

        add_daemon_match_rules(&mock).await.expect("register rule");
        assert_eq!(
            recorded.lock().expect("recorded rules").as_slice(),
            [daemon_match_rule()]
        );
    }

    #[tokio::test]
    async fn add_device_match_rules_stops_and_fails_when_a_rule_fails() {
        use crate::dbus::MockDbus;

        let mut mock = MockDbus::new();
        mock.expect_add_match()
            .times(1)
            .returning(|_rule| Err(anyhow::anyhow!("bus is down")));

        assert!(add_device_match_rules(&mock, DEVICE_PATH).await.is_err());
    }

    #[test]
    fn daemon_rule_matches_every_daemon_signal() {
        assert_eq!(
            daemon_match_rule(),
            "type='signal',path='/modules/kdeconnect',interface='org.kde.kdeconnect.daemon'"
        );
    }
}
