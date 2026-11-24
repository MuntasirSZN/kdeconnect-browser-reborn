//! Command-line interface for the native messaging host.

use clap::Parser;

pub(crate) const CLI_VERSION: &str = "1.0.1";

/// KDE Connect Browser Reborn — Native Messaging Host.
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
pub(crate) struct Cli {
    /// Perform interactive install into browser native messaging hosts dirs.
    #[arg(long)]
    pub(crate) install: bool,

    /// Install as developer — prompts for custom extension ID.
    #[arg(long)]
    pub(crate) dev: bool,

    /// Display version and exit.
    #[arg(long)]
    pub(crate) version: bool,

    /// List visible KDE Connect devices and exit.
    #[arg(long)]
    pub(crate) devices: bool,

    /// Arguments passed by the browser when launching the native messaging
    /// host (Chrome: extension origin; Firefox: manifest path + extension ID).
    /// Ignored.
    #[arg(hide = true, num_args = 0..)]
    #[allow(
        dead_code,
        reason = "browser launchers pass these args; the daemon ignores them"
    )]
    pub(crate) browser_args: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_arguments_are_accepted() {
        let cli = Cli::try_parse_from([
            "kdeconnect-browser-reborn",
            "io.github.muntasirszn.kdeconnect_browser_reborn.json",
            "kdeconnect-browser-reborn@muntasirszn.github.io",
        ])
        .expect("Firefox argv");
        assert!(!cli.install && !cli.dev && !cli.version && !cli.devices);

        Cli::try_parse_from(["kdeconnect-browser-reborn", "chrome-extension://abcdef/"])
            .expect("Chrome argv");
    }
}
