//! Streamable HTTP transport (stateless, JSON responses) with the OAuth endpoints. Listens on
//! loopback only; a tunnel (Cloudflare) brings it to `url`. Requests are handled one at a time,
//! which also keeps the vault writes in order.

use super::oauth::{self, Store};
use super::Ctx;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use tiny_http::{Header, Method, Request, Response, Server};

const MAX_BODY: u64 = 1 << 20;

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("header")
}

struct Reply {
    status: u16,
    ctype: &'static str,
    body: String,
    extra: Vec<(String, String)>,
}

impl Reply {
    fn json(status: u16, v: &Value) -> Reply {
        Reply { status, ctype: "application/json", body: v.to_string(), extra: vec![] }
    }
    fn html(status: u16, s: String) -> Reply {
        Reply { status, ctype: "text/html; charset=utf-8", body: s, extra: vec![] }
    }
    fn empty(status: u16) -> Reply {
        Reply { status, ctype: "text/plain", body: String::new(), extra: vec![] }
    }
    fn with(mut self, k: &str, v: &str) -> Reply {
        self.extra.push((k.into(), v.into()));
        self
    }
}

pub fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                let h = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push(h(b[i + 1]) << 4 | h(b[i + 2]));
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn form(s: &str) -> HashMap<String, String> {
    s.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn get_header(rq: &Request, name: &'static str) -> Option<String> {
    rq.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string())
}

fn route(ctx: &Ctx, url: &str, rq: &mut Request) -> Reply {
    let full = rq.url().to_string();
    let (path, query) = full.split_once('?').unwrap_or((&full, ""));
    let method = rq.method().clone();
    let mut body = String::new();
    if matches!(method, Method::Post) && rq.as_reader().take(MAX_BODY).read_to_string(&mut body).is_err() {
        return Reply::json(400, &json!({"error": "invalid_request", "error_description": "body is not UTF-8"}));
    }
    // Browsers on other sites must not drive the MCP endpoint (DNS rebinding); servers send no Origin.
    if let Some(o) = get_header(rq, "Origin") {
        let ok = o == url || o == "https://claude.ai" || o == "https://claude.com" || o.starts_with("http://localhost") || o.starts_with("http://127.0.0.1");
        if !ok && path == "/mcp" {
            return Reply::json(403, &json!({"error": "origin not allowed"}));
        }
    }
    let p = path.trim_end_matches('/');
    match (&method, p) {
        (Method::Get, p) if p.starts_with("/.well-known/oauth-protected-resource") => Reply::json(200, &oauth::protected_resource(url)),
        (Method::Get, p) if p.starts_with("/.well-known/oauth-authorization-server") || p.starts_with("/.well-known/openid-configuration") => {
            Reply::json(200, &oauth::server_metadata(url))
        }
        (Method::Post, "/register") => {
            let req: Value = serde_json::from_str(&body).unwrap_or(json!({}));
            let mut s = Store::load();
            let r = match s.register(&req) {
                Ok(v) => Reply::json(201, &v),
                Err((st, v)) => Reply::json(st, &v),
            };
            s.save();
            r
        }
        (Method::Get, "/authorize") => {
            let q = form(query);
            let s = Store::load();
            match s.check_authorize(&q) {
                Ok(()) => {
                    let name = s.client(&q["client_id"]).and_then(|c| c["client_name"].as_str()).unwrap_or("MCP client").to_string();
                    Reply::html(200, oauth::login_page(&name, &q, ""))
                }
                Err(e) => Reply::html(400, format!("<!doctype html><meta charset=utf-8><p>Kioku: {}</p>", e.replace('<', "&lt;"))),
            }
        }
        (Method::Post, "/authorize") => {
            let mut q = form(&body);
            let password = q.remove("password").unwrap_or_default();
            let mut s = Store::load();
            let r = match s.login(&q, &password) {
                Ok(code) => {
                    let uri = &q["redirect_uri"];
                    let sep = if uri.contains('?') { '&' } else { '?' };
                    let mut to = format!("{uri}{sep}code={}", encode(&code));
                    if let Some(st) = q.get("state") {
                        to.push_str(&format!("&state={}", encode(st)));
                    }
                    to.push_str(&format!("&iss={}", encode(url)));
                    Reply::empty(302).with("Location", &to)
                }
                Err(e) => {
                    let name = s.client(q.get("client_id").map(String::as_str).unwrap_or("")).and_then(|c| c["client_name"].as_str()).unwrap_or("MCP client").to_string();
                    let st = if s.check_authorize(&q).is_ok() { 401 } else { 400 };
                    Reply::html(st, oauth::login_page(&name, &q, &e))
                }
            };
            s.save();
            r
        }
        (Method::Post, "/token") => {
            let mut s = Store::load();
            let r = match s.token(&form(&body)) {
                Ok(v) => Reply::json(200, &v).with("Cache-Control", "no-store"),
                Err(v) => Reply::json(400, &v).with("Cache-Control", "no-store"),
            };
            s.save();
            r
        }
        (Method::Post, "/mcp") => {
            if !Store::load().bearer_ok(get_header(rq, "Authorization").as_deref()) {
                let meta = format!("Bearer resource_metadata=\"{url}/.well-known/oauth-protected-resource\"");
                return Reply::json(401, &json!({"error": "invalid_token"})).with("WWW-Authenticate", &meta);
            }
            match super::handle_body(ctx, &body) {
                Some(v) => Reply::json(200, &v),
                None => Reply::empty(202),
            }
        }
        (_, "/mcp") => Reply::empty(405).with("Allow", "POST"),
        (Method::Get, "") => Reply::html(200, "<!doctype html><meta charset=utf-8><title>Kioku</title><p>Kioku MCP server.</p>".into()),
        _ => Reply::json(404, &json!({"error": "not found"})),
    }
}

pub fn serve(ctx: Ctx, addr: &str, url: &str) -> Result<i32> {
    let server = Server::http(addr).map_err(|e| anyhow!("cannot listen on {addr}: {e}"))?;
    eprintln!("kioku mcp: listening on http://{addr}, public URL {url}/mcp, records as {}", ctx.me);
    for mut rq in server.incoming_requests() {
        let (m, p) = (rq.method().to_string(), rq.url().split('?').next().unwrap_or("").to_string());
        let r = route(&ctx, url, &mut rq);
        eprintln!("{m} {p} {}", r.status);
        let mut resp = Response::from_string(r.body).with_status_code(r.status).with_header(header("Content-Type", r.ctype)).with_header(header("X-Frame-Options", "DENY"));
        for (k, v) in &r.extra {
            resp = resp.with_header(header(k, v));
        }
        let _ = rq.respond(resp);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_coding() {
        assert_eq!(decode("a+b%20c%2Fd%"), "a b c/d%");
        assert_eq!(encode("a b/ç"), "a%20b%2F%C3%A7");
        let f = form("x=1&redirect_uri=https%3A%2F%2Fclaude.ai%2Fcb&empty=");
        assert_eq!(f["redirect_uri"], "https://claude.ai/cb");
        assert_eq!(f["empty"], "");
    }
}
