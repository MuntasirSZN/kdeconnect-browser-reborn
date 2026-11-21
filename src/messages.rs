//! Request handling for messages received from the browser extension.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tracing::warn;
use zbus::Connection;

use crate::dbus::{self, SharedDevice};
use crate::image::write_image_to_temp;
use crate::types::{Message, MessageType, ShareRequest};

pub(crate) const PROTOCOL_VERSION: &str = "0.1.4";

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
            let response = Message::result(
                MessageType::Version,
                serde_json::Value::String(PROTOCOL_VERSION.to_owned()),
            );
            drop(output_tx.send(response));
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
    let share: ShareRequest = msg
        .data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .ok_or_else(|| anyhow::anyhow!("invalid share payload: {:?}", msg.data))?;

    // Resolve and validate the target *before* touching the filesystem: the
    // id is extension-supplied and must never influence a temp path.
    // Clone the list before awaiting individual device locks. A brief lock
    // contention must not make a valid target appear to be an unknown ID.
    let candidates = devices.lock().await.clone();
    let mut device_arc = None;
    for candidate in candidates {
        let matches = candidate.lock().await.device.id == share.target;
        if matches {
            device_arc = Some(candidate);
            break;
        }
    }

    let Some(d) = device_arc else {
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

    dbus::share_url(conn, &device_id, &url).await?;

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
