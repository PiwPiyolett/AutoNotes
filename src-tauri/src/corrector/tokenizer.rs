//! Pemecah teks menjadi token kata, plus penandaan **zona terlindungi**.
//!
//! # Aturan emas aplikasi ini
//!
//! Lebih baik melewatkan satu typo daripada merusak satu hal yang memang
//! sengaja ditulis begitu. Autocorrect yang salah sekali jauh lebih menyakitkan
//! daripada dua puluh typo yang lolos — apalagi bagi orang yang sedang
//! buru-buru dan tidak sempat memeriksa ulang.
//!
//! Karena itu semua yang di bawah ini **tidak pernah** disentuh mesin koreksi:
//! URL, alamat surel, `@sebutan`, `#tagar`, blok kode, kode sebaris,
//! target tautan Markdown, frontmatter YAML, kata bercampur angka, dan
//! singkatan HURUF BESAR.

use std::ops::Range;

/// Alasan sebuah token dilindungi. Berguna untuk menjelaskan ke pengguna
/// kenapa suatu kata tidak ikut dikoreksi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Protect {
    Frontmatter,
    CodeFence,
    InlineCode,
    Url,
    Email,
    Mention,
    Tag,
    LinkTarget,
    HasDigit,
    Acronym,
}

/// Satu kata di dalam teks, ditandai posisi byte-nya.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub protected: Option<Protect>,
}

impl Token {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }

    pub fn is_protected(&self) -> bool {
        self.protected.is_some()
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// Hitung seluruh rentang byte yang harus dilindungi.
///
/// Hasilnya terurut menaik menurut posisi awal. Rentang boleh tumpang tindih —
/// pemeriksaan di [`tokenize`] hanya peduli "apakah tersentuh", bukan oleh apa.
pub fn protected_ranges(text: &str) -> Vec<(usize, usize, Protect)> {
    let b = text.as_bytes();
    let n = b.len();
    let mut out: Vec<(usize, usize, Protect)> = Vec::new();

    // --- Frontmatter YAML di awal berkas -----------------------------------
    if text.starts_with("---") {
        if let Some(first_nl) = text.find('\n') {
            let mut i = first_nl + 1;
            while i <= n {
                let line_end = text[i..].find('\n').map(|k| i + k).unwrap_or(n);
                if text[i..line_end].trim() == "---" {
                    out.push((0, line_end, Protect::Frontmatter));
                    break;
                }
                if line_end >= n {
                    break;
                }
                i = line_end + 1;
            }
        }
    }

    // --- Blok kode berpagar ``` --------------------------------------------
    // Dihitung lebih dulu supaya backtick tunggal di dalamnya tidak salah baca.
    let mut i = 0usize;
    while i + 3 <= n {
        if &b[i..i + 3] == b"```" {
            let start = i;
            let mut j = i + 3;
            let mut end = n;
            while j + 3 <= n {
                if &b[j..j + 3] == b"```" {
                    end = j + 3;
                    break;
                }
                j += 1;
            }
            out.push((start, end, Protect::CodeFence));
            i = end;
        } else {
            i += 1;
        }
    }
    let fences: Vec<(usize, usize, Protect)> = out
        .iter()
        .filter(|r| r.2 == Protect::CodeFence)
        .copied()
        .collect();

    // --- Kode sebaris `begini` ---------------------------------------------
    let mut i = 0usize;
    while i < n {
        if b[i] == b'`' && !touches(&fences, i, i + 1) {
            if let Some(k) = text[i + 1..].find('`') {
                let end = i + 1 + k + 1;
                out.push((i, end, Protect::InlineCode));
                i = end;
                continue;
            }
        }
        i += 1;
    }

    // --- URL ----------------------------------------------------------------
    for pat in ["https://", "http://", "ftp://", "www."] {
        let mut from = 0usize;
        while let Some(k) = text[from..].find(pat) {
            let s = from + k;
            if !touches(&out, s, s + 1) {
                let mut e = s;
                while e < n && !b[e].is_ascii_whitespace() {
                    e += 1;
                }
                // Tanda baca di ujung kalimat bukan bagian dari URL.
                while e > s && matches!(b[e - 1], b'.' | b',' | b')' | b']' | b';' | b':' | b'!' | b'?') {
                    e -= 1;
                }
                out.push((s, e, Protect::Url));
            }
            from = s + pat.len();
        }
    }

    // --- Surel, sebutan, tagar ---------------------------------------------
    for idx in 0..n {
        let c = b[idx];
        if c != b'@' && c != b'#' {
            continue;
        }
        if touches(&out, idx, idx + 1) {
            continue;
        }
        let word_before = idx > 0 && (b[idx - 1].is_ascii_alphanumeric() || matches!(b[idx - 1], b'.' | b'_' | b'-' | b'+'));

        if c == b'@' && word_before {
            // Bentuk `nama@domain.tld` — lindungi seluruhnya.
            let mut s = idx;
            while s > 0 && (b[s - 1].is_ascii_alphanumeric() || matches!(b[s - 1], b'.' | b'_' | b'-' | b'+')) {
                s -= 1;
            }
            let mut e = idx + 1;
            while e < n && (b[e].is_ascii_alphanumeric() || matches!(b[e], b'.' | b'_' | b'-')) {
                e += 1;
            }
            while e > idx + 1 && b[e - 1] == b'.' {
                e -= 1;
            }
            if text[idx + 1..e].contains('.') {
                out.push((s, e, Protect::Email));
            }
            continue;
        }

        let at_boundary = idx == 0 || b[idx - 1].is_ascii_whitespace() || matches!(b[idx - 1], b'(' | b'[' | b'"' | b'\'');
        if at_boundary {
            let mut e = idx + 1;
            while e < n && (b[e].is_ascii_alphanumeric() || matches!(b[e], b'_' | b'-' | b'/')) {
                e += 1;
            }
            if e > idx + 1 {
                out.push((idx, e, if c == b'@' { Protect::Mention } else { Protect::Tag }));
            }
        }
    }

    // --- Target tautan Markdown: [judul](target) ---------------------------
    let mut from = 0usize;
    while let Some(k) = text[from..].find("](") {
        let s = from + k + 1;
        match text[s..].find(')') {
            Some(cl) => {
                let e = s + cl + 1;
                out.push((s, e, Protect::LinkTarget));
                from = e;
            }
            None => break,
        }
    }

    out.sort_by_key(|r| (r.0, r.1));
    out
}

/// Apakah rentang `[s, e)` bersinggungan dengan salah satu rentang terlindungi?
fn touches(ranges: &[(usize, usize, Protect)], s: usize, e: usize) -> bool {
    ranges.iter().any(|&(rs, re, _)| s < re && rs < e)
}

fn protector(ranges: &[(usize, usize, Protect)], s: usize, e: usize) -> Option<Protect> {
    ranges
        .iter()
        .find(|&&(rs, re, _)| s < re && rs < e)
        .map(|&(_, _, p)| p)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}'
}

/// Pecah teks menjadi token kata beserta status perlindungannya.
pub fn tokenize(text: &str) -> Vec<Token> {
    let ranges = protected_ranges(text);
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;

    let flush = |start: usize, end: usize, tokens: &mut Vec<Token>| {
        let raw = &text[start..end];
        // Apostrof di ujung biasanya tanda kutip, bukan bagian kata.
        let trimmed = raw.trim_matches(|c| c == '\'' || c == '\u{2019}');
        if trimmed.is_empty() {
            return;
        }
        let offset = raw.find(trimmed).unwrap_or(0);
        let (s, e) = (start + offset, start + offset + trimmed.len());

        let protected = protector(&ranges, s, e)
            .or_else(|| trimmed.chars().any(|c| c.is_numeric()).then_some(Protect::HasDigit))
            .or_else(|| {
                let letters: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
                (letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase())).then_some(Protect::Acronym)
            });

        tokens.push(Token { start: s, end: e, protected });
    };

    for (i, ch) in text.char_indices() {
        if is_word_char(ch) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            flush(s, i, &mut tokens);
        }
    }
    if let Some(s) = start {
        flush(s, text.len(), &mut tokens);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kata_bebas(text: &str) -> Vec<&str> {
        tokenize(text)
            .iter()
            .filter(|t| !t.is_protected())
            .map(|t| t.text(text))
            .collect()
    }

    fn alasan(text: &str, kata: &str) -> Option<Protect> {
        tokenize(text)
            .iter()
            .find(|t| t.text(text) == kata)
            .and_then(|t| t.protected)
    }

    #[test]
    fn memecah_kalimat_biasa() {
        assert_eq!(kata_bebas("saya makan nasi"), vec!["saya", "makan", "nasi"]);
    }

    #[test]
    fn posisi_byte_akurat() {
        let t = "aku makan";
        let toks = tokenize(t);
        assert_eq!(toks[1].start, 4);
        assert_eq!(toks[1].end, 9);
        assert_eq!(toks[1].text(t), "makan");
    }

    #[test]
    fn url_dilindungi() {
        let t = "buka https://contoh.co.id/artkel sekarang";
        assert_eq!(kata_bebas(t), vec!["buka", "sekarang"]);
    }

    #[test]
    fn titik_akhir_kalimat_bukan_bagian_url() {
        let t = "lihat https://a.id.";
        let toks = tokenize(t);
        let url = toks.iter().find(|x| x.text(t).starts_with('a')).unwrap();
        assert!(url.is_protected());
    }

    #[test]
    fn surel_dilindungi() {
        let t = "kirim ke budi.santoso@kantor.co.id ya";
        assert_eq!(kata_bebas(t), vec!["kirim", "ke", "ya"]);
    }

    #[test]
    fn sebutan_dan_tagar_dilindungi() {
        let t = "halo @budiman soal #proyek-kilat";
        assert_eq!(kata_bebas(t), vec!["halo", "soal"]);
        assert_eq!(alasan(t, "budiman"), Some(Protect::Mention));
    }

    #[test]
    fn kode_sebaris_dilindungi() {
        let t = "jalankan `npm run dev` dulu";
        assert_eq!(kata_bebas(t), vec!["jalankan", "dulu"]);
    }

    #[test]
    fn blok_kode_dilindungi() {
        let t = "sebelum\n```\nlet mkaan = 1;\n```\nsesudah";
        assert_eq!(kata_bebas(t), vec!["sebelum", "sesudah"]);
    }

    #[test]
    fn blok_kode_tak_tertutup_tetap_dilindungi_sampai_akhir() {
        let t = "mulai\n```\nkode mkaan disini";
        assert_eq!(kata_bebas(t), vec!["mulai"]);
    }

    #[test]
    fn frontmatter_dilindungi() {
        let t = "---\ntitle: Rapat mkaan\ntags: [kerja]\n---\nisi catatan";
        assert_eq!(kata_bebas(t), vec!["isi", "catatan"]);
    }

    #[test]
    fn kata_bercampur_angka_dilindungi() {
        let t = "versi v2 build A1B2";
        assert_eq!(kata_bebas(t), vec!["versi", "build"]);
        assert_eq!(alasan(t, "v2"), Some(Protect::HasDigit));
    }

    #[test]
    fn singkatan_huruf_besar_dilindungi() {
        let t = "kirim PDF ke HRD sekarang";
        assert_eq!(kata_bebas(t), vec!["kirim", "ke", "sekarang"]);
        assert_eq!(alasan(t, "PDF"), Some(Protect::Acronym));
    }

    #[test]
    fn satu_huruf_kapital_bukan_singkatan() {
        // "A" tunggal (mis. penomoran) tidak dianggap akronim.
        let t = "poin A penting";
        assert_eq!(kata_bebas(t), vec!["poin", "A", "penting"]);
    }

    #[test]
    fn target_tautan_markdown_dilindungi() {
        let t = "lihat [dokumen ini](./catatn/rapat.md) ya";
        assert_eq!(kata_bebas(t), vec!["lihat", "dokumen", "ini", "ya"]);
    }

    #[test]
    fn apostrof_di_dalam_kata_dipertahankan() {
        assert_eq!(kata_bebas("don't stop"), vec!["don't", "stop"]);
    }

    #[test]
    fn tanda_kutip_tidak_ikut_jadi_kata() {
        assert_eq!(kata_bebas("'kata' saja"), vec!["kata", "saja"]);
    }

    #[test]
    fn huruf_beraksen_tetap_utuh() {
        let t = "kafé buka";
        let toks = tokenize(t);
        assert_eq!(toks[0].text(t), "kafé");
    }

    #[test]
    fn teks_kosong_aman() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   \n\t ").is_empty());
    }
}
