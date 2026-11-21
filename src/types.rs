use std::collections::HashMap;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

#[cfg(feature = "ts-export")]
use ts_rs::TS;

/// Messages exchanged with the browser extension over native messaging.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub(crate) struct Message {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(type = "unknown"))]
    pub data: Option<serde_json::Value>,
}

impl Message {
    pub(crate) fn result(msg_type: MessageType, data: serde_json::Value) -> Self {
        Self {
            id: None,
            msg_type,
            data: Some(data),
        }
    }

    /// Build an outbound message that echoes the request's correlation id.
    pub(crate) fn reply(id: Option<&str>, msg_type: MessageType, data: serde_json::Value) -> Self {
        Self {
            id: id.map(str::to_owned),
            msg_type,
            data: Some(data),
        }
    }
}

/// Discriminated message type matching Go's `messageType` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub(crate) enum MessageType {
    #[serde(rename = "typeDevices")]
    Devices,
    #[serde(rename = "typeShare")]
    Share,
    #[serde(rename = "typeShareResult")]
    ShareResult,
    #[serde(rename = "typeDeviceUpdate")]
    DeviceUpdate,
    #[serde(rename = "typeError")]
    Error,
    #[serde(rename = "typeVersion")]
    Version,
}

/// Share request payload — sent by the browser extension.
#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub(crate) struct ShareRequest {
    pub target: String,
    pub url: String,
    /// Base64-encoded image bytes, sent for image context clicks. When present,
    /// the host writes the data to a temp file and shares that instead of the
    /// URL — KDE Connect shares local files via their `file://` URL.
    #[serde(default, rename = "imageData")]
    pub image_data: Option<String>,
}

/// An empty JSON object `{}` — matches Go's `map[string]struct{}` which
/// serialises every key to an empty object rather than `null`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(type = "Record<string, never>"))]
pub(crate) struct Empty;

impl Serialize for Empty {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_map(Some(0))?.end()
    }
}

/// A KDE Connect device as seen over D-Bus.
///
/// Field names match the Go `Device` struct's JSON tags exactly for wire
/// compatibility with the existing browser extension.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub(crate) struct Device {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub icon_name: String,
    pub status_icon_name: String,
    pub is_reachable: bool,
    pub is_trusted: bool,
    #[expect(
        clippy::zero_sized_map_values,
        reason = "the set of plugin names is modelled as an object map over the zero-sized marker type; a real set would change the extension wire format"
    )]
    pub supported_plugins: HashMap<String, Empty>,
}

impl Device {
    /// Build a device from a `Properties.GetAll` map.
    ///
    /// Every property is optional. Missing or wrongly typed values fall back to
    /// empty strings, `false`, or an empty plugin map rather than dropping the
    /// device.
    pub(crate) fn from_props(id: &str, props: &HashMap<String, serde_json::Value>) -> Self {
        let string = |key: &str| {
            props
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let boolean = |key: &str| {
            props
                .get(key)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };

        let name = string("name");
        let supported_plugins = props
            .get("supportedPlugins")
            .and_then(serde_json::Value::as_array)
            .map(|plugins| {
                plugins
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(|plugin| (plugin.to_owned(), Empty))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            id: id.to_owned(),
            kind: string("type"),
            name: if name.is_empty() {
                "Unknown device".to_owned()
            } else {
                name
            },
            icon_name: string("iconName"),
            status_icon_name: string("statusIconName"),
            is_reachable: boolean("isReachable"),
            is_trusted: props
                .get("isPaired")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or_else(|| boolean("isTrusted")),
            supported_plugins,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(value: serde_json::Value) -> HashMap<String, serde_json::Value> {
        serde_json::from_value(value).expect("object")
    }

    fn sample() -> HashMap<String, serde_json::Value> {
        props(serde_json::json!({
            "name": "Pixel 9",
            "type": "phone",
            "iconName": "smartphone",
            "statusIconName": "smartphoneconnected",
            "isPaired": true,
            "isReachable": true,
            "supportedPlugins": ["kdeconnect_share"]
        }))
    }

    #[test]
    fn is_paired_wins() {
        let d = Device::from_props("dev1", &sample());
        assert_eq!(d.id, "dev1");
        assert_eq!(d.name, "Pixel 9");
        assert_eq!(d.kind, "phone");
        assert_eq!(d.icon_name, "smartphone");
        assert_eq!(d.status_icon_name, "smartphoneconnected");
        assert!(d.is_reachable);
        assert!(d.is_trusted);
        assert!(d.supported_plugins.contains_key("kdeconnect_share"));
    }

    #[test]
    fn is_trusted_is_a_fallback() {
        let mut p = sample();
        p.remove("isPaired");
        p.insert("isTrusted".into(), serde_json::json!(true));
        assert!(Device::from_props("dev1", &p).is_trusted);
    }

    #[test]
    fn explicit_unpaired_state_overrides_legacy_trusted_value() {
        let mut p = sample();
        p.insert("isPaired".into(), serde_json::json!(false));
        p.insert("isTrusted".into(), serde_json::json!(true));
        assert!(!Device::from_props("dev1", &p).is_trusted);
    }

    #[test]
    fn unpaired_when_both_false() {
        let mut p = sample();
        p.insert("isPaired".into(), serde_json::json!(false));
        p.insert("isTrusted".into(), serde_json::json!(false));
        assert!(!Device::from_props("dev1", &p).is_trusted);
    }

    #[test]
    fn missing_optional_props_get_defaults() {
        let d = Device::from_props("dev1", &props(serde_json::json!({})));
        assert_eq!(d.name, "Unknown device");
        assert_eq!(d.kind, "");
        assert_eq!(d.icon_name, "");
        assert_eq!(d.status_icon_name, "");
        assert!(!d.is_reachable);
        assert!(!d.is_trusted);
        assert!(d.supported_plugins.is_empty());
    }

    #[test]
    fn unexpected_types_do_not_panic() {
        let p = props(serde_json::json!({
            "name": 7,
            "isReachable": "yes",
            "supportedPlugins": "nope"
        }));
        let d = Device::from_props("dev1", &p);
        assert_eq!(d.name, "Unknown device");
        assert!(!d.is_reachable);
        assert!(d.supported_plugins.is_empty());
    }
}
