//! The tools: thin wrappers over the CLI's own functions. Reads return text; writes only append
//! (a daily line, a journal line or todo, an inbox capture), each checked and committed.

use super::Ctx;
use crate::{calendar, context, deep, git, lint, notes, search, secrets, util};
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_OUT: usize = 60_000;

const RECORD_TYPES: &[&str] = &["decision", "done", "learned", "preference", "open", "handoff", "correction"];

fn tool(name: &str, title: &str, desc: &str, props: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": desc,
        "inputSchema": {"type": "object", "properties": props, "required": required},
        "annotations": {"title": title, "readOnlyHint": read_only, "destructiveHint": false, "idempotentHint": read_only, "openWorldHint": false},
    })
}

pub fn list() -> Value {
    json!([
        tool("context", "Session context",
            "The user's Core profile (who they are, how to address them, preferences), active projects, due todos, upcoming events and recent work. Call once at the start of a conversation where the user's past or plans may matter.",
            json!({}), &[], true),
        tool("search", "Search the vault",
            "Ranked full-text search over the vault (Turkish-aware). Each word is a term; put a phrase in double quotes. Returns pages with their best matching lines; read the pages with read_page.",
            json!({"query": {"type": "string", "description": "Search terms"}, "question": {"type": "string", "description": "The user's question in full, if any (improves ranking)"}, "limit": {"type": "integer", "minimum": 1, "maximum": 50}}),
            &["query"], true),
        tool("deep_context", "Project or topic in depth",
            "Everything about one project or topic: its note, decisions, open threads, learnings, recent work and linked pages.",
            json!({"topic": {"type": "string", "description": "A project or topic name, e.g. kioku"}}),
            &["topic"], true),
        tool("read_page", "Read a page",
            "Read one Markdown page of the vault by its path (e.g. projects/kioku.md, wiki/concepts/x, index.md, .kioku/SCHEMA.md).",
            json!({"path": {"type": "string"}}),
            &["path"], true),
        tool("todo", "Open todos",
            "The user's open todos from the journal, by due date.",
            json!({"soon": {"type": "boolean", "description": "Only overdue and due within 3 days"}}),
            &[], true),
        tool("agenda", "Upcoming events",
            "Events from the calendar (recurrences expanded).",
            json!({"days": {"type": "integer", "minimum": 1, "maximum": 366, "description": "How many days ahead (default 7)"}}),
            &[], true),
        tool("recap", "Recent entries",
            "Work log (daily), journal and log entries of the last N days.",
            json!({"days": {"type": "integer", "minimum": 1, "maximum": 90}}),
            &[], true),
        tool("record", "Record to the work log",
            "Append one line to today's work log: a decision, finished work (done), a durable learning (learned), a preference of the user, an open thread, a handoff or a correction. Record without asking; one line per entry, in the conversation's language.",
            json!({
                "project": {"type": "string", "description": "Project slug (as in projects/<slug>.md), or genel"},
                "type": {"type": "string", "enum": RECORD_TYPES},
                "text": {"type": "string", "description": "The entry, one line"},
                "link": {"type": "string", "description": "Optional page it belongs to, e.g. projects/kioku or wiki/concepts/x"}
            }),
            &["project", "type", "text"], false),
        tool("journal_add", "Add to the user's journal",
            "The user's own timeline: something they did or that happened (kind log), or a todo/reminder (kind todo, with an absolute due date if one was given).",
            json!({
                "kind": {"type": "string", "enum": ["log", "todo"]},
                "text": {"type": "string"},
                "due": {"type": "string", "description": "YYYY-MM-DD (todo only)"},
                "at": {"type": "string", "description": "HH:MM (todo only)"}
            }),
            &["kind", "text"], false),
        tool("capture", "Capture to the inbox",
            "Drop a thought, link or note into the vault's inbox (raw/inbox/); an agent with file access files it into the wiki later.",
            json!({"text": {"type": "string"}}),
            &["text"], false),
    ])
}

fn s<'a>(a: &'a Value, k: &str) -> &'a str {
    a.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("")
}

fn n(a: &Value, k: &str, def: i64, lo: i64, hi: i64) -> i64 {
    a.get(k).and_then(Value::as_i64).unwrap_or(def).clamp(lo, hi)
}

fn capped(mut t: String) -> String {
    if t.len() > MAX_OUT {
        let mut i = MAX_OUT;
        while !t.is_char_boundary(i) {
            i -= 1;
        }
        t.truncate(i);
        t.push_str("\n… (cut)");
    }
    if t.trim().is_empty() {
        "(nothing)".into()
    } else {
        t
    }
}

/// Pull at most once a minute before reading, so a long-running server sees other machines' work.
fn fresh(vault: &Path) {
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let mut last = LAST.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(|e| e.into_inner());
    if last.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
        let _ = git::sync(vault, "pull");
        *last = Some(Instant::now());
    }
}

/// None: no such tool.
pub fn call(ctx: &Ctx, name: &str, a: &Value) -> Option<Result<String, String>> {
    let v = ctx.vault.as_path();
    if !list().as_array()?.iter().any(|t| t["name"] == name) {
        return None;
    }
    fresh(v);
    let agent = ctx.me.split('@').next().unwrap_or("mcp");
    Some(match name {
        "context" => Ok(context::build(v, v, "", agent)),
        "search" => {
            let q = s(a, "query");
            let mut args: Vec<String> = shell_words(q);
            if args.is_empty() {
                return Some(Err("query is empty".into()));
            }
            args.extend(["-n".into(), n(a, "limit", 15, 1, 50).to_string()]);
            if !s(a, "question").is_empty() {
                args.extend(["-q".into(), s(a, "question").into()]);
            }
            Ok(search::render(v, &args).unwrap_or_default()).map(|o| if o.trim().is_empty() { "(no results)".into() } else { o })
        }
        "deep_context" if s(a, "topic").is_empty() => Err("topic is empty".into()),
        "deep_context" => Ok(deep::build(v, s(a, "topic"))),
        "read_page" => read_page(v, s(a, "path")),
        "todo" => Ok(notes::todo(v, a.get("soon").and_then(Value::as_bool).unwrap_or(false))),
        "agenda" => Ok(calendar::agenda(v, n(a, "days", 7, 1, 366), false)),
        "recap" => notes::recap(v, &["--days".into(), n(a, "days", 7, 1, 90).to_string()]).map_err(|e| e.to_string()),
        "record" => record(ctx, a),
        "journal_add" => journal_add(ctx, a),
        "capture" => capture(ctx, a),
        _ => return None,
    }
    .map(capped))
}

/// Terms as the CLI gets them from a shell: words, and "quoted phrases" as one term.
fn shell_words(q: &str) -> Vec<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#""([^"]+)"|(\S+)"#).unwrap())
        .captures_iter(q)
        .map(|c| c.get(1).map(|m| format!("\"{}\"", m.as_str())).or_else(|| c.get(2).map(|m| m.as_str().to_string())).unwrap_or_default())
        .collect()
}

fn read_page(vault: &Path, p: &str) -> Result<String, String> {
    let p = p.trim().trim_start_matches("[[").trim_end_matches("]]");
    let p = p.split('|').next().unwrap_or("").trim().trim_start_matches('/');
    if p.is_empty() {
        return Err("path is empty".into());
    }
    let p = if p.ends_with(".md") { p.to_string() } else { format!("{p}.md") };
    if p.split(['/', '\\']).any(|c| c == ".." || c == ".git") || Path::new(&p).is_absolute() {
        return Err(format!("not a page of the vault: {p}"));
    }
    let f = vault.join(&p);
    let inside = util::canon(&f).starts_with(util::canon(vault)) || p == ".kioku/SCHEMA.md";
    if !inside || !f.is_file() {
        return Err(format!("no such page: {p}"));
    }
    Ok(util::read(&f))
}

fn check_text(t: &str) -> Result<(), String> {
    if t.is_empty() {
        return Err("text is empty".into());
    }
    if t.contains('\n') || t.contains('\r') {
        return Err("one line only".into());
    }
    if t.chars().count() > 2000 {
        return Err("too long (max 2000 characters)".into());
    }
    if let Some(h) = secrets::scan(t).first() {
        return Err(format!("refused: the text looks like it contains a secret ({}: {}). Secrets never go into the vault.", h.kind, h.masked));
    }
    Ok(())
}

/// Append `line` to `rel` (or put it under `section`), check the file, commit it.
fn write_line(ctx: &Ctx, rel: &str, section: Option<&str>, line: &str, subject: &str) -> Result<String, String> {
    let v = ctx.vault.as_path();
    let _ = git::sync(v, "pull");
    let f = v.join(rel);
    let old = util::read(&f);
    let new = match section {
        Some(h) => insert_under(&old, h, line),
        None => format!("{}{line}\n", if old.is_empty() || old.ends_with('\n') { old.clone() } else { format!("{old}\n") }),
    };
    util::write(&f, &new).map_err(|e| e.to_string())?;
    let warn = lint::check_file(v, &[rel.to_string()]);
    let me = format!("Kioku-Agent: {}", ctx.me);
    let add = util::run(util::git(v).args(["add", "--", rel]), 10);
    let commit = util::run(util::git(v).args(["commit", "-q", "-m", subject, "-m", &me, "--", rel]), 60);
    let committed = add.is_ok_and(|o| o.status.success()) && commit.as_ref().is_ok_and(|o| o.status.success());
    let mut out = format!("Saved to {rel}: {line}\n");
    if !committed {
        let why = commit.map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string()).unwrap_or_else(|e| e.to_string());
        out.push_str(&format!("(not committed: {why})\n"));
    }
    if !warn.is_empty() {
        out.push_str(&format!("Write check:\n{warn}"));
    }
    Ok(out)
}

/// `line` at the end of the `## heading` section (before its trailing blank lines), or a new
/// section at the end.
fn insert_under(text: &str, heading: &str, line: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let Some(h) = lines.iter().position(|l| l.trim_end() == heading) else {
        let sep = if text.is_empty() || text.ends_with("\n\n") { "" } else if text.ends_with('\n') { "\n" } else { "\n\n" };
        return format!("{text}{sep}{heading}\n{line}\n");
    };
    let end = lines.iter().enumerate().skip(h + 1).find(|(_, l)| l.starts_with("## ")).map(|(i, _)| i).unwrap_or(lines.len());
    let mut at = end;
    while at > h + 1 && lines[at - 1].trim().is_empty() {
        at -= 1;
    }
    lines.insert(at, line.to_string());
    if at + 1 < lines.len() && lines[at + 1].starts_with("## ") {
        lines.insert(at + 1, String::new());
    }
    lines.join("\n") + "\n"
}

fn now_hm() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

fn record(ctx: &Ctx, a: &Value) -> Result<String, String> {
    let (project, ty, text, link) = (s(a, "project").to_lowercase(), s(a, "type"), s(a, "text"), s(a, "link"));
    if !RECORD_TYPES.contains(&ty) {
        return Err(format!("type must be one of: {}", RECORD_TYPES.join(", ")));
    }
    static SLUG: OnceLock<Regex> = OnceLock::new();
    if !SLUG.get_or_init(|| Regex::new(r"^[\p{L}\p{N}][\p{L}\p{N}._-]{0,60}$").unwrap()).is_match(&project) {
        return Err("project must be a slug (as in projects/<slug>.md) or genel".into());
    }
    check_text(text)?;
    let link = link.trim_start_matches("[[").trim_end_matches("]]").trim_end_matches(".md");
    if link.contains(['[', ']', '\n']) {
        return Err("link must be a page path like projects/kioku".into());
    }
    let tail = if link.is_empty() { String::new() } else { format!(" ([[{link}]])") };
    let f = notes::today(&ctx.vault).map_err(|e| e.to_string())?;
    let rel = util::rel(&ctx.vault, &f);
    let line = format!("- {} [{project} · {}] {ty}: {text}{tail}", now_hm(), ctx.me);
    write_line(ctx, &rel, None, &line, &format!("memory: {ty} ({project})"))
}

fn journal_add(ctx: &Ctx, a: &Value) -> Result<String, String> {
    let (kind, text, due, at) = (s(a, "kind"), s(a, "text"), s(a, "due"), s(a, "at"));
    check_text(text)?;
    let f = notes::journal(&ctx.vault).map_err(|e| e.to_string())?;
    let rel = util::rel(&ctx.vault, &f);
    let (heading, line) = match kind {
        "log" => ("## Log", format!("- {} {text}", now_hm())),
        "todo" => {
            let mut l = format!("- [ ] {text}");
            if !due.is_empty() {
                if chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d").is_err() {
                    return Err("due must be YYYY-MM-DD".into());
                }
                l.push_str(&format!(" (due: {due})"));
            }
            if !at.is_empty() {
                if chrono::NaiveTime::parse_from_str(at, "%H:%M").is_err() {
                    return Err("at must be HH:MM".into());
                }
                l.push_str(&format!(" (at: {at})"));
            }
            ("## Todos", l)
        }
        _ => return Err("kind must be log or todo".into()),
    };
    write_line(ctx, &rel, Some(heading), &line, &format!("journal: {kind}"))
}

fn capture(ctx: &Ctx, a: &Value) -> Result<String, String> {
    let text = a.get("text").and_then(Value::as_str).unwrap_or("").trim();
    if text.is_empty() {
        return Err("text is empty".into());
    }
    if text.chars().count() > 20_000 {
        return Err("too long (max 20000 characters)".into());
    }
    if let Some(h) = secrets::scan(text).first() {
        return Err(format!("refused: the text looks like it contains a secret ({}: {}).", h.kind, h.masked));
    }
    let _ = git::sync(&ctx.vault, "pull");
    match notes::capture(&ctx.vault, &["--".into(), text.into()]) {
        Ok(Some(m)) => Ok(m),
        Ok(None) => Err("nothing captured".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_under_a_section() {
        let t = "# d\n\n## Log\n- 10:00 a\n\n## Todos\n\n## Saved\n";
        assert_eq!(insert_under(t, "## Log", "- 11:00 b"), "# d\n\n## Log\n- 10:00 a\n- 11:00 b\n\n## Todos\n\n## Saved\n");
        assert_eq!(insert_under(t, "## Todos", "- [ ] c"), "# d\n\n## Log\n- 10:00 a\n\n## Todos\n- [ ] c\n\n## Saved\n");
        assert_eq!(insert_under("# d\n", "## Todos", "- [ ] c"), "# d\n\n## Todos\n- [ ] c\n");
        assert_eq!(insert_under("## Saved\n- x", "## Saved", "- y"), "## Saved\n- x\n- y\n");
    }

    #[test]
    fn read_page_stays_in_the_vault() {
        let v = std::env::temp_dir().join(format!("kioku-mcp-{}", std::process::id()));
        std::fs::create_dir_all(v.join("projects")).unwrap();
        std::fs::write(v.join("projects/x.md"), "# X\n").unwrap();
        assert_eq!(read_page(&v, "projects/x").unwrap(), "# X\n");
        assert_eq!(read_page(&v, "[[projects/x|X]]").unwrap(), "# X\n");
        assert!(read_page(&v, "../etc/passwd").is_err());
        assert!(read_page(&v, "/etc/passwd").is_err());
        assert!(read_page(&v, ".git/config").is_err());
        assert!(read_page(&v, "projects/none").is_err());
        std::fs::remove_dir_all(&v).unwrap();
    }

    #[test]
    fn checks_written_text() {
        assert!(check_text("bir karar").is_ok());
        assert!(check_text("iki\nsatır").is_err());
        assert!(check_text("key sk-ant-abcdefghijklmnopqrstuvwxyz0123").is_err());
        assert_eq!(shell_words(r#"rebuild "nixos alias" x"#), vec!["rebuild", "\"nixos alias\"", "x"]);
    }
}
