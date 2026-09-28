//! Tier 0 — tabel ganti langsung.
//!
//! Lapisan paling murah dan paling sering kena. Ini yang dipakai papan ketik
//! ponsel: pencocokan persis, tanpa perhitungan apa pun. `yg` → `yang`,
//! `teh` → `the`, `sya` → `saya`. Biayanya satu pencarian hash.
//!
//! Tabelnya sengaja **dipisah dua**:
//!
//! * **typo** — salah ketik sungguhan yang tidak mungkin disengaja
//!   (`yagn`, `recieve`). Selalu aktif.
//! * **slang** — singkatan yang mungkin **disengaja** (`yg`, `bgt`, `gpp`).
//!   Bisa dimatikan pengguna, karena sebagian orang memang ingin catatannya
//!   tetap ringkas.
//!
//! Memaksa keduanya jadi satu tabel adalah kesalahan desain: mengubah `yg`
//! jadi `yang` pada orang yang sengaja menyingkat sama menjengkelkannya
//! dengan membiarkan `yagn` lolos.

use std::collections::HashMap;

#[derive(Default)]
pub struct Table {
    map: HashMap<Box<str>, Box<str>>,
}

impl Table {
    /// Baca format TSV: `salah<TAB>benar`, satu pasangan per baris.
    /// Baris kosong dan baris diawali `#` diabaikan.
    pub fn from_tsv(src: &str) -> Self {
        let mut t = Self::default();
        t.extend_tsv(src);
        t
    }

    pub fn extend_tsv(&mut self, src: &str) {
        for line in src.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((from, to)) = line.split_once('\t') else {
                continue;
            };
            let (from, to) = (from.trim(), to.trim());
            if from.is_empty() || to.is_empty() || from == to {
                continue;
            }
            self.map
                .insert(from.to_lowercase().into_boxed_str(), to.into());
        }
    }

    /// Cari pengganti, dengan huruf besar/kecil mengikuti masukan aslinya.
    pub fn get(&self, word: &str) -> Option<String> {
        let repl = self.map.get(word.to_lowercase().as_str())?;
        Some(match_case(word, repl))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Apakah kata ini terdaftar sebagai bentuk yang **salah**?
    ///
    /// Dipakai untuk menyaring kamus: kalau kita sudah menyatakan `yagn`
    /// itu typo, `yagn` tidak boleh ikut menghuni kamus — jika ikut, ia
    /// menjadi sasaran koreksi yang menarik bagi typo lain.
    pub fn has_key(&self, word: &str) -> bool {
        self.map.contains_key(word.to_lowercase().as_str())
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(|k| &**k)
    }
}

/// Samakan pola kapitalisasi hasil koreksi dengan kata aslinya.
///
/// `yg` → `yang`, `Yg` → `Yang`, `YG` → `YANG`.
/// Tanpa ini, mengoreksi kata di awal kalimat justru merusak kapitalisasi.
pub fn match_case(original: &str, replacement: &str) -> String {
    let letters: Vec<char> = original.chars().filter(|c| c.is_alphabetic()).collect();

    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return replacement.to_uppercase();
    }
    if letters.first().is_some_and(|c| c.is_uppercase()) {
        let mut cs = replacement.chars();
        return match cs.next() {
            Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
            None => String::new(),
        };
    }
    replacement.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabel() -> Table {
        Table::from_tsv("# komentar\nyg\tyang\nteh\tthe\n\nsya\tsaya\nrusak-tanpa-tab\n")
    }

    #[test]
    fn membaca_tsv_dan_melewati_sampah() {
        let t = tabel();
        assert_eq!(t.len(), 3);
        assert_eq!(t.get("yg").as_deref(), Some("yang"));
    }

    #[test]
    fn tidak_peduli_besar_kecil_saat_mencari() {
        assert_eq!(tabel().get("YG").as_deref(), Some("YANG"));
    }

    #[test]
    fn kapitalisasi_awal_dipertahankan() {
        assert_eq!(tabel().get("Yg").as_deref(), Some("Yang"));
    }

    #[test]
    fn kata_tak_dikenal_mengembalikan_none() {
        assert!(tabel().get("makan").is_none());
    }

    #[test]
    fn pemetaan_ke_diri_sendiri_diabaikan() {
        let t = Table::from_tsv("kopi\tkopi\n");
        assert!(t.is_empty());
    }

    #[test]
    fn pencocokan_kapital() {
        assert_eq!(match_case("yg", "yang"), "yang");
        assert_eq!(match_case("Yg", "yang"), "Yang");
        assert_eq!(match_case("YG", "yang"), "YANG");
        // Satu huruf kapital bukan berarti akronim.
        assert_eq!(match_case("I", "i"), "I");
    }
}
