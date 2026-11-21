//! KDE Connect Browser Reborn — Native Messaging Host.
//!
//! Bridges the browser extension with KDE Connect via D-Bus.
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

mod cli;
mod daemon;
mod dbus;
mod image;
mod install;
mod messages;
mod messaging;
mod types;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{CLI_VERSION, Cli};
use tracing_subscriber::EnvFilter;

use crate::daemon::write_protocol_error;
use crate::dbus::SharedDevice;
use crate::messages::PROTOCOL_VERSION;

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

    daemon::run_daemon(conn, devices).await
}
