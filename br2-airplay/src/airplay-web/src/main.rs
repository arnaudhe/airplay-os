//! airplay-web — interface web en lecture seule d'une enceinte AirPlay 2.
//!
//! Petit serveur HTTP/1.1 sans dépendance :
//!   GET /             page d'état (HTML, CSS et JS intégrés au binaire)
//!   GET /api/status   état du système en JSON
//!   GET /api/logs     fin de /var/log/messages (texte)
//!
//! Rien n'est jamais écrit : ni sur la carte SD, ni dans /run.
//!
//! Options (variables d'environnement) :
//!   AIRPLAY_WEB_LISTEN  adresse d'écoute (défaut 0.0.0.0:80)
//!   AIRPLAY_WEB_ROOT    racine des fichiers lus (défaut /, pour le développement)

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
            eprintln!("airplay-web: impossible d'écouter sur {listen} : {e}");
            std::process::exit(1);
        }
    };
    eprintln!("airplay-web: écoute sur {listen}");

    let sys = Arc::new(status::Sys::new(root));
    let active = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
            let _ = respond(&mut stream, "503 Service Unavailable", "text/plain", b"occupe\n", false);
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

    // Lecture de l'en-tête seulement (pas de corps : uniquement GET/HEAD)
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
        return respond(stream, "405 Method Not Allowed", "text/plain", b"lecture seule\n", false);
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
        _ => respond(stream, "404 Not Found", "text/plain", b"introuvable\n", head_only),
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
