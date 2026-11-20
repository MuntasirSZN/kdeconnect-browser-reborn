//! Chrome Native Messaging protocol.
//!
//! Each message is a 4-byte length prefix (native byte order) followed by a
//! UTF-8 JSON payload.  The host reads from **stdin** and writes to **stdout**.
//!
//! This module provides async `Reader` and `Writer` over tokio's stdio handles.

use std::io::{self, Write as IoWrite};
use anyhow::{Context, Result};
use byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::warn;

/// Bound allocations and output frames to 1 MiB.
const MAX_MESSAGE_SIZE: u32 = 1024 * 1024;

use crate::types::Message;

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Collect JSON output without ever allocating beyond the protocol limit.
#[derive(Default)]
struct BoundedBuffer {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl IoWrite for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_MESSAGE_SIZE as usize {
            self.exceeded = true;
            return Err(io::Error::other("native message exceeds 1 MiB"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

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
        let mut output = BoundedBuffer::default();
        let serialized = serde_json::to_writer(&mut output, msg);
        if output.exceeded {
            anyhow::bail!(
                "native message is too large (maximum {MAX_MESSAGE_SIZE} bytes)"
            );
        }
        serialized.context("serialize message")?;

        let buf = output.bytes;
        let len = u32::try_from(buf.len()).context("native message length exceeds u32")?;

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
            // EOF is clean only before the first byte of a new frame. A short
            // header after that is a truncated native-messaging frame.
            match self.inner.read(&mut len_buf[..1]).await {
                Ok(0) => return Ok(None),
                Ok(_) => {}
                Err(e) => return Err(e).context("read length prefix"),
            }
            self.inner
                .read_exact(&mut len_buf[1..])
                .await
                .context("read remaining length prefix")?;

            let mut cursor = std::io::Cursor::new(&len_buf[..]);
            let msg_len = ReadBytesExt::read_u32::<NativeEndian>(&mut cursor)
                .context("parse length prefix")?;
            if msg_len > MAX_MESSAGE_SIZE {
                anyhow::bail!(
                    "native message is too large: {msg_len} bytes (maximum {MAX_MESSAGE_SIZE})"
                );
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context as TaskContext, Poll};

    #[derive(Default)]
    struct RecordingWriter(Vec<u8>);

    impl AsyncWrite for RecordingWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut TaskContext<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            let this = self.get_mut();
            this.0.extend_from_slice(bytes);
            Poll::Ready(Ok(bytes.len()))
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut TaskContext<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut TaskContext<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn oversized_output_fails_before_writing_any_bytes() {
        let message = Message::result(
            crate::types::MessageType::Error,
            serde_json::Value::String("x".repeat(MAX_MESSAGE_SIZE as usize)),
        );
        let mut writer = Writer::new(RecordingWriter::default());
        let error = writer.write(&message).await.expect_err("oversized output must fail");
        assert!(
            error.to_string().contains("too large"),
            "unexpected error: {error}"
        );
        assert!(writer.inner.0.is_empty(), "writer emitted bytes before rejection");
    }

    #[tokio::test]
    async fn empty_input_is_clean_eof() {
        let mut reader = Reader::new(&[][..]);
        assert!(reader.read().await.expect("read").is_none());
    }

    #[tokio::test]
    async fn partial_length_prefix_is_a_protocol_error() {
        let mut reader = Reader::new(&[0u8, 1][..]);
        assert!(reader.read().await.is_err());
    }

    #[tokio::test]
    async fn oversized_message_is_rejected_before_reading_payload() {
        let header = (MAX_MESSAGE_SIZE + 1).to_ne_bytes();
        let mut reader = Reader::new(&header[..]);
        let error = reader.read().await.expect_err("oversized frame must fail");
        assert!(
            error.to_string().contains("too large"),
            "unexpected error: {error}"
        );
    }
}
