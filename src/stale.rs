//! Freshness: wiki and project pages whose facts have not been checked for a while.
//!
//! A page's last check is `reviewed:` (set when its claims were confirmed), else `updated:`,
//! else `created:`. How often it should be checked: `review_every: N` (days) or `never`, else by
//! kind of page: active projects 60 days; entities that are tools, sites, apps, libraries,
//! services or servers 90, people and organizations 180, other kinds (anime, games …) never;
//! concepts and syntheses 180; sources, ideas, topic hubs and projects that are not active never.

use crate::{fm, util};
use chrono::NaiveDate;
use std::path::Path;

pub struct Due {
    pub rel: String,
    pub title: String,
    pub last: Option<NaiveDate>,
    pub every: i64,
    /// Days past the due date (negative: days left).
    pub over: i64,
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&util::take(s.trim(), 10), "%Y-%m-%d").ok()
}

/// The review interval in days, None for "never"; Err for an invalid `review_every`.
pub fn interval(rel: &str, text: &str) -> Result<Option<i64>, String> {
    let v = fm::field(text, "review_every");
    let v = v.trim();
    if !v.is_empty() {
        if v == "never" {
            return Ok(None);
        }
        let n = v.strip_suffix('d').unwrap_or(v);
        return n
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .map(Some)
            .ok_or_else(|| "review_every must be a number of days (like 60) or never".to_string());
    }
    Ok(if rel.starts_with("projects/") {
        (fm::field(text, "status") == "active").then_some(60)
    } else if rel.starts_with("wiki/entities/") {
        // Things that change get checked; works of fiction, games and the like do not.
        match fm::field(text, "kind").trim() {
            "tool" | "site" | "app" | "library" | "service" | "server" | "" => Some(90),
            "person" | "org" => Some(180),
            _ => None,
        }
    } else if rel.starts_with("wiki/concepts/") || rel.starts_with("wiki/synthesis/") {
        Some(180)
    } else {
        None
    })
}

/// Problems with the freshness fields of one page (for check-file).
pub fn check(rel: &str, text: &str) -> Vec<String> {
    let mut p = Vec::new();
    let r = fm::field(text, "reviewed");
    if !r.trim().is_empty() && date(&r).is_none() {
        p.push("reviewed must be a date (YYYY-MM-DD)".to_string());
    }
    if let Err(e) = interval(rel, text) {
        p.push(e);
    }
    p
}

/// Pages with an interval, most overdue first. `all`: also those not due yet.
pub fn pages(vault: &Path, all: bool) -> Vec<Due> {
    let today = util::today();
    let mut out = Vec::new();
    for dir in ["projects", "wiki"] {
        for f in util::md_files(&vault.join(dir)) {
            let rel = util::rel(vault, &f);
            let text = util::read(&f);
            let Ok(Some(every)) = interval(&rel, &text) else { continue };
            let last = ["reviewed", "updated", "created"].iter().find_map(|k| date(&fm::field(&text, k)));
            let over = match last {
                Some(d) => (today - d).num_days() - every,
                None => i64::MAX / 2,
            };
            if all || over >= 0 {
                let title = fm::field(&text, "title");
                let title = title.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
                out.push(Due {
                    rel,
                    title,
                    last,
                    every,
                    over,
                });
            }
        }
    }
    out.sort_by(|a, b| b.over.cmp(&a.over).then_with(|| a.rel.cmp(&b.rel)));
    out
}

pub fn line(d: &Due) -> String {
    let title = if d.title.is_empty() { "?" } else { &d.title };
    let when = match d.last {
        Some(l) => format!("last checked {}, {} days ago", util::ymd(l), (util::today() - l).num_days()),
        None => "never checked (no reviewed/updated/created date)".to_string(),
    };
    format!("- {} — {title} ({when}; every {} days)", d.rel, d.every)
}

/// `kioku stale [--all]`
pub fn run(vault: &Path, args: &[String]) -> i32 {
    let all = args.first().map(String::as_str) == Some("--all");
    let v = pages(vault, all);
    if v.is_empty() {
        println!("(no page is due for review)");
    }
    for d in &v {
        let mut l = line(d);
        if all && d.over < 0 {
            l.push_str(&format!(" — due in {} days", -d.over));
        }
        println!("{l}");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intervals() {
        let fmt = |extra: &str| format!("---\ntitle: X\n{extra}\n---\n");
        assert_eq!(interval("wiki/entities/x.md", &fmt("kind: tool")), Ok(Some(90)));
        assert_eq!(interval("wiki/entities/x.md", &fmt("kind: person")), Ok(Some(180)));
        assert_eq!(interval("wiki/entities/x.md", &fmt("kind: anime")), Ok(None));
        assert_eq!(interval("wiki/sources/x.md", &fmt("")), Ok(None));
        assert_eq!(interval("projects/x.md", &fmt("status: active")), Ok(Some(60)));
        assert_eq!(interval("projects/x.md", &fmt("status: done")), Ok(None));
        assert_eq!(interval("wiki/ideas/x.md", &fmt("review_every: 30")), Ok(Some(30)));
        assert_eq!(interval("wiki/entities/x.md", &fmt("review_every: never")), Ok(None));
        assert!(interval("wiki/entities/x.md", &fmt("review_every: often")).is_err());
        assert_eq!(check("wiki/entities/x.md", &fmt("reviewed: dün")).len(), 1);
    }
}
