//! Lingering close for every accepted connection (round 3 fix: large responses cut
//! with `Connection: close`).
//!
//! hyper ends a `Connection: close` exchange by shutting down the write side and
//! dropping the socket as soon as the last byte is in the kernel.  On Windows that
//! lost the tail of large bodies still queued for a slow reader: the client stalled
//! and got a reset ~19 s later (Python `urllib` lost the z0 tile, 1.4 MB, after
//! ~1.43 MB; curl exited with 56).  Measured in `tests/close.rs` with a 600 KB body:
//! a FIN (`shutdown`) or a plain close while ~11 KB were queued both lost them.
//!
//! So the close never happens while data may be queued: no FIN, keep reading until
//! the client closes its side (with `Content-Length` it does so after the last
//! byte), it errors, sends more than `MAX_DRAIN`, or `LINGER` passes; then drop.
//! A client that waits for the server's EOF instead waits up to `LINGER`.

use axum::serve::Listener;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Sleep;

/// Longest wait for the client to close before the server drops the socket.
const LINGER: Duration = Duration::from_secs(10);
/// Bytes read and discarded while lingering before giving up on a chatty client.
const MAX_DRAIN: usize = 64 * 1024;

/// `TcpListener` whose connections use TCP_NODELAY and close by lingering.
pub struct LingerListener(pub TcpListener);

impl Listener for LingerListener {
    type Io = LingerIo;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (LingerIo, Self::Addr) {
        let (tcp, addr) = Listener::accept(&mut self.0).await;
        // TCP_NODELAY: without it Nagle + delayed ACK holds small responses ~10-40 ms
        // on loopback (measured: /api/health p50 13.6 ms with 16 keep-alive connections).
        let _ = tcp.set_nodelay(true);
        (LingerIo { tcp, linger: None, drained: 0 }, addr)
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.0.local_addr()
    }
}

pub struct LingerIo {
    tcp: TcpStream,
    /// Deadline of the lingering read, set on the first `poll_shutdown`.
    linger: Option<Pin<Box<Sleep>>>,
    drained: usize,
}

impl AsyncRead for LingerIo {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().tcp).poll_read(cx, buf)
    }
}

impl AsyncWrite for LingerIo {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().tcp).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().tcp).poll_flush(cx)
    }

    /// No FIN: read until the client's EOF, an error, `MAX_DRAIN` or `LINGER`.
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        let deadline = me.linger.get_or_insert_with(|| Box::pin(tokio::time::sleep(LINGER)));
        let mut scratch = [0u8; 4096];
        loop {
            let mut buf = ReadBuf::new(&mut scratch);
            match Pin::new(&mut me.tcp).poll_read(cx, &mut buf) {
                Poll::Ready(Ok(())) if buf.filled().is_empty() => return Poll::Ready(Ok(())),
                Poll::Ready(Ok(())) => {
                    me.drained += buf.filled().len();
                    if me.drained > MAX_DRAIN {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(_)) => return Poll::Ready(Ok(())),
                Poll::Pending => return deadline.as_mut().poll(cx).map(Ok),
            }
        }
    }
}
