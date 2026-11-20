# KDE Connect Browser Reborn ![License-MIT](https://img.shields.io/badge/license-MIT-red.svg)

A Firefox and Chromium-based browser extension to send pages and content from
your browser to connected KDE Connect devices, via browser action or context menu.

Originally created by [@pdf](https://github.com/pdf). It was unmaintained, so it was forked
and maintained by [@MuntasirSZN](https://github.com/MuntasirSZN). It was rewritten in Rust, made it
very close to KDE Connect part of plasma-browser-integration, added UX improvements, and fixed some long standing
issues as found in the original repo.

## Installation

This extension relies on a Native Messaging Host to communicate with KDE Connect
devices. As a result, installation is a little more involved than stand-alone
extensions.

1. Download the [latest release](https://github.com/MuntasirSZN/kdeconnect-chrome-extension/releases/latest) of the host application.
2. Close your browser.
3. Extract the host application anywhere, open a terminal and run the install
   command:

   ```bash
   ./kdeconnect-browser-reborn --install
   ```

   It will prompt you to select the browser you wish to install for, use
   `Space` to select your browser(s), and `Enter` to confirm. The installer
   will deploy itself to the appropriate location for the selected browser.

   If you wish to install as a developer (custom extension ID), add `--dev`:

   ```bash
   ./kdeconnect-browser-reborn --install --dev
   ```

   If you wish to deploy system-wide, rather than for the current user, simply
   run as root:

   ```bash
   sudo ./kdeconnect-browser-reborn --install
   ```

4. Start your browser.
5. Install the extension from the [Chrome Web Store](https://chrome.google.com/webstore/detail/kde-connect/ofmplbbfigookafjahpeepbggpofdhbo) or [Mozilla Add-ons](https://addons.mozilla.org/en-US/firefox/addon/kde_connect/).
6. Done.

Currently only Linux x86-64 builds are produced; however the code is portable
and should run anywhere the Rust toolchain supports.

## Upgrading

The native host and browser extension are released separately. To update the
native host, download the new release archive, close the browser, and run the
new `kdeconnect-browser-reborn --install` binary again. The installer
replaces the host binary and manifest for the selected browser. Restart the
browser after installation.

For a source build, run `cargo build --release`, then install the resulting
`target/release/kdeconnect-browser-reborn` binary with
`./target/release/kdeconnect-browser-reborn --install`.

## Building from source

1. Install the [Rust toolchain](https://rustup.rs) (v1.90+).
2. Clone the repository:

   ```bash
   git clone https://github.com/MuntasirSZN/kdeconnect-chrome-extension.git
   cd kdeconnect-chrome-extension
   ```

3. Build:

   ```bash
   cargo build --release
   ```

You will find the binary at `target/release/kdeconnect-browser-reborn`.

### Building for other architectures (e.g. aarch64, Pinephone)

```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

When several binaries are shipped side by side in one directory (e.g.
`kdeconnect-browser-reborn-x86_64` and `kdeconnect-browser-reborn-aarch64`),
`--install` detects the current architecture and installs the matching one.

## Usage

```
Usage: kdeconnect-browser-reborn [OPTIONS]

Options:
      --install    Perform interactive install into browser native messaging hosts dirs
      --dev        Install as developer — prompts for custom extension ID
      --version    Display version and exit
      --devices    List visible KDE Connect devices and exit
  -h, --help       Print help
```

Without any flags the program runs as a native messaging host daemon, reading
JSON messages from stdin and writing responses to stdout.
