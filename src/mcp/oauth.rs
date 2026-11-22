//! OAuth 2.1 for the HTTP server, the way claude.ai connects to a custom connector: metadata
//! (RFC 9728, RFC 8414), dynamic client registration (RFC 7591), a password login page,
//! authorization codes with PKCE S256, short access tokens and rotating refresh tokens.
//!
//! The password is set on the server by the user (`kioku mcp passwd`); only its argon2 hash is
//! kept (config_dir/mcp.json). Codes and tokens are kept only as sha256 hashes
//! (state_dir/mcp/store.json).

use crate::util;
use anyhow::Result;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const ACCESS_SECS: u64 = 3600;
const REFRESH_SECS: u64 = 30 * 86400;
const CODE_SECS: u64 = 60;
const MAX_FAILS: u64 = 5;
const LOCK_SECS: u64 = 900;

/// Where a client may be sent back to after the login: claude.ai's callback, and this machine
/// (local MCP clients and tests). mcp.json `redirect_uris` replaces the claude.ai part.
const CALLBACKS: &[&str] = &["https://claude.ai/api/mcp/auth_callback", "https://claude.com/api/mcp/auth_callback"];

fn config_file() -> PathBuf {
    util::config_dir().join("mcp.json")
}

fn store_file() -> PathBuf {
    util::state_dir().join("mcp/store.json")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load(p: &Path) -> Map<String, Value> {
    serde_json::from_str::<Value>(&util::read(p)).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn save(p: &Path, v: &Map<String, Value>) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(v)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, p)?;
    Ok(())
}

pub fn b64url(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn random() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    b64url(&b)
}

fn sha(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn pkce_ok(verifier: &str, challenge: &str) -> bool {
    (43..=128).contains(&verifier.len()) && b64url(&Sha256::digest(verifier.as_bytes())) == challenge
}

fn allowed_redirect(uri: &str) -> bool {
    let cfg = load(&config_file());
    let listed: Vec<String> = match cfg.get("redirect_uris").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
        None => CALLBACKS.iter().map(|s| s.to_string()).collect(),
    };
    if listed.iter().any(|u| u == uri) {
        return true;
    }
    ["http://localhost:", "http://127.0.0.1:", "http://localhost/", "http://127.0.0.1/"].iter().any(|p| uri.starts_with(p))
}

/// The server's whole OAuth state, loaded per request (the CLI may change it in between).
pub struct Store(Map<String, Value>);

impl Store {
    pub fn load() -> Store {
        let mut m = load(&store_file());
        for k in ["clients", "codes", "tokens"] {
            if !m.get(k).is_some_and(Value::is_object) {
                m.insert(k.into(), json!({}));
            }
        }
        // Expired codes and tokens out.
        let t = now();
        for k in ["codes", "tokens"] {
            if let Some(o) = m.get_mut(k).and_then(Value::as_object_mut) {
                o.retain(|_, v| v["exp"].as_u64().unwrap_or(0) > t);
            }
        }
        Store(m)
    }
    pub fn save(&self) {
        if let Err(e) = save(&store_file(), &self.0) {
            eprintln!("kioku mcp: cannot save the token store: {e}");
        }
    }
    fn map(&mut self, k: &str) -> &mut Map<String, Value> {
        self.0.get_mut(k).and_then(Value::as_object_mut).expect("store map")
    }
    pub fn client(&self, id: &str) -> Option<&Value> {
        self.0["clients"].get(id)
    }

    /// RFC 7591. Ok(the registration), Err((status, error)).
    pub fn register(&mut self, req: &Value) -> Result<Value, (u16, Value)> {
        let uris: Vec<String> = req["redirect_uris"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect()).unwrap_or_default();
        if uris.is_empty() {
            return Err((400, json!({"error": "invalid_redirect_uri", "error_description": "redirect_uris is required"})));
        }
        if let Some(u) = uris.iter().find(|u| !allowed_redirect(u)) {
            return Err((400, json!({"error": "invalid_redirect_uri", "error_description": format!("redirect_uri not allowed: {u}")})));
        }
        let name: String = req["client_name"].as_str().unwrap_or("MCP client").chars().take(80).collect();
        let id = random();
        let c = json!({"client_name": name, "redirect_uris": uris, "created": now()});
        self.map("clients").insert(id.clone(), c);
        Ok(json!({
            "client_id": id,
            "client_id_issued_at": now(),
            "client_name": name,
            "redirect_uris": uris,
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        }))
    }

    /// Whether an authorize request names a known client, one of its redirect URIs and a S256
    /// challenge. Err: what is wrong (shown on the page, not redirected).
    pub fn check_authorize(&self, q: &HashMap<String, String>) -> Result<(), String> {
        let g = |k: &str| q.get(k).map(String::as_str).unwrap_or("");
        let c = self.client(g("client_id")).ok_or("unknown client_id")?;
        if !c["redirect_uris"].as_array().is_some_and(|a| a.iter().any(|u| u == g("redirect_uri"))) {
            return Err("redirect_uri is not registered for this client".into());
        }
        if g("response_type") != "code" {
            return Err("response_type must be code".into());
        }
        if g("code_challenge").len() < 43 || g("code_challenge_method") != "S256" {
            return Err("PKCE with S256 is required".into());
        }
        Ok(())
    }

    /// The password from the login page. Ok(code) or Err(message for the page).
    pub fn login(&mut self, q: &HashMap<String, String>, password: &str) -> Result<String, String> {
        self.check_authorize(q)?;
        let t = now();
        let locked = self.0.get("locked_until").and_then(Value::as_u64).unwrap_or(0);
        if locked > t {
            return Err(format!("Çok fazla yanlış deneme. {} dakika sonra tekrar dene.", (locked - t).div_ceil(60)));
        }
        let cfg = load(&config_file());
        let Some(hash) = cfg.get("password_hash").and_then(Value::as_str) else {
            return Err("Sunucuda şifre ayarlanmamış (kioku mcp passwd).".into());
        };
        let good = PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok());
        if !good {
            let fails = self.0.get("fails").and_then(Value::as_u64).unwrap_or(0) + 1;
            if fails >= MAX_FAILS {
                self.0.insert("locked_until".into(), json!(t + LOCK_SECS));
                self.0.insert("fails".into(), json!(0));
            } else {
                self.0.insert("fails".into(), json!(fails));
            }
            return Err("Şifre yanlış.".into());
        }
        self.0.insert("fails".into(), json!(0));
        let g = |k: &str| q.get(k).cloned().unwrap_or_default();
        let code = random();
        let rec = json!({"client": g("client_id"), "redirect_uri": g("redirect_uri"), "challenge": g("code_challenge"), "scope": g("scope"), "exp": t + CODE_SECS});
        self.map("codes").insert(sha(&code), rec);
        Ok(code)
    }

    fn issue(&mut self, client: &str, scope: &str) -> Value {
        let (access, refresh, t) = (random(), random(), now());
        self.map("tokens").insert(sha(&access), json!({"kind": "access", "client": client, "exp": t + ACCESS_SECS}));
        self.map("tokens").insert(sha(&refresh), json!({"kind": "refresh", "client": client, "scope": scope, "exp": t + REFRESH_SECS}));
        let mut r = json!({"access_token": access, "token_type": "Bearer", "expires_in": ACCESS_SECS, "refresh_token": refresh});
        if !scope.is_empty() {
            r["scope"] = json!(scope);
        }
        r
    }

    /// The token endpoint. Ok(token response) or Err(error response), both with status 200/400.
    pub fn token(&mut self, f: &HashMap<String, String>) -> Result<Value, Value> {
        let g = |k: &str| f.get(k).map(String::as_str).unwrap_or("");
        let bad = |e: &str, d: &str| json!({"error": e, "error_description": d});
        match g("grant_type") {
            "authorization_code" => {
                let Some(c) = self.map("codes").remove(&sha(g("code"))) else {
                    return Err(bad("invalid_grant", "unknown or used code"));
                };
                if c["client"] != g("client_id") || c["redirect_uri"] != g("redirect_uri") {
                    return Err(bad("invalid_grant", "client_id or redirect_uri does not match"));
                }
                if !pkce_ok(g("code_verifier"), c["challenge"].as_str().unwrap_or("")) {
                    return Err(bad("invalid_grant", "code_verifier does not match"));
                }
                Ok(self.issue(g("client_id"), c["scope"].as_str().unwrap_or("")))
            }
            "refresh_token" => {
                let key = sha(g("refresh_token"));
                let ok = self.0["tokens"].get(&key).is_some_and(|t| t["kind"] == "refresh" && (g("client_id").is_empty() || t["client"] == g("client_id")));
                if !ok {
                    return Err(bad("invalid_grant", "unknown refresh token"));
                }
                let t = self.map("tokens").remove(&key).unwrap_or_default();
                let client = t["client"].as_str().unwrap_or("").to_string();
                if self.client(&client).is_none() {
                    return Err(bad("invalid_grant", "client was revoked"));
                }
                Ok(self.issue(&client, t["scope"].as_str().unwrap_or("")))
            }
            _ => Err(bad("unsupported_grant_type", "authorization_code or refresh_token")),
        }
    }

    /// Whether `Authorization: Bearer …` carries a live access token of a known client.
    pub fn bearer_ok(&self, header: Option<&str>) -> bool {
        let Some(tok) = header.and_then(|h| h.strip_prefix("Bearer ").or_else(|| h.strip_prefix("bearer "))) else {
            return false;
        };
        self.0["tokens"].get(sha(tok.trim())).is_some_and(|t| t["kind"] == "access" && t["exp"].as_u64().unwrap_or(0) > now() && self.client(t["client"].as_str().unwrap_or("")).is_some())
    }
}

pub fn protected_resource(url: &str) -> Value {
    json!({"resource": format!("{url}/mcp"), "authorization_servers": [url], "bearer_methods_supported": ["header"], "resource_name": "Kioku"})
}

pub fn server_metadata(url: &str) -> Value {
    json!({
        "issuer": url,
        "authorization_endpoint": format!("{url}/authorize"),
        "token_endpoint": format!("{url}/token"),
        "registration_endpoint": format!("{url}/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// The login page; `q` carries the authorize request through as hidden fields.
pub fn login_page(client: &str, q: &HashMap<String, String>, error: &str) -> String {
    let hidden: String = ["client_id", "redirect_uri", "response_type", "state", "code_challenge", "code_challenge_method", "scope", "resource"]
        .iter()
        .filter_map(|k| q.get(*k).map(|v| format!("<input type=\"hidden\" name=\"{k}\" value=\"{}\">", esc(v))))
        .collect();
    let err = if error.is_empty() { String::new() } else { format!("<p class=\"err\">{}</p>", esc(error)) };
    let host = q.get("redirect_uri").and_then(|u| u.split('/').nth(2)).unwrap_or("");
    format!(
        r#"<!doctype html><html lang="tr"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Kioku</title>
<style>body{{font:16px system-ui,sans-serif;background:#f6f5f2;color:#1d1d1b;display:grid;place-items:center;min-height:100vh;margin:0;padding:16px;box-sizing:border-box}}
@media(prefers-color-scheme:dark){{body{{background:#1b1b1a;color:#ecebe6}}input{{background:#262624;color:inherit;border-color:#444}}}}
main{{max-width:360px;width:100%}}h1{{font-size:22px;margin:0 0 8px}}p{{line-height:1.45}}input[type=password]{{width:100%;box-sizing:border-box;padding:10px;font-size:16px;border:1px solid #bbb;border-radius:8px;margin:8px 0 12px}}
button{{width:100%;padding:11px;font-size:16px;border:0;border-radius:8px;background:#c96442;color:#fff;cursor:pointer}}.err{{color:#c0392b}}.dim{{opacity:.7;font-size:14px}}</style></head>
<body><main><h1>Kioku</h1><p><b>{}</b> ikinci beynine erişmek istiyor: arama, okuma ve günlüğe kayıt.</p><p class="dim">Dönüş adresi: {}</p>{err}
<form method="post" action="/authorize">{hidden}<label>Şifre<input type="password" name="password" autocomplete="current-password" autofocus required></label><button type="submit">İzin ver</button></form></main></body></html>"#,
        esc(client),
        esc(host)
    )
}

pub fn passwd() -> Result<i32> {
    let a = rpassword::prompt_password("Kioku MCP password: ")?;
    if a.chars().count() < 12 {
        return util::die("use at least 12 characters");
    }
    let b = rpassword::prompt_password("Again: ")?;
    if a != b {
        return util::die("the two do not match");
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(a.as_bytes(), &salt).map_err(|e| anyhow::anyhow!("{e}"))?.to_string();
    let mut cfg = load(&config_file());
    cfg.insert("password_hash".into(), json!(hash));
    save(&config_file(), &cfg)?;
    println!("password set ({}); existing tokens stay valid until revoked (kioku mcp revoke --all)", config_file().display());
    Ok(0)
}

pub fn list() -> Result<i32> {
    let s = Store::load();
    let clients = s.0["clients"].as_object().cloned().unwrap_or_default();
    if clients.is_empty() {
        println!("no clients");
        return Ok(0);
    }
    let t = now();
    for (id, c) in &clients {
        let live = s.0["tokens"].as_object().map(|o| o.values().filter(|v| v["client"] == id.as_str() && v["kind"] == "refresh").count()).unwrap_or(0);
        let days = (t.saturating_sub(c["created"].as_u64().unwrap_or(t))) / 86400;
        println!("{id}  {}  registered {days} day(s) ago, {live} live session(s)", c["client_name"].as_str().unwrap_or("?"));
    }
    Ok(0)
}

pub fn revoke(which: Option<&str>) -> Result<i32> {
    let mut s = Store::load();
    match which {
        Some("--all") => {
            for k in ["clients", "codes", "tokens"] {
                s.map(k).clear();
            }
            println!("all clients and tokens revoked");
        }
        Some(id) if s.client(id).is_some() => {
            s.map("clients").remove(id);
            s.map("tokens").retain(|_, v| v["client"] != id);
            s.map("codes").retain(|_, v| v["client"] != id);
            println!("revoked {id}");
        }
        _ => return util::die("usage: kioku mcp revoke CLIENT_ID|--all (kioku mcp tokens lists them)"),
    }
    s.save();
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_and_pkce() {
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"foobar"), "Zm9vYmFy");
        // Checked against Python: urlsafe_b64encode(sha256(verifier)) without padding.
        assert!(pkce_ok("dBjftJeZ4CVP-mJ92K1vjSpzHJ3L5gTCPNPb0O8nmvQ", "VN9s0g8MBDU0C64fuhSd5221aOjcS5StxftN5wEkgRU"));
        assert!(!pkce_ok("dBjftJeZ4CVP-mJ92K1vjSpzHJ3L5gTCPNPb0O8nmvX", "VN9s0g8MBDU0C64fuhSd5221aOjcS5StxftN5wEkgRU"));
    }

    #[test]
    fn redirects_and_codes() {
        assert!(allowed_redirect("https://claude.ai/api/mcp/auth_callback"));
        assert!(allowed_redirect("http://localhost:6274/oauth/callback"));
        assert!(!allowed_redirect("https://evil.example/cb"));
        let mut s = Store(Map::new());
        for k in ["clients", "codes", "tokens"] {
            s.0.insert(k.into(), json!({}));
        }
        assert!(s.register(&json!({"redirect_uris": ["https://evil.example/cb"]})).is_err());
        let r = s.register(&json!({"redirect_uris": ["https://claude.ai/api/mcp/auth_callback"], "client_name": "Claude"})).unwrap();
        let id = r["client_id"].as_str().unwrap().to_string();
        let verifier = "dBjftJeZ4CVP-mJ92K1vjSpzHJ3L5gTCPNPb0O8nmvQ";
        // A code as login() leaves it, without the password part.
        s.map("codes").insert(sha("c1"), json!({"client": id, "redirect_uri": "https://claude.ai/api/mcp/auth_callback", "challenge": "VN9s0g8MBDU0C64fuhSd5221aOjcS5StxftN5wEkgRU", "scope": "", "exp": now() + 60}));
        let f = |p: &[(&str, &str)]| p.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<HashMap<_, _>>();
        let base = [("grant_type", "authorization_code"), ("code", "c1"), ("client_id", id.as_str()), ("redirect_uri", "https://claude.ai/api/mcp/auth_callback")];
        let mut wrong = base.to_vec();
        wrong.push(("code_verifier", "x".repeat(43).leak()));
        assert!(s.token(&f(&wrong)).is_err());
        // The code was used up by the failed try.
        s.map("codes").insert(sha("c1"), json!({"client": id, "redirect_uri": "https://claude.ai/api/mcp/auth_callback", "challenge": "VN9s0g8MBDU0C64fuhSd5221aOjcS5StxftN5wEkgRU", "scope": "", "exp": now() + 60}));
        let mut good = base.to_vec();
        good.push(("code_verifier", verifier));
        let t = s.token(&f(&good)).unwrap();
        assert!(s.token(&f(&good)).is_err(), "a code works once");
        let access = t["access_token"].as_str().unwrap();
        assert!(s.bearer_ok(Some(&format!("Bearer {access}"))));
        assert!(!s.bearer_ok(Some("Bearer nope")) && !s.bearer_ok(None));
        let refresh = t["refresh_token"].as_str().unwrap().to_string();
        let t2 = s.token(&f(&[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", &id)])).unwrap();
        assert!(s.token(&f(&[("grant_type", "refresh_token"), ("refresh_token", &refresh)])).is_err(), "refresh tokens rotate");
        s.map("clients").remove(&id);
        assert!(!s.bearer_ok(Some(&format!("Bearer {}", t2["access_token"].as_str().unwrap()))), "revoked client");
    }
}
