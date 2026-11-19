//! airplay-web — read-only web interface of an AirPlay 2 speaker.
//!
//! Small dependency-free HTTP/1.1 server:
//!   GET /             status page (HTML, CSS and JS embedded in the binary)
//!   GET /api/status   system status as JSON
//!   GET /api/logs     tail of /var/log/messages (plain text)
//!
//! Nothing is ever written: neither to the SD card nor to /run.
//!
//! Options (environment variables):
//!   AIRPLAY_WEB_LISTEN  listen address (default 0.0.0.0:80)
//!   AIRPLAY_WEB_ROOT    root of the files read (default /, for development)

mod json;
mod status;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const INDEX_HTML: &str = include_str!("../static/index.html");
const MAX_CONNECTIONS: usize = 16;
const MAX_REQUEST: usize = 8 * 1024;
const LOG_LINES: usize = 200;

fn main() {
    let listen = std::env::var("AIRPLAY_WEB_LISTEN").unwrap_or_else(|_| "0.0.0.0:80".into());
    let root = std::env::var("AIRPLAY_WEB_ROOT").unwrap_or_else(|_| "/".into());

    let listener = match TcpListener::bind(&listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("airplay-web: cannot listen on {listen}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("airplay-web: listening on {listen}");

    let sys = Arc::new(status::Sys::new(root));
    let active = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
            let _ = respond(&mut stream, "503 Service Unavailable", "text/plain", b"busy\n", false);
            continue;
        }
        active.fetch_add(1, Ordering::Relaxed);
        let (sys, active) = (Arc::clone(&sys), Arc::clone(&active));
        thread::spawn(move || {
            let _ = handle(&mut stream, &sys);
            active.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

fn handle(stream: &mut TcpStream, sys: &status::Sys) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Read the header only (no body: GET/HEAD only)
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > MAX_REQUEST {
            return respond(stream, "431 Request Header Fields Too Large", "text/plain", b"", false);
        }
    }

    let head = String::from_utf8_lossy(&buf);
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let path = target.split('?').next().unwrap_or("");

    let head_only = method == "HEAD";
    if method != "GET" && !head_only {
        return respond(stream, "405 Method Not Allowed", "text/plain", b"read-only\n", false);
    }

    match path {
        "/" | "/index.html" => {
            respond(stream, "200 OK", "text/html; charset=utf-8", INDEX_HTML.as_bytes(), head_only)
        }
        "/api/status" => {
            let body = sys.status().to_string();
            respond(stream, "200 OK", "application/json", body.as_bytes(), head_only)
        }
        "/api/logs" => {
            let body = sys.logs(LOG_LINES);
            respond(stream, "200 OK", "text/plain; charset=utf-8", body.as_bytes(), head_only)
        }
        _ => respond(stream, "404 Not Found", "text/plain", b"not found\n", head_only),
    }
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         X-Frame-Options: DENY\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; \
         script-src 'unsafe-inline'; connect-src 'self'; img-src data:\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}
