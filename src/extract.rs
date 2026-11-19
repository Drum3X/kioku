//! `kioku extract <file|url> [-o FILE] [--lang tr,en]`: a source as a ready raw/ page
//! (frontmatter + the text as-is), for kinds an agent cannot read by itself:
//!
//!   YouTube and other video links   subtitles through yt-dlp (the video's own language first,
//!                                   then --lang); without subtitles, the audio is transcribed
//!   audio and video files           transcribed ($KIOKU_TRANSCRIBE, else the whisper CLI)
//!   .docx .odt .rtf .epub .html     pandoc (else LibreOffice for .docx/.odt/.rtf)
//!   .pdf                            pdftotext
//!   .md .txt                        as they are
//!
//! Web pages are not handled here: the agent's own web-fetch tool reads them better.
//! $KIOKU_TRANSCRIBE is a command that gets the audio file as its last argument and prints the
//! text, e.g. a script around faster-whisper.

use crate::util::{self, die, which};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Page {
    title: String,
    url: String,
    author: String,
    published: String,
    lang: String,
    ty: &'static str,
    method: String,
    text: String,
}

fn need(tool: &str, what: &str, hint: &str) -> Result<()> {
    if which(tool) {
        return Ok(());
    }
    die(format!("{what} needs {tool}, which was not found. {hint}"))
}
fn out_of(cmd: &mut Command, what: &str) -> Result<String> {
    let o = cmd.output().with_context(|| format!("running {what}"))?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        return die(format!("{what} failed: {}", err.trim().lines().last().unwrap_or("")));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// WebVTT subtitles → plain text: cue timings, settings and tags dropped, the rolling repeats
/// of automatic captions removed, sentences joined into paragraphs.
pub fn vtt_text(vtt: &str) -> String {
    let tag = regex::Regex::new(r"<[^>]+>").unwrap();
    let mut lines: Vec<String> = Vec::new();
    for l in vtt.lines() {
        let l = l.trim();
        if l.is_empty()
            || l == "WEBVTT"
            || l.contains("-->")
            || l.starts_with("Kind:")
            || l.starts_with("Language:")
            || l.starts_with("NOTE")
        {
            continue;
        }
        if l.chars().all(|c| c.is_ascii_digit()) {
            continue; // cue number
        }
        let t = tag
            .replace_all(l, "")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&gt;", ">")
            .replace("&lt;", "<");
        let t = t.trim().to_string();
        if t.is_empty() || lines.last() == Some(&t) {
            continue;
        }
        lines.push(t);
    }
    // Paragraphs of about 8 lines, broken after a sentence end when possible.
    let mut out = String::new();
    let mut n = 0;
    for l in lines {
        out.push_str(&l);
        n += 1;
        let end = l.ends_with(['.', '?', '!', '…']);
        if (n >= 8 && end) || n >= 14 {
            out.push_str("\n\n");
            n = 0;
        } else {
            out.push(' ');
        }
    }
    out.trim().to_string()
}

fn transcribe(file: &Path, tmp: &Path) -> Result<(String, String)> {
    if let Ok(cmd) = std::env::var("KIOKU_TRANSCRIBE") {
        if !cmd.trim().is_empty() {
            let mut parts = cmd.split_whitespace();
            let prog = parts.next().unwrap();
            let text = out_of(Command::new(prog).args(parts).arg(file), "KIOKU_TRANSCRIBE")?;
            return Ok((
                text.trim().to_string(),
                format!("transcribed ({})", prog.rsplit('/').next().unwrap_or(prog)),
            ));
        }
    }
    need(
        "whisper",
        "transcribing audio",
        "Install openai-whisper (pip install openai-whisper; it needs ffmpeg), or set KIOKU_TRANSCRIBE to a command that prints the text of the audio file given as its last argument.",
    )?;
    out_of(
        Command::new("whisper")
            .arg(file)
            .args(["--output_format", "txt", "--output_dir"])
            .arg(tmp)
            .args(["--verbose", "False"]),
        "whisper",
    )?;
    let txt = tmp.join(format!("{}.txt", file.file_stem().unwrap_or_default().to_string_lossy()));
    Ok((util::read(&txt).trim().to_string(), "transcribed (whisper)".into()))
}

fn is_video_url(u: &str) -> bool {
    ["youtube.com/", "youtu.be/", "vimeo.com/", "dailymotion.com/", "twitch.tv/videos/"]
        .iter()
        .any(|h| u.contains(h))
}

fn video(url: &str, langs: &[String], tmp: &Path) -> Result<Page> {
    need(
        "yt-dlp",
        "a video link",
        "Install yt-dlp (for example: nix profile install nixpkgs#yt-dlp, apt install yt-dlp, or pip install yt-dlp).",
    )?;
    let meta: Value = serde_json::from_str(&out_of(
        Command::new("yt-dlp").args(["-J", "--skip-download", "--no-warnings", url]),
        "yt-dlp",
    )?)?;
    let s = |k: &str| meta.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let date = s("upload_date");
    let published = if date.len() == 8 {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..])
    } else {
        String::new()
    };
    let mut page = Page {
        title: s("title"),
        url: Some(s("webpage_url")).filter(|u| !u.is_empty()).unwrap_or_else(|| url.to_string()),
        author: Some(s("uploader")).filter(|u| !u.is_empty()).unwrap_or_else(|| s("channel")),
        published,
        lang: s("language"),
        ty: "video",
        method: String::new(),
        text: String::new(),
    };
    // Subtitle languages: the video's own language, then --lang; written ones before automatic.
    let has = |kind: &str, l: &str| meta.get(kind).and_then(|m| m.get(l)).is_some();
    let mut want: Vec<String> = Vec::new();
    if !page.lang.is_empty() {
        want.push(page.lang.clone());
    }
    want.extend(langs.iter().cloned());
    let pick = want
        .iter()
        .find_map(|l| has("subtitles", l).then(|| (l.clone(), "subtitles")))
        .or_else(|| {
            want.iter()
                .find_map(|l| has("automatic_captions", l).then(|| (l.clone(), "automatic captions")))
        })
        .or_else(|| {
            want.iter().find_map(|l| {
                // automatic captions in the original language are named like "tr-orig"
                let orig = format!("{l}-orig");
                has("automatic_captions", &orig).then_some((orig, "automatic captions"))
            })
        });
    if let Some((l, kind)) = pick {
        let sub = if kind == "subtitles" { "--write-subs" } else { "--write-auto-subs" };
        out_of(
            Command::new("yt-dlp")
                .args([
                    "--skip-download",
                    sub,
                    "--sub-langs",
                    &l,
                    "--sub-format",
                    "vtt",
                    "--no-warnings",
                    "-o",
                ])
                .arg(tmp.join("sub.%(ext)s"))
                .arg(url),
            "yt-dlp subtitles",
        )?;
        let f = std::fs::read_dir(tmp)?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.to_string_lossy().ends_with(".vtt"));
        if let Some(f) = f {
            page.text = vtt_text(&util::read(&f));
            if page.lang.is_empty() {
                page.lang = l.trim_end_matches("-orig").to_string();
            }
            page.method = format!("{kind} ({l}) via yt-dlp");
            return Ok(page);
        }
    }
    // No subtitles: the audio, transcribed.
    need("ffmpeg", "taking the audio of a video without subtitles", "Install ffmpeg.")?;
    out_of(
        Command::new("yt-dlp")
            .args(["-x", "--audio-format", "m4a", "--no-warnings", "-o"])
            .arg(tmp.join("audio.%(ext)s"))
            .arg(url),
        "yt-dlp audio",
    )?;
    let (text, how) = transcribe(&tmp.join("audio.m4a"), tmp)?;
    page.text = text;
    page.method = format!("no subtitles; audio {how}");
    Ok(page)
}

fn file(path: &Path, tmp: &Path) -> Result<Page> {
    if !path.is_file() {
        return die(format!("no such file: {}", path.display()));
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let mut page = Page {
        title: stem.replace(['_', '-'], " "),
        url: String::new(),
        author: String::new(),
        published: String::new(),
        lang: String::new(),
        ty: "doc",
        method: String::new(),
        text: String::new(),
    };
    match ext.as_str() {
        "md" | "markdown" | "txt" => {
            page.text = util::read(path).trim().to_string();
            page.method = "as is".into();
        }
        "pdf" => {
            need(
                "pdftotext",
                "a PDF",
                "Install poppler-utils (pdftotext), or let the agent read the PDF with its file-reading tool.",
            )?;
            page.text = out_of(
                Command::new("pdftotext").args(["-layout", "-enc", "UTF-8"]).arg(path).arg("-"),
                "pdftotext",
            )?
            .trim()
            .to_string();
            page.method = "pdftotext".into();
        }
        "docx" | "odt" | "rtf" | "epub" | "html" | "htm" | "doc" => {
            if which("pandoc") && ext != "doc" {
                page.text = out_of(Command::new("pandoc").arg(path).args(["-t", "gfm", "--wrap=none"]), "pandoc")?
                    .trim()
                    .to_string();
                // Title and author from the document's own metadata, when it has them.
                let tpl = tmp.join("meta.tpl");
                std::fs::write(&tpl, "$title$\n$for(author)$$author$$sep$, $endfor$\n")?;
                if let Ok(m) = out_of(
                    Command::new("pandoc").arg(path).args(["-t", "plain", "--template"]).arg(&tpl),
                    "pandoc",
                ) {
                    let mut it = m.lines();
                    if let Some(t) = it.next().map(str::trim).filter(|t| !t.is_empty()) {
                        page.title = t.to_string();
                    }
                    if let Some(a) = it.next().map(str::trim).filter(|a| !a.is_empty()) {
                        page.author = a.to_string();
                    }
                }
                page.method = "pandoc".into();
            } else {
                let lo = if which("soffice") { "soffice" } else { "libreoffice" };
                need(lo, "this document", "Install pandoc (or LibreOffice).")?;
                out_of(
                    Command::new(lo)
                        .args(["--headless", "--convert-to", "txt:Text", "--outdir"])
                        .arg(tmp)
                        .arg(path),
                    "LibreOffice",
                )?;
                page.text = util::read(&tmp.join(format!("{stem}.txt"))).trim().to_string();
                page.method = "LibreOffice".into();
            }
            if ext == "epub" {
                page.ty = "book";
            }
        }
        "mp3" | "m4a" | "wav" | "ogg" | "oga" | "opus" | "flac" | "aac" | "mp4" | "mkv" | "webm" | "mov" => {
            let (text, how) = transcribe(path, tmp)?;
            page.text = text;
            page.method = how;
            page.ty = if ["mp4", "mkv", "webm", "mov"].contains(&ext.as_str()) {
                "video"
            } else {
                "audio"
            };
        }
        _ => {
            return die(format!(
                "kioku extract does not know .{ext} files (videos, audio, docx, odt, rtf, epub, html, pdf, md, txt)"
            ))
        }
    }
    Ok(page)
}

fn render(p: &Page) -> String {
    let q = |s: &str| {
        if s.contains([':', '#', '"']) || s.starts_with(['[', '{', '\'']) {
            serde_json::to_string(s).unwrap()
        } else {
            s.to_string()
        }
    };
    let mut o = String::from("---\n");
    o.push_str(&format!("title: {}\n", q(&p.title)));
    o.push_str(&format!("url: {}\n", p.url));
    o.push_str(&format!("author: {}\n", q(&p.author)));
    o.push_str(&format!("published: {}\n", p.published));
    o.push_str(&format!("fetched: {}\n", util::ymd(util::today())));
    o.push_str(&format!("lang: {}\n", p.lang));
    o.push_str(&format!("type: {}\n", p.ty));
    o.push_str(&format!("extracted: {}\n", q(&p.method)));
    o.push_str("---\n\n");
    o.push_str(&p.text);
    o.push('\n');
    o
}

pub fn run(args: &[String]) -> Result<i32> {
    let usage = "usage: kioku extract <file|video url> [-o FILE] [--lang tr,en]";
    let (mut src, mut outf, mut langs) = (None::<String>, None::<PathBuf>, vec!["tr".to_string(), "en".to_string()]);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                outf = Some(PathBuf::from(args.get(i + 1).ok_or_else(|| anyhow::anyhow!("{usage}"))?));
                i += 1;
            }
            "--lang" => {
                langs = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("{usage}"))?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                i += 1;
            }
            "-h" | "--help" => {
                println!("{usage}\n\nA video link, an audio/video file, a .docx/.odt/.rtf/.epub/.html document or a PDF as a raw/ page\n(frontmatter + the text). -o writes it to FILE (never over an existing one). Tools: yt-dlp,\nffmpeg, pandoc, pdftotext; audio needs whisper or KIOKU_TRANSCRIBE (a command printing the text\nof the audio file given as its last argument).");
                return Ok(0);
            }
            a if src.is_none() => src = Some(a.to_string()),
            _ => return die(usage),
        }
        i += 1;
    }
    let src = src.ok_or_else(|| anyhow::anyhow!("{usage}"))?;
    let tmp = std::env::temp_dir().join(format!("kioku-extract-{}", std::process::id()));
    std::fs::create_dir_all(&tmp)?;
    let res = if src.starts_with("http://") || src.starts_with("https://") {
        if is_video_url(&src) {
            video(&src, &langs, &tmp)
        } else {
            die("for web pages use your web-fetch tool; kioku extract reads video links, documents, PDFs and audio")
        }
    } else {
        file(Path::new(&src), &tmp)
    };
    let _ = std::fs::remove_dir_all(&tmp);
    let page = res?;
    if page.text.trim().is_empty() {
        return die("no text came out (empty subtitles or an image-only document?)");
    }
    let text = render(&page);
    match outf {
        Some(f) => {
            if f.exists() {
                return die(format!("{} exists; raw/ files are never overwritten", f.display()));
            }
            util::write(&f, &text)?;
            println!("{} ({} characters, {})", f.display(), page.text.chars().count(), page.method);
        }
        None => print!("{text}"),
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vtt() {
        let v = "WEBVTT\nKind: captions\nLanguage: en\n\n00:00:00.000 --> 00:00:02.000 align:start\nHello <c>world</c>\n\n00:00:02.000 --> 00:00:04.000\nHello world\nthis is a test.\n\n1\n00:00:04.000 --> 00:00:05.000\n&amp; more\n";
        assert_eq!(vtt_text(v), "Hello world this is a test. & more");
    }
}
