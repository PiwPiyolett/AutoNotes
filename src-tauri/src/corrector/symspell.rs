//! SymSpell — indeks penghapusan yang dihitung di muka (Wolf Garbe).
//!
//! # Kenapa ditulis sendiri, bukan pakai crate `symspell`?
//!
//! Kita butuh tiga hal yang tidak disediakan crate itu:
//!   1. biaya edit **berbobot QWERTY** (lihat [`super::qwerty`]),
//!   2. **daftar kandidat ber-skor**, bukan satu jawaban tunggal — lapisan
//!      pemeringkat (T2) nanti perlu melihat semua pilihan,
//!   3. tata letak **tabel datar terurut** demi hemat memori.
//!
//! # Cara kerjanya
//!
//! Saat membangun indeks, untuk tiap kata kamus kita hasilkan semua varian
//! hasil *penghapusan* huruf sampai kedalaman `max_edit` (dibatasi pada
//! `prefix_len` huruf pertama supaya jumlahnya tidak meledak). Saat mencari,
//! kita lakukan hal yang sama pada kata masukan. Kalau sebuah kata typo dan
//! kata benar berjarak ≤ 2 edit, varian-penghapusan keduanya pasti bertemu di
//! satu titik yang sama — jadi cukup cocokkan hash, tanpa menyisir kamus.
//!
//! # Memori
//!
//! Naifnya, `HashMap<String, Vec<u32>>` untuk 100rb kata memakan ratusan MB.
//! Di sini indeks disimpan sebagai `Vec<(u64, u32)>` terurut — 12 byte per
//! entri, dicari dengan binary search. Untuk 100rb kata hasilnya ~20 MB,
//! dan pencarian tetap di bawah satu mikrodetik.
//!
//! Hash boleh saja bertabrakan; kandidat palsu yang muncul akibat tabrakan
//! tetap disaring oleh perhitungan jarak edit sesungguhnya di bawah.

use std::collections::{HashMap, HashSet};

use super::distance::{osa, weighted_osa};

/// Bobot biaya edit terhadap frekuensi kata, dalam ruang logaritma.
///
/// Ini inti model *noisy channel*:
///
/// ```text
/// P(benar | typo)  ∝  P(typo | benar) · P(benar)
///                     └── e^(-λ·biaya)  └── frekuensi
/// ```
///
/// Diambil logaritmanya menjadi `ln(frekuensi) − λ·biaya`.
///
/// λ = 5.0 berarti selisih satu edit penuh sepadan dengan keunggulan
/// frekuensi e⁵ ≈ 148 kali lipat. Cukup besar untuk mencegah kata yang
/// sangat umum menarik semua typo ke dirinya, cukup kecil agar frekuensi
/// tetap bisa mengalahkan selisih biaya yang tipis.
///
/// # Kenapa ini menggantikan pengurutan "biaya dulu, frekuensi belakangan"
///
/// Dulu kandidat diurutkan biaya menaik lalu frekuensi menurun, sehingga
/// frekuensi hanya jadi pemecah seri. Akibatnya `dpan` dikoreksi menjadi
/// `span`, bukan `depan`: huruf `d` dan `s` bertetangga di QWERTY sehingga
/// substitusinya berbiaya 0,7, sementara menyisipkan huruf `e` berbiaya 1,0.
/// Biaya menang, padahal `depan` ribuan kali lebih sering dipakai.
pub const LAMBDA: f32 = 5.0;

/// Satu usulan koreksi beserta alasan numeriknya.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub word: String,
    /// Jarak edit bilangan bulat (untuk penyaringan & penjelasan ke pengguna).
    pub distance: usize,
    /// Biaya berbobot QWERTY (untuk pemeringkatan halus).
    pub cost: f32,
    /// Frekuensi mentah di korpus — ditampilkan ke alat diagnosis.
    pub freq: u32,
    /// `ln(frekuensi / total frekuensi kamus)` — peluang kata ini muncul.
    ///
    /// # Kenapa dinormalkan, bukan pakai `ln(freq)` mentah
    ///
    /// Korpus Indonesia dan Inggris punya ukuran yang sangat berbeda, jadi
    /// frekuensi mentahnya tidak sebanding. Kata Inggris yang biasa saja bisa
    /// punya angka jauh lebih besar daripada kata Indonesia yang sangat umum,
    /// semata karena korpusnya lebih besar. Dibagi total, keduanya menjadi
    /// peluang sungguhan dan boleh diadu langsung.
    pub log_prob: f32,
}

impl Candidate {
    /// Skor noisy-channel. Makin besar makin mungkin ini kata yang dimaksud.
    pub fn score(&self) -> f32 {
        self.log_prob - LAMBDA * self.cost
    }
}

pub struct SymSpell {
    max_edit: usize,
    prefix_len: usize,
    words: Vec<Box<str>>,
    freqs: Vec<u32>,
    /// Bentuk `Vec<char>` tiap kata, di-cache agar loop penghitung jarak
    /// tidak perlu mendekode UTF-8 berulang kali.
    chars: Vec<Box<[char]>>,
    /// (hash varian penghapusan, indeks kata) — terurut menaik.
    entries: Vec<(u64, u32)>,
    lookup: HashMap<Box<str>, u32>,
    /// Jumlah seluruh frekuensi, penyebut untuk menghitung log-peluang.
    total_freq: f64,
}

impl SymSpell {
    /// Bangun indeks dari daftar `(kata, frekuensi)`.
    ///
    /// `prefix_len` 7 dengan `max_edit` 2 adalah setelan baku SymSpell:
    /// ~29 varian per kata. Turunkan ke 5 kalau memori jadi masalah.
    pub fn build(dict: impl IntoIterator<Item = (String, u32)>, max_edit: usize, prefix_len: usize) -> Self {
        let mut words: Vec<Box<str>> = Vec::new();
        let mut freqs: Vec<u32> = Vec::new();
        let mut chars: Vec<Box<[char]>> = Vec::new();
        let mut lookup: HashMap<Box<str>, u32> = HashMap::new();
        let mut entries: Vec<(u64, u32)> = Vec::new();

        for (raw, freq) in dict {
            let w = raw.trim().to_lowercase();
            if w.is_empty() {
                continue;
            }
            // Kata ganda di berkas kamus: ambil frekuensi tertinggi.
            if let Some(&i) = lookup.get(w.as_str()) {
                let i = i as usize;
                if freqs[i] < freq {
                    freqs[i] = freq;
                }
                continue;
            }

            let idx = words.len() as u32;
            for variant in delete_variants(&w, max_edit, prefix_len) {
                entries.push((fnv1a(&variant), idx));
            }
            lookup.insert(w.clone().into_boxed_str(), idx);
            chars.push(w.chars().collect::<Vec<_>>().into_boxed_slice());
            words.push(w.into_boxed_str());
            freqs.push(freq);
        }

        entries.sort_unstable();
        entries.dedup();
        entries.shrink_to_fit();

        let total_freq = freqs.iter().map(|&f| f as f64).sum::<f64>().max(1.0);

        Self { max_edit, prefix_len, words, freqs, chars, entries, lookup, total_freq }
    }

    /// `ln(frekuensi / total)`. Nilai selalu negatif; makin mendekati nol
    /// berarti kata makin umum.
    fn log_prob(&self, freq: u32) -> f32 {
        ((freq.max(1) as f64) / self.total_freq).ln() as f32
    }

    /// Apakah kata ini ada persis di kamus? (sudah dinormalkan ke huruf kecil)
    pub fn contains(&self, word: &str) -> bool {
        self.lookup.contains_key(word.to_lowercase().as_str())
    }

    /// Frekuensi kata, 0 kalau tidak ada di kamus.
    pub fn freq(&self, word: &str) -> u32 {
        self.lookup
            .get(word.to_lowercase().as_str())
            .map(|&i| self.freqs[i as usize])
            .unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Cari kandidat koreksi untuk `term`, terurut dari yang paling mungkin.
    ///
    /// Urutan peringkat: biaya berbobot menaik, lalu frekuensi menurun.
    /// Kalau `term` sendiri ada di kamus, ia ikut muncul dengan jarak 0 —
    /// pemanggil yang memutuskan apakah itu berarti "tidak perlu dikoreksi".
    pub fn lookup_candidates(&self, term: &str, max_edit: usize, max_results: usize) -> Vec<Candidate> {
        let max_edit = max_edit.min(self.max_edit);
        let lowered = term.to_lowercase();
        let tchars: Vec<char> = lowered.chars().collect();
        if tchars.is_empty() {
            return Vec::new();
        }

        let mut seen: HashSet<u32> = HashSet::new();
        let mut out: Vec<Candidate> = Vec::new();

        for variant in delete_variants(&lowered, max_edit, self.prefix_len) {
            for &(_, idx) in self.entries_with_hash(fnv1a(&variant)) {
                if !seen.insert(idx) {
                    continue;
                }
                let cand = &self.chars[idx as usize];
                if tchars.len().abs_diff(cand.len()) > max_edit {
                    continue;
                }
                let Some(distance) = osa(&tchars, cand, max_edit) else {
                    continue;
                };
                let freq = self.freqs[idx as usize];
                out.push(Candidate {
                    word: self.words[idx as usize].to_string(),
                    distance,
                    // Urutan argumen penting: yang diketik dulu, calon kata
                    // benar belakangan — biayanya asimetris.
                    cost: weighted_osa(&tchars, cand),
                    freq,
                    log_prob: self.log_prob(freq),
                });
            }
        }

        // Urut menurut skor noisy-channel, bukan biaya semata — lihat [`LAMBDA`].
        // Ini juga penting karena daftar dipotong di `max_results`: dengan
        // pengurutan biaya, jawaban yang benar bisa terbuang oleh sekumpulan
        // kandidat murah yang tidak relevan.
        out.sort_by(|a, b| {
            // Kata yang persis sama SELALU di depan. Tanpa pagar ini, kata
            // sah tapi jarang (`makam`, 300×) bisa dikalahkan kata umum yang
            // berjarak satu edit (`makan`, 20.000×) — dan pemanggil yang
            // mengandalkan "jarak 0 berarti tidak perlu dikoreksi" akan
            // mulai merusak kata yang sebenarnya benar.
            (a.distance == 0)
                .cmp(&(b.distance == 0))
                .reverse()
                .then_with(|| {
                    b.score()
                        .partial_cmp(&a.score())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.word.cmp(&b.word))
        });
        out.truncate(max_results);
        out
    }

    /// Potongan entri yang hash-nya sama persis, lewat dua kali binary search.
    fn entries_with_hash(&self, h: u64) -> &[(u64, u32)] {
        let lo = self.entries.partition_point(|e| e.0 < h);
        let hi = self.entries.partition_point(|e| e.0 <= h);
        &self.entries[lo..hi]
    }
}

/// Semua varian hasil penghapusan huruf dari `prefix_len` huruf pertama,
/// sampai kedalaman `max_edit`. Termasuk prefiks utuhnya sendiri.
fn delete_variants(word: &str, max_edit: usize, prefix_len: usize) -> HashSet<String> {
    let cs: Vec<char> = word.chars().collect();
    let take = cs.len().min(prefix_len);
    let prefix: String = cs[..take].iter().collect();

    let mut out = HashSet::new();
    out.insert(prefix.clone());
    recurse_deletes(&prefix, max_edit, &mut out);
    out
}

fn recurse_deletes(s: &str, depth: usize, out: &mut HashSet<String>) {
    if depth == 0 {
        return;
    }
    let cs: Vec<char> = s.chars().collect();
    if cs.len() <= 1 {
        return;
    }
    for skip in 0..cs.len() {
        let shorter: String = cs
            .iter()
            .enumerate()
            .filter_map(|(i, c)| (i != skip).then_some(*c))
            .collect();
        if out.insert(shorter.clone()) {
            recurse_deletes(&shorter, depth - 1, out);
        }
    }
}

/// FNV-1a 64-bit. Dipilih karena sangat cepat untuk string pendek dan
/// tidak butuh dependensi tambahan. Tabrakan tidak berbahaya di sini.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in s.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kamus() -> SymSpell {
        let kata = [
            ("makan", 9000u32),
            ("makam", 300),
            ("minum", 8000),
            ("mahal", 2000),
            ("malam", 7000),
            ("berlari", 1500),
            ("mengerjakan", 900),
            ("catatan", 4000),
            ("cepat", 6000),
            ("the", 50000),
            ("their", 9000),
            ("there", 12000),
        ];
        SymSpell::build(kata.into_iter().map(|(w, f)| (w.to_string(), f)), 2, 7)
    }

    #[test]
    fn menemukan_kata_persis() {
        let s = kamus();
        assert!(s.contains("makan"));
        assert!(!s.contains("makanx"));
        assert_eq!(s.freq("makan"), 9000);
        assert_eq!(s.freq("tidakada"), 0);
    }

    #[test]
    fn huruf_hilang_terkoreksi() {
        let s = kamus();
        let c = s.lookup_candidates("makn", 2, 5);
        assert_eq!(c[0].word, "makan");
        assert_eq!(c[0].distance, 1);
    }

    #[test]
    fn huruf_tertukar_terkoreksi() {
        let s = kamus();
        let c = s.lookup_candidates("mkaan", 2, 5);
        assert_eq!(c[0].word, "makan", "kandidat: {c:?}");
    }

    #[test]
    fn frekuensi_memutus_seri() {
        // "makam" dan "makan" sama-sama berjarak 1 dari "makaN"/"makaM"-an;
        // yang lebih sering dipakai harus menang.
        let s = kamus();
        let c = s.lookup_candidates("makaan", 2, 5);
        assert_eq!(c[0].word, "makan", "kandidat: {c:?}");
    }

    #[test]
    fn kata_yang_sudah_benar_muncul_dengan_jarak_nol() {
        let s = kamus();
        let c = s.lookup_candidates("makan", 2, 5);
        assert_eq!(c[0].distance, 0);
        assert_eq!(c[0].word, "makan");
    }

    #[test]
    fn kata_asing_total_tidak_menghasilkan_apa_apa() {
        let s = kamus();
        assert!(s.lookup_candidates("zxqwvbn", 2, 5).is_empty());
    }

    #[test]
    fn kata_ganda_diambil_frekuensi_tertinggi() {
        let s = SymSpell::build(
            [("kopi".to_string(), 10u32), ("kopi".to_string(), 500)],
            2,
            7,
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s.freq("kopi"), 500);
    }

    #[test]
    fn kata_panjang_tetap_terjangkau_lewat_prefiks() {
        let s = kamus();
        let c = s.lookup_candidates("mengerjakn", 2, 5);
        assert_eq!(c[0].word, "mengerjakan", "kandidat: {c:?}");
    }

    #[test]
    fn masukan_kosong_aman() {
        let s = kamus();
        assert!(s.lookup_candidates("", 2, 5).is_empty());
    }

    #[test]
    fn varian_penghapusan_dibatasi_prefiks() {
        // "abcdefghij" dengan prefix_len 3 hanya boleh menghasilkan varian
        // dari "abc": abc, ab, ac, bc, a, b, c.
        let v = delete_variants("abcdefghij", 2, 3);
        assert!(v.contains("abc"));
        assert!(v.contains("ab"));
        assert!(v.contains("c"));
        assert!(!v.contains("abcd"));
    }
}
