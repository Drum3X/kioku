//! Secrets that must never land in the vault (SCHEMA.md, Prohibited): API keys and tokens of the
//! common services, private key blocks, JWTs, `password = …` style assignments and card numbers.
//! Used by check-file (the write check), lint-check and the vault's pre-commit hook
//! (`kioku check-secrets --staged`). A line with `kioku:allow-secret` is skipped (a documented
//! example). The patterns are kept narrow: a false alarm on every long random string would be
//! ignored soon.

use crate::util;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub struct Hit {
    pub line: usize,
    pub kind: &'static str,
    /// The match with all but its first few characters hidden.
    pub masked: String,
}

fn patterns() -> &'static [(&'static str, Regex)] {
    static P: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    P.get_or_init(|| {
        [
            ("Anthropic API key", r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
            ("OpenAI API key", r"\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{32,}"),
            ("GitHub token", r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})"),
            ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
            ("AWS access key", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
            ("Slack token", r"\bxox[abprs]-[A-Za-z0-9-]{10,}"),
            ("Telegram bot token", r"\b\d{8,10}:AA[A-Za-z0-9_-]{33}\b"),
            ("private key", r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----"),
            ("JWT", r"\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"),
            (
                "password or key assignment",
                r#"(?i)\b(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?key|auth[_-]?token|client[_-]?secret)\s*[:=]\s*["']?([^\s"'`<>$\{\}\[\]]{8,})"#,
            ),
        ]
        .into_iter()
        .map(|(k, p)| (k, Regex::new(p).unwrap()))
        .collect()
    })
}
fn card_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\b(?:\d[ -]?){12,18}\d\b").unwrap())
}

fn luhn(digits: &[u32]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| if i % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { *d })
        .sum();
    sum % 10 == 0
}
/// An assignment's value that is a placeholder or a reference, not a secret.
fn placeholder(v: &str) -> bool {
    let l = v.to_lowercase();
    let distinct: std::collections::HashSet<char> = l.chars().collect();
    distinct.len() < 4
        || ["xxx", "***", "...", "example", "placeholder", "changeme", "your", "redacted", "env:", "keychain", "secret-tool"]
            .iter()
            .any(|p| l.contains(p))
        || !l.chars().any(|c| c.is_ascii_digit()) && l.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c == '_' || c == '.')
}
fn mask(s: &str) -> String {
    let keep: String = s.chars().take(4).collect();
    format!("{keep}…({} chars)", s.chars().count())
}

pub fn scan(text: &str) -> Vec<Hit> {
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if l.contains("kioku:allow-secret") {
            continue;
        }
        for (kind, re) in patterns() {
            for c in re.captures_iter(l) {
                let m = c.get(1).unwrap_or_else(|| c.get(0).unwrap()).as_str();
                if *kind == "password or key assignment" && placeholder(m) {
                    continue;
                }
                if *kind == "OpenAI API key" && m.starts_with("sk-ant-") {
                    continue;
                }
                out.push(Hit { line: i + 1, kind, masked: mask(m) });
            }
        }
        for m in card_re().find_iter(l) {
            let d: Vec<u32> = m.as_str().chars().filter_map(|c| c.to_digit(10)).collect();
            if (13..=19).contains(&d.len()) && luhn(&d) && d.iter().collect::<std::collections::HashSet<_>>().len() > 3 {
                out.push(Hit { line: i + 1, kind: "card number", masked: mask(m.as_str()) });
            }
        }
    }
    out
}

/// "- file:line: <kind> (abcd…(40 chars))" lines for a file's text.
pub fn report(rel: &str, text: &str) -> String {
    scan(text)
        .into_iter()
        .map(|h| format!("- {rel}:{}: looks like a secret ({}: {}); remove it, keep secrets out of the vault (a documented example: add kioku:allow-secret to the line)\n", h.line, h.kind, h.masked))
        .collect()
}

/// `kioku check-secrets [--staged | FILE…]`: report secrets, exit 1 when any. --staged checks
/// what is about to be committed (the pre-commit hook).
pub fn run(vault: &Path, args: &[String]) -> anyhow::Result<i32> {
    let mut out = String::new();
    if args.first().map(String::as_str) == Some("--staged") {
        let names = util::git_out(vault, &["diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z"]).unwrap_or_default();
        for f in names.split('\0').filter(|f| !f.is_empty()) {
            let Some(text) = util::git_out(vault, &["show", &format!(":{f}")]) else { continue };
            out.push_str(&report(f, &text));
        }
    } else {
        for a in args {
            let p = Path::new(a);
            let p = if p.is_absolute() { p.to_path_buf() } else { vault.join(p) };
            out.push_str(&report(&util::rel(vault, &p).to_string(), &util::read(&p)));
        }
    }
    if out.is_empty() {
        return Ok(0);
    }
    eprint!("Kioku: secrets in the vault are not allowed (SCHEMA.md, Prohibited):\n{out}");
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kinds(t: &str) -> Vec<&'static str> {
        scan(t).into_iter().map(|h| h.kind).collect()
    }
    #[test]
    fn finds_secrets() {
        assert_eq!(kinds("key sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz0123"), ["Anthropic API key"]);
        assert_eq!(kinds("ghp_0123456789abcdefABCDEF0123456789abcd"), ["GitHub token"]);
        assert_eq!(kinds("AKIAIOSFODNN7EXAMPL3"), ["AWS access key"]);
        assert_eq!(kinds("-----BEGIN OPENSSH PRIVATE KEY-----"), ["private key"]);
        assert_eq!(kinds("password: Tr0ub4dor&3xyz"), ["password or key assignment"]);
        assert_eq!(kinds("kart 4539 1488 0343 6467"), ["card number"]);
        assert_eq!(kinds("123456789:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw0"), ["Telegram bot token"]);
    }
    #[test]
    fn leaves_ordinary_text() {
        assert!(kinds("password: keychain'de duruyor").is_empty());
        assert!(kinds("api_key: <your key here>").is_empty());
        assert!(kinds("token: ${GITHUB_TOKEN}").is_empty());
        assert!(kinds("password: changeme123").is_empty());
        assert!(kinds("secret: secret-tool lookup service kioku").is_empty());
        assert!(kinds("commit 25aa56c2f9e1d3b4a5c6d7e8f9a0b1c2d3e4f5a6").is_empty());
        assert!(kinds("phone 0532 123 45 67, date 2026-11-21").is_empty());
        assert!(kinds("sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz0123 kioku:allow-secret").is_empty());
    }
}
