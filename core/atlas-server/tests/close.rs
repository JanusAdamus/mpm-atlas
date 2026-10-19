//! Regression (round 3): a large response to a client that sends `Connection: close`
//! must arrive whole.  Before the fix the server closed the socket while the tail of
//! the body was still queued; on Windows the client stalled and got a reset (Python
//! `urllib` lost the z0 tile with ECONNRESET; curl exit 56).  The client here reads
//! like `urllib`: the head, then `Content-Length` bytes, then it closes.

use axum::Router;
use axum::routing::get;
use std::io::{BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

const BODY: usize = 600_000;

/// One `Connection: close` GET, reading after `delay`; the body length, or the I/O error.
fn fetch(addr: std::net::SocketAddr, delay: Duration) -> std::io::Result<usize> {
    let mut s = std::net::TcpStream::connect(addr)?;
    s.set_read_timeout(Some(Duration::from_secs(30)))?;
    s.write_all(b"GET /big HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n")?;
    std::thread::sleep(delay);
    let mut r = BufReader::new(s);
    let (mut line, mut len) = (String::new(), 0usize);
    r.read_line(&mut line)?;
    assert!(line.starts_with("HTTP/1.1 200"), "status line {line:?}");
    loop {
        line.clear();
        r.read_line(&mut line)?;
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().expect("content-length");
        }
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok(body.len())
}

#[test]
fn large_response_with_connection_close_arrives_whole() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route("/big", get(|| async { vec![b'x'; BODY] }));
    rt.spawn(atlas_server::serve_on(listener, app, std::future::pending()));
    // A slow reader is the case that failed: the server finished writing and closed first.
    for delay_ms in [0u64, 200, 0, 300, 100] {
        let t = Instant::now();
        let got = fetch(addr, Duration::from_millis(delay_ms));
        assert!(matches!(got, Ok(BODY)), "delay {delay_ms} ms: {got:?} after {:?}", t.elapsed());
    }
}
