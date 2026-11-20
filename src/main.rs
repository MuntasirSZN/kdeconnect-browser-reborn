//! KDE Connect Browser Reborn — Native Messaging Host.
//!
//! Bridges the browser extension with KDE Connect over D-Bus.
//!
//! ## Usage modes
//!
//! | Flag | Action |
//! |------|--------|
//! | _(none)_ | Run native messaging daemon (stdin → D-Bus → stdout) |
//! | `--install` | Interactive self-install for one or more browsers |
//! | `--install --dev`  | Same, but prompts for a custom extension ID |
//! | `--version`   | Print version info and exit |
//! | `--devices`   | List known KDE Connect devices and exit |

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

mod dbus;
mod install;
mod messaging;
mod types;

use dbus::SharedDevice;
use messaging::{Reader, Writer};
use types::{Message, MessageType, ShareRequest};

// ---------------------------------------------------------------------------
// Constants (matching Go's protocol/cli versions)
// ---------------------------------------------------------------------------

const PROTOCOL_VERSION: &str = "0.1.3";
const CLI_VERSION: &str = "0.1.7";

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "kdeconnect-browser-reborn",
    version = CLI_VERSION,
    disable_version_flag = true,
)]
struct Cli {
    /// Perform interactive install into browser native messaging hosts dirs.
    #[arg(long)]
    install: bool,

    /// Install as developer — prompts for custom extension ID.
    #[arg(long)]
    dev: bool,

    /// Display version and exit.
    #[arg(long)]
    version: bool,

    /// List visible KDE Connect devices and exit.
    #[arg(long)]
    devices: bool,

    /// Arguments passed by the browser when launching the native messaging
    /// host (Chrome: extension origin; Firefox: manifest path + extension ID).
    /// Ignored.
    #[arg(hide = true, num_args = 0..)]
    #[allow(dead_code)]
    browser_args: Vec<String>,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stderr());
    // Initialise structured logging to stderr (replaces Go's log()).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(non_blocking)
        .init();

    let cli = Cli::parse();

    // --version
    if cli.version {
        println!(
            "kdeconnect-browser-reborn version {} (protocol {}), built with rustc",
            CLI_VERSION, PROTOCOL_VERSION,
        );
        return Ok(());
    }

    // --install / --install --dev
    if cli.install {
        return install::install(cli.dev).context("installation failed");
    }

    // ---- D-Bus connection (required for --devices and daemon mode) ----
    let conn = match dbus::open_connection().await {
        Ok(c) => c,
        Err(e) => {
            if !cli.devices {
                write_protocol_error(&format!("cannot connect to the session bus: {e:#}")).await;
            }
            return Err(e).context("open session bus");
        }
    };

    // Enumerate devices
    let devices: Vec<SharedDevice> = match dbus::discover_devices(&conn).await {
        Ok(d) => d,
        Err(e) => {
            if !cli.devices {
                write_protocol_error(&format!("KDE Connect unavailable: {e:#}")).await;
            }
            return Err(e).context("discover KDE Connect devices");
        }
    };

    // --devices
    if cli.devices {
        for state in &devices {
            let dev = state.lock().await;
            println!(
                "- {}: {} (paired: {}; reachable: {})",
                dev.device.name, dev.device.id, dev.device.is_trusted, dev.device.is_reachable,
            );
        }
        return Ok(());
    }

    // ---- Default: native messaging daemon ----
    run_daemon(conn, devices).await
}

// ---------------------------------------------------------------------------
// Daemon mode
// ---------------------------------------------------------------------------

async fn run_daemon(conn: zbus::Connection, devices: Vec<SharedDevice>) -> Result<()> {
    info!(count = devices.len(), "devices discovered");

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

    // ---- Channels ----
    // Outbound: signal handlers & request responses → stdout.
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    // ---- Spawn signal listener (D-Bus signals → output_tx) ----
    let signal_task = dbus::spawn_signal_listener(conn.clone(), devices.clone(), output_tx.clone());

    // ---- Stdout writer task ----
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

    // ---- Stdin reader (main loop) ----
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

    // ---- Cleanup ----
    // The listener holds a sender clone; stopping it lets the writer drain and
    // finish instead of waiting forever.
    drop(output_tx);
    signal_task.abort();
    let _ = stdout_handle.await;

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
        _ = output_tx.closed() => InputEvent::StdoutClosed,
        result = reader.read() => InputEvent::Read(result),
    }
}

// ---------------------------------------------------------------------------
// Message dispatch
// ---------------------------------------------------------------------------

/// Tell the extension why the host cannot run, before exiting.  Failures to
/// write are ignored — stderr already carries the error.
async fn write_protocol_error(text: &str) {
    let mut writer = Writer::stdout();
    let msg = Message::result(
        MessageType::Error,
        serde_json::Value::String(text.to_owned()),
    );
    let _ = writer.write(&msg).await;
}

async fn handle_message(
    conn: &zbus::Connection,
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    msg: &Message,
    output_tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> Result<()> {
    match msg.msg_type {
        MessageType::Devices => {
            let current_devices = devices.lock().await.clone();
            let data = dbus::devices_snapshot(&current_devices).await;
            let response = Message::result(MessageType::Devices, data);
            let _ = output_tx.send(response);
        }

        MessageType::Share => {
            if let Err(e) = handle_share(conn, devices, msg).await {
                let _ = output_tx.send(Message::result(
                    MessageType::Error,
                    serde_json::Value::String(format!("{e:#}")),
                ));
            }
        }

        MessageType::Version => {
            let response = Message::result(
                MessageType::Version,
                serde_json::Value::String(PROTOCOL_VERSION.to_owned()),
            );
            let _ = output_tx.send(response);
        }

        MessageType::DeviceUpdate => {
            // Outbound-only — log if the extension sends one.
            warn!("unexpected DeviceUpdate from extension");
        }

        MessageType::Error => {
            // Outbound-only — log anything we receive.
            warn!(data = ?msg.data, "error message from extension");
        }
    }

    Ok(())
}

async fn handle_share(
    conn: &zbus::Connection,
    devices: &Arc<Mutex<Vec<SharedDevice>>>,
    msg: &Message,
) -> Result<()> {
    let share: ShareRequest = msg
        .data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.clone()).ok())
        .ok_or_else(|| anyhow::anyhow!("invalid share payload: {:?}", msg.data))?;

    if !is_shareable_url(&share.url) {
        anyhow::bail!("unsupported URL scheme; only http and https URLs can be shared");
    }

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

    match device_arc {
        Some(d) => {
            let state = d.lock().await;
            dbus::check_capability(&state, dbus::PLUGIN_SHARE)?;
            dbus::share_url(conn, &state.device.id, &share.url).await?;
        }
        None => anyhow::bail!("unknown device: {}", share.target),
    }

    Ok(())
}

fn is_shareable_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    url.starts_with("http://") || url.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_url_schemes_are_shareable() {
        assert!(is_shareable_url("https://example.com/page"));
        assert!(is_shareable_url("http://example.com"));
        assert!(!is_shareable_url("tel:+15551234"));
        assert!(!is_shareable_url("javascript:alert(1)"));
        assert!(!is_shareable_url("file:///etc/passwd"));
        assert!(!is_shareable_url("chrome://settings"));
    }

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

    #[test]
    fn browser_arguments_are_accepted() {
        let cli = Cli::try_parse_from([
            "kdeconnect-browser-reborn",
            "/home/user/.mozilla/native-messaging-hosts/com.0xc0dedbad.kdeconnect_chrome.json",
            "kde-connect@0xc0dedbad.com",
        ])
        .expect("Firefox argv");
        assert!(!cli.install && !cli.dev && !cli.version && !cli.devices);

        Cli::try_parse_from(["kdeconnect-browser-reborn", "chrome-extension://abcdef/"])
            .expect("Chrome argv");
    }
}
