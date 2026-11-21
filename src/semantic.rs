//! Optional hybrid search: `kioku search` also ranks the vault's sections by meaning with the
//! multilingual-e5-small model and fuses that with BM25 (RRF, the vector side weighted ×2).
//! Measured on the user's own questions: MRR 0.58 → 0.72 on held-out ones (2026-11-20).
//!
//! Off twice by default: the release binaries are built without it (cargo feature `semantic`),
//! and a build with it still needs `kioku semantic on`. It also needs the ONNX Runtime library
//! (`KIOKU_ORT_LIB` or `ORT_DYLIB_PATH`; without one it stays off: ort would abort the process) and the model
//! (`kioku semantic pull`, ~135 MB into ~/.cache/kioku/models). Section vectors are cached in
//! ~/.cache/kioku/index and only changed sections are embedded again. Whenever anything is
//! missing, search quietly stays BM25.

use crate::util;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub const MODEL: &str = "multilingual-e5-small";
const REPO: &str = "Xenova/multilingual-e5-small";
const REV: &str = "761b726dd34fb83930e26aab4e9ac3899aa1fa78";
/// (file in the repo, local name, sha256)
const FILES: [(&str, &str, &str); 2] = [
    ("onnx/model_quantized.onnx", "model_quantized.onnx", "f80102d3f2a1229f387d3c81909990d8945513e347b0eab049f7de3c6f98c193"),
    ("tokenizer.json", "tokenizer.json", "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39"),
];
/// How much the vector ranking counts against BM25 in the fusion (chosen on the user's dev questions).
pub const WEIGHT: f64 = 2.0;
/// Results taken from each side into the fusion, and the RRF constant.
pub const DEPTH: usize = 50;
pub const RRF_K: f64 = 60.0;

fn switch_file() -> PathBuf {
    util::config_dir().join("semantic")
}
pub fn model_dir() -> PathBuf {
    util::cache_dir().join("models").join(MODEL)
}
/// Switched on with `kioku semantic on` (or KIOKU_SEMANTIC=1 / 0 for one command).
pub fn switched_on() -> bool {
    match std::env::var("KIOKU_SEMANTIC").ok().as_deref() {
        Some("1") | Some("on") => return true,
        Some("0") | Some("off") => return false,
        _ => {}
    }
    util::read(&switch_file()).trim() == "on"
}
pub const BUILT: bool = cfg!(feature = "semantic");
/// Whether the model is downloaded (kioku semantic pull, also run by the install step).
pub fn ready() -> bool {
    BUILT && model_ok()
}

/// Doc indices by meaning, best first, with each doc's best section: None when hybrid search
/// is off or unavailable. `sections`: (doc index, section index, text).
pub fn rank(vault: &Path, sections: &[(usize, usize, String)], query: &str) -> Option<Vec<(usize, usize)>> {
    if !BUILT || !switched_on() || query.trim().is_empty() {
        return None;
    }
    #[cfg(feature = "semantic")]
    {
        match imp::rank(vault, sections, query) {
            Ok(r) => return Some(r),
            Err(e) => {
                eprintln!("kioku: semantic search unavailable ({e:#}); using BM25 only. See: kioku semantic status");
                return None;
            }
        }
    }
    #[allow(unreachable_code)]
    {
        let _ = (vault, sections);
        None
    }
}

/// Reciprocal rank fusion of the BM25 order and the vector order (doc indices).
pub fn fuse(bm25: &[usize], vector: &[usize]) -> Vec<(usize, f64)> {
    let mut s: Vec<(usize, f64)> = Vec::new();
    let mut add = |d: usize, w: f64, r: usize| match s.iter_mut().find(|x| x.0 == d) {
        Some(x) => x.1 += w / (RRF_K + r as f64 + 1.0),
        None => s.push((d, w / (RRF_K + r as f64 + 1.0))),
    };
    for (r, d) in bm25.iter().take(DEPTH).enumerate() {
        add(*d, 1.0, r);
    }
    for (r, d) in vector.iter().take(DEPTH).enumerate() {
        add(*d, WEIGHT, r);
    }
    s.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    s
}

fn sha_file(p: &Path) -> String {
    use sha2::{Digest, Sha256};
    let Ok(b) = std::fs::read(p) else { return String::new() };
    Sha256::digest(&b).iter().map(|x| format!("{x:02x}")).collect()
}
fn model_ok() -> bool {
    FILES.iter().all(|(_, n, sha)| sha_file(&model_dir().join(n)) == *sha)
}
/// Where ONNX Runtime is loaded from (semantic-dynamic): KIOKU_ORT_LIB or ORT_DYLIB_PATH, else
/// the path baked in at build time (the Nix package). None with the library linked in.
fn ort_lib() -> Option<String> {
    if cfg!(not(feature = "semantic-dynamic")) {
        return None;
    }
    ["KIOKU_ORT_LIB", "ORT_DYLIB_PATH"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .or_else(|| option_env!("KIOKU_ORT_LIB_DEFAULT").map(str::to_string))
}

/// Download the pinned model files (checksums checked). `loud`: say what is happening.
pub fn pull(loud: bool) -> Result<()> {
    if !util::which("curl") {
        return util::die("curl is needed to download the model");
    }
    let dir = model_dir();
    std::fs::create_dir_all(&dir)?;
    for (f, n, sha) in FILES {
        let dst = dir.join(n);
        if sha_file(&dst) == sha {
            if loud {
                println!("{n}: already here");
            }
            continue;
        }
        let url = format!("https://huggingface.co/{REPO}/resolve/{REV}/{f}");
        let tmp = dir.join(format!("{n}.part"));
        if loud {
            println!("{n}: downloading {url}");
        }
        let st = std::process::Command::new("curl").args(["-fsSL", "--retry", "2", "-o"]).arg(&tmp).arg(&url).status()?;
        if !st.success() {
            let _ = std::fs::remove_file(&tmp);
            return util::die(format!("download failed: {url}"));
        }
        if sha_file(&tmp) != sha {
            let _ = std::fs::remove_file(&tmp);
            return util::die(format!("{n}: checksum mismatch, not kept"));
        }
        std::fs::rename(&tmp, &dst)?;
    }
    if loud {
        println!("Model ready in {}", dir.display());
    }
    Ok(())
}
/// The install step: a build with hybrid search gets its model, so lint-check and the review can
/// match by meaning; search itself stays off until `kioku semantic on`.
pub fn ensure_model() -> Option<String> {
    if !BUILT || model_ok() || std::env::var("KIOKU_NO_MODEL").is_ok() {
        return None;
    }
    Some(match pull(false) {
        Ok(()) => format!("hybrid search model downloaded ({MODEL}, ~135 MB); search stays BM25 until: kioku semantic on"),
        Err(e) => format!("hybrid search model not downloaded ({e:#}); later: kioku semantic pull"),
    })
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    let usage = "usage: kioku semantic status|on|off|pull|index";
    match args.first().map(String::as_str).unwrap_or("status") {
        "status" => {
            println!("Hybrid search (e5 + BM25): {}", if switched_on() { "on" } else { "off" });
            println!(
                "  this kioku: {}",
                if BUILT { "built with it" } else { "built without it (cargo build --release --features semantic-bundled)" }
            );
            println!(
                "  model {MODEL}: {}",
                if model_ok() { format!("ready ({})", model_dir().display()) } else { "not downloaded (kioku semantic pull)".into() }
            );
            println!(
                "  ONNX Runtime library: {}",
                if cfg!(feature = "semantic-dynamic") {
                    ort_lib().unwrap_or_else(|| "not set (KIOKU_ORT_LIB=/path/to/libonnxruntime.so); hybrid search stays off".into())
                } else if BUILT {
                    "linked in".into()
                } else {
                    "-".into()
                }
            );
            if !(BUILT && switched_on() && model_ok()) {
                println!("  search uses BM25 only.");
            }
            Ok(0)
        }
        "on" | "off" => {
            let on = args[0] == "on";
            if on && !BUILT {
                return util::die("this kioku was built without hybrid search: cargo build --release --features semantic-bundled");
            }
            util::write(&switch_file(), if on { "on\n" } else { "off\n" })?;
            println!("Hybrid search {}.", if on { "on" } else { "off" });
            if on && !model_ok() {
                println!("The model is not downloaded yet: kioku semantic pull");
            }
            Ok(0)
        }
        "pull" => {
            pull(true)?;
            Ok(0)
        }
        "index" => {
            if !BUILT {
                return util::die("this kioku was built without hybrid search");
            }
            let n = crate::search::index_all(vault)?;
            println!("{n} section(s) indexed");
            Ok(0)
        }
        _ => util::die(usage),
    }
}

#[cfg(feature = "semantic")]
mod imp {
    use super::*;
    use anyhow::{anyhow, Context};
    use ort::session::Session;
    use ort::value::Tensor;
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use tokenizers::{PaddingParams, Tokenizer, TruncationParams};

    pub struct Model {
        tok: Tokenizer,
        sess: Session,
        type_ids: bool,
    }

    pub fn load() -> Result<Model> {
        if !model_ok() {
            return Err(anyhow!("model not downloaded (kioku semantic pull)"));
        }
        // Loaded at run time: ort aborts the process when it cannot load the library, so check
        // it is there first.
        #[cfg(feature = "semantic-dynamic")]
        {
            let lib = ort_lib().ok_or_else(|| anyhow!("ONNX Runtime library not set (KIOKU_ORT_LIB)"))?;
            if !Path::new(&lib).is_file() {
                return Err(anyhow!("ONNX Runtime library not found: {lib}"));
            }
            ort::init_from(lib).commit().context("loading the ONNX Runtime library")?;
        }
        let mut tok = Tokenizer::from_file(model_dir().join("tokenizer.json")).map_err(|e| anyhow!("tokenizer: {e}"))?;
        tok.with_truncation(Some(TruncationParams { max_length: 512, ..Default::default() }))
            .map_err(|e| anyhow!("tokenizer: {e}"))?;
        tok.with_padding(Some(PaddingParams::default()));
        let sess = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(model_dir().join("model_quantized.onnx"))
            .context("loading the model")?;
        let type_ids = sess.inputs.iter().any(|i| i.name == "token_type_ids");
        Ok(Model { tok, sess, type_ids })
    }

    impl Model {
        /// Unit vectors, e5 style: "query: " / "passage: " prefix, mean pooling.
        pub fn embed(&mut self, texts: &[String], query: bool) -> Result<Vec<Vec<f32>>> {
            let pre = if query { "query: " } else { "passage: " };
            let mut out = Vec::new();
            for batch in texts.chunks(16) {
                let enc = self
                    .tok
                    .encode_batch(batch.iter().map(|t| format!("{pre}{t}")).collect::<Vec<_>>(), true)
                    .map_err(|e| anyhow!("tokenizer: {e}"))?;
                let (b, l) = (enc.len(), enc[0].get_ids().len());
                let ids: Vec<i64> = enc.iter().flat_map(|e| e.get_ids().iter().map(|x| *x as i64)).collect();
                let mask: Vec<i64> = enc.iter().flat_map(|e| e.get_attention_mask().iter().map(|x| *x as i64)).collect();
                let mut inputs = vec![
                    ("input_ids", Tensor::from_array(([b, l], ids))?.into_dyn()),
                    ("attention_mask", Tensor::from_array(([b, l], mask.clone()))?.into_dyn()),
                ];
                if self.type_ids {
                    inputs.push(("token_type_ids", Tensor::from_array(([b, l], vec![0i64; b * l]))?.into_dyn()));
                }
                let outputs = self.sess.run(inputs)?;
                let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
                let d = shape[2] as usize;
                for i in 0..b {
                    let mut v = vec![0f32; d];
                    let mut n = 0f32;
                    for t in 0..l {
                        if mask[i * l + t] == 1 {
                            n += 1.0;
                            let row = &data[(i * l + t) * d..(i * l + t + 1) * d];
                            for k in 0..d {
                                v[k] += row[k];
                            }
                        }
                    }
                    let norm = v.iter().map(|x| (x / n.max(1.0)).powi(2)).sum::<f32>().sqrt().max(1e-9);
                    out.push(v.iter().map(|x| x / n.max(1.0) / norm).collect());
                }
            }
            Ok(out)
        }
    }

    fn index_file(vault: &Path) -> PathBuf {
        let h: String = Sha256::digest(vault.display().to_string().as_bytes()).iter().take(8).map(|x| format!("{x:02x}")).collect();
        util::cache_dir().join("index").join(format!("{h}-{MODEL}.vec"))
    }
    fn key(text: &str) -> [u8; 32] {
        Sha256::digest(text.as_bytes()).into()
    }
    fn load_index(f: &Path) -> HashMap<[u8; 32], Vec<f32>> {
        let mut m = HashMap::new();
        let Ok(b) = std::fs::read(f) else { return m };
        if b.len() < 13 || &b[..9] != b"KIOKUVEC1" {
            return m;
        }
        let d = u32::from_le_bytes(b[9..13].try_into().unwrap()) as usize;
        let rec = 32 + 4 * d;
        for r in b[13..].chunks_exact(rec) {
            let k: [u8; 32] = r[..32].try_into().unwrap();
            let v = r[32..].chunks_exact(4).map(|x| f32::from_le_bytes(x.try_into().unwrap())).collect();
            m.insert(k, v);
        }
        m
    }
    fn save_index(f: &Path, m: &HashMap<[u8; 32], Vec<f32>>) -> Result<()> {
        let d = m.values().next().map(|v| v.len()).unwrap_or(0);
        let mut b = Vec::with_capacity(13 + m.len() * (32 + 4 * d));
        b.extend_from_slice(b"KIOKUVEC1");
        b.extend_from_slice(&(d as u32).to_le_bytes());
        for (k, v) in m {
            b.extend_from_slice(k);
            for x in v {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        if let Some(p) = f.parent() {
            std::fs::create_dir_all(p)?;
        }
        let tmp = f.with_extension("tmp");
        std::fs::write(&tmp, b)?;
        std::fs::rename(tmp, f)?;
        Ok(())
    }

    /// Vectors for the sections, embedding only those not in the cache; the cache keeps only
    /// the current sections. Returns the vectors in section order, and the model.
    pub fn vectors(vault: &Path, sections: &[(usize, usize, String)]) -> Result<(Vec<Vec<f32>>, Model)> {
        let f = index_file(vault);
        let mut m = load_index(&f);
        let keys: Vec<[u8; 32]> = sections.iter().map(|s| key(&s.2)).collect();
        let missing: Vec<usize> = (0..sections.len()).filter(|i| !m.contains_key(&keys[*i])).collect();
        let mut model = load()?;
        if !missing.is_empty() {
            let texts: Vec<String> = missing.iter().map(|i| sections[*i].2.clone()).collect();
            let vs = model.embed(&texts, false)?;
            for (i, v) in missing.iter().zip(vs) {
                m.insert(keys[*i], v);
            }
        }
        let keep: std::collections::HashSet<&[u8; 32]> = keys.iter().collect();
        let stale = m.len() > keep.len();
        if !missing.is_empty() || stale {
            m.retain(|k, _| keep.contains(k));
            save_index(&f, &m)?;
        }
        Ok((keys.iter().map(|k| m[k].clone()).collect(), model))
    }

    pub fn rank(vault: &Path, sections: &[(usize, usize, String)], query: &str) -> Result<Vec<(usize, usize)>> {
        let (vs, mut model) = vectors(vault, sections)?;
        let q = model.embed(&[query.to_string()], true)?.remove(0);
        let mut sims: Vec<(f32, usize, usize)> = vs
            .iter()
            .zip(sections)
            .map(|(v, s)| (v.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>(), s.0, s.1))
            .collect();
        sims.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut out: Vec<(usize, usize)> = Vec::new();
        for (_, d, p) in sims {
            if !out.iter().any(|o| o.0 == d) {
                out.push((d, p));
            }
        }
        Ok(out)
    }

    /// For each text, the section most like it: (doc index, cosine similarity).
    pub fn best(vault: &Path, sections: &[(usize, usize, String)], texts: &[String]) -> Result<Vec<(usize, f32)>> {
        let (vs, mut model) = vectors(vault, sections)?;
        let qs = model.embed(texts, true)?;
        Ok(qs
            .iter()
            .map(|q| {
                vs.iter()
                    .zip(sections)
                    .map(|(v, s)| (s.0, v.iter().zip(q).map(|(a, b)| a * b).sum::<f32>()))
                    .fold((0, f32::MIN), |b, x| if x.1 > b.1 { x } else { b })
            })
            .collect())
    }

    pub fn index(vault: &Path, sections: &[(usize, usize, String)]) -> Result<usize> {
        vectors(vault, sections).map(|_| sections.len())
    }
}

/// Vectors for short texts (compared with each other: both sides embedded as queries), not
/// cached. None without hybrid search in the build, or without the model or ONNX Runtime.
pub fn embed(texts: &[String]) -> Option<Vec<Vec<f32>>> {
    #[cfg(feature = "semantic")]
    {
        if !model_ok() || texts.is_empty() {
            return None;
        }
        return imp::load().and_then(|mut m| m.embed(texts, true)).map_err(|e| eprintln!("kioku: semantic match unavailable ({e:#})")).ok();
    }
    #[allow(unreachable_code)]
    {
        let _ = texts;
        None
    }
}

/// For each text, the most similar section by meaning: (doc index, cosine). None when this
/// build has no hybrid search or the model or ONNX Runtime is missing (whether switched on or not:
/// the caller asked for it explicitly).
pub fn best(vault: &Path, sections: &[(usize, usize, String)], texts: &[String]) -> Option<Vec<(usize, f32)>> {
    #[cfg(feature = "semantic")]
    {
        if !model_ok() {
            return None;
        }
        match imp::best(vault, sections, texts) {
            Ok(r) => return Some(r),
            Err(e) => {
                eprintln!("kioku: semantic match unavailable ({e:#})");
                return None;
            }
        }
    }
    #[allow(unreachable_code)]
    {
        let _ = (vault, sections, texts);
        None
    }
}

#[cfg(feature = "semantic")]
pub fn index(vault: &Path, sections: &[(usize, usize, String)]) -> Result<usize> {
    imp::index(vault, sections)
}
#[cfg(not(feature = "semantic"))]
pub fn index(_vault: &Path, _sections: &[(usize, usize, String)]) -> Result<usize> {
    util::die("this kioku was built without hybrid search")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fusion_weights_vector() {
        // doc 1 is first by BM25, doc 2 first by vector; the vector side counts double.
        let f = fuse(&[1, 2, 3], &[2, 3, 1]);
        assert_eq!(f[0].0, 2);
        assert_eq!(f.len(), 3);
    }
}
