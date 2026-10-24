//! Probe the running HTTP endpoint without opening a database or starting Tokio.
use anyhow::{Context, ensure};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    time::{Duration, Instant},
};

pub const FLAG: &str = "--health-check";

pub fn probe() -> anyhow::Result<()> {
    let address = target(std::env::var("BIND_ADDR").ok().as_deref())?;
    probe_at(address, Duration::from_secs(2))
}

fn target(bind: Option<&str>) -> anyhow::Result<SocketAddr> {
    let mut address: SocketAddr = bind
        .unwrap_or("127.0.0.1:8080")
        .parse()
        .context("BIND_ADDR must be an IP address and port")?;
    if address.ip().is_unspecified() {
        address.set_ip(match address.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
        });
    }
    Ok(address)
}

fn remaining(deadline: Instant) -> anyhow::Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(!remaining.is_zero(), "health probe deadline exceeded");
    Ok(remaining)
}

fn probe_at(address: SocketAddr, timeout: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    let mut stream = TcpStream::connect_timeout(&address, remaining(deadline)?)
        .context("connect to health endpoint")?;
    stream.set_write_timeout(Some(remaining(deadline)?))?;
    let request =
        format!("GET /api/health HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    let mut pending = request.as_bytes();
    while !pending.is_empty() {
        stream.set_write_timeout(Some(remaining(deadline)?))?;
        let sent = stream.write(pending).context("write health request")?;
        ensure!(
            sent != 0,
            "health endpoint closed before receiving the request"
        );
        pending = &pending[sent..];
    }

    let mut response = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        let bytes = stream
            .read(&mut buffer)
            .context("read health response within deadline")?;
        ensure!(
            bytes != 0,
            "health endpoint closed before completing its headers"
        );
        ensure!(
            response.len() + bytes <= 4096,
            "health response headers exceed 4096 bytes"
        );
        response.extend_from_slice(&buffer[..bytes]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let status_line = response
        .split(|byte| *byte == b'\n')
        .next()
        .context("missing health status")?;
    let status_line = std::str::from_utf8(status_line).context("invalid health status")?;
    let mut fields = status_line.split_whitespace();
    ensure!(
        matches!(fields.next(), Some("HTTP/1.0" | "HTTP/1.1")),
        "invalid health HTTP version"
    );
    let status: u16 = fields
        .next()
        .context("missing health status code")?
        .parse()
        .context("invalid health status code")?;
    ensure!(status == 200, "health endpoint returned HTTP {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn wildcard_binds_map_to_the_same_family_on_loopback() {
        assert_eq!(
            target(Some("0.0.0.0:9000")).unwrap().to_string(),
            "127.0.0.1:9000"
        );
        assert_eq!(target(Some("[::]:9000")).unwrap().to_string(), "[::1]:9000");
        assert_eq!(target(None).unwrap().to_string(), "127.0.0.1:8080");
        assert!(target(Some("invalid")).is_err());
    }

    fn fixture(response: &'static [u8], pause: Duration) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            std::thread::sleep(pause);
            let _ = stream.write_all(response);
        });
        address
    }

    #[test]
    fn probe_checks_http_status_and_bounds_a_stalled_response() {
        for (response, healthy) in [
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".as_slice(),
                true,
            ),
            (
                b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n".as_slice(),
                false,
            ),
            (
                b"HTTP/1.1 302 Found\r\nLocation: http://example.com\r\n\r\n".as_slice(),
                false,
            ),
            (b"not HTTP\r\n\r\n".as_slice(), false),
        ] {
            let address = fixture(response, Duration::ZERO);
            assert_eq!(probe_at(address, Duration::from_secs(1)).is_ok(), healthy);
        }
        let address = fixture(b"", Duration::from_millis(250));
        let started = Instant::now();
        assert!(probe_at(address, Duration::from_millis(50)).is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
