//! Handoffs: `handoff:` lines in the daily log, written when a session stops so that another
//! session (or another machine) can carry on. They are shown in the session context, Home.md and
//! the deep context until someone picks them up (`resumed:`) or they are 14 days old.

use crate::{fm, util};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// How long a handoff is shown, in days.
pub const DAYS: i64 = 14;

pub struct Handoff {
    pub date: String,
    pub time: String,
    /// The project tag.
    pub tag: String,
    /// Who wrote it (agent@host), when the line says.
    pub by: String,
    pub text: String,
}

/// Open handoffs from the last `DAYS` days, newest first. Only a project's newest handoff
/// counts: once it is resumed, older ones for that project are not shown again.
pub fn open(vault: &Path) -> Vec<Handoff> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"^- ([0-9:]+) \[([^\]]*)\] (handoff|resumed): (.*)$").unwrap());
    let since = util::ymd(util::day(-DAYS));
    let mut out: Vec<Handoff> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for f in util::md_files(&vault.join("daily")).into_iter().rev() {
        let date = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if date < since {
            continue;
        }
        let t = util::read(&f);
        if fm::is_private(&t) {
            continue;
        }
        let mut day: Vec<(bool, Handoff)> = Vec::new();
        for l in t.lines() {
            let Some(c) = r.captures(l) else { continue };
            let (tag, by) = match c[2].split_once('·') {
                Some((t, b)) => (t.trim().to_string(), b.trim().to_string()),
                None => (c[2].trim().to_string(), String::new()),
            };
            day.push((&c[3] == "handoff", Handoff {
                date: date.clone(),
                time: c[1].to_string(),
                tag,
                by,
                text: c[4].to_string(),
            }));
        }
        day.sort_by(|a, b| b.1.time.cmp(&a.1.time));
        for (open, h) in day {
            if seen.contains(&h.tag) {
                continue;
            }
            seen.push(h.tag.clone());
            if open {
                out.push(h);
            }
        }
    }
    out
}

impl Handoff {
    /// "2026-11-20 14:20 [kioku · claude@nitro] text"
    pub fn line(&self, max: usize) -> String {
        let by = if self.by.is_empty() { String::new() } else { format!(" · {}", self.by) };
        format!("{} {} [{}{by}] {}", self.date, self.time, self.tag, util::take(&self.text, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newest_per_project() {
        let v = std::env::temp_dir().join(format!("kioku-handoff-{}", std::process::id()));
        let d = v.join("daily");
        std::fs::create_dir_all(&d).unwrap();
        let today = util::ymd(util::day(0));
        let old = util::ymd(util::day(-(DAYS + 1)));
        std::fs::write(
            d.join(format!("{today}.md")),
            "# t\n- 09:00 [a · x@h] handoff: old a\n- 10:00 [a · x@h] resumed: new a\n- 11:00 [b] handoff: b\n- 08:00 [c · y@h] handoff: c1\n- 12:00 [c · y@h] handoff: c2\n",
        )
        .unwrap();
        std::fs::write(d.join(format!("{old}.md")), "# o\n- 10:00 [d] handoff: too old\n").unwrap();
        let hs = open(&v);
        let got: Vec<&str> = hs.iter().map(|h| h.text.as_str()).collect();
        assert_eq!(got, ["c2", "b"]);
        assert_eq!(hs[0].line(100), format!("{today} 12:00 [c · y@h] c2"));
        std::fs::remove_dir_all(&v).unwrap();
    }
}
