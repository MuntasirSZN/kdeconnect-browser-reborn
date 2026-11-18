use std::collections::HashMap;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

#[cfg(feature = "ts-export")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Chrome native messaging protocol types
// ---------------------------------------------------------------------------

/// Messages exchanged with the browser extension over native messaging.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub struct Message {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(type = "unknown"))]
    pub data: Option<serde_json::Value>,
}

impl Message {
    #[allow(dead_code)]
    pub fn new(msg_type: MessageType) -> Self {
        Self {
            id: None,
            msg_type,
            data: None,
        }
    }

    pub fn result(msg_type: MessageType, data: serde_json::Value) -> Self {
        Self {
            id: None,
            msg_type,
            data: Some(data),
        }
    }
}

/// Discriminated message type matching Go's `messageType` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(export))]
pub enum MessageType {
    #[serde(rename = "typeDevices")]
    Devices,
    #[serde(rename = "typeShare")]
    Share,
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
pub struct ShareRequest {
    pub target: String,
    pub url: String,
}

// ---------------------------------------------------------------------------
// Device type (wire-compatible with Go serialization)
// ---------------------------------------------------------------------------

/// An empty JSON object `{}` — matches Go's `map[string]struct{}` which
/// serialises every key to an empty object rather than `null`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts-export", derive(TS), ts(type = "Record<string, never>"))]
pub struct Empty;

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
pub struct Device {
    pub id: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub name: String,
    pub icon_name: String,
    pub status_icon_name: String,
    pub is_reachable: bool,
    pub is_trusted: bool,
    pub supported_plugins: HashMap<String, Empty>,
}
