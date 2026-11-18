//! Chrome Native Messaging protocol.
//!
//! Each message is a 4-byte length prefix (native byte order) followed by a
//! UTF-8 JSON payload.  The host reads from **stdin** and writes to **stdout**.
//!
//! This module provides async `Reader` and `Writer` over tokio's stdio handles.

use std::io::ErrorKind;

use anyhow::{Context, Result};
use byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::warn;

use crate::types::Message;

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Serialises `Message` values to a length-prefixed JSON stream (stdout).
pub struct Writer<W: AsyncWrite + Unpin> {
    inner: W,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Encode and flush a message.
    pub async fn write(&mut self, msg: &Message) -> Result<()> {
        let buf = serde_json::to_vec(msg).context("serialize message")?;
        let len = buf.len() as u32;

        let mut header = Vec::with_capacity(4);
        WriteBytesExt::write_u32::<NativeEndian>(&mut header, len)?;
        self.inner.write_all(&header).await?;
        self.inner.write_all(&buf).await?;
        self.inner.flush().await?;

        Ok(())
    }
}

/// Convenience alias: writes to stdout.
pub type StdoutWriter = Writer<tokio::io::Stdout>;

impl StdoutWriter {
    pub fn stdout() -> Self {
        Self::new(tokio::io::stdout())
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Reads length-prefixed JSON `Message` values from a byte stream (stdin).
pub struct Reader<R: AsyncRead + Unpin> {
    inner: R,
}

impl<R: AsyncRead + Unpin> Reader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// Read the next message, returning `None` on clean EOF.
    ///
    /// Malformed payloads are logged and skipped transparently (matching the
    /// Go code's behaviour of logging & continuing).
    pub async fn read(&mut self) -> Result<Option<Message>> {
        loop {
            // Read 4-byte length prefix (native endian)
            let mut len_buf = [0u8; 4];
            match self.inner.read_exact(&mut len_buf).await {
                Ok(_n) => {}
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                Err(e) => return Err(e).context("read length prefix"),
            }

            let mut cursor = std::io::Cursor::new(&len_buf[..]);
            let msg_len = ReadBytesExt::read_u32::<NativeEndian>(&mut cursor)
                .context("parse length prefix")?;

            // Read the JSON payload
            let mut buf = vec![0u8; msg_len as usize];
            self.inner
                .read_exact(&mut buf)
                .await
                .with_context(|| format!("read {} byte payload", msg_len))?;

            match serde_json::from_slice::<Message>(&buf) {
                Ok(msg) => return Ok(Some(msg)),
                Err(e) => {
                    // Log the bad payload and skip; the Go code just logs & continues.
                    let preview = String::from_utf8_lossy(&buf[..buf.len().min(256)]);
                    warn!(error = %e, payload = %preview, "malformed message from extension");
                    // Loop tries the next message
                }
            }
        }
    }
}

/// Convenience alias: reads from stdin.
pub type StdinReader = Reader<tokio::io::Stdin>;

impl StdinReader {
    pub fn stdin() -> Self {
        Self::new(tokio::io::stdin())
    }
}
