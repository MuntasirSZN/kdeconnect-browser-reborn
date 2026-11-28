//! Native-messaging manifest rendering and file installation.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
#[cfg(not(windows))]
use tracing::info;
use tracing::warn;

/// Native-messaging host binary: the name registered in native-messaging manifests.
pub(crate) const HOST_NAME: &str = "io.github.muntasirszn.kdeconnect_browser_reborn";
/// The name of this executable, `kdeconnect-browser-reborn`.
pub(crate) const BINARY_NAME: &str = "kdeconnect-browser-reborn";
/// Chrome Web Store id; the extension is not published yet, so --dev falls back to it.
pub(crate) const DEFAULT_EXTENSION_ID: &str = "ofmplbbfigookafjahpeepbggpofdhbo";
/// Web Store id of the extension.
pub(crate) const FIREFOX_EXTENSION_ID: &str = "kdeconnect-browser-reborn@muntasirszn.github.io";

/// Chrome extension IDs contain exactly 32 lowercase characters in a-p.
pub(crate) fn is_valid_chrome_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}

/// Which of the two manifest shapes the host installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManifestFamily {
    Chromium,
    Gecko,
}

/// Render the native messaging manifest for the given family.
fn render_manifest(family: ManifestFamily, ctx: &ManifestContext) -> Result<String> {
    let (allowed_origins, allowed_extensions) = match family {
        ManifestFamily::Gecko => (None, Some(vec![FIREFOX_EXTENSION_ID.to_owned()])),
        ManifestFamily::Chromium => {
            let extension_id = ctx
                .extension_id
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Chrome native host requires an extension ID"))?;
            (
                Some(vec![format!("chrome-extension://{extension_id}/")]),
                None,
            )
        }
    };

    let manifest = NativeManifest {
        name: HOST_NAME.to_owned(),
        description: "KDE Connect".to_owned(),
        path: ctx.path.clone(),
        host_type: "stdio",
        allowed_origins,
        allowed_extensions,
    };

    serde_json::to_string_pretty(&manifest).context("serialize native messaging manifest")
}

/// Pick the daemon binary to install: the architecture-tagged sibling
/// (`<exe>-<arch>`) when the running executable lives in a multi-architecture
/// bundle, otherwise the running executable itself.
pub(crate) fn pick_daemon_binary(exe: &Path) -> PathBuf {
    if let Some(stem) = exe.file_stem() {
        let tagged = exe.with_file_name(format!(
            "{}-{}",
            stem.to_string_lossy(),
            std::env::consts::ARCH
        ));
        if tagged.is_file() {
            return tagged;
        }
    }
    exe.to_path_buf()
}

/// Select the daemon binary for the current platform.
pub(crate) fn select_daemon_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("get current executable path")?;
    Ok(pick_daemon_binary(&exe))
}

/// File name the host is copied to; Windows needs the `.exe` suffix.
pub(crate) fn host_binary_name() -> String {
    if cfg!(windows) {
        format!("{BINARY_NAME}.exe")
    } else {
        BINARY_NAME.to_owned()
    }
}

fn prepare_dir(dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    std::fs::canonicalize(dir)
        .with_context(|| format!("resolve installation directory {}", dir.display()))
}

/// Copy the running binary into `dir` and return its absolute path.
pub(crate) fn install_host_binary(dir: &Path) -> Result<PathBuf> {
    let dir = prepare_dir(dir)?;
    let dest = dir.join(host_binary_name());
    let exe = select_daemon_binary().context("select host binary")?;
    std::fs::copy(&exe, &dest)
        .with_context(|| format!("copy {} -> {}", exe.display(), dest.display()))?;
    // On unix, restore executable bit (copy may drop it)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
            .context("set executable permission")?;
    }
    Ok(dest)
}

/// Render and write the manifest for `family`, returning its absolute path.
pub(crate) fn write_manifest(
    dir: &Path,
    family: ManifestFamily,
    host: &Path,
    extension_id: &str,
) -> Result<PathBuf> {
    let dir = prepare_dir(dir)?;
    let manifest_path = dir.join(format!("{HOST_NAME}.json"));
    let host = host.to_string_lossy();
    let ctx = match family {
        ManifestFamily::Gecko => ManifestContext::firefox(&host),
        ManifestFamily::Chromium => ManifestContext::chrome(&host, extension_id),
    };
    let content = render_manifest(family, &ctx)?;

    // Check before overwriting
    if manifest_path.exists() {
        warn!(path = %manifest_path.display(), "overwriting existing manifest");
    }

    std::fs::write(&manifest_path, &content)
        .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(manifest_path)
}

/// Copy the host and write its manifest into one directory (Unix layout).
#[cfg(not(windows))]
pub(crate) fn do_install(
    target_dir: &str,
    family: ManifestFamily,
    extension_id: &str,
) -> Result<()> {
    let dir = PathBuf::from(target_dir);
    let host = install_host_binary(&dir)?;
    let manifest = write_manifest(&dir, family, &host, extension_id)?;
    info!(binary = %host.display(), manifest = %manifest.display(), "installed host for {family:?}");
    Ok(())
}

#[derive(Debug)]
struct ManifestContext {
    /// Path of the host binary the manifest points to.
    path: String,
    /// Extension id to allow (Chrome only; `None` for Gecko).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_manifest_escapes_paths() {
        let path = r#"/home/user/a "quoted" folder/host\binary"#;
        let ctx = ManifestContext::chrome(path, DEFAULT_EXTENSION_ID);
        let json: serde_json::Value = serde_json::from_str(
            &render_manifest(ManifestFamily::Chromium, &ctx).expect("manifest"),
        )
        .unwrap();
        assert_eq!(json["path"], path);
        assert_eq!(
            json["allowed_origins"][0],
            format!("chrome-extension://{DEFAULT_EXTENSION_ID}/")
        );
    }

    /// Parse the Gecko manifest rendered for the standard test host path.
    fn gecko_manifest_json() -> serde_json::Value {
        let ctx = ManifestContext::firefox("/usr/lib/native-host");
        serde_json::from_str(&render_manifest(ManifestFamily::Gecko, &ctx).expect("manifest"))
            .expect("gecko manifest parses")
    }

    #[test]
    fn firefox_manifest_uses_allowed_extensions() {
        let json = gecko_manifest_json();
        assert_eq!(json["allowed_extensions"][0], FIREFOX_EXTENSION_ID);
        assert!(json.get("allowed_origins").is_none());
    }

    #[test]
    fn firefox_manifest_name_is_the_new_host_name() {
        let json = gecko_manifest_json();
        assert_eq!(json["name"], HOST_NAME);
        assert_eq!(json["allowed_extensions"][0], FIREFOX_EXTENSION_ID);
    }

    #[test]
    fn gecko_manifest_is_shared_by_firefox_and_tor() {
        let json = gecko_manifest_json();
        assert_eq!(json["allowed_extensions"][0], FIREFOX_EXTENSION_ID);
        assert!(json.get("allowed_origins").is_none());
    }

    #[test]
    fn host_binary_name_has_an_exe_suffix_on_windows() {
        let name = host_binary_name();
        if cfg!(windows) {
            assert_eq!(name, format!("{BINARY_NAME}.exe"));
        } else {
            assert_eq!(name, BINARY_NAME);
        }
    }

    #[test]
    fn write_manifest_returns_the_written_path() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().join("gecko");
        let host = temp.path().join(host_binary_name());
        std::fs::write(&host, b"x").unwrap();

        let path = write_manifest(&dir, ManifestFamily::Gecko, &host, "").expect("manifest");

        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name, format!("{HOST_NAME}.json"));
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["path"], host.to_string_lossy().as_ref());
        assert_eq!(json["allowed_extensions"][0], FIREFOX_EXTENSION_ID);
    }

    #[test]
    fn write_manifest_keeps_the_two_families_apart() {
        let temp = tempfile::TempDir::new().unwrap();
        let host = temp.path().join(host_binary_name());
        std::fs::write(&host, b"x").unwrap();

        let chromium = write_manifest(
            &temp.path().join("chromium"),
            ManifestFamily::Chromium,
            &host,
            DEFAULT_EXTENSION_ID,
        )
        .unwrap();
        let gecko =
            write_manifest(&temp.path().join("gecko"), ManifestFamily::Gecko, &host, "").unwrap();

        assert_ne!(chromium, gecko);
        let chromium_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&chromium).unwrap()).unwrap();
        assert!(chromium_json.get("allowed_origins").is_some());
        assert!(chromium_json.get("allowed_extensions").is_none());
    }

    #[test]
    fn chrome_extension_id_validation_matches_browser_format() {
        assert!(is_valid_chrome_extension_id(DEFAULT_EXTENSION_ID));
        assert!(is_valid_chrome_extension_id("a".repeat(32).as_str()));
        // Every letter a-p is accepted by the documented "a-p" range.
        assert!(is_valid_chrome_extension_id("g".repeat(32).as_str()));
        assert!(!is_valid_chrome_extension_id("not-an-extension-id"));
        assert!(!is_valid_chrome_extension_id("q".repeat(32).as_str()));
    }

    /// Create a `<dir>/kdeconnect-browser-reborn` test binary.
    fn fake_exe(dir: &Path, with_tagged_sibling: bool) -> PathBuf {
        let exe = dir.join("kdeconnect-browser-reborn");
        std::fs::write(&exe, b"#!/bin/sh\n").expect("write test exe");
        if with_tagged_sibling {
            let tagged = dir.join(format!(
                "kdeconnect-browser-reborn-{}",
                std::env::consts::ARCH
            ));
            std::fs::write(&tagged, b"#!/bin/sh\n").expect("write tagged sibling");
        }
        exe
    }

    #[test]
    fn pick_daemon_binary_prefers_arch_tagged_sibling() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().to_path_buf();
        let exe = fake_exe(&dir, true);
        let tagged = dir.join(format!(
            "kdeconnect-browser-reborn-{}",
            std::env::consts::ARCH
        ));
        assert_eq!(pick_daemon_binary(&exe), tagged);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pick_daemon_binary_falls_back_to_running_exe() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().to_path_buf();
        let exe = fake_exe(&dir, false);
        assert_eq!(pick_daemon_binary(&exe), exe);
        std::fs::remove_dir_all(&dir).ok();
    }
}
