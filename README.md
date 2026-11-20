# KDE Connect Browser Extension ![License-MIT](https://img.shields.io/badge/license-MIT-red.svg)

A Chrome (and compatible) browser extension to send pages and content from your
browser to connected KDE Connect devices, via browser action or context menu.

__Update__: As of v0.2.0 the native messaging host has been rewritten in Rust
for improved reliability, performance, and maintainability.

## Installation

This extension relies on a Native Messaging Host to communicate with KDE Connect
devices.  As a result, installation is a little more involved than stand-alone
extensions.

1. Download the [latest release](https://github.com/MuntasirSZN/kdeconnect-chrome-extension/releases/latest) of the host application.
2. Close your browser.
3. Extract the host application anywhere, open a terminal and run the install
   command:

   ```bash
   ./kdeconnect-chrome-extension --install
   ```

   It will prompt you to select the browser you wish to install for, use
   `Space` to select your browser(s), and `Enter` to confirm. The installer
   will deploy itself to the appropriate location for the selected browser.

   If you wish to install as a developer (custom extension ID), add `--dev`:

   ```bash
   ./kdeconnect-chrome-extension --install --dev
   ```

   If you wish to deploy system-wide, rather than for the current user, simply
   run as root:

   ```bash
   sudo ./kdeconnect-chrome-extension --install
   ```

4. Start your browser.
5. Install the extension from the [Chrome Web Store](https://chrome.google.com/webstore/detail/kde-connect/ofmplbbfigookafjahpeepbggpofdhbo) or [Mozilla Add-ons](https://addons.mozilla.org/en-US/firefox/addon/kde_connect/).
6. Done.

Currently only Linux x86-64 builds are produced; however the code is portable
and should run anywhere the Rust toolchain supports.

## Building from source

1. Install the [Rust toolchain](https://rustup.rs) (v1.75+).
2. Clone the repository:

   ```bash
   git clone https://github.com/MuntasirSZN/kdeconnect-chrome-extension.git
   cd kdeconnect-chrome-extension
   ```

3. Build:

   ```bash
   cargo build --release
   ```

You will find the binary at `target/release/kdeconnect-chrome-extension`.

## Usage

```
Usage: kdeconnect-chrome-extension [OPTIONS]

Options:
      --install    Perform interactive install into browser native messaging hosts dirs
      --dev        Install as developer — prompts for custom extension ID
      --version    Display version and exit
      --devices    List visible KDE Connect devices and exit
  -h, --help       Print help
```

Without any flags the program runs as a native messaging host daemon, reading
JSON messages from stdin and writing responses to stdout.
