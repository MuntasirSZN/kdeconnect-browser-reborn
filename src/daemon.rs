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
use crate::types::{Message, MessageType};

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
    let msg = Message::result(
        MessageType::Error,
        serde_json::Value::String(text.to_owned()),
    );
    drop(writer.write(&msg).await);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;

    #[tokio::test]
    async fn stdin_reader_stops_when_stdout_receiver_is_dropped() {
        let (_stdin_tx, stdin_rx) = tokio::io::duplex(64);
        let mut reader = Reader::new(stdin_rx);
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

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
        let (_stdin_tx, stdin_rx) = tokio::io::duplex(64);
        let mut reader = Reader::new(stdin_rx);
        let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
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
