//! Pages and notes: today, journal, todo, new, log, recap, capture.

use crate::fm;
use crate::text::slugify;
use crate::util::{self, die, ymd};
use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// A template with {{title}} and {{date}} filled in (trailing newlines collapsed to one).
fn render(vault: &Path, tpl: &str, title: &str) -> String {
    let c = util::read(&vault.join(".kioku/templates").join(tpl));
    let c = c.trim_end_matches('\n');
    format!("{}\n", c.replace("{{title}}", title).replace("{{date}}", &ymd(util::today())))
}

fn dated(vault: &Path, dir: &str, tpl: &str) -> Result<PathBuf> {
    let t = ymd(util::today());
    let f = vault.join(dir).join(format!("{t}.md"));
    if !f.exists() {
        util::write(&f, &render(vault, tpl, &t))?;
    }
    Ok(f)
}
pub fn today(vault: &Path) -> Result<PathBuf> {
    dated(vault, "daily", "daily.md")
}
pub fn journal(vault: &Path) -> Result<PathBuf> {
    dated(vault, "journal", "journal.md")
}

/// Open todos ("- [ ] …") in journal/, sorted by "due: YYYY-MM-DD". `soon`: only overdue and due
/// within 3 days, plus a count of the rest.
pub fn todo(vault: &Path, soon: bool) -> String {
    static OPEN: OnceLock<Regex> = OnceLock::new();
    static ID: OnceLock<Regex> = OnceLock::new();
    static DUE: OnceLock<Regex> = OnceLock::new();
    let open = OPEN.get_or_init(|| Regex::new(r"^[ \t\n\r\x0B\x0C]*- \[ \] ").unwrap());
    let id = ID.get_or_init(|| Regex::new(r"[ \t]+\^t-[0-9a-z]+[ \t]*$").unwrap());
    let due_re = DUE.get_or_init(|| Regex::new(r"due: ?[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap());
    let mut rows: Vec<String> = Vec::new();
    for f in util::md_files(&vault.join("journal")) {
        let rel = util::rel(vault, &f);
        for (i, line) in util::read(&f).lines().enumerate() {
            let Some(m) = open.find(line) else { continue };
            let text = id.replace(&line[m.end()..], "").into_owned();
            let due = due_re
                .find(&text)
                .map(|m| m.as_str()[m.as_str().len() - 10..].to_string())
                .unwrap_or_else(|| "9999-99-99".into());
            rows.push(format!("{due}\t{text}\t{rel}:{}", i + 1));
        }
    }
    if rows.is_empty() {
        return "(no open todos)\n".into();
    }
    rows.sort();
    let t = ymd(util::today());
    let s = ymd(util::day(3));
    let (mut out, mut rest) = (String::new(), 0);
    for r in rows {
        let mut p = r.splitn(3, '\t');
        let (due, text, loc) = (p.next().unwrap(), p.next().unwrap_or(""), p.next().unwrap_or(""));
        let late = if due != "9999-99-99" && due < t.as_str() {
            " (overdue)"
        } else {
            ""
        };
        if soon && (due == "9999-99-99" || due > s.as_str()) {
            rest += 1;
            continue;
        }
        out.push_str(&format!("- {text}{late} — {loc}\n"));
    }
    if rest > 0 {
        out.push_str(&format!("- … {rest} more open todo(s) without a near due date (kioku todo)\n"));
    }
    out
}

pub fn new(vault: &Path, args: &[String]) -> Result<PathBuf> {
    if args.len() < 2 {
        return die("usage: kioku new <type> \"<Title>\"   (event: kioku new event \"<Title>\" YYYY-MM-DD)");
    }
    let ty = args[0].as_str();
    if ty == "event" {
        let d = args.last().unwrap();
        static D: OnceLock<Regex> = OnceLock::new();
        if args.len() < 3 || !D.get_or_init(|| Regex::new(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$").unwrap()).is_match(d) {
            return die("usage: kioku new event \"<Title>\" YYYY-MM-DD");
        }
        let title = args[1..args.len() - 1].join(" ");
        let slug = slugify(&title);
        if slug.is_empty() {
            return die("could not make a slug from the title");
        }
        let f = vault.join("calendar").join(&d[..4]).join(format!("{d}-{slug}.md"));
        if f.exists() {
            return die(format!("already exists: {}", f.display()));
        }
        let body: String = render(vault, "event.md", &title)
            .lines()
            .map(|l| if l == "start:" { format!("start: {d}\n") } else { format!("{l}\n") })
            .collect();
        util::write(&f, &body)?;
        return Ok(f);
    }
    let title = args[1..].join(" ");
    let dir = match ty {
        "contact" => "contacts".to_string(),
        "review" => format!("reviews/{}", &ymd(util::today())[..4]),
        "source" => "wiki/sources".into(),
        "concept" => "wiki/concepts".into(),
        "entity" | "tool" => "wiki/entities".into(),
        "synthesis" => "wiki/synthesis".into(),
        "idea" => "wiki/ideas".into(),
        "topic" => "wiki/topics".into(),
        "project" => "projects".into(),
        _ => return die(format!("unknown type: {ty}")),
    };
    let slug = slugify(&title);
    if slug.is_empty() {
        return die("could not make a slug from the title");
    }
    let f = vault.join(&dir).join(format!("{slug}.md"));
    if f.exists() {
        return die(format!("already exists: {}", f.display()));
    }
    util::write(&f, &render(vault, &format!("{ty}.md"), &title))?;
    Ok(f)
}

pub fn log(vault: &Path, args: &[String]) -> Result<String> {
    if args.len() < 2 {
        return die("usage: kioku log <op> \"<Title>\"");
    }
    let op = args[0].as_str();
    if !matches!(op, "ingest" | "query" | "lint" | "note" | "setup") {
        return die(format!("unknown op: {op}"));
    }
    let title = args[1..].join(" ");
    let t = ymd(util::today());
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(vault.join("log.md"))?;
    write!(f, "\n## [{t}] {op} | {title}\n")?;
    Ok(format!("log.md: [{t}] {op} | {title}"))
}

/// Body lines without "# " titles and blanks; a heading is printed only if something follows it.
fn compact_heads(lines: &[&str]) -> String {
    let mut out = String::new();
    let mut head: Option<&str> = None;
    for l in lines {
        if l.trim().is_empty() || l.starts_with("# ") {
            continue;
        }
        if l.starts_with('#') {
            head = Some(l);
            continue;
        }
        if let Some(h) = head.take() {
            out.push_str(h);
            out.push('\n');
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

pub fn recap(vault: &Path, args: &[String]) -> Result<String> {
    let mut n: i64 = 7;
    if args.first().map(String::as_str) == Some("--days") {
        let v = args.get(1).map(String::as_str).unwrap_or("7");
        n = v.parse().map_err(|_| anyhow::anyhow!("--days must be a number"))?;
    }
    let since = ymd(util::day(-n));
    let mut out = format!("# Last {n} days ({since} → {})\n", ymd(util::today()));
    for kind in ["daily", "journal"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(vault.join(kind))
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.retain(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            n.ends_with(".md") && n.starts_with(|c: char| c.is_ascii_digit()) && p.is_file()
        });
        files.sort();
        for f in files {
            let d = f.file_stem().unwrap().to_string_lossy().into_owned();
            if d.as_str() < since.as_str() {
                continue;
            }
            let text = util::read(&f);
            if fm::is_private(&text) {
                out.push_str(&format!("## {kind} {d} [private]\n"));
                continue;
            }
            out.push_str(&format!("## {kind} {d}\n"));
            out.push_str(&compact_heads(&fm::body_lines(&text)));
        }
    }
    out.push_str("## log.md\n");
    for l in util::read(&vault.join("log.md")).lines().filter(|l| l.starts_with("## [")) {
        let d = util::take(&l.chars().skip(4).collect::<String>(), 10);
        if d.as_str() >= since.as_str() {
            out.push_str(&l[3..]);
            out.push('\n');
        }
    }
    Ok(out)
}

/// path, or path-2, path-3 … (before the extension) if it exists.
fn free_name(p: PathBuf) -> PathBuf {
    if !p.exists() {
        return p;
    }
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    let (b, e) = match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i..].to_string()),
        _ => (name.clone(), String::new()),
    };
    let dir = p.parent().unwrap().to_path_buf();
    let mut i = 2;
    loop {
        let c = dir.join(format!("{b}-{i}{e}"));
        if !c.exists() {
            return c;
        }
        i += 1;
    }
}

const CAPTURE_USAGE: &str = "usage: kioku capture \"text or link\" | - (stdin) | --file PATH | --gui";

pub fn capture(vault: &Path, args: &[String]) -> Result<Option<String>> {
    let (mut gui, mut file, mut text) = (false, None::<String>, String::new());
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--gui" => gui = true,
            "--file" => {
                file = Some(args.get(i + 1).cloned().ok_or_else(|| anyhow::anyhow!("--file needs a path"))?);
                i += 1;
            }
            "-" => {
                text = util::read_stdin();
                break;
            }
            "--" => {
                text = args[i + 1..].join(" ");
                break;
            }
            a if a.starts_with('-') => return die("usage: kioku capture \"text or link\" | --file PATH | --gui"),
            _ => {
                text = args[i..].join(" ");
                break;
            }
        }
        i += 1;
    }
    if gui {
        let prompt = "Capture to Kioku (a thought, a todo or a link):";
        let out = if util::which("kdialog") {
            Command::new("kdialog")
                .args(["--title", "Kioku", "--textinputbox", prompt, ""])
                .output()
        } else if util::which("zenity") {
            Command::new("zenity")
                .args(["--entry", "--title", "Kioku", "--text", prompt])
                .output()
        } else {
            return die("--gui needs kdialog or zenity");
        }?;
        if !out.status.success() {
            return Ok(None);
        }
        text = String::from_utf8_lossy(&out.stdout).into_owned();
    }
    let text = text.trim_end().to_string();
    if text.is_empty() && file.is_none() {
        if gui {
            return Ok(None);
        }
        return die(CAPTURE_USAGE);
    }
    let inbox = vault.join("raw/inbox");
    std::fs::create_dir_all(&inbox)?;
    let now = chrono::Local::now();
    let stamp = now.format("%Y-%m-%d-%H%M%S").to_string();
    let out = if let Some(f) = file {
        let src = PathBuf::from(&f);
        if !src.is_file() {
            return die(format!("no such file: {f}"));
        }
        let o = free_name(inbox.join(format!("{stamp}-{}", src.file_name().unwrap().to_string_lossy())));
        std::fs::copy(&src, &o)?;
        o
    } else {
        static LINK: OnceLock<Regex> = OnceLock::new();
        let kind = if LINK.get_or_init(|| Regex::new(r"^\s*https?://\S+$").unwrap()).is_match(&text) {
            "link"
        } else {
            "note"
        };
        let o = free_name(inbox.join(format!("{stamp}-{kind}.md")));
        util::write(
            &o,
            &format!(
                "---\ntype: capture\nkind: {kind}\ncaptured: {}\n---\n{text}\n",
                now.format("%Y-%m-%d %H:%M")
            ),
        )?;
        o
    };
    if vault.join(".git").is_dir() {
        let name = out.file_name().unwrap().to_string_lossy().into_owned();
        if util::run(util::git(vault).arg("add").arg("--").arg(&out), 0).is_ok_and(|o| o.status.success()) {
            let _ = util::run(
                util::git(vault)
                    .args(["commit", "-q", "-m", &format!("capture: {name}"), "--"])
                    .arg(&out),
                0,
            );
        }
    }
    if gui && util::which("notify-send") {
        let _ = Command::new("notify-send")
            .args(["-a", "Kioku", "Kioku", "Captured to the inbox"])
            .output();
    }
    Ok(Some(format!("captured: {}", util::rel(vault, &out))))
}
