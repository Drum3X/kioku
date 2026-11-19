//! Text helpers shared by search, slugs and checks.

use regex::Regex;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

/// Case and Turkish folding: İ/I/ı → i, lowercase, ç ş ğ ö ü â î û → c s g o u a i u.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'İ' | 'I' | 'ı' => out.push('i'),
            _ => {
                for c in ch.to_lowercase() {
                    out.push(match c {
                        'ç' => 'c',
                        'ş' => 's',
                        'ğ' => 'g',
                        'ö' => 'o',
                        'ü' => 'u',
                        'â' => 'a',
                        'î' => 'i',
                        'û' => 'u',
                        c => c,
                    });
                }
            }
        }
    }
    out
}

pub fn word_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[\p{L}\p{N}]+").unwrap())
}
/// Folded words.
pub fn words(s: &str) -> Vec<String> {
    let f = fold(s);
    word_re().find_iter(&f).map(|m| m.as_str().to_string()).collect()
}

/// File name from a title: Latin letters → ASCII (ç→c, é→e, ı→i, ß→ss…), other scripts kept,
/// everything else → "-". At most 80 characters.
pub fn slugify(title: &str) -> String {
    static LATIN_MARKS: OnceLock<Regex> = OnceLock::new();
    static NON_WORD: OnceLock<Regex> = OnceLock::new();
    let lm = LATIN_MARKS.get_or_init(|| Regex::new(r"(\p{Latin})\p{M}+").unwrap());
    let nw = NON_WORD.get_or_init(|| Regex::new(r"[^\p{L}\p{N}\p{M}]+").unwrap());
    let nfkd: String = title.nfkd().collect();
    let s = lm.replace_all(&nfkd, "$1");
    let mut t = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'ı' | 'İ' => t.push('i'),
            'ß' => t.push_str("ss"),
            'æ' | 'Æ' => t.push_str("ae"),
            'œ' | 'Œ' => t.push_str("oe"),
            'ø' | 'Ø' => t.push('o'),
            'đ' | 'Đ' => t.push('d'),
            'ł' | 'Ł' => t.push('l'),
            'þ' | 'Þ' => t.push_str("th"),
            c => t.push(c),
        }
    }
    let t: String = t.nfc().collect::<String>().to_lowercase();
    let t = nw.replace_all(&t, "-");
    let t = t.trim_matches('-');
    let t: String = t.chars().take(80).collect();
    t.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folding() {
        assert_eq!(fold("İSTANBUL Işık açılış"), "istanbul isik acilis");
    }
    #[test]
    fn slugs() {
        assert_eq!(slugify("KDE Plasma'yı canlı ayarlama"), "kde-plasma-yi-canli-ayarlama");
        assert_eq!(slugify("Çağrı Şükrü Öğüt"), "cagri-sukru-ogut");
        assert_eq!(slugify("Straße & Œuvre"), "strasse-oeuvre");
        assert_eq!(slugify("Машинное обучение"), "машинное-обучение");
        assert_eq!(slugify("機械学習"), "機械学習");
        assert_eq!(slugify("!!!"), "");
    }
}
