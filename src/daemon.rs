//! Native-messaging daemon: the stdin input loop, the stdout writer task,
//! and stdin-shutdown handling.

use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Result;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::dbus::{self, SharedDevice};
use crate::image::{SHARE_TEMP_MAX_AGE, cleanup_stale_share_dirs};
use crate::messages::handle_message;
use crate::messaging::{Reader, Writer};
use crate::types::{Device, Message, MessageType};

/// One `--devices` listing line.
pub(crate) fn devices_list_line(dev: &Device) -> String {
    format!(
        "- {}: {} (paired: {}; reachable: {})",
        dev.name, dev.id, dev.is_trusted, dev.is_reachable
    )
}

/// Run the daemon: register match rules, forward device signals, and serve stdin.
pub(crate) async fn run_daemon(conn: zbus::Connection, devices: Vec<SharedDevice>) -> Result<()> {
    info!(count = devices.len(), "devices discovered");

    cleanup_stale_share_dirs(&std::env::temp_dir(), SystemTime::now(), SHARE_TEMP_MAX_AGE);

    // Register D-Bus signal match rules for each device.
    for state in &devices {
        let obj_path = state.lock().await.object_path.clone();
        if let Err(e) = dbus::add_device_match_rules(&conn, &obj_path).await {
            warn!(path = %obj_path, error = %e, "failed to add match rules");
        }
    }
    if let Err(e) = dbus::add_daemon_match_rules(&conn).await {
        warn!(error = %e, "failed to add daemon match rules");
    }

    let devices = Arc::new(Mutex::new(devices));

    // Outbound: signal handlers & request responses → stdout.
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let signal_task = dbus::spawn_signal_listener(conn.clone(), devices.clone(), output_tx.clone());

    let mut writer = Writer::stdout();
    let stdout_handle = tokio::spawn(async move {
        while let Some(msg) = output_rx.recv().await {
            if let Err(e) = writer.write(&msg).await {
                error!(error = %e, "stdout write error");

                // Never silently lose an oversized device snapshot. Send a
                // small bounded protocol error so the extension can surface it.
                let fallback = Message::result(
                    MessageType::Error,
                    serde_json::Value::String(format!("Unable to send host response: {e}")),
                );
                if let Err(fallback_error) = writer.write(&fallback).await {
                    error!(error = %fallback_error, "failed to send fallback protocol error");
                    break;
                }
            }
        }
    });

    let mut reader = Reader::stdin();
    loop {
        let msg = match read_input_or_output_closed(&mut reader, &output_tx).await {
            InputEvent::StdoutClosed => {
                error!("stdout writer exited, shutting down stdin reader");
                break;
            }
            InputEvent::Read(Ok(Some(m))) => m,
            InputEvent::Read(Ok(None)) => {
                info!("stdin closed, shutting down");
                break;
            }
            InputEvent::Read(Err(e)) => {
                // Framing and pipe errors are fatal: retrying cannot recover
                // the stream position and may otherwise spin forever.
                error!(error = %e, "stdin read error, shutting down");
                break;
            }
        };

        if let Err(e) = handle_message(&conn, &devices, &msg, &output_tx).await {
            error!(error = %e, "handling message {:?}", msg.msg_type);
        }
    }

    // The listener holds a sender clone; stopping it lets the writer drain and
    // finish instead of waiting forever.
    drop(output_tx);
    signal_task.abort();
    drop(stdout_handle.await);

    Ok(())
}

/** Outcome of one poll of `read_input_or_output_closed`: a message, or the shutdown signal. */
enum InputEvent {
    StdoutClosed,
    Read(Result<Option<Message>>),
}

/// Read the next stdin message, or stop waiting if stdout can no longer
/// deliver responses. The select also wakes a pending stdin read on shutdown.
async fn read_input_or_output_closed<R>(
    reader: &mut Reader<R>,
    output_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> InputEvent
where
    R: tokio::io::AsyncRead + Unpin,
{
    if output_tx.is_closed() {
        return InputEvent::StdoutClosed;
    }

    tokio::select! {
        biased;
        () = output_tx.closed() => InputEvent::StdoutClosed,
        result = reader.read() => InputEvent::Read(result),
    }
}

/// Tell the extension why the host cannot run, before exiting.  Failures to
/// write are ignored — stderr already carries the error.
pub(crate) async fn write_protocol_error(text: &str) {
    let mut writer = Writer::stdout();
    drop(
        writer
            .write(&Message::result(
                MessageType::Error,
                serde_json::Value::String(text.to_owned()),
            ))
            .await,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use std::collections::HashMap;

    #[expect(
        clippy::zero_sized_map_values,
        reason = "mirrors the wire format: supported plugins are a set of names"
    )]
    fn device() -> Device {
        Device {
            id: "dev1".to_owned(),
            kind: "phone".to_owned(),
            name: "Pixel".to_owned(),
            icon_name: String::new(),
            status_icon_name: String::new(),
            is_reachable: false,
            is_trusted: false,
            supported_plugins: HashMap::new(),
        }
    }

    #[test]
    fn devices_list_lines_show_pairing_and_reachability() {
        let mut paired = device();
        paired.is_reachable = true;
        paired.is_trusted = true;
        assert_eq!(
            devices_list_line(&paired),
            "- Pixel: dev1 (paired: true; reachable: true)"
        );
        assert_eq!(
            devices_list_line(&device()),
            "- Pixel: dev1 (paired: false; reachable: false)"
        );
    }

    /// Fresh `Reader` over a duplex stream plus the output channel and the
    /// stdin writer for the `read_input_or_output_closed` tests.
    fn reader_with_output() -> (
        Reader<tokio::io::DuplexStream>,
        tokio::sync::mpsc::UnboundedSender<Message>,
        tokio::sync::mpsc::UnboundedReceiver<Message>,
        tokio::io::DuplexStream,
    ) {
        let (stdin_tx, stdin_rx) = tokio::io::duplex(64);
        let reader = Reader::new(stdin_rx);
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        (reader, output_tx, output_rx, stdin_tx)
    }

    #[tokio::test]
    async fn stdin_reader_stops_when_stdout_receiver_is_dropped() {
        let (mut reader, output_tx, output_rx, _stdin_tx) = reader_with_output();

        let reader_task =
            tokio::spawn(async move { read_input_or_output_closed(&mut reader, &output_tx).await });

        // Let the reader reach its pending stdin read before closing stdout.
        tokio::task::yield_now().await;
        drop(output_rx);

        assert!(matches!(
            reader_task.await.expect("reader task"),
            InputEvent::StdoutClosed
        ));
    }

    #[tokio::test]
    async fn an_already_closed_stdout_short_circuits_the_read() {
        let (mut reader, output_tx, output_rx, _stdin_tx) = reader_with_output();
        drop(output_rx);

        assert!(matches!(
            read_input_or_output_closed(&mut reader, &output_tx).await,
            InputEvent::StdoutClosed
        ));
    }

    #[tokio::test]
    async fn stdin_eof_is_a_clean_read_none() {
        let (stdin_tx, stdin_rx) = tokio::io::duplex(64);
        drop(stdin_tx);
        let mut reader = Reader::new(stdin_rx);
        let (output_tx, _output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        assert!(matches!(
            read_input_or_output_closed(&mut reader, &output_tx).await,
            InputEvent::Read(Ok(None))
        ));
    }

    #[tokio::test]
    async fn framed_stdin_is_read_as_one_message() {
        let (stdin_tx, stdin_rx) = tokio::io::duplex(1024);
        let mut writer = Writer::new(stdin_tx);
        writer
            .write(&Message::result(
                MessageType::Version,
                serde_json::json!("probe"),
            ))
            .await
            .expect("write frame");
        let mut reader = Reader::new(stdin_rx);
        let (output_tx, _output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        let InputEvent::Read(Ok(Some(msg))) =
            read_input_or_output_closed(&mut reader, &output_tx).await
        else {
            panic!("expected one framed message");
        };
        assert_eq!(msg.msg_type, MessageType::Version);
        assert_eq!(msg.data, Some(serde_json::json!("probe")));
    }

    #[tokio::test]
    async fn a_truncated_frame_surfaces_as_a_read_error() {
        let mut reader = Reader::new(&[0_u8, 1][..]);
        let (output_tx, _output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        assert!(matches!(
            read_input_or_output_closed(&mut reader, &output_tx).await,
            InputEvent::Read(Err(_))
        ));
    }
}
