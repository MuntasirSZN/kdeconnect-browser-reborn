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

#![deny(missing_docs)]
// LINEBENDER LINT SET - lib.rs - v4
// See https://linebender.org/wiki/canonical-lints/
// These lints shouldn't apply to examples or tests.
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
// These lints shouldn't apply to examples.
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use base64::Engine;
use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

// The `Simd` engine is only exported on x86_64/aarch64; elsewhere we fall
// back to the scalar standard engine.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use base64::engine::Simd;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use base64::engine::general_purpose::GeneralPurposeConfig;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;

mod dbus;
mod install;
mod messaging;
mod types;

use dbus::SharedDevice;
use messaging::{Reader, Writer};
use types::{Message, MessageType, ShareRequest};

const PROTOCOL_VERSION: &str = "0.1.4";
const CLI_VERSION: &str = "0.1.7";

#[expect(
    clippy::struct_excessive_bools,
    reason = "four boolean mode flags; each gates a distinct exit path"
)]
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
    #[allow(
        dead_code,
        reason = "browser launchers pass these args; the daemon ignores them"
    )]
    browser_args: Vec<String>,
}

#[tokio::main(flavor = "current_thread")]
#[expect(
    clippy::print_stdout,
    reason = "CLI output: version and device listings go to stdout"
)]
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
            "kdeconnect-browser-reborn version {CLI_VERSION} (protocol {PROTOCOL_VERSION}), built with rustc",
        );
        return Ok(());
    }

    // --install / --install --dev
    if cli.install {
        return install::install(cli.dev).context("installation failed");
    }

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

    run_daemon(conn, devices).await
}

async fn run_daemon(conn: zbus::Connection, devices: Vec<SharedDevice>) -> Result<()> {
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
async fn write_protocol_error(text: &str) {
    let mut writer = Writer::stdout();
    let msg = Message::result(
        MessageType::Error,
        serde_json::Value::String(text.to_owned()),
    );
    drop(writer.write(&msg).await);
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

/// Decode helper: uses the `Simd` engine, which picks the best instruction
/// set at runtime (AVX2 on `x86_64`, NEON on aarch64) and falls back to the
/// scalar engine. Other targets use the scalar `STANDARD` engine.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn decode_image_data(data: &str) -> Result<Vec<u8>> {
    let engine = Simd::standard(GeneralPurposeConfig::new());
    Ok(engine.decode(data)?)
}

/// Decode helper: uses the scalar `STANDARD` engine on non-SIMD targets.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn decode_image_data(data: &str) -> Result<Vec<u8>> {
    Ok(BASE64_ENGINE.decode(data)?)
}

/// Write the base64 image data sent by the extension to a temp file and
/// return its `file://` URL.
///
/// KDE Connect's share plugin shares local files via their `file://` URL
/// (`SharePlugin::shareUrl` in kdeconnect-kde sends a payload when the URL
/// is a local file), so handing it a real file is how images reach the phone.
fn write_image_to_temp(share: &ShareRequest) -> Result<String> {
    let data = share.image_data.as_deref().unwrap_or_default();
    let bytes = decode_image_data(data).context("decode imageData")?;
    if bytes.is_empty() {
        anyhow::bail!("imageData is empty");
    }

    // Prefer the bytes' own signature; the URL extension is only a fallback.
    let ext = sniff_image_extension(&bytes).map_or_else(
        || {
            share
                .url
                .split('?')
                .next()
                .and_then(|path| path.rsplit('.').next())
                .filter(|ext| {
                    !ext.is_empty()
                        && ext.len() <= 4
                        && ext.bytes().all(|b| b.is_ascii_alphanumeric())
                })
                .map_or_else(|| "png".to_owned(), str::to_lowercase)
        },
        str::to_owned,
    );

    // KDE Connect's daemon shares the file:// URL with the phone after the
    // host reports it, so the file must outlive this function: `keep`
    // disables the TempDir's automatic cleanup. The file name is generated
    // here, never derived from extension-supplied data.
    let dir = tempfile::Builder::new()
        .prefix(SHARE_TEMP_PREFIX)
        .tempdir()
        .context("create temp dir")?;
    let path = dir.path().join(format!("image.{ext}"));
    std::fs::write(&path, bytes).with_context(|| format!("write image file {}", path.display()))?;
    let _kept = dir.keep();

    file_url(&path)
}

/// Convert an absolute local path into a percent-encoded `file://` URL.
fn file_url(path: &Path) -> Result<String> {
    url::Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|()| anyhow::anyhow!("not an absolute path: {}", path.display()))
}

/// Detect the image format from its magic bytes using the `image` crate's
/// signature table (no decoders or encoders are compiled in).
fn sniff_image_extension(bytes: &[u8]) -> Option<&'static str> {
    if let Ok(format) = image::guess_format(bytes) {
        return format.extensions_str().first().copied();
    }

    // `image` has no SVG support, but browsers serve SVG images too.
    let head = &bytes[..bytes.len().min(512)];
    if String::from_utf8_lossy(head).contains("<svg") {
        return Some("svg");
    }
    None
}

const SHARE_TEMP_PREFIX: &str = "kdeconnect-share";

/// Share files may outlive the daemon call while a transfer runs, so they are
/// only swept after a generous grace period.
const SHARE_TEMP_MAX_AGE: Duration = Duration::from_hours(24);

fn is_stale(modified: SystemTime, now: SystemTime, max_age: Duration) -> bool {
    now.duration_since(modified).is_ok_and(|age| age > max_age)
}

/// Remove share temp directories older than `max_age` left behind by earlier
/// runs. Recent directories are kept in case a transfer is still in flight.
fn cleanup_stale_share_dirs(root: &Path, now: SystemTime, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(SHARE_TEMP_PREFIX)
        {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_dir() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if !is_stale(modified, now, max_age) {
            continue;
        }

        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => info!(path = %entry.path().display(), "removed stale share temp dir"),
            Err(e) => {
                warn!(path = %entry.path().display(), error = %e, "failed to remove stale share temp dir");
            }
        }
    }
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

    #[test]
    fn file_urls_are_percent_encoded() {
        let url = file_url(Path::new("/tmp/a b/фото.png")).expect("file url");
        assert_eq!(url, "file:///tmp/a%20b/%D1%84%D0%BE%D1%82%D0%BE.png");
        assert!(file_url(Path::new("relative.png")).is_err());
    }

    #[test]
    fn image_extension_is_sniffed_from_bytes() {
        assert_eq!(sniff_image_extension(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(sniff_image_extension(b"\xff\xd8\xff\xe0rest"), Some("jpg"));
        assert_eq!(sniff_image_extension(b"GIF89arest"), Some("gif"));
        assert_eq!(
            sniff_image_extension(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some("webp")
        );
        assert_eq!(sniff_image_extension(b"BMrest"), Some("bmp"));
        assert_eq!(
            sniff_image_extension(b"\x00\x00\x00\x18ftypavif"),
            Some("avif")
        );
        // Formats the `image` crate knows but the old hand-rolled sniffing did not.
        assert!(matches!(
            sniff_image_extension(b"II*\x00rest"),
            Some("tif" | "tiff")
        ));
        assert_eq!(sniff_image_extension(b"\x00\x00\x01\x00rest"), Some("ico"));
        assert_eq!(sniff_image_extension(b"qoifrest"), Some("qoi"));
        // `image` does not handle SVG; browsers still serve it.
        assert_eq!(
            sniff_image_extension(b"<?xml version=\"1.0\"?><svg>"),
            Some("svg")
        );
        assert_eq!(sniff_image_extension(b"plain text"), None);
    }

    #[test]
    fn stale_share_dirs_are_detected_by_age() {
        let now = SystemTime::now();
        let old = now - Duration::from_hours(48);
        let fresh = now - Duration::from_mins(1);
        let max_age = Duration::from_hours(24);
        assert!(is_stale(old, now, max_age));
        assert!(!is_stale(fresh, now, max_age));
    }

    #[test]
    fn write_image_to_temp_rejects_invalid_image_data() {
        let share = ShareRequest {
            target: "dev1".into(),
            url: "https://example.com/photo.jpg".into(),
            image_data: Some("!!!not-base64!!!".into()),
        };
        assert!(write_image_to_temp(&share).is_err());

        let share_without_data = ShareRequest {
            target: "dev1".into(),
            url: "https://example.com/photo.jpg".into(),
            image_data: None,
        };
        assert!(write_image_to_temp(&share_without_data).is_err());
    }

    #[test]
    fn write_image_to_temp_writes_the_file_and_returns_file_url() {
        let share = ShareRequest {
            target: "dev1".into(),
            url: "https://example.com/photo.jpg".into(),
            image_data: Some("ZmFrZSBpbWFnZSBieXRlcw==".into()),
        };
        let url = write_image_to_temp(&share).expect("write image file");
        assert!(url.starts_with("file://"));
        let path = url::Url::parse(&url)
            .expect("parse file url")
            .to_file_path()
            .expect("file path");
        assert_eq!(
            std::fs::read(&path).expect("read image file"),
            b"fake image bytes"
        );
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("jpg"));
        // The name must not embed the (untrusted) device id.
        assert!(!path.to_string_lossy().contains("dev1"));
        std::fs::remove_file(&path).ok();
        if let Some(dir) = path.parent() {
            std::fs::remove_dir_all(dir).ok();
        }
    }

    #[test]
    fn image_extension_comes_from_bytes_not_the_url() {
        // A URL claiming .jpg while the bytes are PNG must produce a .png file.
        let share = ShareRequest {
            target: "dev1".into(),
            url: "https://example.com/photo.jpg".into(),
            image_data: Some(
                base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\nfake"),
            ),
        };
        let url = write_image_to_temp(&share).expect("write image file");
        let path = url::Url::parse(&url)
            .expect("parse file url")
            .to_file_path()
            .expect("file path");
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
        std::fs::remove_file(&path).ok();
        if let Some(dir) = path.parent() {
            std::fs::remove_dir_all(dir).ok();
        }
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
