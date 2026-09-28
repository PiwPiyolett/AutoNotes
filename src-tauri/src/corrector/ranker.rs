//! Pemeringkat kandidat — **titik colok untuk Tier 2**.
//!
//! SymSpell menghasilkan *daftar* kandidat. Yang memutuskan mana yang dipakai
//! (dan seberapa yakin) adalah implementasi [`Ranker`] di sini.
//!
//! Pemisahan ini yang membuat peningkatan ke koreksi berbasis konteks nanti
//! tidak menyentuh sisa kode sama sekali:
//!
//! ```ignore
//! // sekarang — memilih berdasarkan frekuensi kata
//! let ranker = Box::new(FrequencyRanker::default());
//!
//! // nanti — memilih berdasarkan kata sebelum & sesudahnya
//! let ranker = Box::new(NgramRanker::load("id.arpa")?);
//! ```
//!
//! Bedanya nyata: `FrequencyRanker` tidak akan pernah bisa memperbaiki
//! "saya makam nasi" → "saya makan nasi", karena "makam" itu kata yang sah.
//! Pemeringkat berkonteks bisa, karena ia tahu "makam nasi" praktis tidak
//! pernah muncul sementara "makan nasi" sangat sering.

use std::sync::Arc;

use super::bigram::BigramModel;
use super::symspell::Candidate;

/// Kata di sekitar token yang sedang diperiksa. `FrequencyRanker` mengabaikan
/// semuanya; pemeringkat berkonteks nanti akan memakainya.
#[derive(Debug, Default, Clone)]
pub struct Context<'a> {
    pub prev: Option<&'a str>,
    pub next: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub word: String,
    /// 0.0–1.0. Ambang penerapan otomatis ditentukan oleh setelan agresivitas.
    pub confidence: f32,
    pub distance: usize,
    /// Frekuensi kata yang diusulkan. Dipakai lapisan di atas untuk menimbang
    /// koreksi kata-nyata: kata sah yang langka hanya boleh ditandai kalau
    /// usulannya jauh lebih umum.
    pub freq: u32,
}

pub trait Ranker: Send + Sync {
    /// Pilih satu kandidat terbaik, atau `None` kalau tidak ada yang layak.
    ///
    /// `cands` sudah terurut dari yang paling mungkin (biaya menaik,
    /// frekuensi menurun).
    fn pick(&self, original: &str, cands: &[Candidate], ctx: &Context) -> Option<Suggestion>;

    fn name(&self) -> &'static str;
}

/// Tier 1 — memilih murni dari jarak edit berbobot dan frekuensi kata.
pub struct FrequencyRanker {
    /// Frekuensi minimum agar sebuah kata boleh diusulkan. Menyaring kata
    /// langka yang kebetulan mirip.
    pub min_freq: u32,
}

impl Default for FrequencyRanker {
    fn default() -> Self {
        Self { min_freq: 1 }
    }
}

impl Ranker for FrequencyRanker {
    fn name(&self) -> &'static str {
        "frekuensi"
    }

    fn pick(&self, original: &str, cands: &[Candidate], _ctx: &Context) -> Option<Suggestion> {
        let best = cands.first()?;
        if best.distance == 0 {
            return None; // kata sudah ada persis di kamus
        }
        if best.freq < self.min_freq {
            return None;
        }
        let len = original.chars().count();
        if len <= 2 {
            return None; // kata satu-dua huruf terlalu berbahaya ditebak
        }

        // Skor pemeringkat = skor noisy-channel murni (tanpa konteks).
        let best_score = best.score();
        let second = cands.get(1);
        let second_score = second.map(|c| c.score());
        let confidence = confidence_from(len, best, second, best_score, second_score);

        Some(Suggestion {
            word: best.word.clone(),
            confidence,
            distance: best.distance,
            freq: best.freq,
        })
    }
}

/// Hitung keyakinan dari kandidat terurut (terbaik dulu).
///
/// Dipisah agar [`FrequencyRanker`] dan [`NgramRanker`] memakai matematika yang
/// sama; bedanya hanya pada **skor** yang dipakai memeringkat — frekuensi murni
/// vs frekuensi + konteks.
///
/// Tiga bukti dikalikan:
/// 1. **Porsi kata yang diubah** — biaya dinormalkan panjang. Satu huruf dari
///    kata 10 huruf = bukti kuat; satu huruf dari kata 3 huruf = nyaris tak
///    berarti.
/// 2. **Keunggulan atas pesaing** — selisih skor pemenang vs kedua, dalam log.
/// 3. **Penalti kata pendek & kata langka.**
fn confidence_from(
    len: usize,
    best: &Candidate,
    second: Option<&Candidate>,
    best_score: f32,
    second_score: Option<f32>,
) -> f32 {
    let ratio = best.cost / len as f32;
    let mut confidence = (1.0 - ratio * 1.5).clamp(0.0, 1.0);

    if let (Some(_), Some(ss)) = (second, second_score) {
        let margin = best_score - ss;
        confidence *= 0.45 + 0.55 * (margin / 3.0).clamp(0.0, 1.0);
    }

    // Kata tiga huruf: banyak kata sah berjarak satu edit. Penalti sedang
    // (0.82) — cukup untuk kehati-hatian tanpa membunuh `ino`/`inu` → `ini`.
    if len == 3 {
        confidence *= 0.82;
    }
    if best.freq < 100 {
        confidence *= 0.85;
    }

    confidence.clamp(0.0, 1.0)
}

/// Tier 2 — memilih kandidat dengan mempertimbangkan kata tetangga.
///
/// Perbedaannya dengan [`FrequencyRanker`] hanya satu: skor tiap kandidat
/// ditambah bonus konteks dari model bigram sebelum diperingkat. Itu cukup
/// untuk membalik keputusan pada kasus yang membuat frekuensi murni tak
/// berdaya — "saya makam nasi" jadi "makan" karena "makan nasi" jauh lebih
/// sering muncul daripada "makam nasi".
///
/// Sisa pipa (tokenizer, kamus, morfologi, penyimpanan) tidak berubah sama
/// sekali: cukup tukar `Box<dyn Ranker>` di [`super::Corrector`].
pub struct NgramRanker {
    model: Arc<BigramModel>,
    /// Bobot konteks relatif terhadap skor frekuensi. 0 = sama dengan
    /// FrequencyRanker; makin besar makin percaya pada tetangga.
    ctx_weight: f32,
}

impl NgramRanker {
    pub fn new(model: Arc<BigramModel>) -> Self {
        Self { model, ctx_weight: 1.6 }
    }

    fn scored(&self, cand: &Candidate, ctx: &Context) -> f32 {
        let (ctx_score, _) = self.model.context_score(ctx.prev, &cand.word, ctx.next);
        cand.score() + self.ctx_weight * ctx_score
    }
}

impl Ranker for NgramRanker {
    fn name(&self) -> &'static str {
        "ngram"
    }

    fn pick(&self, original: &str, cands: &[Candidate], ctx: &Context) -> Option<Suggestion> {
        if cands.is_empty() {
            return None;
        }
        let len = original.chars().count();
        if len <= 2 {
            return None;
        }

        // Peringkat ulang berdasar skor + konteks.
        let mut ranked: Vec<(&Candidate, f32)> =
            cands.iter().map(|c| (c, self.scored(c, ctx))).collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (best, best_score) = ranked[0];
        if best.distance == 0 {
            return None; // kata sudah pas
        }
        let (second, second_score) = match ranked.get(1) {
            Some(&(c, s)) => (Some(c), Some(s)),
            None => (None, None),
        };

        let confidence = confidence_from(len, best, second, best_score, second_score);
        Some(Suggestion {
            word: best.word.clone(),
            confidence,
            distance: best.distance,
            freq: best.freq,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Total frekuensi kamus tiruan, penyebut untuk log-peluang.
    const TOTAL: f32 = 1_000_000.0;

    fn c(word: &str, distance: usize, cost: f32, freq: u32) -> Candidate {
        Candidate {
            word: word.into(),
            distance,
            cost,
            freq,
            log_prob: (freq.max(1) as f32 / TOTAL).ln(),
        }
    }

    fn pick(original: &str, cands: &[Candidate]) -> Option<Suggestion> {
        FrequencyRanker::default().pick(original, cands, &Context::default())
    }

    #[test]
    fn kata_yang_sudah_benar_tidak_diusulkan() {
        assert!(pick("makan", &[c("makan", 0, 0.0, 9000)]).is_none());
    }

    #[test]
    fn daftar_kosong_aman() {
        assert!(pick("makan", &[]).is_none());
    }

    #[test]
    fn kandidat_tunggal_yang_jelas_dapat_keyakinan_tinggi() {
        let s = pick("makanx", &[c("makan", 1, 1.0, 9000)]).unwrap();
        assert_eq!(s.word, "makan");
        assert!(s.confidence > 0.6, "confidence={}", s.confidence);
    }

    #[test]
    fn dua_kandidat_yang_bersaing_menurunkan_keyakinan() {
        let jelas = pick("makanx", &[c("makan", 1, 1.0, 9000)]).unwrap();
        let ambigu = pick(
            "makanx",
            &[c("makan", 1, 1.0, 9000), c("makam", 1, 1.0, 8500)],
        )
        .unwrap();
        assert!(
            ambigu.confidence < jelas.confidence,
            "ambigu={} jelas={}",
            ambigu.confidence,
            jelas.confidence
        );
    }

    #[test]
    fn kata_sangat_pendek_ditolak_mentah() {
        assert!(pick("ak", &[c("aku", 1, 1.0, 9000)]).is_none());
    }

    #[test]
    fn kata_tiga_huruf_diperlakukan_hati_hati() {
        let pendek = pick("mkn", &[c("makan", 2, 2.0, 9000)]);
        // Boleh saja mengusulkan, tapi keyakinannya harus rendah.
        if let Some(s) = pendek {
            assert!(s.confidence < 0.45, "confidence={}", s.confidence);
        }
    }

    #[test]
    fn jarak_dua_kurang_meyakinkan_dari_jarak_satu() {
        let dekat = pick("mengerjakn", &[c("mengerjakan", 1, 1.0, 900)]).unwrap();
        let jauh = pick("mengerjkn", &[c("mengerjakan", 2, 2.0, 900)]).unwrap();
        assert!(jauh.confidence < dekat.confidence);
    }

    #[test]
    fn kata_langka_diturunkan_keyakinannya() {
        let umum = pick("catatanx", &[c("catatan", 1, 1.0, 4000)]).unwrap();
        let langka = pick("catatanx", &[c("catatan", 1, 1.0, 2)]).unwrap();
        assert!(langka.confidence < umum.confidence);
    }

    #[test]
    fn transposisi_lebih_dipercaya_daripada_huruf_asing() {
        // Biaya rendah (khas transposisi) harus menghasilkan keyakinan tinggi.
        let tertukar = pick("mkaan", &[c("makan", 1, 0.45, 9000)]).unwrap();
        let huruf_jauh = pick("mqkan", &[c("makan", 1, 1.0, 9000)]).unwrap();
        assert!(tertukar.confidence > huruf_jauh.confidence);
    }
}
