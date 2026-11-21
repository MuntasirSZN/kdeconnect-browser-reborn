//! Image file transfer helpers.
//!
//! The extension sends base64 image bytes for image context clicks; we write
//! them to a `file://` temp file because KDE Connect's share plugin shares
//! local files rather than byte payloads.  Also cleans up stale share temp
//! dirs at daemon startup.

use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use base64::Engine;
use tracing::{info, warn};

use crate::types::ShareRequest;

// The `Simd` engine is only exported on x86_64/aarch64; elsewhere we fall
// back to the scalar standard engine.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use base64::engine::Simd;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use base64::engine::general_purpose::GeneralPurposeConfig;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;

const SHARE_TEMP_PREFIX: &str = "kdeconnect-share";

/// Share files may outlive the daemon call while a transfer runs, so they are
/// only swept after a generous grace period.
pub(crate) const SHARE_TEMP_MAX_AGE: Duration = Duration::from_hours(24);

fn is_stale(modified: SystemTime, now: SystemTime, max_age: Duration) -> bool {
    now.duration_since(modified).is_ok_and(|age| age > max_age)
}

/// Remove share temp directories older than `max_age` left behind by earlier
/// runs. Recent directories are kept in case a transfer is still in flight.
pub(crate) fn cleanup_stale_share_dirs(root: &Path, now: SystemTime, max_age: Duration) {
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
pub(crate) fn write_image_to_temp(share: &ShareRequest) -> Result<String> {
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
pub(crate) fn file_url(path: &Path) -> Result<String> {
    url::Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|()| anyhow::anyhow!("not an absolute path: {}", path.display()))
}

/// Detect the image format from its magic bytes using the `image` crate's
/// signature table (no decoders or encoders are compiled in).
pub(crate) fn sniff_image_extension(bytes: &[u8]) -> Option<&'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
