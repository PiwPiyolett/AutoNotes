//! Model bigram — jantung Tier 2 (koreksi sadar-konteks).
//!
//! # Apa yang diselesaikan
//!
//! Tier 1 tidak bisa membedakan `makan` dari `makam`: keduanya kata sah.
//! Hanya kalimatnya yang tahu mana yang dimaksud — "makan nasi" lazim,
//! "makam nasi" tidak pernah. Model ini menyimpan seberapa sering dua kata
//! muncul bersebelahan di korpus Bahasa Indonesia sungguhan (Leipzig Corpora,
//! ~279 rb pasangan), sehingga pemeringkat bisa bertanya: "dengan kata
//! sebelum/sesudahnya, kandidat mana yang paling masuk akal?"
//!
//! # Bentuk data
//!
//! Kata di-*intern* jadi `u32` sekali, lalu pasangan disimpan sebagai satu
//! `u64` (id-kiri di 32 bit atas, id-kanan di bawah) → hitungan. Ini membuat
//! pencarian satu pasangan jadi satu lookup hash, dan seluruh model muat di
//! ~12–15 MB untuk 279 rb pasangan.
//!
//! Model ini **hanya membaca konteks lokal** (kata tetangga langsung). Ia tidak
//! memahami topik paragraf — itu wilayah model bahasa besar. Tapi konteks lokal
//! sudah menyelesaikan mayoritas kasus nyata: `makam`↔`makan`, `haru`↔`hari`,
//! `baru`↔`terbaru`.

use std::collections::HashMap;

/// Pelicin aditif saat menghitung log-skor konteks. Membuat pasangan yang tak
/// pernah terlihat (hitungan 0) tetap punya skor terhingga, sekaligus meredam
/// pasangan berhitungan sangat kecil agar tidak terlalu percaya diri.
pub const SMOOTH: f32 = 0.5;

#[derive(Default)]
pub struct BigramModel {
    interner: HashMap<Box<str>, u32>,
    pairs: HashMap<u64, u32>,
}

impl BigramModel {
    /// Baca format TSV: `kiri<TAB>kanan<TAB>hitungan`, satu pasangan per baris.
    pub fn from_tsv(src: &str) -> Self {
        let mut m = BigramModel::default();
        for line in src.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split('\t');
            let (Some(a), Some(b), Some(c)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            let Ok(count) = c.trim().parse::<u32>() else {
                continue;
            };
            let (a, b) = (a.trim(), b.trim());
            if a.is_empty() || b.is_empty() {
                continue;
            }
            let ida = m.intern(a);
            let idb = m.intern(b);
            *m.pairs.entry(key(ida, idb)).or_insert(0) += count;
        }
        m.interner.shrink_to_fit();
        m.pairs.shrink_to_fit();
        m
    }

    fn intern(&mut self, w: &str) -> u32 {
        if let Some(&id) = self.interner.get(w) {
            return id;
        }
        let id = self.interner.len() as u32;
        self.interner.insert(w.to_lowercase().into_boxed_str(), id);
        id
    }

    fn id(&self, w: &str) -> Option<u32> {
        self.interner.get(w.to_lowercase().as_str()).copied()
    }

    /// Berapa kali `left` langsung diikuti `right` di korpus.
    pub fn cooc(&self, left: &str, right: &str) -> u32 {
        match (self.id(left), self.id(right)) {
            (Some(a), Some(b)) => self.pairs.get(&key(a, b)).copied().unwrap_or(0),
            _ => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Skor kecocokan sebuah kata dengan konteksnya, dalam log.
    ///
    /// Menjumlahkan bukti dari tetangga kiri (`prev → word`) dan kanan
    /// (`word → next`). Mengembalikan `(skor, bukti)` — `bukti` adalah berapa
    /// sisi yang benar-benar punya pasangan tercatat (0, 1, atau 2), dipakai
    /// pemanggil untuk tahu seberapa dalam konteks yang tersedia.
    pub fn context_score(&self, prev: Option<&str>, word: &str, next: Option<&str>) -> (f32, u8) {
        let mut score = 0.0;
        let mut evidence = 0u8;

        if let Some(p) = prev {
            let c = self.cooc(p, word);
            score += ((c as f32) + SMOOTH).ln();
            if c > 0 {
                evidence += 1;
            }
        }
        if let Some(n) = next {
            let c = self.cooc(word, n);
            score += ((c as f32) + SMOOTH).ln();
            if c > 0 {
                evidence += 1;
            }
        }
        (score, evidence)
    }
}

#[inline]
fn key(a: u32, b: u32) -> u64 {
    ((a as u64) << 32) | (b as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> BigramModel {
        BigramModel::from_tsv(
            "makan\tnasi\t30\n\
             makan\tsiang\t352\n\
             hari\tini\t4014\n\
             sangat\tterharu\t17\n\
             rasa\tharu\t15\n\
             versi\tterbaru\t65\n\
             model\tbaru\t44\n\
             saya\tmakan\t80\n",
        )
    }

    #[test]
    fn membaca_dan_mencari_pasangan() {
        let m = model();
        assert_eq!(m.cooc("makan", "nasi"), 30);
        assert_eq!(m.cooc("hari", "ini"), 4014);
        assert_eq!(m.cooc("makam", "nasi"), 0); // tak pernah terlihat
        assert_eq!(m.cooc("tidak", "ada"), 0);
    }

    #[test]
    fn pencarian_tak_peduli_besar_kecil() {
        let m = model();
        assert_eq!(m.cooc("Makan", "Nasi"), 30);
    }

    #[test]
    fn agregasi_saat_muat() {
        // Pasangan identik dijumlahkan.
        let m = BigramModel::from_tsv("a\tb\t3\na\tb\t5\n");
        assert_eq!(m.cooc("a", "b"), 8);
    }

    #[test]
    fn skor_konteks_memihak_kata_yang_pas() {
        let m = model();
        // "makan nasi" tercatat, "makam nasi" tidak → makan lebih tinggi.
        let (s_makan, ev_makan) = m.context_score(Some("saya"), "makan", Some("nasi"));
        let (s_makam, ev_makam) = m.context_score(Some("saya"), "makam", Some("nasi"));
        assert!(s_makan > s_makam, "makan={s_makan} makam={s_makam}");
        assert_eq!(ev_makan, 2, "makan didukung dua sisi");
        assert_eq!(ev_makam, 0, "makam tak didukung");
    }

    #[test]
    fn konteks_perasaan_memihak_terharu() {
        let m = model();
        let (s_terharu, _) = m.context_score(Some("sangat"), "terharu", None);
        let (s_haru, _) = m.context_score(Some("sangat"), "haru", None);
        assert!(s_terharu > s_haru, "terharu={s_terharu} haru={s_haru}");
    }

    #[test]
    fn tanpa_konteks_skor_netral_tanpa_bukti() {
        let m = model();
        let (_, evidence) = m.context_score(None, "apa", None);
        assert_eq!(evidence, 0);
    }

    #[test]
    fn model_kosong_aman() {
        let m = BigramModel::from_tsv("");
        assert!(m.is_empty());
        assert_eq!(m.cooc("a", "b"), 0);
        let (_, ev) = m.context_score(Some("a"), "b", Some("c"));
        assert_eq!(ev, 0);
    }
}
