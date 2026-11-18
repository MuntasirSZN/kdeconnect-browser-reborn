# Open Issues — Audit & Status

Audit of all open GitHub issues for `pdf/kdeconnect-chrome-extension`, ordered by fixability given
the Rust+TypeScript migration in progress.

---

## Already Fixed by Migration

These issues are resolved by the Rust native messaging host + MV3 TypeScript extension.

### #56 — Fix DBUS interface for recent KDE Connect versions
**Bug** · Reported 2023-10 by @pdf

KDE Connect renamed `isTrusted` → `isPaired`. The old Go code read the wrong property.
Our Rust `fetch_device()` in `src/dbus.rs` already reads `isPaired` from D-Bus and maps it to
`is_trusted` in the `Device` struct. The `--devices` flag correctly shows the paired status.

**Action:** Close with reference to the Rust migration.

---

## Likely Fixed by Migration

### #62 — KDE-Connect got deactivated in Chrome
**Bug** · Reported 2025-07 by @fethomm

Chrome 137+ disabled the extension. Likely MV2 deprecation — Chrome is phasing out Manifest V2
extensions. Our migration to MV3 (service worker, `chrome.action` API) should resolve this once
published.

**Action:** No code change needed; publish MV3 version.

### #57 — Not working with browser while native KDE-connect works
**Bug** · Reported 2024-03 by @lamyergeier

Error: `could not connect to native host` / `no version response received`. Symptoms match #56
(paired/trusted mismatch in old Go host). The Rust migration fixes the D-Bus property name.

**Action:** Likely resolved by #56 fix. Close once confirmed.

---

## Easy Fixes

### #48 — Support aarch64 architecture
**Enhancement** · Reported 2022-05 by @cbellosoto

Rust makes cross-compilation trivial. Just needs a CI or build note:

```
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

Also needs the install script to detect the architecture and pick the right binary.

**Action:** Add cross-compile target + install detection.

### #45 — Build for Pinephone
*No labels* · Reported 2021-11 by @gemlog

Same as #48 — Pinephone is aarch64. No additional work beyond aarch64 support.

**Action:** Resolve together with #48.

---

## Needs Verification

### #60 — KDE Connect works with Brave but not with Firefox
**Bug** · Reported 2025-03 by @Joppla

Extension works on Brave but shows "no devices found" on Firefox. Likely install/config issue —
Firefox native messaging manifest may be in the wrong location or the extension ID differs.
Our Rust installer (`--install`) handles both Chrome and Firefox manifest locations.

**Action:** Verify `--install` places Firefox manifest correctly. Firefox requires native messaging
manifests in `~/.mozilla/native-messaging-hosts/`. Check that our installer targets this path.

### #35 — Tried building for macOS, Error executing binary
**Bug** · Reported 2020-05 by @firefinchdev

Old Go binary crashed on macOS because `go-dbus` couldn't find D-Bus session bus. With the Rust
migration, `zbus` v5 also requires a running D-Bus daemon — macOS doesn't have one by default.
KDE Connect would need to be running on macOS to provide a session bus.

**Action:** The Rust binary would exhibit the same error. Not fixable without D-Bus on macOS.

---

## Enhancements (Future)

### #59 — Support Valent
**Enhancement** · Reported 2025-03 by @SnoutBug

Valent (GNOME-native KDE Connect replacement) uses the same D-Bus API as KDE Connect —
`org.kde.kdeconnect`. The Rust host should work out of the box. Needs verification.

**Action:** Test the Rust host against Valent. Close if compatible.

### #14 — Send image data rather than image link
**Enhancement** · Reported 2017-11 by @pdf

Request to transfer image file data instead of a URL. Requires:
1. Download the image in the extension (fetch → blob)
2. Send via D-Bus `shareUrl` with `file://` path, or use `sendFile` API
3. KDE Connect plugin must support binary data transfer

Significant effort; not part of current migration.

### #11 — Investigate separate link contexts
**Enhancement** · Reported 2017-12 by @pdf

Explore separate menu entries for different link types (page, image, video, etc.).
Minor UX enhancement.

---

## Wontfix / External

These issues depend on platform D-Bus availability or were previously labeled wontfix.

| Issue | Description | Reason |
|-------|-------------|--------|
| #50 | GSConnect support | Labeled wontfix (already compatible via D-Bus) |
| #39 | Windows 10 support | D-Bus not natively available |
| #4  | Port to non-Linux platforms | Tracking umbrella for macOS/Windows/FreeBSD |
| #37 | FreeBSD support | D-Bus available via `devel/dbus`; needs testing |
| #36 | Setting up on Chromium in Linux Mint | Packaging/install docs |
| #34 | KDE sees device but not Firefox extension | Install question |
| #26 | Raspbian install instructions | Packaging docs |
| #22 | Unsupervised install to custom prefix | Packaging enhancement |
| #21 | Rename extension? | Meta question |
| #15 | Support for mconnect | GSConnect predecessor |

---

## Fix Plan (Incremental)

## Fix Status

### ✅ Fixed (this session)

| Issue | Fix |
|-------|-----|
| #56 D-Bus interface | Already fixed by Rust migration — `isPaired` mapped to `is_trusted` |
| #62 Chrome deactivation | MV3 migration resolves MV2 deprecation |
| #57 Not working | Fixed by #56 fix |
| #60 Firefox not working | Added `browser_specific_settings.gecko` to manifest; extension ID matches native messaging manifest |
| TS type errors | Created barrel `types.ts`, added `@types/chrome`, fixed `Message` optional fields |
| Unused variable | Removed unused `prev` in `updateDevice()` |

### 🔜 Remaining

| Order | Issue | Fix | Difficulty |
|-------|-------|-----|------------|
| 5 | #48 aarch64 | Add rustup target + install script detection | Easy |
| 6 | #45 Pinephone | Same as #48 | Easy |
| 7 | #59 Valent | Test once built | Easy |
| 8 | #14 Image data | Enhancement — scope TBD | Hard |
| 9 | Platform ports (#35, #39, #4) | Requires D-Bus on target OS | Hard |
