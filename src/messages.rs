//! Request handling for messages received from the browser extension.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tracing::warn;
use zbus::Connection;

use crate::dbus::{self, Dbus, SharedDevice};
use crate::image::write_image_to_temp;
use crate::types::{Message, MessageType, ShareRequest};

pub(crate) const PROTOCOL_VERSION: &str = "1.0.0";

/// The host's reply to a `typeVersion` request.
fn version_response() -> Message {
    Message::result(
        MessageType::Version,
        serde_json::Value::String(PROTOCOL_VERSION.to_owned()),
    )
}

/// Decode the extension's share payload, or explain why it is unusable.
fn parse_share_request(data: Option<&serde_json::Value>) -> Result<ShareRequest> {
    data.and_then(|d| serde_json::from_value(d.clone()).ok())
        .ok_or_else(|| anyhow::anyhow!("invalid share payload: {data:?}"))
}

/// Find one watched device by its KDE Connect id.
async fn find_device_by_id(
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    id: &str,
) -> Option<SharedDevice> {
    // Clone the list before awaiting individual device locks. A brief lock
    // contention must not make a valid target appear to be an unknown ID.
    let candidates = devices.lock().await.clone();
    for candidate in candidates {
        let matches = candidate.lock().await.device.id == id;
        if matches {
            return Some(candidate);
        }
    }
    None
}

pub(crate) async fn handle_message(
    conn: &Connection,
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    msg: &Message,
    output_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<()> {
    match msg.msg_type {
        MessageType::Devices => {
            let current_devices = devices.lock().await.clone();
            let data = dbus::devices_snapshot(&current_devices).await;
            let response = Message::result(MessageType::Devices, data);
            drop(output_tx.send(response));
        }

        MessageType::Share => {
            let result = match handle_share(conn, devices, msg).await {
                Ok(()) => serde_json::json!({ "ok": true }),
                Err(e) => serde_json::json!({ "ok": false, "error": format!("{e:#}") }),
            };
            drop(output_tx.send(Message::reply(
                msg.id.as_deref(),
                MessageType::ShareResult,
                result,
            )));
        }

        MessageType::Version => {
            drop(output_tx.send(version_response()));
        }

        MessageType::DeviceUpdate => {
            // Outbound-only — log if the extension sends one.
            warn!("unexpected DeviceUpdate from extension");
        }

        MessageType::ShareResult => {
            // Outbound-only — log if the extension sends one.
            warn!("unexpected ShareResult from extension");
        }

        MessageType::Error => {
            // Outbound-only — log anything we receive.
            warn!(data = ?msg.data, "error message from extension");
        }
    }

    Ok(())
}

pub(crate) async fn handle_share(
    conn: &Connection,
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    msg: &Message,
) -> Result<()> {
    let share: ShareRequest = parse_share_request(msg.data.as_ref())?;

    // Resolve and validate the target *before* touching the filesystem: the
    // id is extension-supplied and must never influence a temp path.
    let Some(d) = find_device_by_id(devices, &share.target).await else {
        anyhow::bail!("unknown device: {}", share.target);
    };
    let state = d.lock().await;
    dbus::check_capability(&state, dbus::PLUGIN_SHARE)?;
    let device_id = state.device.id.clone();
    drop(state);

    // The extension sends the image bytes directly for image context clicks —
    // KDE Connect shares local files via their file:// URL, so write the data
    // to a temp file and share that instead of the link.
    let url = if share.image_data.is_some() {
        write_image_to_temp(&share)?
    } else {
        if !is_shareable_url(&share.url) {
            anyhow::bail!("unsupported URL scheme; only http, https, and tel URLs can be shared");
        }
        share.url.clone()
    };

    conn.share_url(&device_id, &url).await?;

    Ok(())
}

/// Returns whether `url` is a shareable `http(s)` link with a non-empty host
/// or a `tel:` link with a non-empty number.
fn is_shareable_url(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| match u.scheme() {
        "http" | "https" => u.has_host(),
        "tel" => !u.path().is_empty(),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::DeviceState;
    use crate::types::Device;
    use std::collections::HashMap;

    #[expect(
        clippy::zero_sized_map_values,
        reason = "mirrors the wire format: supported plugins are a set of names"
    )]
    fn device(id: &str) -> SharedDevice {
        Arc::new(Mutex::new(DeviceState {
            device: Device {
                id: id.to_owned(),
                kind: "phone".to_owned(),
                name: format!("name-{id}"),
                icon_name: String::new(),
                status_icon_name: String::new(),
                is_reachable: true,
                is_trusted: true,
                supported_plugins: HashMap::new(),
            },
            object_path: format!("/modules/kdeconnect/devices/{id}"),
        }))
    }

    #[test]
    fn protocol_version_matches_the_crate_version() {
        assert_eq!(PROTOCOL_VERSION, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn version_response_carries_the_protocol_version() {
        let json = serde_json::to_value(version_response()).expect("serialize version reply");
        assert_eq!(
            json,
            serde_json::json!({ "type": "typeVersion", "data": PROTOCOL_VERSION })
        );
    }

    #[test]
    fn share_payload_decodes_url_and_image_requests() {
        let url = parse_share_request(Some(&serde_json::json!({
            "target": "dev1",
            "url": "https://example.com",
        })))
        .expect("URL payload");
        assert_eq!(url.target, "dev1");
        assert_eq!(url.url, "https://example.com");
        assert!(url.image_data.is_none());

        let image = parse_share_request(Some(&serde_json::json!({
            "target": "dev1",
            "url": "https://example.com/img.png",
            "imageData": "aGVsbG8=",
        })))
        .expect("image payload");
        assert_eq!(image.image_data.as_deref(), Some("aGVsbG8="));
    }

    #[test]
    fn share_payload_rejects_missing_or_incomplete_data() {
        assert!(parse_share_request(None).is_err());
        assert!(parse_share_request(Some(&serde_json::json!({}))).is_err());
        assert!(
            parse_share_request(Some(&serde_json::json!({ "target": "dev1" }))).is_err(),
            "url is required"
        );
        assert!(
            parse_share_request(Some(&serde_json::json!({
                "target": 7,
                "url": "https://example.com",
            })))
            .is_err(),
            "target must be a string"
        );
    }

    #[tokio::test]
    async fn find_device_by_id_selects_the_matching_device() {
        let devices = Arc::new(Mutex::new(vec![device("dev1"), device("dev2")]));

        let found = find_device_by_id(&devices, "dev2")
            .await
            .expect("dev2 is watched");
        let state = found.lock().await;
        assert_eq!(state.device.id, "dev2");
        assert_eq!(state.object_path, "/modules/kdeconnect/devices/dev2");
        drop(state);

        assert!(find_device_by_id(&devices, "missing").await.is_none());
    }

    #[test]
    fn only_supported_url_schemes_are_shareable() {
        assert!(is_shareable_url("https://example.com/page"));
        assert!(is_shareable_url("http://example.com"));
        assert!(is_shareable_url("HTTP://Example.COM/Page"));
        assert!(is_shareable_url("tel:+15551234"));
        assert!(is_shareable_url("TEL:+1-555-1234;ext=5"));
        assert!(!is_shareable_url("http://"));
        assert!(!is_shareable_url("not a url"));
        assert!(!is_shareable_url("tel:"));
        assert!(!is_shareable_url("javascript:alert(1)"));
        assert!(!is_shareable_url("file:///etc/passwd"));
        assert!(!is_shareable_url("chrome://settings"));
    }
}
