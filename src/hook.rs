//! `kioku hook <agent> <event>`: the agents' hooks. They read the agent's JSON on stdin and
//! print its JSON answer. A hook never breaks a session: on any problem it prints the "do
//! nothing" answer.
//!
//!   claude       session-start | prompt | post-write | pre-compact | stop
//!   codex        session-start | prompt | tool | stop
//!   antigravity  session-start | pull | stop
//!   hermes       session-start | prompt | tool | stop  (called by the Kioku plugin for Hermes,
//!                hermes-plugin/kioku; it answers {"context"}, {"append"} or {"message"})

use crate::{context, git, lint, util};
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn run(args: &[String]) -> i32 {
    let agent = args.first().map(String::as_str).unwrap_or("");
    let ev = args.get(1).map(String::as_str).unwrap_or("");
    let input = util::read_stdin();
    let j: Value = serde_json::from_str(&input).unwrap_or(json!({}));
    let vault = util::vault();
    let out = match agent {
        "claude" => claude(&vault, ev, &j),
        "codex" => codex(&vault, ev, &j, &input),
        "antigravity" => antigravity(&vault, ev, &j, &input),
        "hermes" => hermes(&vault, ev, &j),
        _ => None,
    };
    if let Some(o) = out {
        println!("{o}");
    }
    0
}

fn s<'a>(j: &'a Value, k: &str) -> &'a str {
    j.get(k).and_then(Value::as_str).unwrap_or("")
}
fn off(var: &str) -> bool {
    std::env::var(var).as_deref() == Ok("0")
}
fn block(reason: String) -> String {
    json!({"decision": "block", "reason": reason}).to_string()
}

// ---------- shared pieces ----------

fn session_start(vault: &Path, j: &Value, event: &str, agent: &str) -> Option<String> {
    if !vault.is_dir() {
        return None;
    }
    let cwd = Some(s(j, "cwd"))
        .filter(|c| !c.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    Some(context::hook_json(event, &context::build(vault, &cwd, s(j, "source"), agent)))
}

/// With autosync on, pull before a prompt (at most every 30 s) and say which files came in.
fn prompt_pull(vault: &Path, event: &str) -> Option<String> {
    if !vault.join(".git").is_dir() || !util::autosync(vault) {
        return None;
    }
    let st = util::state_dir();
    fs::create_dir_all(&st).ok()?;
    let stamp = st.join("prompt-pull.stamp");
    let now = chrono::Utc::now().timestamp();
    let last: i64 = util::read(&stamp).trim().parse().unwrap_or(0);
    if now - last < 30 {
        return None;
    }
    let _ = fs::write(&stamp, format!("{now}\n"));
    let old = util::git_out(vault, &["merge-base", "HEAD", "@{u}"])?;
    if !git::sync(vault, "pull").unwrap_or(false) {
        return None;
    }
    let new = util::git_out(vault, &["rev-parse", "-q", "--verify", "@{u}"])?;
    if old == new {
        return None;
    }
    let range = format!("{old}..{new}");
    let commits: String = util::git_out(vault, &["log", "--no-merges", "--format=- %s", &range])
        .unwrap_or_default()
        .lines()
        .take(10)
        .collect::<Vec<_>>()
        .join("\n");
    let files: String = util::git_out(vault, &["diff", "--name-only", &old, &new])
        .unwrap_or_default()
        .lines()
        .take(20)
        .map(|l| format!("- {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    let msg = format!(
        "The vault was just updated from its git remote (written on another machine):\n{commits}\nChanged files:\n{files}\nAnything about these (e.g. the todos, agenda or recent notes in the session-start context) is out of date: read the files again before answering."
    );
    Some(context::hook_json(event, &msg))
}

/// The path as `<vault>/…` when it is inside the vault (given as written or with symlinks resolved).
fn in_vault(vault: &Path, f: &str) -> Option<String> {
    let v = vault.display().to_string();
    let real = fs::canonicalize(vault)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| v.clone());
    if let Some(r) = f.strip_prefix(&format!("{v}/")) {
        return Some(format!("{v}/{r}"));
    }
    if let Some(r) = f.strip_prefix(&format!("{real}/")) {
        return Some(format!("{v}/{r}"));
    }
    None
}
fn write_check(vault: &Path, files: &[String]) -> Option<String> {
    let out = lint::check_file(vault, files);
    if out.is_empty() {
        return None;
    }
    Some(block(format!("Kioku: the vault file you just wrote has problems:\n{}\nFix them now (see the Kioku schema). A link to a page you are about to create is fine: create that page next.", out.trim_end_matches('\n'))))
}

const PRE_COMPACT: &str = "Kioku (the user's second brain, {V}): add a section to the summary titled \"Kioku: not yet recorded\". List, one line each, the decisions, finished results, durable learnings, user preferences and open threads from this conversation that have NOT been written to the vault yet (vault writes are file edits under {V} and kioku or git commit commands run there). Give each line the details needed to record it later: what, why, which project, file paths, dates. If everything was recorded or nothing is worth recording, write: \"Kioku: not yet recorded: none\".";
const LEARNED_REASON: &str = "Following step 3 of \"Memory: capturing from conversations\" in the Kioku schema, add each durable one to its wiki/concepts or wiki/entities page (Topics line, topic hub, source link to the daily note), then end its daily line with a link to that page, e.g. ([[wiki/concepts/x]]), and commit. Mention it to the user in one line.";
const RECORD_REASON: &str = "Kioku: work was done in this session but nothing was recorded to the second brain. Following \"Memory: capturing from conversations\" in the Kioku schema (.kioku/SCHEMA.md in the vault), record any durable decision, result, learning, preference or open thread and commit; otherwise say \"Kioku: nothing to record\" in one line (in the user's language). Do not explain this check to the user at length.";

fn learned_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^- [0-9:]+ \[[^\]]*\] learned:").unwrap())
}
/// Learned lines of today's and yesterday's daily note without a [[wiki/…]] link whose text (first
/// 40 characters) appears in one of the `inputs` lines (this session's tool calls).
fn unlinked_learned_in(vault: &Path, inputs: &str) -> String {
    let mut hits = String::new();
    for d in [util::day(0), util::day(-1)] {
        for l in util::read(&vault.join("daily").join(format!("{}.md", util::ymd(d)))).lines() {
            if !learned_re().is_match(l) || l.contains("[[wiki/") {
                continue;
            }
            let txt = l.split_once("learned: ").map(|x| x.1).unwrap_or(l);
            let key = util::take(txt, 40);
            if inputs.lines().any(|i| i.contains(&key)) {
                hits.push_str(&format!("- {}\n", util::take(txt, 160)));
            }
        }
    }
    hits
}
fn learned_block(hits: &str) -> String {
    block(format!(
        "Kioku: these learnings went into the daily log in this session but not into the wiki:\n{hits}{LEARNED_REASON}"
    ))
}

/// All string leaves of a JSON value, one per line (jq '.. | strings').
fn strings(v: &Value, out: &mut String) {
    match v {
        Value::String(s) => {
            out.push_str(s);
            out.push('\n');
        }
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

/// The Stop reminder from a list of tool calls (name, input): asks once per session to move
/// learnings into the wiki, and once to record when real work left nothing in the vault.
fn stop_check(vault: &Path, sid: &str, calls: &[(String, Value)]) -> Option<String> {
    if off("KIOKU_STOP_CHECK") || !vault.is_dir() {
        return None;
    }
    let mdir = PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())).join("kioku-stop");
    fs::create_dir_all(&mdir).ok()?;
    let marker = mdir.join(sid);
    let learned_marker = mdir.join(format!("{sid}.learned"));
    if !learned_marker.exists() {
        let mut inputs = String::new();
        for (_, i) in calls {
            strings(i, &mut inputs);
        }
        let hits = unlinked_learned_in(vault, &inputs);
        if !hits.is_empty() {
            let _ = fs::write(&learned_marker, "");
            return Some(learned_block(&hits));
        }
    }
    if marker.exists() {
        return None;
    }
    let v = vault.display().to_string();
    let tilde = match v.strip_prefix(&util::home().display().to_string()) {
        Some(r) => format!("~{r}"),
        None => v.clone(),
    };
    static WRITES: OnceLock<Regex> = OnceLock::new();
    let writes = WRITES.get_or_init(|| Regex::new(r"(kioku (today|journal|new|log)|commit|>>|tee )").unwrap());
    let (mut total, mut edits, mut wrote) = (0, 0, 0);
    for (name, i) in calls {
        total += 1;
        let file = i
            .get("file_path")
            .or_else(|| i.get("notebook_path"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let cmd = i.get("command").and_then(Value::as_str).unwrap_or("");
        if matches!(name.as_str(), "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
            if file.starts_with(&v) {
                wrote += 1;
            } else {
                edits += 1;
            }
        }
        if name == "Bash" && (cmd.contains(&v) || cmd.contains(&tilde)) && writes.is_match(cmd) {
            wrote += 1;
        }
    }
    let min: usize = std::env::var("KIOKU_STOP_MIN_TOOLS")
        .ok()
        .and_then(|x| x.parse().ok())
        .unwrap_or(10);
    if wrote > 0 || !(edits >= 1 || total >= min) {
        return None;
    }
    let _ = fs::write(&marker, "");
    Some(block(RECORD_REASON.into()))
}

// ---------- Claude Code ----------

fn claude(vault: &Path, ev: &str, j: &Value) -> Option<String> {
    match ev {
        "session-start" => session_start(vault, j, "SessionStart", "claude"),
        "prompt" => prompt_pull(vault, "UserPromptSubmit"),
        "post-write" => {
            if off("KIOKU_WRITE_CHECK") {
                return None;
            }
            let ti = j.get("tool_input")?;
            let f = ti.get("file_path").or_else(|| ti.get("notebook_path")).and_then(Value::as_str)?;
            write_check(vault, &[in_vault(vault, f)?])
        }
        "pre-compact" => (!off("KIOKU_STOP_CHECK") && vault.is_dir()).then(|| PRE_COMPACT.replace("{V}", &vault.display().to_string())),
        "stop" => {
            if j.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let sid = s(j, "session_id");
            let tp = s(j, "transcript_path");
            if sid.is_empty() || !Path::new(tp).is_file() {
                return None;
            }
            let mut calls = Vec::new();
            for l in util::read(Path::new(tp)).lines() {
                let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
                if s(&e, "type") != "assistant" {
                    continue;
                }
                for c in e.pointer("/message/content").and_then(Value::as_array).into_iter().flatten() {
                    if s(c, "type") == "tool_use" {
                        calls.push((s(c, "name").to_string(), c.get("input").cloned().unwrap_or(json!({}))));
                    }
                }
            }
            stop_check(vault, sid, &calls)
        }
        _ => None,
    }
}

// ---------- Codex ----------
// Its transcript format is not a stable interface, so the Stop reminder does not read it: the
// tool hook keeps this session's tool calls (count, edits outside the vault, writes to the
// vault, their text) in ~/.local/state/kioku/codex/.

fn codex(vault: &Path, ev: &str, j: &Value, raw: &str) -> Option<String> {
    let st = util::state_dir().join("codex");
    fs::create_dir_all(&st).ok()?;
    let _ = fs::write(st.join(format!("last-{ev}.json")), format!("{raw}\n"));
    let sid: String = s(j, "session_id")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
        .collect();
    let f = |ext: &str| st.join(format!("{sid}.{ext}"));
    let append = |ext: &str, text: &str| {
        use std::io::Write;
        if let Ok(mut h) = fs::OpenOptions::new().create(true).append(true).open(f(ext)) {
            let _ = h.write_all(text.as_bytes());
        }
    };
    match ev {
        "session-start" => {
            if !sid.is_empty() {
                append("total", "");
                clean_old(&st, 14);
            }
            session_start(vault, j, "SessionStart", "codex")
        }
        "prompt" => prompt_pull(vault, "UserPromptSubmit"),
        "tool" => {
            if sid.is_empty() {
                return None;
            }
            append("total", ".");
            let tn = s(j, "tool_name");
            let ti = j.get("tool_input").cloned().unwrap_or(json!({}));
            let cmd = match ti.get("command").or_else(|| ti.get("input")).or_else(|| ti.get("patch")) {
                Some(Value::String(x)) => x.clone(),
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "),
                _ => String::new(),
            };
            let cwd = Some(s(j, "cwd"))
                .filter(|c| !c.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().display().to_string());
            append("inputs", &format!("{cmd}\n"));
            if tn == "Bash" {
                static W: OnceLock<Regex> = OnceLock::new();
                let w = W.get_or_init(|| Regex::new(r"commit|kioku (today|journal|new|log|capture)|>>|tee ").unwrap());
                let v = vault.display().to_string();
                let tilde = v
                    .strip_prefix(&util::home().display().to_string())
                    .map(|r| format!("~{r}"))
                    .unwrap_or(v.clone());
                let here = in_vault(vault, &format!("{cwd}/")).is_some() || cwd == v;
                if (here || cmd.contains(&v) || cmd.contains(&tilde)) && w.is_match(&cmd) {
                    append("vault", "");
                }
                return None;
            }
            if tn != "apply_patch" {
                return None;
            }
            static H: OnceLock<Regex> = OnceLock::new();
            let h = H.get_or_init(|| Regex::new(r"^\*\*\* (?:(?:Add|Update|Delete) File|Move to): (.+)$").unwrap());
            let mut files = Vec::new();
            for line in cmd.lines() {
                let Some(c) = h.captures(line) else { continue };
                let p = &c[1];
                let abs = if p.starts_with('/') { p.to_string() } else { format!("{cwd}/{p}") };
                match in_vault(vault, &abs) {
                    Some(x) => files.push(x),
                    None => append("edits", "."),
                }
            }
            if files.is_empty() {
                return None;
            }
            append("vault", "");
            if off("KIOKU_WRITE_CHECK") {
                return None;
            }
            write_check(vault, &files)
        }
        "stop" => {
            // Commits made inside Codex's sandbox may not have been pushed (no network there).
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe)
                    .args(["remote", "now"])
                    .env("KIOKU_DIR", vault)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            if off("KIOKU_STOP_CHECK")
                || j.get("stop_hook_active").and_then(Value::as_bool) == Some(true)
                || sid.is_empty()
                || !f("total").exists()
            {
                return None;
            }
            if !f("learned").exists() {
                let inputs = util::read(&f("inputs"));
                if !inputs.is_empty() {
                    let hits = unlinked_learned_in(vault, &inputs);
                    if !hits.is_empty() {
                        let _ = fs::write(f("learned"), "");
                        return Some(learned_block(&hits));
                    }
                }
            }
            if f("asked").exists() || f("vault").exists() {
                return None;
            }
            let n = |ext: &str| util::read(&f(ext)).len();
            let min: usize = std::env::var("KIOKU_STOP_MIN_TOOLS")
                .ok()
                .and_then(|x| x.parse().ok())
                .unwrap_or(10);
            if !(n("edits") >= 1 || n("total") >= min) {
                return None;
            }
            let _ = fs::write(f("asked"), "");
            Some(block(RECORD_REASON.into()))
        }
        _ => None,
    }
}

fn clean_old(dir: &Path, days: u64) {
    let limit = std::time::Duration::from_secs(days * 86400);
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let old = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|a| a > limit);
        if old && e.path().is_file() {
            let _ = fs::remove_file(e.path());
        }
    }
}

// ---------- Hermes ----------
// The Kioku plugin for Hermes (hermes-plugin/kioku/__init__.py) passes Hermes's hook payloads
// here: pre_llm_call → session-start (first turn, or after a compaction) or prompt; every
// transform_tool_result → tool; pre_verify → stop. Answers are plain JSON the plugin unwraps.

/// The `additionalContext` of a hook_json answer.
fn ctx_of(hook_json: Option<String>) -> Option<String> {
    let v: Value = serde_json::from_str(&hook_json?).ok()?;
    v.pointer("/hookSpecificOutput/additionalContext").and_then(Value::as_str).map(str::to_string)
}
/// The `reason` of a block answer.
fn reason_of(block_json: Option<String>) -> Option<String> {
    let v: Value = serde_json::from_str(&block_json?).ok()?;
    v.get("reason").and_then(Value::as_str).map(str::to_string)
}

fn hermes(vault: &Path, ev: &str, j: &Value) -> Option<String> {
    let st = util::state_dir().join("hermes");
    fs::create_dir_all(&st).ok()?;
    let sid: String = s(j, "session_id")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
        .collect();
    let f = |ext: &str| st.join(format!("{sid}.{ext}"));
    let append = |ext: &str, text: &str| {
        use std::io::Write;
        if let Ok(mut h) = fs::OpenOptions::new().create(true).append(true).open(f(ext)) {
            let _ = h.write_all(text.as_bytes());
        }
    };
    match ev {
        "session-start" => {
            if !vault.is_dir() {
                return None;
            }
            if !sid.is_empty() {
                append("total", "");
                clean_old(&st, 14);
            }
            let cwd = Some(s(j, "cwd")).filter(|c| !c.is_empty()).map(PathBuf::from).unwrap_or_else(|| vault.to_path_buf());
            let text = context::build(vault, &cwd, s(j, "source"), "hermes");
            Some(json!({ "context": text }).to_string())
        }
        "prompt" => ctx_of(prompt_pull(vault, "UserPromptSubmit")).map(|t| json!({ "context": t }).to_string()),
        "tool" => {
            if sid.is_empty() {
                return None;
            }
            append("total", ".");
            let tn = s(j, "tool_name");
            let args = j.get("args").cloned().unwrap_or(json!({}));
            let mut inputs = String::new();
            strings(&args, &mut inputs);
            append("inputs", &inputs);
            match tn {
                "terminal" => {
                    let cmd = args.get("command").and_then(Value::as_str).unwrap_or("");
                    static W: OnceLock<Regex> = OnceLock::new();
                    let w = W.get_or_init(|| Regex::new(r"commit|kioku (today|journal|new|log|capture)|>>|tee ").unwrap());
                    let v = vault.display().to_string();
                    let tilde = v.strip_prefix(&util::home().display().to_string()).map(|r| format!("~{r}")).unwrap_or(v.clone());
                    if (cmd.contains(&v) || cmd.contains(&tilde) || s(j, "cwd").starts_with(&v)) && w.is_match(cmd) {
                        append("vault", "");
                    }
                    None
                }
                "write_file" | "patch" => {
                    // The tool's result says where it wrote; else the path as given.
                    let res: Value = serde_json::from_str(s(j, "result")).unwrap_or(json!({}));
                    let mut paths: Vec<String> = res
                        .get("files_modified")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                        .unwrap_or_default();
                    if let Some(r) = res.get("resolved_path").and_then(Value::as_str) {
                        paths.push(r.to_string());
                    }
                    if paths.is_empty() {
                        if let Some(p) = args.get("path").and_then(Value::as_str) {
                            let p = match p.strip_prefix("~/") {
                                Some(r) => format!("{}/{r}", util::home().display()),
                                None => p.to_string(),
                            };
                            paths.push(if p.starts_with('/') { p } else { format!("{}/{p}", s(j, "cwd")) });
                        }
                    }
                    paths.dedup();
                    let mut files = Vec::new();
                    for p in &paths {
                        match in_vault(vault, p) {
                            Some(x) => files.push(x),
                            None => append("edits", "."),
                        }
                    }
                    if files.is_empty() {
                        return None;
                    }
                    append("vault", "");
                    if off("KIOKU_WRITE_CHECK") {
                        return None;
                    }
                    reason_of(write_check(vault, &files)).map(|m| json!({ "append": m }).to_string())
                }
                _ => None,
            }
        }
        "stop" => {
            if off("KIOKU_STOP_CHECK") || sid.is_empty() || !f("total").exists() {
                return None;
            }
            let msg = |b: String| reason_of(Some(b)).map(|m| json!({ "message": m }).to_string());
            if !f("learned").exists() {
                let inputs = util::read(&f("inputs"));
                if !inputs.is_empty() {
                    let hits = unlinked_learned_in(vault, &inputs);
                    if !hits.is_empty() {
                        let _ = fs::write(f("learned"), "");
                        return msg(learned_block(&hits));
                    }
                }
            }
            if f("asked").exists() || f("vault").exists() {
                return None;
            }
            let n = |ext: &str| util::read(&f(ext)).len();
            let min: usize = std::env::var("KIOKU_STOP_MIN_TOOLS").ok().and_then(|x| x.parse().ok()).unwrap_or(10);
            if !(n("edits") >= 1 || n("total") >= min) {
                return None;
            }
            let _ = fs::write(f("asked"), "");
            msg(block(RECORD_REASON.into()))
        }
        _ => None,
    }
}

// ---------- Antigravity ----------
// Turns Antigravity's input (camelCase, its own transcript) into what the shared pieces read,
// and their answers into Antigravity's output.

fn antigravity(vault: &Path, ev: &str, j: &Value, raw: &str) -> Option<String> {
    let noop = || Some(if ev == "stop" { json!({"decision": "allow"}) } else { json!({}) }.to_string());
    let st = util::state_dir();
    if fs::create_dir_all(&st).is_ok() {
        let _ = fs::write(st.join(format!("antigravity-{ev}.json")), format!("{raw}\n"));
    }
    let inject = |o: Option<String>| -> Option<String> {
        let c = o
            .and_then(|o| serde_json::from_str::<Value>(&o).ok())
            .and_then(|v| {
                v.pointer("/hookSpecificOutput/additionalContext")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .filter(|c| !c.is_empty());
        match c {
            Some(c) => Some(json!({"injectSteps": [{"systemMessage": {"systemMessage": c}}]}).to_string()),
            None => noop(),
        }
    };
    match ev {
        "session-start" => {
            let cwd = j
                .pointer("/workspacePaths/0")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().display().to_string());
            inject(session_start(vault, &json!({"cwd": cwd}), "SessionStart", "antigravity"))
        }
        "pull" => inject(prompt_pull(vault, "UserPromptSubmit")),
        "stop" => {
            // Only a normal end of a turn, not an error or the step limit, and only when idle.
            let reason = s(j, "terminationReason").to_lowercase();
            let bad = ["error", "max_steps", "exceeded", "cancel"].iter().any(|b| reason.contains(b));
            if bad || j.get("fullyIdle") == Some(&json!(false)) {
                return noop();
            }
            let sid = s(j, "conversationId");
            let tp = s(j, "transcriptPath");
            if sid.is_empty() || !Path::new(tp).is_file() {
                return noop();
            }
            let calls = antigravity_calls(Path::new(tp));
            match stop_check(vault, &format!("ag-{sid}"), &calls).and_then(|o| serde_json::from_str::<Value>(&o).ok()) {
                Some(o) if s(&o, "decision") == "block" => Some(json!({"decision": "continue", "reason": s(&o, "reason")}).to_string()),
                _ => noop(),
            }
        }
        _ => noop(),
    }
}

/// Antigravity transcript (one step per line, tool args as JSON strings) → tool calls. File
/// writes become Edit (file_path), commands become Bash (command); all args are kept.
fn antigravity_calls(tp: &Path) -> Vec<(String, Value)> {
    static W: OnceLock<Regex> = OnceLock::new();
    let w = W.get_or_init(|| Regex::new(r"(?i)write|edit|replace|create_file").unwrap());
    let mut out = Vec::new();
    for l in util::read(tp).lines() {
        let Ok(step) = serde_json::from_str::<Value>(l) else { continue };
        for c in step.get("tool_calls").and_then(Value::as_array).into_iter().flatten() {
            let mut a = serde_json::Map::new();
            if let Some(obj) = c.get("args").and_then(Value::as_object) {
                for (k, v) in obj {
                    let parsed = v
                        .as_str()
                        .and_then(|x| serde_json::from_str::<Value>(x).ok())
                        .unwrap_or_else(|| v.clone());
                    a.insert(k.clone(), parsed);
                }
            }
            let n = s(c, "name");
            let first = |keys: &[&str]| {
                keys.iter()
                    .find_map(|k| a.get(*k).and_then(Value::as_str))
                    .unwrap_or("")
                    .to_string()
            };
            if n == "run_command" {
                let cl = first(&["CommandLine"]);
                a.insert("command".into(), json!(cl));
                out.push(("Bash".to_string(), Value::Object(a)));
            } else if w.is_match(n) {
                let p = first(&["Path", "TargetFile", "AbsolutePath", "FilePath"]);
                a.insert("file_path".into(), json!(p));
                out.push(("Edit".to_string(), Value::Object(a)));
            } else {
                out.push((n.to_string(), Value::Object(a)));
            }
        }
    }
    out
}
