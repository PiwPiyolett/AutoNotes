//! Pencarian catatan di atas SQLite FTS5.
//!
//! # Dua hal yang membuatnya terasa cepat
//!
//! **Pencarian awalan.** Kata terakhir kueri selalu dicari sebagai awalan,
//! jadi mengetik `ang` sudah menemukan `anggaran` tanpa menunggu kata selesai.
//! Hasil berubah di tiap ketukan tombol.
//!
//! **Toleran typo.** Kueri ikut dilewatkan mesin koreksi. Orang yang buru-buru
//! juga salah ketik saat *mencari*, bukan hanya saat menulis — dan pencarian
//! yang mengembalikan nol hasil karena satu huruf meleset adalah pengalaman
//! yang membuat orang menyerah. Bentuk asli dan bentuk terkoreksi dicari
//! bersamaan, jadi mengetik `anggran` tetap menemukan `anggaran` **dan**
//! catatan yang memang berisi kata `anggran`.

use anyhow::Result;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub pinned: bool,
    /// Potongan isi dengan kata yang cocok ditandai `«...»`.
    pub snippet: String,
    /// Skor BM25 — makin kecil (makin negatif) makin cocok.
    pub score: f64,
}

/// Ubah teks jadi nama berkas yang aman dan terbaca manusia.
pub fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut strip = false;
    for c in s.chars().take(60) {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                out.push(l);
            }
            strip = false;
        } else if !strip && !out.is_empty() {
            out.push('-');
            strip = true;
        }
    }
    let hasil = out.trim_matches('-').to_string();
    if hasil.is_empty() {
        "catatan".to_string()
    } else {
        hasil
    }
}

/// Bersihkan satu kata agar aman dimasukkan ke kueri FTS5.
///
/// FTS5 punya sintaksnya sendiri (`AND`, `OR`, `NOT`, `NEAR`, `*`, `"`, `:`,
/// `^`). Teks pengguna tidak boleh ditafsirkan sebagai sintaks — bukan hanya
/// karena bisa membuat kueri gagal, tapi karena mengetik tanda kutip di kotak
/// pencarian lalu mendapat pesan galat adalah pengalaman yang buruk.
fn bersihkan(kata: &str) -> String {
    kata.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '\'')
        .collect()
}

/// Susun ekspresi FTS5 dari kueri pengguna dan bentuk alternatifnya.
///
/// Semua kata wajib ada (AND), kecuali kata terakhir yang dicari sebagai
/// awalan. Bentuk alternatif digabung dengan OR sebagai kelompok terpisah.
pub fn build_query(query: &str, varian: &[String]) -> Option<String> {
    let mut kelompok: Vec<String> = Vec::new();
    for teks in std::iter::once(query).chain(varian.iter().map(|s| s.as_str())) {
        if let Some(k) = satu_kelompok(teks) {
            if !kelompok.contains(&k) {
                kelompok.push(k);
            }
        }
    }
    if kelompok.is_empty() {
        return None;
    }
    Some(kelompok.join(" OR "))
}

fn satu_kelompok(teks: &str) -> Option<String> {
    let kata: Vec<String> = teks
        .split_whitespace()
        .map(bersihkan)
        .filter(|k| !k.is_empty())
        .collect();
    if kata.is_empty() {
        return None;
    }
    let terakhir = kata.len() - 1;
    let bagian: Vec<String> = kata
        .iter()
        .enumerate()
        .map(|(i, k)| {
            if i == terakhir {
                format!("\"{k}\"*") // kata terakhir: cocokkan sebagai awalan
            } else {
                format!("\"{k}\"")
            }
        })
        .collect();
    Some(format!("({})", bagian.join(" AND ")))
}

/// Jalankan pencarian yang sudah disusun.
pub fn jalankan(db: &Connection, fts: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let mut stmt = db.prepare(
        "SELECT n.id, n.title, n.updated_at, n.pinned,
                snippet(notes_fts, 1, '«', '»', '…', 12) AS cuplikan,
                bm25(notes_fts, 12.0, 1.0) AS skor
         FROM notes_fts
         JOIN notes n ON n.rowid = notes_fts.rowid
         WHERE notes_fts MATCH ?1
           AND n.trashed_at IS NULL
         ORDER BY n.pinned DESC, skor
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![fts, limit as i64], |r| {
        Ok(SearchHit {
            id: r.get(0)?,
            title: r.get(1)?,
            updated_at: r.get(2)?,
            pinned: r.get::<_, i64>(3)? != 0,
            snippet: r.get(4)?,
            score: r.get(5)?,
        })
    });

    // Kueri FTS5 yang tidak terbentuk sempurna sebaiknya menghasilkan
    // "tidak ada hasil", bukan pesan galat di wajah pengguna.
    match rows {
        Ok(iter) => Ok(iter.filter_map(|h| h.ok()).collect()),
        Err(_) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_menyederhanakan_judul() {
        assert_eq!(slug("Rapat Anggaran Q3"), "rapat-anggaran-q3");
        assert_eq!(slug("  Judul: dengan / tanda!  "), "judul-dengan-tanda");
        assert_eq!(slug("Café Ngopi"), "café-ngopi");
    }

    #[test]
    fn slug_tidak_pernah_kosong() {
        assert_eq!(slug(""), "catatan");
        assert_eq!(slug("!!!???"), "catatan");
    }

    #[test]
    fn slug_dibatasi_panjangnya() {
        assert!(slug(&"panjang ".repeat(40)).len() <= 70);
    }

    #[test]
    fn kata_terakhir_dicari_sebagai_awalan() {
        let q = build_query("rapat ang", &[]).unwrap();
        assert_eq!(q, "(\"rapat\" AND \"ang\"*)");
    }

    #[test]
    fn satu_kata_pun_dicari_sebagai_awalan() {
        assert_eq!(build_query("ang", &[]).unwrap(), "(\"ang\"*)");
    }

    #[test]
    fn varian_digabung_dengan_or() {
        let q = build_query("anggran", &["anggaran".to_string()]).unwrap();
        assert_eq!(q, "(\"anggran\"*) OR (\"anggaran\"*)");
    }

    #[test]
    fn varian_yang_sama_dengan_aslinya_tidak_diduakan() {
        let q = build_query("rapat", &["rapat".to_string()]).unwrap();
        assert_eq!(q, "(\"rapat\"*)");
    }

    /// Mengetik tanda kutip atau kata `OR` di kotak pencarian tidak boleh
    /// ditafsirkan sebagai sintaks FTS5.
    #[test]
    fn sintaks_fts_dari_pengguna_dilucuti() {
        let q = build_query("\"rapat\" OR NEAR(x)", &[]).unwrap();
        assert!(!q.contains("NEAR("));
        assert_eq!(q, "(\"rapat\" AND \"OR\" AND \"NEARx\"*)");
    }

    #[test]
    fn tanda_baca_murni_menghasilkan_kueri_kosong() {
        assert!(build_query("   ", &[]).is_none());
        assert!(build_query("!!! ???", &[]).is_none());
    }

    #[test]
    fn tanda_kutip_satu_dalam_kata_inggris_dipertahankan() {
        assert_eq!(build_query("don't", &[]).unwrap(), "(\"don't\"*)");
    }
}
