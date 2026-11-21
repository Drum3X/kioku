//! Shared plumbing: where the vault and the system files are, dates, files, external commands.

use anyhow::{bail, Context, Result};
use chrono::{Duration, Local, NaiveDate};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

/// The program as a vault reaches it, `<vault>/.kioku/bin/kioku` (with `/`, without `.exe`).
pub const SELF_SUFFIX: &str = "/.kioku/bin/kioku";
/// The binary's file name: kioku, or kioku.exe on Windows.
pub const EXE: &str = if cfg!(windows) { "kioku.exe" } else { "kioku" };

fn env_path(var: &str) -> Option<PathBuf> {
    env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}
/// $HOME, else %USERPROFILE% (Windows outside Git Bash).
pub fn home() -> PathBuf {
    env_path("HOME").or_else(|| env_path("USERPROFILE")).unwrap_or_else(|| PathBuf::from("/"))
}
/// An XDG directory: the variable when set; else on Windows the given known folder
/// (%APPDATA% or %LOCALAPPDATA%) with `win_sub`; else ~/`default`.
fn xdg(var: &str, default: &str, win: &str, win_sub: &str) -> PathBuf {
    if let Some(v) = env_path(var) {
        return v.join("kioku");
    }
    if cfg!(windows) {
        if let Some(w) = env_path(win) {
            return w.join("kioku").join(win_sub);
        }
    }
    home().join(default).join("kioku")
}
/// ~/.config/kioku (Windows: %APPDATA%\kioku)
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config", "APPDATA", "")
}
/// ~/.cache/kioku (Windows: %LOCALAPPDATA%\kioku\cache)
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache", "LOCALAPPDATA", "cache")
}
/// ~/.local/state/kioku (Windows: %LOCALAPPDATA%\kioku\state)
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state", "LOCALAPPDATA", "state")
}
/// Hermes's home: $HERMES_HOME, else %LOCALAPPDATA%\\hermes on Windows, else ~/.hermes.
pub fn hermes_home() -> PathBuf {
    if let Some(h) = env_path("HERMES_HOME") {
        return h;
    }
    if cfg!(windows) {
        if let Some(l) = env_path("LOCALAPPDATA") {
            return l.join("hermes");
        }
    }
    home().join(".hermes")
}
/// For short-lived markers: $XDG_RUNTIME_DIR, else the system temp folder.
pub fn temp_dir() -> PathBuf {
    env_path("XDG_RUNTIME_DIR").unwrap_or_else(env::temp_dir)
}
/// A path as a string with `/` separators (Windows paths too), for comparing and for links.
pub fn slash(p: &Path) -> String {
    let s = p.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}
/// fs::canonicalize without Windows' `\\?\` prefix (falls back to the path as given).
pub fn canon(p: &Path) -> PathBuf {
    match fs::canonicalize(p) {
        Ok(c) => {
            let s = c.to_string_lossy();
            match s.strip_prefix(r"\\?\") {
                Some(r) if !r.starts_with("UNC") => PathBuf::from(r),
                _ => c,
            }
        }
        Err(_) => p.to_path_buf(),
    }
}
/// A folder as written in a note (`~/Projects/x`, `/home/u/x`, `C:\\Users\\u\\x`) in the form
/// folders are compared in: `~` is this machine's home, `/` separators, no trailing `/`, and
/// lower case on Windows.
pub fn folder_key(p: &str) -> String {
    let p = p.trim();
    let p = match p.strip_prefix('~') {
        Some(r) if r.is_empty() || r.starts_with('/') || r.starts_with('\\') => format!("{}{r}", slash(&home())),
        _ => p.to_string(),
    };
    let p = p.replace('\\', "/");
    let p = p.trim_end_matches('/').to_string();
    if cfg!(windows) {
        p.to_lowercase()
    } else {
        p
    }
}
/// Whether `dir` is the folder `root` (as written in a note) or inside it.
pub fn is_under(dir: &Path, root: &str) -> bool {
    let r = folder_key(root);
    let d = folder_key(&slash(dir));
    !r.is_empty() && (d == r || d.starts_with(&format!("{r}/")))
}
/// Whether the path, written with `/` and without `.exe`, is `<vault>/.kioku/bin/kioku`; the vault.
pub fn vault_of_self(p: &Path) -> Option<PathBuf> {
    let s = slash(p);
    let s = s.strip_suffix(".exe").unwrap_or(&s);
    s.strip_suffix(SELF_SUFFIX).map(PathBuf::from)
}
pub fn record_file() -> PathBuf {
    config_dir().join("installed")
}
/// One `key=value` line from the install record.
pub fn record_get(key: &str) -> Option<String> {
    let s = fs::read_to_string(record_file()).ok()?;
    s.lines().find_map(|l| l.strip_prefix(&format!("{key}=")).map(str::to_string))
}

/// The path this program was called by, as typed (symlinks not resolved), followed back through
/// links on PATH (~/.local/bin/kioku) until it is `<vault>/.kioku/bin/kioku`.
pub fn logical_self() -> Option<PathBuf> {
    let a0 = env::args_os().next()?;
    let a0 = PathBuf::from(a0);
    let mut p = if a0.components().count() > 1 || a0.is_absolute() {
        if a0.is_absolute() {
            a0
        } else {
            env::current_dir().ok()?.join(a0)
        }
    } else {
        env::split_paths(&env::var_os("PATH")?).map(|d| d.join(&a0)).find(|c| c.is_file())?
    };
    for _ in 0..20 {
        if vault_of_self(&p).is_some() {
            return Some(p);
        }
        match fs::read_link(&p) {
            Ok(t) => p = if t.is_absolute() { t } else { p.parent()?.join(t) },
            Err(_) => return Some(p),
        }
    }
    Some(p)
}

/// The vault: $KIOKU_DIR, else <vault> of <vault>/.kioku/bin/kioku, else the install record,
/// else ~/Documents/Kioku.
pub fn vault() -> PathBuf {
    if let Some(v) = env::var_os("KIOKU_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    if let Some(v) = logical_self().as_deref().and_then(vault_of_self) {
        return v;
    }
    if let Some(v) = record_get("vault").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    home().join("Documents/Kioku")
}

/// The system files (schema, skills, templates, skeleton): $KIOKU_SYS, else the folder two levels
/// above this program that holds SCHEMA.md (a checkout or share/kioku in the Nix store), else the
/// vault's .kioku link.
pub fn sys_dir() -> PathBuf {
    if let Some(v) = env::var_os("KIOKU_SYS").filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(p) = logical_self() {
        if let Some(d) = p.parent().and_then(Path::parent) {
            cands.push(d.to_path_buf());
        }
    }
    if let Ok(exe) = env::current_exe() {
        cands.extend(exe.ancestors().skip(1).take(4).map(Path::to_path_buf));
    }
    for c in cands {
        if c.join("SCHEMA.md").is_file() {
            return canon(&c);
        }
    }
    canon(&vault().join(".kioku"))
}

/// This machine's short name: $KIOKU_HOST, else the name given at install (--host), else the
/// hostname up to the first dot.
pub fn host() -> String {
    if let Some(h) = env::var("KIOKU_HOST").ok().filter(|h| !h.trim().is_empty()) {
        return h.trim().to_string();
    }
    if let Some(h) = record_get("host").filter(|h| !h.trim().is_empty()) {
        return h.trim().to_string();
    }
    let h = fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| fs::read_to_string("/etc/hostname"))
        .ok()
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        })
        .unwrap_or_default();
    let h = h.trim().split('.').next().unwrap_or("").to_string();
    if h.is_empty() {
        "host".into()
    } else {
        h
    }
}
/// Who writes the records: "<agent>@<host>" (agent: the argument, else $KIOKU_AGENT, else "agent").
pub fn trace(agent: &str) -> String {
    let a = Some(agent.trim().to_string())
        .filter(|a| !a.is_empty())
        .or_else(|| env::var("KIOKU_AGENT").ok().filter(|a| !a.trim().is_empty()))
        .unwrap_or_else(|| "agent".into());
    format!("{}@{}", a.trim(), host())
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}
pub fn day(offset: i64) -> NaiveDate {
    today() + Duration::days(offset)
}
pub fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// A file as text ("" if unreadable); CRLF line ends (Windows editors) read as LF.
pub fn read(p: &Path) -> String {
    let s = fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    if s.contains('\r') {
        s.replace("\r\n", "\n")
    } else {
        s
    }
}
pub fn read_stdin() -> String {
    let mut s = String::new();
    let _ = std::io::stdin().read_to_string(&mut s);
    s
}
pub fn write(p: &Path, s: &str) -> Result<()> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).with_context(|| format!("creating {}", d.display()))?;
    }
    fs::write(p, s).with_context(|| format!("writing {}", p.display()))
}
/// Path relative to `base` as a string with forward slashes ("" if not inside).
pub fn rel(base: &Path, p: &Path) -> String {
    p.strip_prefix(base).map(slash).unwrap_or_default()
}
/// Character-safe prefix (the bash `${s:0:n}` / JS `slice(0, n)`).
pub fn take(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
/// Byte cap that does not cut a UTF-8 character (head -c + iconv -c).
pub fn cap(s: &str, n: usize) -> &str {
    if s.len() <= n {
        return s;
    }
    let mut i = n;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}
/// Whether a command is on PATH (on Windows also as cmd.exe, cmd.cmd … per PATHEXT).
pub fn which(cmd: &str) -> bool {
    let exts: Vec<String> = if cfg!(windows) {
        let pe = env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        std::iter::once(String::new()).chain(pe.split(';').map(|e| e.to_lowercase())).collect()
    } else {
        vec![String::new()]
    };
    env::var_os("PATH")
        .map(|p| env::split_paths(&p).any(|d| exts.iter().any(|e| d.join(format!("{cmd}{e}")).is_file())))
        .unwrap_or(false)
}

/// Run a command, waiting at most `secs` seconds (0 = no limit); stdin is closed.
pub fn run(cmd: &mut Command, secs: u64) -> Result<Output> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("running {:?}", cmd.get_program()))?;
    if secs > 0 {
        let start = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if start.elapsed().as_secs() >= secs {
                let _ = child.kill();
                let out = child.wait_with_output()?;
                return Ok(Output {
                    status: out.status,
                    stdout: out.stdout,
                    stderr: b"timed out".to_vec(),
                });
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    Ok(child.wait_with_output()?)
}

pub fn git(vault: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("-C").arg(vault);
    c
}
/// `git -C vault args…` → trimmed stdout, or None when it fails.
pub fn git_out(vault: &Path, args: &[&str]) -> Option<String> {
    let o = run(git(vault).args(args), 0).ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}
pub fn autosync(vault: &Path) -> bool {
    git_out(vault, &["config", "--bool", "kioku.autosync"]).as_deref() == Some("true")
}

pub fn die<T>(msg: impl std::fmt::Display) -> Result<T> {
    bail!("{msg}")
}

/// All `*.md` files under `dir`, sorted, without descending into folders whose name starts with
/// a dot.
pub fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.into_path())
        .collect();
    v.sort();
    v
}
/// Files in raw/inbox (recursive) except .gitkeep.
pub fn inbox_count(vault: &Path) -> usize {
    walkdir::WalkDir::new(vault.join("raw/inbox"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name() != ".gitkeep")
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn self_suffix() {
        assert_eq!(vault_of_self(Path::new("/home/u/Brain/.kioku/bin/kioku")), Some(PathBuf::from("/home/u/Brain")));
        assert_eq!(vault_of_self(Path::new("/home/u/Brain/.kioku/bin/kioku.exe")), Some(PathBuf::from("/home/u/Brain")));
        assert_eq!(vault_of_self(Path::new("/usr/bin/kioku")), None);
    }
    #[test]
    fn folders() {
        let h = slash(&home());
        assert!(is_under(Path::new(&format!("{h}/Projects/kioku/src")), "~/Projects/kioku"));
        assert!(is_under(Path::new(&format!("{h}/Projects/kioku")), "~/Projects/kioku/"));
        assert!(!is_under(Path::new(&format!("{h}/Projects/kioku2")), "~/Projects/kioku"));
        assert!(is_under(Path::new("/srv/x/y"), "/srv/x"));
        assert!(!is_under(Path::new("/srv/x"), ""));
    }
    #[test]
    fn crlf() {
        let f = env::temp_dir().join(format!("kioku-crlf-{}.md", std::process::id()));
        fs::write(&f, "---\r\ntitle: X\r\n---\r\n# X\r\n").unwrap();
        assert_eq!(read(&f), "---\ntitle: X\n---\n# X\n");
        fs::remove_file(&f).unwrap();
    }
}
