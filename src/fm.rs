//! YAML frontmatter, the small subset Kioku pages use.

use std::collections::BTreeMap;

/// One field, as the bash `fm()` read it: the text after `key:` on the first line starting with
/// it inside the frontmatter (leading blanks removed). None when missing.
pub fn get(text: &str, key: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    let pre = format!("{key}:");
    for l in lines {
        if l == "---" {
            return None;
        }
        if let Some(v) = l.strip_prefix(&pre) {
            return Some(v.trim_start_matches([' ', '\t']).to_string());
        }
    }
    None
}
/// A project's folders: `path:` and `paths:` (a value or a list), one per machine it lives
/// on. Empty values are left out.
pub fn paths(text: &str) -> Vec<String> {
    let (data, _) = parse(text);
    let mut out = Vec::new();
    for k in ["path", "paths"] {
        match data.get(k) {
            Some(Val::Str(s)) if !s.trim().is_empty() => out.push(s.trim().to_string()),
            Some(Val::List(v)) => out.extend(v.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
            _ => {}
        }
    }
    out
}
/// A list field (`[a, b]`, `- a` lines, or one value), items trimmed, empty ones left out.
pub fn list(text: &str, key: &str) -> Vec<String> {
    match parse(text).0.get(key) {
        Some(Val::Str(s)) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        Some(Val::List(v)) => v.iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        None => vec![],
    }
}
/// The field, or "" when missing.
pub fn field(text: &str, key: &str) -> String {
    get(text, key).unwrap_or_default()
}
pub fn is_private(text: &str) -> bool {
    get(text, "private").as_deref() == Some("true")
}

/// The lines outside the frontmatter block (awk: NR==1&&---{f=1} f&&---{f=0} !f).
pub fn body_lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut inside = false;
    for (i, l) in text.lines().enumerate() {
        if i == 0 && l == "---" {
            inside = true;
            continue;
        }
        if inside {
            if l == "---" {
                inside = false;
            }
            continue;
        }
        out.push(l);
    }
    out
}

#[derive(Debug, Clone)]
pub enum Val {
    Str(String),
    List(Vec<String>),
}
impl Val {
    pub fn str(&self) -> String {
        match self {
            Val::Str(s) => s.clone(),
            Val::List(v) => v.join(","),
        }
    }
}

fn unq(s: &str) -> String {
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// The calendar engine's parser: `key: value`, `[a, b]` lists, `- item` lists, quotes stripped,
/// ` # comment` removed from unquoted values. Returns the fields and the body after the block.
pub fn parse(text: &str) -> (BTreeMap<String, Val>, String) {
    let mut data = BTreeMap::new();
    let norm = text.replace("\r\n", "\n");
    let Some(rest) = norm.strip_prefix("---\n") else {
        return (data, norm);
    };
    let Some(end) = rest.find("\n---").filter(|&e| {
        let after = &rest[e + 4..];
        after.is_empty() || after.starts_with('\n')
    }) else {
        return (data, norm.clone());
    };
    let block = &rest[..end];
    let mut body = &rest[end + 4..];
    if let Some(b) = body.strip_prefix('\n') {
        body = b;
    }
    let mut key: Option<String> = None;
    for line in block.split('\n') {
        let t = line.trim_start();
        if t.starts_with("- ") && line.starts_with(char::is_whitespace) {
            if let Some(k) = &key {
                let item = unq(t[2..].trim());
                match data.get_mut(k) {
                    Some(Val::List(v)) => v.push(item),
                    _ => {
                        data.insert(k.clone(), Val::List(vec![item]));
                    }
                }
                continue;
            }
        }
        let Some(colon) = line.find(':') else { continue };
        let k = &line[..colon];
        let valid = k.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid {
            continue;
        }
        key = Some(k.to_string());
        let mut v = line[colon + 1..].trim().to_string();
        if !v.starts_with('"') && !v.starts_with('\'') {
            if let Some(i) = find_comment(&v) {
                v.truncate(i);
                v = v.trim_end().to_string();
            }
        }
        if v.starts_with('[') && v.ends_with(']') {
            let items = v[1..v.len() - 1]
                .split(',')
                .map(|s| unq(s.trim()))
                .filter(|s| !s.is_empty())
                .collect();
            data.insert(k.to_string(), Val::List(items));
        } else {
            data.insert(k.to_string(), Val::Str(unq(&v)));
        }
    }
    (data, body.to_string())
}
/// Start of a ` # comment` (whitespace then #).
fn find_comment(v: &str) -> Option<usize> {
    let b = v.as_bytes();
    (1..b.len())
        .find(|&i| b[i] == b'#' && (b[i - 1] == b' ' || b[i - 1] == b'\t'))
        .map(|i| {
            let mut j = i;
            while j > 0 && (b[j - 1] == b' ' || b[j - 1] == b'\t') {
                j -= 1;
            }
            j
        })
}

pub fn s(data: &BTreeMap<String, Val>, k: &str) -> String {
    data.get(k).map(Val::str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_field() {
        let t = "---\ntitle: Hello\nprivate: true\n---\n# x\ntitle: no\n";
        assert_eq!(get(t, "title").as_deref(), Some("Hello"));
        assert!(is_private(t));
        assert_eq!(get(t, "lang"), None);
        assert_eq!(get("# no fm\ntitle: x", "title"), None);
    }
    #[test]
    fn parse_lists_and_quotes() {
        let t = "---\ntitle: \"A # b\"\ntags: [x, 'y']\nemails:\n  - a@b\n  - c@d\nrrule: FREQ=WEEKLY # c\n---\nbody\n";
        let (d, b) = parse(t);
        assert_eq!(s(&d, "title"), "A # b");
        assert_eq!(s(&d, "tags"), "x,y");
        assert_eq!(s(&d, "emails"), "a@b,c@d");
        assert_eq!(s(&d, "rrule"), "FREQ=WEEKLY");
        assert_eq!(b, "body\n");
    }
}
