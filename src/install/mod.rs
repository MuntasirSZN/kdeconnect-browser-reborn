//! Self-install / deploy for the native messaging host.
//!
//! Interactively selects browsers and installs the daemon binary plus a
//! native-messaging manifest for each.

mod manifest;

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use inquire::{MultiSelect, Text};
use tracing::info;

use manifest::{DEFAULT_EXTENSION_ID, do_install, is_valid_chrome_extension_id};

/// Browser kind and its display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Browser {
    Chrome,
    Chromium,
    Brave,
    Vivaldi,
    Firefox,
    Custom,
}

impl Browser {
    fn all() -> &'static [Self] {
        &[
            Self::Chrome,
            Self::Chromium,
            Self::Brave,
            Self::Vivaldi,
            Self::Firefox,
            Self::Custom,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Chrome => "Google Chrome",
            Self::Chromium => "Chromium",
            Self::Brave => "Brave",
            Self::Vivaldi => "Vivaldi",
            Self::Firefox => "Firefox",
            Self::Custom => "Custom",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Chrome | Self::Custom => "default",
            Self::Chromium => "chromium",
            Self::Brave => "brave",
            Self::Vivaldi => "vivaldi",
            Self::Firefox => "firefox",
        }
    }
}

/// Resolve the native messaging hosts directory for a given OS, browser, and
/// privilege level, matching the Go code's `installMappings` table.
fn resolve_path(browser: &str, is_root: bool) -> Option<PathBuf> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let base = dirs::config_dir()?;

    cfg_select! {
        target_os = "linux" => {
            let (user_path, root_path): (&str, &str) = match browser {
                "default" | "chrome" => (
                    &format!("{}/google-chrome/NativeMessagingHosts", base.display()),
                    "/etc/opt/chrome/native-messaging-hosts",
                ),
                "chromium" => (
                    &format!("{}/chromium/NativeMessagingHosts", base.display()),
                    "/etc/chromium/native-messaging-hosts",
                ),
                "brave" => (
                    &format!(
                        "{}/BraveSoftware/Brave-Browser/NativeMessagingHosts",
                        base.display()
                    ),
                    "/etc/chromium/native-messaging-hosts",
                ),
                "vivaldi" => (
                    &format!("{}/vivaldi/NativeMessagingHosts", base.display()),
                    "/etc/vivaldi/native-messaging-hosts",
                ),
                "firefox" => (
                    &format!(
                        "{}/.mozilla/native-messaging-hosts",
                        dirs::home_dir()?.display()
                    ),
                    "/usr/lib/mozilla/native-messaging-hosts",
                ),
                _ => return None,
            };
            let p = if is_root { root_path } else { user_path };
            Some(PathBuf::from(p))
        },
        target_os = "macos" => {
            let (user_path, root_path): (&str, &str) = match browser {
                "default" | "chrome" => (
                    &format!("{}/Google/Chrome/NativeMessagingHosts", base.display()),
                    "/Library/Google/Chrome/NativeMessagingHosts",
                ),
                "chromium" => (
                    &format!("{}/Chromium/NativeMessagingHosts", base.display()),
                    "/Library/Application Support/Chromium/NativeMessagingHosts",
                ),
                "brave" => (
                    &format!(
                        "{}/BraveSoftware/Brave-Browser/NativeMessagingHosts",
                        base.display()
                    ),
                    "/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts",
                ),
                "vivaldi" => (
                    &format!("{}/Vivaldi/NativeMessagingHosts", base.display()),
                    "/Library/Vivaldi/NativeMessagingHosts",
                ),
                "firefox" => (
                    &format!("{}/Mozilla/NativeMessagingHosts", base.display()),
                    "/Library/Application Support/Mozilla/NativeMessagingHosts",
                ),
                _ => return None,
            };
            let p = if is_root { root_path } else { user_path };
            Some(PathBuf::from(p))
        },
        _ => {
            // Other platforms have no native-messaging host directories.
            let _ = (browser, is_root);
            None
        },
    }
}

/// Run the interactive install flow.
pub(crate) fn install(developer: bool) -> Result<()> {
    // Matching Go behaviour: check the USER env var (real root has USER=root)
    let is_root = std::env::var("USER").as_deref() == Ok("root");

    let browser_options: Vec<&str> = Browser::all().iter().copied().map(Browser::label).collect();
    let selections = MultiSelect::new(
        "Select browser(s) for native host installation (Space to select, Enter to confirm)",
        browser_options,
    )
    .with_help_message("↑↓ move, Space select, Enter confirm")
    .prompt()
    .map_err(|e| anyhow!("browser selection cancelled: {e}"))?;

    if selections.is_empty() {
        bail!("no browser selected");
    }

    // Map labels back to Browser variants
    let chosen: Vec<Browser> = Browser::all()
        .iter()
        .filter(|b| selections.contains(&b.label()))
        .copied()
        .collect();

    // If "Custom" is selected, prompt for a path
    let custom_path = if chosen.contains(&Browser::Custom) {
        let default_path = resolve_path("default", is_root).map_or_else(
            || "./native-messaging-hosts".into(),
            |p| p.to_string_lossy().to_string(),
        );
        Some(
            Text::new("Enter the destination native messaging hosts path")
                .with_default(&default_path)
                .prompt()
                .map_err(|e| anyhow!("custom path cancelled: {e}"))?,
        )
    } else {
        None
    };

    // Firefox uses the fixed Gecko ID. Only Chrome-family installs need the
    // developer-supplied Chrome extension ID.
    let needs_chrome_id = chosen.iter().any(|browser| *browser != Browser::Firefox);
    let extension_id = if developer && needs_chrome_id {
        Text::new("Chrome extension ID (Enter accepts default)")
            .with_default(DEFAULT_EXTENSION_ID)
            .prompt()
            .map_err(|e| anyhow!("extension ID cancelled: {e}"))?
    } else {
        DEFAULT_EXTENSION_ID.to_owned()
    };
    if needs_chrome_id && !is_valid_chrome_extension_id(&extension_id) {
        bail!("invalid Chrome extension ID: expected 32 lowercase characters from a-p");
    }

    for browser in &chosen {
        if browser == &Browser::Custom {
            if let Some(path) = &custom_path {
                do_install(path, "default", &extension_id)?;
            }
        } else {
            let path = resolve_path(browser.key(), is_root)
                .ok_or_else(|| anyhow!("unsupported browser: {}", browser.label()))?;
            do_install(
                path.to_string_lossy().as_ref(),
                browser.key(),
                &extension_id,
            )?;
        }
    }

    info!("Done.");
    Ok(())
}
