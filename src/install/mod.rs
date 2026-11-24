//! Self-install / deploy for the native messaging host.
//!
//! Interactively selects browsers and installs the daemon binary plus a
//! native-messaging manifest for each.

pub(crate) mod manifest;

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use inquire::{MultiSelect, Text};
use tracing::info;

#[cfg(not(windows))]
use manifest::do_install;
use manifest::{DEFAULT_EXTENSION_ID, ManifestFamily, is_valid_chrome_extension_id};

/// Browser kind and its display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Browser {
    Chrome,
    Chromium,
    Edge,
    Brave,
    Vivaldi,
    Firefox,
    Tor,
    Custom,
}

impl Browser {
    fn all() -> &'static [Self] {
        &[
            Self::Chrome,
            Self::Chromium,
            Self::Edge,
            Self::Brave,
            Self::Vivaldi,
            Self::Firefox,
            Self::Tor,
            Self::Custom,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Chrome => "Google Chrome",
            Self::Chromium => "Chromium",
            Self::Edge => "Microsoft Edge",
            Self::Brave => "Brave",
            Self::Vivaldi => "Vivaldi",
            Self::Firefox => "Firefox",
            Self::Tor => "Tor Browser",
            Self::Custom => "Custom",
        }
    }

    #[cfg_attr(
        windows,
        allow(
            dead_code,
            reason = "Windows installs register via registry roots, not keys"
        )
    )]
    fn key(self) -> &'static str {
        match self {
            Self::Chrome | Self::Custom => "default",
            Self::Chromium => "chromium",
            Self::Edge => "edge",
            Self::Brave => "brave",
            Self::Vivaldi => "vivaldi",
            Self::Firefox => "firefox",
            Self::Tor => "tor",
        }
    }

    /// Manifest family: Tor Browser is a Gecko product, not a Chrome one.
    fn family(self) -> ManifestFamily {
        match self {
            Self::Firefox | Self::Tor => ManifestFamily::Gecko,
            _ => ManifestFamily::Chromium,
        }
    }

    /// `HKCU` subkey holding this browser's native-messaging keys, on Windows.
    ///
    /// Unused in library builds on Unix, where `install()` resolves directories
    /// instead; the table is covered by `mod.rs` tests everywhere.
    #[cfg_attr(
        not(windows),
        allow(
            dead_code,
            reason = "registration is a Windows-only path; the table is tested elsewhere"
        )
    )]
    fn registry_root(self) -> Option<&'static str> {
        match self {
            Self::Chrome => Some(r"Software\Google\Chrome\NativeMessagingHosts"),
            Self::Chromium => Some(r"Software\Chromium\NativeMessagingHosts"),
            Self::Edge => Some(r"Software\Microsoft\Edge\NativeMessagingHosts"),
            Self::Brave => Some(r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts"),
            Self::Vivaldi => Some(r"Software\Vivaldi\NativeMessagingHosts"),
            Self::Firefox | Self::Tor => Some(r"Software\Mozilla\NativeMessagingHosts"),
            Self::Custom => None,
        }
    }
}

/// `<bundle>/Browser/TorBrowser/Data/Browser/.mozilla/native-messaging-hosts`,
/// but only when the bundle actually has that layout.
#[cfg_attr(
    windows,
    allow(dead_code, reason = "directory discovery is the Unix install flow")
)]
pub(crate) fn tor_native_dir(bundle: &Path) -> Option<PathBuf> {
    let data = bundle.join("Browser/TorBrowser/Data/Browser");
    data.is_dir()
        .then(|| data.join(".mozilla/native-messaging-hosts"))
}

/// First standard Tor Browser bundle under `home` with the expected layout.
///
/// Tor Browser has no fixed Linux user path, and packagers put the bundle in
/// different places, so probe the layouts seen in the wild in preference order.
#[cfg_attr(
    windows,
    allow(dead_code, reason = "directory discovery is the Unix install flow")
)]
pub(crate) fn discover_tor_bundle(home: &Path) -> Option<PathBuf> {
    let mut candidates = vec![home.join("tor-browser")];

    // Locale-suffixed tarballs (`tor-browser_en-US`, `tor-browser_de`, …).
    if let Ok(entries) = std::fs::read_dir(home) {
        let mut locales: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("tor-browser_"))
            })
            .collect();
        locales.sort();
        candidates.append(&mut locales);
    }

    candidates.push(home.join(".tb/tor-browser")); // Whonix/Tails

    // torbrowser-launcher: ~/.local/share/torbrowser/tbb/<arch>/<bundle>
    if let Ok(archs) = std::fs::read_dir(home.join(".local/share/torbrowser/tbb")) {
        let mut bundles: Vec<PathBuf> = archs
            .flatten()
            .flat_map(|arch| {
                std::fs::read_dir(arch.path())
                    .into_iter()
                    .flatten()
                    .flatten()
            })
            .map(|bundle| bundle.path())
            .collect();
        bundles.sort();
        candidates.append(&mut bundles);
    }

    // AUR-style packages.
    candidates.push(home.join(".tor-browser/app"));
    candidates.push(home.join(".local/opt/tor-browser/app"));

    candidates
        .into_iter()
        .find(|bundle| tor_native_dir(bundle).is_some())
}

/// Resolve the native messaging hosts directory for a given OS, browser, and
/// privilege level, matching the Go code's `installMappings` table.
#[cfg_attr(
    windows,
    allow(
        dead_code,
        reason = "Windows installs register via registry roots, not directories"
    )
)]
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
                "edge" => (
                    &format!("{}/microsoft-edge/NativeMessagingHosts", base.display()),
                    "/etc/opt/edge/native-messaging-hosts",
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
                // Tor Browser reads `/usr/lib/mozilla/native-messaging-hosts` (strace-confirmed)
                // but never `~/.mozilla`, so a user install resolves the bundle instead.
                "tor" if !is_root => return None,
                "tor" => (
                    "/usr/lib/mozilla/native-messaging-hosts",
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
                "edge" => (
                    &format!("{}/Microsoft Edge/NativeMessagingHosts", base.display()),
                    "/Library/Microsoft/Edge/NativeMessagingHosts",
                ),
                "tor" => (
                    &format!(
                        "{}/TorBrowser-Data/Browser/Mozilla/NativeMessagingHosts",
                        base.display()
                    ),
                    "/Library/Application Support/TorBrowser-Data/Browser/Mozilla/NativeMessagingHosts",
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
    // Matching Go behaviour: check the USER env var (real root has USER=root).
    // Windows has no privilege levels: every install is per-user.
    let is_root = !cfg!(windows) && std::env::var("USER").as_deref() == Ok("root");
    #[cfg(windows)]
    let _ = is_root;

    let chosen = select_browsers()?;

    // If "Custom" is selected, ask where to install:
    // a directory on Unix, an `HKCU` registry root on Windows.
    #[cfg(windows)]
    let custom_root = prompt_custom_root(&chosen)?;

    #[cfg(not(windows))]
    let custom_path = prompt_custom_path(&chosen, is_root)?;

    // Chromium-family installs need the developer-supplied Chrome extension ID
    // (Firefox and Tor Browser both use the fixed Gecko id).
    let needs_chrome_id = chosen
        .iter()
        .any(|browser| browser.family() == ManifestFamily::Chromium);
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

    #[cfg(windows)]
    {
        install_windows(&chosen, &extension_id, custom_root.as_deref())?;
        info!("Done.");
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let tor_path = resolve_tor_path(&chosen, is_root)?;
        install_unix(
            &chosen,
            custom_path.as_ref(),
            tor_path.as_ref(),
            &extension_id,
            is_root,
        )?;
        info!("Done.");
        Ok(())
    }
}

/// Show the multi-select browser prompt and map labels back to variants.
fn select_browsers() -> Result<Vec<Browser>> {
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

    Ok(Browser::all()
        .iter()
        .filter(|b| selections.contains(&b.label()))
        .copied()
        .collect())
}

/// Windows Custom prompt: the `HKCU` registry root for this browser.
#[cfg(windows)]
fn prompt_custom_root(chosen: &[Browser]) -> Result<Option<String>> {
    if !chosen.contains(&Browser::Custom) {
        return Ok(None);
    }
    Text::new(r"Registry root under HKEY_CURRENT_USER (e.g. Software\Vendor\NativeMessagingHosts)")
        .with_default(r"Software\Custom\NativeMessagingHosts")
        .prompt()
        .map_err(|e| anyhow!("custom registry root cancelled: {e}"))
        .map(Some)
}

/// Unix Custom prompt: the destination directory.
#[cfg(not(windows))]
fn prompt_custom_path(chosen: &[Browser], is_root: bool) -> Result<Option<String>> {
    if !chosen.contains(&Browser::Custom) {
        return Ok(None);
    }
    let default_path = resolve_path("default", is_root).map_or_else(
        || "./native-messaging-hosts".into(),
        |p| p.to_string_lossy().to_string(),
    );
    Text::new("Enter the destination native messaging hosts path")
        .with_default(&default_path)
        .prompt()
        .map_err(|e| anyhow!("custom path cancelled: {e}"))
        .map(Some)
}

/// Unix install: copy the host and write one manifest per browser.
#[cfg(not(windows))]
fn install_unix(
    chosen: &[Browser],
    custom_path: Option<&String>,
    tor_path: Option<&PathBuf>,
    extension_id: &str,
    is_root: bool,
) -> Result<()> {
    for browser in chosen {
        let path = if browser == &Browser::Tor {
            tor_path.cloned()
        } else if browser == &Browser::Custom {
            custom_path.cloned().map(PathBuf::from)
        } else {
            resolve_path(browser.key(), is_root)
        };
        match path {
            Some(path) => {
                do_install(
                    path.to_string_lossy().as_ref(),
                    browser.family(),
                    extension_id,
                )?;
            }
            None => {
                if *browser != Browser::Custom {
                    return Err(anyhow!("unsupported browser: {}", browser.label()));
                }
                // Custom without a path: nothing to install.
            }
        }
    }
    Ok(())
}

/// Unix Tor resolution: root uses the system path Tor reads; a user install
/// discovers the bundle or prompts for it.
#[cfg(not(windows))]
fn resolve_tor_path(chosen: &[Browser], is_root: bool) -> Result<Option<PathBuf>> {
    if !chosen.contains(&Browser::Tor) {
        return Ok(None);
    }
    if let Some(path) = resolve_path("tor", is_root) {
        return Ok(Some(path));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    let default = discover_tor_bundle(&home)
        .unwrap_or_else(|| home.join("tor-browser"))
        .display()
        .to_string();
    let bundle = Text::new("Tor Browser installation directory (contains Browser/)")
        .with_default(&default)
        .prompt()
        .map_err(|e| anyhow!("Tor Browser path cancelled: {e}"))?;
    Ok(Some(tor_native_dir(Path::new(&bundle)).ok_or_else(
        || anyhow!("not a Tor Browser bundle: {bundle} (expected Browser/TorBrowser/Data/Browser)"),
    )?))
}

/// Windows install: copy the host once under `%LOCALAPPDATA%`, then register each
/// selected browser's manifest in its `HKCU` key.
#[cfg(windows)]
fn install_windows(
    chosen: &[Browser],
    extension_id: &str,
    custom_root: Option<&str>,
) -> Result<()> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("cannot resolve %LOCALAPPDATA%"))?
        .join("kdeconnect-browser-reborn");
    let host = manifest::install_host_binary(&dir)?;

    let mut chromium: Option<PathBuf> = None;
    let mut gecko: Option<PathBuf> = None;
    for browser in chosen {
        let family = browser.family();
        let manifest = match family {
            ManifestFamily::Chromium => manifest_for(
                &mut chromium,
                &dir.join("chromium"),
                family,
                &host,
                extension_id,
            )?,
            ManifestFamily::Gecko => {
                manifest_for(&mut gecko, &dir.join("gecko"), family, &host, extension_id)?
            }
        };
        let root = match browser.registry_root() {
            Some(root) => root,
            None => custom_root.ok_or_else(|| anyhow!("custom browser needs a registry root"))?,
        };
        register_windows_host(root, manifest::HOST_NAME, &manifest)?;
    }
    Ok(())
}

/// Write each family's manifest once, then reuse it for every browser of that family.
#[cfg(windows)]
fn manifest_for(
    cache: &mut Option<PathBuf>,
    dir: &Path,
    family: ManifestFamily,
    host: &Path,
    extension_id: &str,
) -> Result<PathBuf> {
    if let Some(path) = cache {
        return Ok(path.clone());
    }
    let path = manifest::write_manifest(dir, family, host, extension_id)?;
    *cache = Some(path.clone());
    Ok(path)
}

/// Register the manifest in the browser's `HKCU` key, whose default value is the
/// manifest path.
#[cfg(windows)]
pub(crate) fn register_windows_host(root: &str, host_name: &str, manifest: &Path) -> Result<()> {
    use anyhow::Context;
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let subkey = format!("{root}\\{host_name}");
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(&subkey)
        .with_context(|| format!("create HKCU\\{subkey}"))?;
    let value = manifest.to_string_lossy().into_owned();
    key.set_value("", &value)
        .with_context(|| format!("write default value of HKCU\\{subkey}"))?;
    info!(key = %subkey, path = %value, "registered native host");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_match_the_manifest_format() {
        assert_eq!(Browser::Edge.family(), ManifestFamily::Chromium);
        assert_eq!(Browser::Tor.family(), ManifestFamily::Gecko);
        let chromium = Browser::all()
            .iter()
            .filter(|b| b.family() == ManifestFamily::Chromium);
        let gecko = Browser::all()
            .iter()
            .filter(|b| b.family() == ManifestFamily::Gecko);
        assert_eq!(chromium.count(), 6); // Chrome, Chromium, Edge, Brave, Vivaldi, Custom
        assert_eq!(gecko.count(), 2); // Firefox, Tor
    }

    #[test]
    fn browser_labels_are_unique() {
        let mut labels: Vec<_> = Browser::all().iter().map(|b| b.label()).collect();
        let total = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), total);
        assert!(labels.contains(&"Microsoft Edge"));
        assert!(labels.contains(&"Tor Browser"));
    }

    #[test]
    fn tor_native_dir_requires_a_tor_bundle() {
        let temp = tempfile::TempDir::new().unwrap();
        let bundle = temp.path();
        assert_eq!(tor_native_dir(bundle), None);
        std::fs::create_dir_all(bundle.join("Browser/TorBrowser/Data/Browser")).unwrap();
        assert_eq!(
            tor_native_dir(bundle).unwrap(),
            bundle.join("Browser/TorBrowser/Data/Browser/.mozilla/native-messaging-hosts")
        );
    }

    #[test]
    fn registry_roots_follow_browser_vendors() {
        assert_eq!(
            Browser::Chrome.registry_root(),
            Some(r"Software\Google\Chrome\NativeMessagingHosts")
        );
        assert_eq!(
            Browser::Chromium.registry_root(),
            Some(r"Software\Chromium\NativeMessagingHosts")
        );
        assert_eq!(
            Browser::Edge.registry_root(),
            Some(r"Software\Microsoft\Edge\NativeMessagingHosts")
        );
        assert_eq!(
            Browser::Brave.registry_root(),
            Some(r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts")
        );
        assert_eq!(
            Browser::Vivaldi.registry_root(),
            Some(r"Software\Vivaldi\NativeMessagingHosts")
        );
        assert_eq!(
            Browser::Firefox.registry_root(),
            Some(r"Software\Mozilla\NativeMessagingHosts")
        );
        // Tor Browser registers under the Firefox key on Windows.
        assert_eq!(
            Browser::Tor.registry_root(),
            Browser::Firefox.registry_root()
        );
        // Custom asks the user for a root instead of having one.
        assert_eq!(Browser::Custom.registry_root(), None);
    }

    #[test]
    fn registry_roots_are_well_formed() {
        for browser in Browser::all() {
            let Some(root) = browser.registry_root() else {
                continue;
            };
            assert!(root.starts_with("Software\\"), "{root}");
            assert!(root.ends_with("\\NativeMessagingHosts"), "{root}");
            assert!(!root.split('\\').any(str::is_empty), "{root}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn edge_uses_its_own_config_directory() {
        let path = resolve_path("edge", false).expect("edge path");
        assert!(
            path.ends_with("microsoft-edge/NativeMessagingHosts"),
            "{path:?}"
        );
        assert_eq!(
            resolve_path("edge", true).unwrap(),
            PathBuf::from("/etc/opt/edge/native-messaging-hosts")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn tor_uses_the_system_mozilla_path_only_as_root() {
        // Tor Browser reads `/usr/lib/mozilla/native-messaging-hosts` but never
        // `~/.mozilla`, so a user install must resolve the bundle instead.
        assert_eq!(
            resolve_path("tor", true).unwrap(),
            PathBuf::from("/usr/lib/mozilla/native-messaging-hosts")
        );
        assert!(resolve_path("tor", false).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn discovers_tor_bundles_in_known_layouts() {
        let home = tempfile::TempDir::new().unwrap();
        let layout = "Browser/TorBrowser/Data/Browser";
        // Nothing installed yet.
        assert_eq!(discover_tor_bundle(home.path()), None);
        // torbrowser-launcher layout, locale-suffixed.
        let launcher = home
            .path()
            .join(".local/share/torbrowser/tbb/x86_64/tor-browser_en-US");
        std::fs::create_dir_all(launcher.join(layout)).unwrap();
        assert_eq!(discover_tor_bundle(home.path()), Some(launcher));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn tor_bundle_discovery_prefers_the_official_tarball() {
        let home = tempfile::TempDir::new().unwrap();
        let layout = "Browser/TorBrowser/Data/Browser";
        let launcher = home
            .path()
            .join(".local/share/torbrowser/tbb/x86_64/tor-browser");
        let tarball = home.path().join("tor-browser");
        std::fs::create_dir_all(launcher.join(layout)).unwrap();
        std::fs::create_dir_all(tarball.join(layout)).unwrap();
        assert_eq!(discover_tor_bundle(home.path()), Some(tarball));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn edge_and_tor_use_their_macos_directories() {
        let edge = resolve_path("edge", false).expect("edge path");
        assert!(
            edge.ends_with("Microsoft Edge/NativeMessagingHosts"),
            "{edge:?}"
        );
        let tor = resolve_path("tor", false).expect("tor path");
        assert!(
            tor.ends_with("TorBrowser-Data/Browser/Mozilla/NativeMessagingHosts"),
            "{tor:?}"
        );
        assert!(tor.starts_with(dirs::home_dir().unwrap()), "{tor:?}");
        assert_eq!(
            resolve_path("tor", true).unwrap(),
            PathBuf::from(
                "/Library/Application Support/TorBrowser-Data/Browser/Mozilla/NativeMessagingHosts"
            )
        );
    }
}
