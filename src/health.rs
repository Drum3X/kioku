//! `kioku health [--offline]`: one look at everything Kioku depends on; says what to do about
//! each problem. --offline skips the network check (git fetch).

use crate::util::{self, die, home};
use crate::{lint, version};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

struct R {
    out: String,
    fails: usize,
    warns: usize,
}
impl R {
    fn ok(&mut self, s: &str) {
        self.out.push_str(&format!("  ok    {s}\n"));
    }
    fn bad(&mut self, s: &str) {
        self.out.push_str(&format!("  FAIL  {s}\n"));
        self.fails += 1;
    }
    fn wrn(&mut self, s: &str) {
        self.out.push_str(&format!("  warn  {s}\n"));
        self.warns += 1;
    }
    fn head(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }
    fn check(&mut self, cond: bool, ok: &str, bad: &str) {
        if cond {
            self.ok(ok)
        } else {
            self.bad(bad)
        }
    }
}

fn has_block(f: &Path) -> bool {
    util::read(f).contains("kioku:start")
}
fn skills_missing(sys: &Path, dir: &Path) -> Vec<String> {
    let mut m: Vec<String> = std::fs::read_dir(sys.join("skills"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !dir.join(n).join("SKILL.md").exists())
        .collect();
    m.sort();
    m
}
fn hook_cmds(f: &Path) -> Vec<String> {
    let j: Value = serde_json::from_str(&util::read(f)).unwrap_or(Value::Null);
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                if let Some(c) = m.get("command").and_then(Value::as_str) {
                    out.push(c.to_string());
                }
                m.values().for_each(|x| walk(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(&j, &mut out);
    out
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    let offline = match args.first().map(String::as_str) {
        Some("--offline") => true,
        None => false,
        _ => return die("usage: kioku health [--offline]"),
    };
    let sys = util::sys_dir();
    let fp = version::fingerprint(&sys);
    let mut r = R {
        out: String::new(),
        fails: 0,
        warns: 0,
    };
    r.head(&format!(
        "Kioku {} ({}), vault {}",
        version::VERSION,
        sys.display(),
        vault.display()
    ));

    r.head("Tools");
    if util::which("git") {
        r.ok("git");
    } else {
        r.bad("git not found (required)");
    }
    // Optional: what kioku extract uses (not counted as problems or warnings).
    let tools = [
        ("yt-dlp", "video links"),
        ("pandoc", "docx/odt/epub"),
        ("pdftotext", "PDF"),
        ("ffmpeg", "audio of videos"),
    ];
    let have: Vec<&str> = tools.iter().filter(|(t, _)| util::which(t)).map(|(t, _)| *t).collect();
    let mut miss: Vec<String> = tools
        .iter()
        .filter(|(t, _)| !util::which(t))
        .map(|(t, w)| format!("{t} ({w})"))
        .collect();
    if std::env::var("KIOKU_TRANSCRIBE").is_ok_and(|v| !v.trim().is_empty()) || util::which("whisper") {
    } else {
        miss.push("whisper or KIOKU_TRANSCRIBE (audio)".into());
    }
    let mut info = String::from("kioku extract:");
    if !have.is_empty() {
        info.push_str(&format!(" has {}", have.join(", ")));
    }
    if !miss.is_empty() {
        info.push_str(&format!("{} missing {}", if have.is_empty() { "" } else { ";" }, miss.join(", ")));
    }
    r.head(&format!("  info  {info} (optional)"));

    r.head("Vault");
    let link = vault.join(".kioku");
    if link.join("SCHEMA.md").is_file() {
        let t = std::fs::read_link(&link)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| link.display().to_string());
        r.ok(&format!(".kioku → {t}"));
    } else {
        r.bad(&format!("{} is missing or broken; run the installer again", link.display()));
    }
    let am = vault.join("AGENTS.md");
    if !am.is_file() {
        r.bad(&format!(
            "{} is missing (the rules every agent reads); run: kioku update",
            am.display()
        ));
    } else if util::read(&am).contains(".kioku/SCHEMA.md") {
        r.ok("AGENTS.md points to the schema");
    } else {
        r.bad(&format!(
            "{} does not mention .kioku/SCHEMA.md; add a line telling the agent to read it",
            am.display()
        ));
    }
    let core = vault.join("Core.md");
    if !core.is_file() {
        r.bad("Core.md is missing");
    } else {
        let t = util::read(&core);
        if t.contains("kioku:empty") || t.contains("brain:empty") {
            r.wrn("Core.md is still empty; say \"get to know me\" in a session");
        } else {
            r.ok("Core.md is filled in");
        }
    }
    if let Some(p) = crate::update::state("pinned") {
        r.wrn(&format!(
            "Kioku is pinned to {p} (kioku update --to/--rollback); kioku update returns to the latest"
        ));
    }
    let rec = util::record_file();
    if !rec.is_file() {
        r.wrn(&format!("no install record ({}); run: kioku update", rec.display()));
    } else if util::record_get("fingerprint").as_deref() == Some(fp.as_str()) {
        r.ok("install step up to date");
    } else {
        r.wrn("Kioku changed since its install step ran; run: kioku update (Nix module: rebuild)");
    }

    // Each agent is checked only where it is set up (the install record's agents= line).
    let agents = format!(" {} ", util::record_get("agents").unwrap_or_default());
    let k = format!("{}/.kioku/bin/kioku", vault.display());
    if agents.contains(" claude ") {
        r.head("Claude Code");
        let cm = util::read(&vault.join("CLAUDE.md"));
        r.check(
            cm.lines().any(|l| l == "@AGENTS.md") && cm.lines().any(|l| l == "@.kioku/SCHEMA.md"),
            "vault CLAUDE.md imports AGENTS.md and the schema",
            &format!(
                "{}/CLAUDE.md should hold the two lines @AGENTS.md and @.kioku/SCHEMA.md; run: kioku update",
                vault.display()
            ),
        );
        let s = home().join(".claude/settings.json");
        if s.is_file() {
            let j: Value = serde_json::from_str(&util::read(&s)).unwrap_or(Value::Null);
            for (ev, name) in [
                ("SessionStart", "session-start"),
                ("UserPromptSubmit", "prompt"),
                ("PostToolUse", "post-write"),
                ("PreCompact", "pre-compact"),
                ("Stop", "stop"),
            ] {
                let want = format!("{k} hook claude {name}");
                let has = j
                    .pointer(&format!("/hooks/{ev}"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .any(|g| {
                        g.get("hooks")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .any(|h| h.get("command").and_then(Value::as_str) == Some(&want))
                    });
                r.check(has, &format!("{ev} hook"), &format!("{ev} hook is not wired; run: kioku update"));
            }
        } else {
            r.bad(&format!("{} not found; run the installer", s.display()));
        }
        let m = skills_missing(&sys, &home().join(".claude/skills"));
        r.check(
            m.is_empty(),
            "skills linked",
            &format!("skills not linked: {}; run: kioku update", m.join(" ")),
        );
        r.check(
            has_block(&home().join(".claude/CLAUDE.md")),
            "~/.claude/CLAUDE.md has the Kioku block",
            "~/.claude/CLAUDE.md has no Kioku block; run: kioku update",
        );
    }

    let cx: PathBuf = std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".codex"));
    if agents.contains(" codex ") {
        r.head("Codex");
        let am = cx.join("AGENTS.md");
        r.check(
            has_block(&am),
            &format!("{} has the Kioku block", am.display()),
            &format!("{} has no Kioku block; run: kioku update", am.display()),
        );
        let m = skills_missing(&sys, &home().join(".agents/skills"));
        r.check(
            m.is_empty(),
            "skills linked (~/.agents/skills)",
            &format!("skills not linked: {}; run: kioku update", m.join(" ")),
        );
        let n = hook_cmds(&cx.join("hooks.json"))
            .iter()
            .filter(|c| c.starts_with(&format!("{k} hook codex ")))
            .count();
        r.check(
            n >= 4,
            "hooks in hooks.json (trust them once with /hooks in Codex)",
            &format!("Kioku hooks missing from {}/hooks.json; run: kioku update", cx.display()),
        );
        if util::read(&cx.join("config.toml")).contains(&format!("\"{}\"", vault.display())) {
            r.ok("the vault is a writable root of Codex's sandbox");
        } else {
            r.wrn(&format!(
                "the vault is not in writable_roots of {}/config.toml; Codex can only write to it when started inside it",
                cx.display()
            ));
        }
    }

    if agents.contains(" antigravity ") {
        r.head("Antigravity");
        let gem = home().join(".gemini");
        r.check(
            has_block(&gem.join("GEMINI.md")),
            "GEMINI.md has the Kioku block",
            "~/.gemini/GEMINI.md has no Kioku block; run: kioku update",
        );
        let m = skills_missing(&sys, &gem.join("config/skills"));
        r.check(
            m.is_empty(),
            "skills linked",
            &format!("skills not linked: {}; run: kioku update", m.join(" ")),
        );
        let j: Value = serde_json::from_str(&util::read(&gem.join("config/hooks.json"))).unwrap_or(Value::Null);
        r.check(
            ["kioku-session-start", "kioku-pull", "kioku-stop"]
                .iter()
                .all(|k| j.get(*k).is_some()),
            "hooks in hooks.json",
            "Kioku hooks missing from ~/.gemini/config/hooks.json; run: kioku update",
        );
    }

    if agents.contains(" hermes ") {
        r.head("Hermes");
        r.check(
            has_block(&vault.join(".hermes.md")),
            ".hermes.md has the Kioku block",
            &format!("{}/.hermes.md has no Kioku block; run: kioku update", vault.display()),
        );
    }
    if agents.trim().is_empty() {
        r.wrn("no agent is set up (install record has no agents= line); run: kioku update");
    }

    r.head("Git");
    let g = |a: &[&str]| util::git_out(vault, a);
    if !vault.join(".git").is_dir() {
        r.bad(&format!("the vault is not a git repository (git -C {} init)", vault.display()));
    } else {
        match g(&["config", "user.email"]).filter(|e| !e.is_empty()) {
            Some(e) => r.ok(&format!("commits as {} <{e}>", g(&["config", "user.name"]).unwrap_or_default())),
            None => r.bad("git user.email is not set; commits will fail"),
        }
        let pc = util::read(&vault.join(".git/hooks/post-commit"));
        r.check(
            pc.lines().any(|l| l.starts_with("# kioku:") || l.starts_with("# second-brain:")),
            "post-commit hook",
            "post-commit hook missing; run: kioku remote setup",
        );
        let pre = util::read(&vault.join(".git/hooks/pre-commit"));
        if pre.contains("kioku sync") || pre.contains("sync --ids") {
            r.wrn("an old pre-commit hook (CalDAV ids) is left; run: kioku remote setup");
        }
        if vault.join(".git/rebase-merge").is_dir() || vault.join(".git/rebase-apply").is_dir() {
            r.bad("a rebase is in progress in the vault; finish or abort it");
        }
        let dirty = g(&["status", "--porcelain"]).unwrap_or_default().lines().count();
        if dirty == 0 {
            r.ok("working tree clean");
        } else {
            r.wrn(&format!("{dirty} uncommitted change(s) (git -C {} status)", vault.display()));
        }
        if util::autosync(vault) {
            if g(&["rev-parse", "-q", "--verify", "@{u}"]).is_none() {
                r.bad("autosync is on but the branch has no upstream");
            } else {
                if !offline && !util::run(util::git(vault).args(["fetch", "-q"]), 20).is_ok_and(|o| o.status.success()) {
                    r.wrn("git fetch failed (offline? key?)");
                }
                let ab = g(&["rev-list", "--left-right", "--count", "HEAD...@{u}"]).unwrap_or_default();
                let mut it = ab.split_whitespace();
                let (a, b) = (it.next().unwrap_or("0"), it.next().unwrap_or("0"));
                r.ok(&format!(
                    "autosync on ({}; {a} ahead, {b} behind)",
                    g(&["rev-parse", "--abbrev-ref", "@{u}"]).unwrap_or_default()
                ));
            }
            let cf = util::state_dir().join("git-sync.conflict");
            if cf.exists() {
                r.bad(&format!("sync conflict: {}", util::read(&cf).trim_end()));
            }
        } else {
            r.ok("autosync off (one machine; kioku remote on to keep several in step)");
        }
    }

    r.head("Vault health");
    let (_, n) = lint::lint_check(vault);
    if n == 0 {
        r.ok("lint-check: no issues");
    } else {
        r.wrn(&format!(
            "lint-check: {n} issue(s); run: kioku lint-check, or ask your agent to \"check the brain\""
        ));
    }
    r.head("");
    r.head(&format!("{} problem(s), {} warning(s)", r.fails, r.warns));
    print!("{}", r.out);
    Ok(if r.fails == 0 { 0 } else { 1 })
}
