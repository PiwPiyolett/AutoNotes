//! Pembaca & penulis frontmatter catatan.
//!
//! Tiap catatan adalah satu berkas `.md` biasa yang bisa dibuka Notepad,
//! di-`git commit`, dan disinkronkan lewat Google Drive:
//!
//! ```text
//! ---
//! id: 01J8XQ2K3M4N5P6Q7R8S9T0V1W
//! title: Rapat anggaran kuartal tiga
//! tags: [kerja, rapat]
//! created: 2026-08-19T09:12:00+07:00
//! updated: 2026-08-19T10:45:00+07:00
//! pinned: true
//! ---
//! isi catatan di sini...
//! ```
//!
//! # Kenapa ditulis sendiri, bukan pakai pustaka YAML
//!
//! Bentuk frontmatter di sini tetap dan sangat sempit — tujuh kunci, semuanya
//! skalar atau daftar datar. Pustaka YAML umum membawa mesin parser penuh
//! beserta seluruh kerumitannya (jangkar, tag, blok multi-baris, `norway
//! problem`), dan ekosistem YAML Rust sedang berpindah-pindah pemeliharaan.
//! Parser sempit yang bisa diuji tuntas lebih murah dan lebih dapat diramalkan.
//!
//! Yang penting: berkas yang **tidak** punya frontmatter tetap terbaca. Berkas
//! Markdown apa pun yang pengguna lempar ke folder catatan harus bisa dibuka,
//! bukan ditolak.

use chrono::{DateTime, Local, TimeZone, Utc};

/// Satu catatan, apa adanya seperti tersimpan di disk.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    /// Detik Unix.
    pub created: i64,
    pub updated: i64,
    pub pinned: bool,
    pub archived: bool,
    /// `false` (baku): judul otomatis mengikuti baris pertama isi — nyaman
    /// untuk catatan cepat. `true`: pengguna sudah mengetik judul sendiri,
    /// jadi judul **tidak** boleh ditimpa oleh isi.
    #[serde(default)]
    pub title_manual: bool,
    pub body: String,
}

impl Note {
    pub fn new(id: String, title: String, body: String, now: i64) -> Self {
        Self {
            id,
            title,
            tags: Vec::new(),
            created: now,
            updated: now,
            pinned: false,
            archived: false,
            title_manual: false,
            body,
        }
    }
}

/// Ambil judul dari isi catatan: judul H1 pertama, atau baris tak-kosong
/// pertama, atau teks cadangan.
///
/// Pengguna yang sedang buru-buru tidak akan mengisi kolom judul. Jadi judul
/// harus muncul sendiri dari apa yang mereka ketik.
pub fn derive_title(body: &str, fallback: &str) -> String {
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let bersih = line.trim_start_matches('#').trim();
        if bersih.is_empty() {
            continue;
        }
        // Judul yang terlalu panjang bikin daftar catatan tidak terbaca.
        return bersih.chars().take(120).collect();
    }
    fallback.to_string()
}

fn parse_time(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|t| t.timestamp())
}

fn format_time(ts: i64) -> String {
    match Local.timestamp_opt(ts, 0).single() {
        Some(t) => t.to_rfc3339(),
        None => Utc.timestamp_opt(0, 0).unwrap().to_rfc3339(),
    }
}

/// Pecah `[a, b, c]` atau `a, b, c` menjadi daftar tag.
fn parse_tags(raw: &str) -> Vec<String> {
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn parse_bool(raw: &str) -> bool {
    matches!(raw.trim().to_lowercase().as_str(), "true" | "yes" | "1")
}

/// Buang tanda kutip pembungkus kalau ada.
fn unquote(raw: &str) -> String {
    let t = raw.trim();
    if t.len() >= 2 && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\''))) {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

/// Baca berkas catatan.
///
/// `id_cadangan` dipakai kalau berkas belum punya `id` — biasanya berkas
/// Markdown yang dibuat pengguna di luar aplikasi.
pub fn parse(src: &str, id_cadangan: &str, waktu_cadangan: i64) -> Note {
    let (header, body) = pisah_frontmatter(src);

    let mut note = Note::new(
        id_cadangan.to_string(),
        String::new(),
        body.to_string(),
        waktu_cadangan,
    );

    for line in header.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((kunci, nilai)) = line.split_once(':') else {
            continue;
        };
        let nilai = nilai.trim();
        match kunci.trim().to_lowercase().as_str() {
            "id" if !nilai.is_empty() => note.id = unquote(nilai),
            "title" => note.title = unquote(nilai),
            "tags" => note.tags = parse_tags(nilai),
            "created" => note.created = parse_time(nilai).unwrap_or(waktu_cadangan),
            "updated" => note.updated = parse_time(nilai).unwrap_or(waktu_cadangan),
            "pinned" => note.pinned = parse_bool(nilai),
            "archived" => note.archived = parse_bool(nilai),
            "title_manual" => note.title_manual = parse_bool(nilai),
            _ => {}
        }
    }

    if note.title.is_empty() {
        // Tak ada judul tersimpan → turunkan dari isi (dan biarkan otomatis).
        note.title = derive_title(&note.body, "Tanpa judul");
        note.title_manual = false;
    }
    if note.updated < note.created {
        note.updated = note.created;
    }
    note
}

/// Pisahkan blok frontmatter dari isi. Kalau tidak ada, seluruh berkas
/// dianggap isi.
fn pisah_frontmatter(src: &str) -> (&str, &str) {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src); // BOM dari Notepad
    if !src.starts_with("---") {
        return ("", src);
    }
    let Some(after_first) = src.find('\n') else {
        return ("", src);
    };
    let sisa = &src[after_first + 1..];

    let mut offset = 0usize;
    for line in sisa.split_inclusive('\n') {
        if line.trim() == "---" {
            let header = &sisa[..offset];
            let body = &sisa[offset + line.len()..];
            return (header, body.strip_prefix('\n').unwrap_or(body));
        }
        offset += line.len();
    }
    // Penanda penutup tidak ketemu: perlakukan semuanya sebagai isi supaya
    // catatan tidak "hilang" gara-gara frontmatter rusak.
    ("", src)
}

/// Tulis catatan kembali ke bentuk berkas.
pub fn render(note: &Note) -> String {
    let mut s = String::with_capacity(note.body.len() + 256);
    s.push_str("---\n");
    s.push_str(&format!("id: {}\n", note.id));
    s.push_str(&format!("title: {}\n", escape_scalar(&note.title)));
    if !note.tags.is_empty() {
        s.push_str(&format!("tags: [{}]\n", note.tags.join(", ")));
    }
    s.push_str(&format!("created: {}\n", format_time(note.created)));
    s.push_str(&format!("updated: {}\n", format_time(note.updated)));
    if note.pinned {
        s.push_str("pinned: true\n");
    }
    if note.archived {
        s.push_str("archived: true\n");
    }
    if note.title_manual {
        s.push_str("title_manual: true\n");
    }
    s.push_str("---\n");
    s.push_str(&note.body);
    s
}

/// Judul yang mengandung `:` atau diawali karakter khusus harus dikutip,
/// kalau tidak berkasnya tidak akan terbaca ulang dengan benar.
fn escape_scalar(s: &str) -> String {
    let perlu_kutip = s.contains(':')
        || s.contains('#')
        || s.starts_with(['[', '{', '-', '*', '&', '!', '|', '>', '\'', '"', '%', '@'])
        || s.trim() != s;
    if perlu_kutip {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTOH: &str = "---\nid: 01ABC\ntitle: Rapat anggaran\ntags: [kerja, rapat]\ncreated: 2026-08-19T09:12:00+07:00\nupdated: 2026-08-19T10:45:00+07:00\npinned: true\n---\nisi catatan\nbaris kedua";

    #[test]
    fn membaca_frontmatter_lengkap() {
        let n = parse(CONTOH, "cadangan", 0);
        assert_eq!(n.id, "01ABC");
        assert_eq!(n.title, "Rapat anggaran");
        assert_eq!(n.tags, vec!["kerja", "rapat"]);
        assert!(n.pinned);
        assert!(!n.archived);
        assert_eq!(n.body, "isi catatan\nbaris kedua");
    }

    #[test]
    fn berkas_tanpa_frontmatter_tetap_terbaca() {
        let n = parse("# Catatan biasa\n\nisi apa adanya", "id-baru", 1234);
        assert_eq!(n.id, "id-baru");
        assert_eq!(n.title, "Catatan biasa");
        assert_eq!(n.created, 1234);
        assert_eq!(n.body, "# Catatan biasa\n\nisi apa adanya");
    }

    #[test]
    fn frontmatter_rusak_tidak_menghilangkan_isi() {
        // Penanda penutup tidak ada — isi tidak boleh raib.
        let src = "---\ntitle: Rusak\nisi yang berharga";
        let n = parse(src, "x", 0);
        assert!(n.body.contains("isi yang berharga"));
    }

    #[test]
    fn bom_dari_notepad_diabaikan() {
        let src = format!("\u{feff}{CONTOH}");
        let n = parse(&src, "x", 0);
        assert_eq!(n.id, "01ABC");
    }

    #[test]
    fn bolak_balik_tidak_mengubah_apa_pun() {
        let asli = parse(CONTOH, "x", 0);
        let ditulis = render(&asli);
        let dibaca = parse(&ditulis, "y", 0);
        assert_eq!(asli, dibaca);
    }

    #[test]
    fn judul_bertitik_dua_tetap_utuh_setelah_bolak_balik() {
        let mut n = Note::new("id1".into(), "Rapat: anggaran Q3".into(), "isi".into(), 1000);
        n.tags = vec!["kerja".into()];
        let dibaca = parse(&render(&n), "z", 0);
        assert_eq!(dibaca.title, "Rapat: anggaran Q3");
        assert_eq!(dibaca, n);
    }

    #[test]
    fn judul_diturunkan_dari_isi_kalau_kosong() {
        assert_eq!(derive_title("# Judul Besar\nisi", "-"), "Judul Besar");
        assert_eq!(derive_title("\n\n  baris pertama\nlain", "-"), "baris pertama");
        assert_eq!(derive_title("", "Tanpa judul"), "Tanpa judul");
        assert_eq!(derive_title("###\n\nisi sebenarnya", "-"), "isi sebenarnya");
    }

    #[test]
    fn judul_sangat_panjang_dipotong() {
        let panjang = "a".repeat(500);
        assert_eq!(derive_title(&panjang, "-").chars().count(), 120);
    }

    #[test]
    fn tag_menerima_beberapa_bentuk() {
        assert_eq!(parse_tags("[a, b]"), vec!["a", "b"]);
        assert_eq!(parse_tags("a, b"), vec!["a", "b"]);
        assert_eq!(parse_tags("[\"a\", 'b']"), vec!["a", "b"]);
        assert!(parse_tags("[]").is_empty());
        assert!(parse_tags("").is_empty());
    }

    #[test]
    fn waktu_rusak_jatuh_ke_cadangan() {
        let n = parse("---\ncreated: bukan-tanggal\n---\nisi", "x", 555);
        assert_eq!(n.created, 555);
    }

    #[test]
    fn updated_tidak_boleh_mendahului_created() {
        let n = parse(
            "---\ncreated: 2026-08-19T10:00:00+07:00\nupdated: 2020-01-01T00:00:00+07:00\n---\nisi",
            "x",
            0,
        );
        assert_eq!(n.updated, n.created);
    }

    #[test]
    fn kunci_tak_dikenal_diabaikan_bukan_bikin_gagal() {
        let n = parse("---\ntitle: Halo\nwarna: biru\n---\nisi", "x", 0);
        assert_eq!(n.title, "Halo");
        assert_eq!(n.body, "isi");
    }

    #[test]
    fn berkas_kosong_aman() {
        let n = parse("", "id", 99);
        assert_eq!(n.title, "Tanpa judul");
        assert_eq!(n.body, "");
    }

    #[test]
    fn judul_manual_bertahan_setelah_bolak_balik() {
        let mut n = Note::new("id1".into(), "Judul Pilihanku".into(), "# Baris pertama beda\nisi".into(), 1000);
        n.title_manual = true;
        let dibaca = parse(&render(&n), "x", 0);
        assert!(dibaca.title_manual, "penanda manual harus tersimpan");
        assert_eq!(dibaca.title, "Judul Pilihanku", "judul manual tidak boleh berubah jadi baris pertama");
    }

    #[test]
    fn judul_otomatis_tidak_menulis_penanda() {
        // Catatan mode otomatis tidak boleh mengotori frontmatter dengan
        // title_manual.
        let n = Note::new("id1".into(), "Judul".into(), "isi".into(), 1000);
        assert!(!render(&n).contains("title_manual"));
    }

    #[test]
    fn berkas_tanpa_judul_kembali_ke_mode_otomatis() {
        let n = parse("# Dari isi\n\nteks", "id", 0);
        assert!(!n.title_manual, "tanpa judul tersimpan = mode otomatis");
        assert_eq!(n.title, "Dari isi");
    }
}
