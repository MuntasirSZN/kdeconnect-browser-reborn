//! Native-messaging manifest rendering and file installation.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use tracing::{info, warn};

pub(crate) const APP_NAME: &str = "com.0xc0dedbad.kdeconnect_chrome";
pub(crate) const DEFAULT_EXTENSION_ID: &str = "ofmplbbfigookafjahpeepbggpofdhbo";

pub(crate) const FIREFOX_EXTENSION_ID: &str = "kde-connect@0xc0dedbad.com";

/// Chrome extension IDs contain exactly 32 lowercase characters in a-p.
pub(crate) fn is_valid_chrome_extension_id(id: &str) -> bool {
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
        (
            Some(vec![format!("chrome-extension://{extension_id}/")]),
            None,
        )
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

/// Copy the selected binary and write the manifest for one browser.
pub(crate) fn do_install(target_dir: &str, browser: &str, extension_id: &str) -> Result<()> {
    let dir = PathBuf::from(target_dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let dir = std::fs::canonicalize(&dir)
        .with_context(|| format!("resolve installation directory {}", dir.display()))?;

    // Destination path must be absolute in the native messaging manifest.
    let daemon_path = dir.join(APP_NAME);

    // Copy self
    let exe = select_daemon_binary().context("select host binary")?;
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
    let manifest_path = dir.join(format!("{APP_NAME}.json"));
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
