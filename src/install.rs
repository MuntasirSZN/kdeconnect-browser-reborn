//! Self-install / deploy for the native messaging host.
//!
//! Copies the running binary to the target location and writes a native
//! messaging manifest JSON file for each selected browser.

use std::path::PathBuf;

use anyhow::{Context, Result};
use inquire::{MultiSelect, Text};
use serde::Serialize;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const APP_NAME: &str = "com.0xc0dedbad.kdeconnect_chrome";
const DEFAULT_EXTENSION_ID: &str = "ofmplbbfigookafjahpeepbggpofdhbo";

const FIREFOX_EXTENSION_ID: &str = "kde-connect@0xc0dedbad.com";

// ---------------------------------------------------------------------------
// Browser entries
// ---------------------------------------------------------------------------

/// Browser kind and its display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Browser {
    Chrome,
    Chromium,
    Brave,
    Vivaldi,
    Firefox,
    Custom,
}

impl Browser {
    fn all() -> &'static [Browser] {
        &[
            Browser::Chrome,
            Browser::Chromium,
            Browser::Brave,
            Browser::Vivaldi,
            Browser::Firefox,
            Browser::Custom,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Browser::Chrome => "Chrome / Opera",
            Browser::Chromium => "Chromium",
            Browser::Brave => "Brave",
            Browser::Vivaldi => "Vivaldi",
            Browser::Firefox => "Firefox",
            Browser::Custom => "Custom",
        }
    }

    fn key(&self) -> &'static str {
        match self {
            Browser::Chrome | Browser::Custom => "default",
            Browser::Chromium => "chromium",
            Browser::Brave => "brave",
            Browser::Vivaldi => "vivaldi",
            Browser::Firefox => "firefox",
        }
    }
}

// ---------------------------------------------------------------------------
// Install path resolution
// ---------------------------------------------------------------------------

/// Resolve the native messaging hosts directory for a given OS, browser, and
/// privilege level, matching the Go code's `installMappings` table.
fn resolve_path(browser: &str, is_root: bool) -> Option<PathBuf> {
    let xdg_config = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty());

    let home = std::env::var("HOME").ok()?;

    // ---- Linux paths ----
    #[cfg(target_os = "linux")]
    {
        let base = xdg_config.unwrap_or_else(|| format!("{}/.config", home));
        let (user_path, root_path): (&str, &str) = match browser {
            "default" | "chrome" => (
                &format!("{}/google-chrome/NativeMessagingHosts", base),
                "/etc/opt/chrome/native-messaging-hosts",
            ),
            "chromium" => (
                &format!("{}/chromium/NativeMessagingHosts", base),
                "/etc/chromium/native-messaging-hosts",
            ),
            "brave" => (
                &format!("{}/BraveSoftware/Brave-Browser/NativeMessagingHosts", base),
                "/etc/chromium/native-messaging-hosts",
            ),
            "vivaldi" => (
                &format!("{}/vivaldi/NativeMessagingHosts", base),
                "/etc/vivaldi/native-messaging-hosts",
            ),
            "firefox" => (
                &format!("{}/.mozilla/native-messaging-hosts", home),
                "/usr/lib/mozilla/native-messaging-hosts",
            ),
            _ => return None,
        };
        let p = if is_root { root_path } else { user_path };
        Some(PathBuf::from(p))
    }

    // ---- macOS paths ----
    #[cfg(target_os = "macos")]
    {
        let lib = format!("{}/Library/Application Support", home);
        let (user_path, root_path): (&str, &str) = match browser {
            "default" | "chrome" => (
                &format!("{}/Google/Chrome/NativeMessagingHosts", lib),
                "/Library/Google/Chrome/NativeMessagingHosts",
            ),
            "chromium" => (
                &format!("{}/Chromium/NativeMessagingHosts", lib),
                "/Library/Application Support/Chromium/NativeMessagingHosts",
            ),
            "brave" => (
                &format!("{}/Chromium/NativeMessagingHosts", lib),
                "/Library/Application Support/Chromium/NativeMessagingHosts",
            ),
            "vivaldi" => (
                &format!("{}/Vivaldi/NativeMessagingHosts", lib),
                "/Library/Vivaldi/NativeMessagingHosts",
            ),
            "firefox" => (
                &format!("{}/Mozilla/NativeMessagingHosts", lib),
                "/Library/Application Support/Mozilla/NativeMessagingHosts",
            ),
            _ => return None,
        };
        let p = if is_root { root_path } else { user_path };
        Some(PathBuf::from(p))
    }

    // ---- Other OS (fallback to linux-style) ----
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (xdg_config, home, browser, is_root);
        None
    }
}

// ---------------------------------------------------------------------------
// Manifest template
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ManifestContext {
    path: String,
    extension_id: Option<String>,
}

impl ManifestContext {
    fn chrome(path: &str, ext_id: &str) -> Self {
        Self {
            path: path.to_owned(),
            extension_id: Some(ext_id.to_owned()),
        }
    }

    fn firefox(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            extension_id: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct NativeManifest {
    name: String,
    description: String,
    path: String,
    #[serde(rename = "type")]
    host_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_origins: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_extensions: Option<Vec<String>>,
}

/// Chrome extension IDs contain exactly 32 lowercase characters in a-p.
fn is_valid_chrome_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}

/// Render the native messaging manifest for the given browser.
fn render_manifest(browser: &str, ctx: &ManifestContext) -> Result<String> {
    let (allowed_origins, allowed_extensions) = if browser == "firefox" {
        (None, Some(vec![FIREFOX_EXTENSION_ID.to_owned()]))
    } else {
        let extension_id = ctx
            .extension_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Chrome native host requires an extension ID"))?;
        (Some(vec![format!("chrome-extension://{extension_id}/")]), None)
    };

    let manifest = NativeManifest {
        name: APP_NAME.to_owned(),
        description: "KDE Connect".to_owned(),
        path: ctx.path.clone(),
        host_type: "stdio",
        allowed_origins,
        allowed_extensions,
    };

    serde_json::to_string_pretty(&manifest).context("serialize native messaging manifest")
}

// ---------------------------------------------------------------------------
// Install logic
// ---------------------------------------------------------------------------

/// Copy the running binary and write the manifest for one browser.
fn do_install(target_dir: &str, browser: &str, extension_id: &str) -> Result<()> {
    let dir = PathBuf::from(target_dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let dir = std::fs::canonicalize(&dir)
        .with_context(|| format!("resolve installation directory {}", dir.display()))?;

    // Destination path must be absolute in the native messaging manifest.
    let daemon_path = dir.join(APP_NAME);

    // Copy self
    let exe = std::env::current_exe().context("get current executable path")?;
    std::fs::copy(&exe, &daemon_path)
        .with_context(|| format!("copy {} -> {}", exe.display(), daemon_path.display()))?;
    // On unix, restore executable bit (copy may drop it)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&daemon_path, std::fs::Permissions::from_mode(0o755))
            .context("set executable permission")?;
    }

    // Render and write manifest
    let manifest_path = dir.join(format!("{}.json", APP_NAME));
    let ctx = if browser == "firefox" {
        ManifestContext::firefox(daemon_path.to_string_lossy().as_ref())
    } else {
        ManifestContext::chrome(daemon_path.to_string_lossy().as_ref(), extension_id)
    };
    let content = render_manifest(browser, &ctx)?;

    // Check before overwriting
    if manifest_path.exists() {
        warn!(path = %manifest_path.display(), "overwriting existing manifest");
    }

    std::fs::write(&manifest_path, &content)
        .with_context(|| format!("write {}", manifest_path.display()))?;

    info!(binary = %daemon_path.display(), manifest = %manifest_path.display(), "installed for {}", browser);
    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the interactive install flow.
///
/// Mirrors the Go `install()` function:
///  1. Determine root/non-root and OS
///  2. Show browser checkbox menu
///  3. If developer mode, prompt for extension ID
///  4. For each selected browser, install
pub fn install(developer: bool) -> Result<()> {
    // Matching Go behaviour: check the USER env var (real root has USER=root)
    let is_root = std::env::var("USER").as_deref() == Ok("root");

    // ---- Browser selection ----
    let browser_options: Vec<&str> = Browser::all().iter().map(|b| b.label()).collect();
    let selections = MultiSelect::new(
        "Select browser(s) for native host installation (Space to select, Enter to confirm)",
        browser_options,
    )
    .with_help_message("↑↓ move, Space select, Enter confirm")
    .prompt()
    .map_err(|e| anyhow::anyhow!("browser selection cancelled: {}", e))?;

    if selections.is_empty() {
        anyhow::bail!("no browser selected");
    }

    // Map labels back to Browser variants
    let chosen: Vec<Browser> = Browser::all()
        .iter()
        .filter(|b| selections.contains(&b.label()))
        .copied()
        .collect();

    // If "Custom" is selected, prompt for a path
    let custom_path = if chosen.contains(&Browser::Custom) {
        let default_path = resolve_path("default", is_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "./native-messaging-hosts".into());
        Some(
            Text::new("Enter the destination native messaging hosts path")
                .with_default(&default_path)
                .prompt()
                .map_err(|e| anyhow::anyhow!("custom path cancelled: {}", e))?,
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
            .map_err(|e| anyhow::anyhow!("extension ID cancelled: {}", e))?
    } else {
        DEFAULT_EXTENSION_ID.to_owned()
    };
    if needs_chrome_id && !is_valid_chrome_extension_id(&extension_id) {
        anyhow::bail!("invalid Chrome extension ID: expected 32 lowercase characters from a-p");
    }

    // ---- Install for each selected browser ----
    for browser in &chosen {
        match browser {
            Browser::Custom => {
                if let Some(path) = &custom_path {
                    do_install(path, "default", &extension_id)?;
                }
            }
            _ => {
                let path = resolve_path(browser.key(), is_root)
                    .ok_or_else(|| anyhow::anyhow!("unsupported browser: {}", browser.label()))?;
                do_install(
                    path.to_string_lossy().as_ref(),
                    browser.key(),
                    &extension_id,
                )?;
            }
        }
    }

    info!("Done.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_manifest_escapes_paths() {
        let path = r#"/home/user/a "quoted" folder/host\binary"#;
        let ctx = ManifestContext::chrome(path, DEFAULT_EXTENSION_ID);
        let json: serde_json::Value =
            serde_json::from_str(&render_manifest("chrome", &ctx).expect("manifest")).unwrap();
        assert_eq!(json["path"], path);
        assert_eq!(
            json["allowed_origins"][0],
            format!("chrome-extension://{DEFAULT_EXTENSION_ID}/")
        );
    }

    #[test]
    fn firefox_manifest_uses_allowed_extensions() {
        let ctx = ManifestContext::firefox("/usr/lib/native-host");
        let json: serde_json::Value =
            serde_json::from_str(&render_manifest("firefox", &ctx).expect("manifest")).unwrap();
        assert_eq!(json["allowed_extensions"][0], FIREFOX_EXTENSION_ID);
        assert!(json.get("allowed_origins").is_none());
    }

    #[test]
    fn chrome_extension_id_validation_matches_browser_format() {
        assert!(is_valid_chrome_extension_id(DEFAULT_EXTENSION_ID));
        assert!(is_valid_chrome_extension_id("a".repeat(32).as_str()));
        assert!(!is_valid_chrome_extension_id("not-an-extension-id"));
        assert!(!is_valid_chrome_extension_id("g".repeat(32).as_str()));
    }
}
