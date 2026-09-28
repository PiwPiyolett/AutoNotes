//! Mesin koreksi typo bertingkat.
//!
//! ```text
//! teks masuk
//!    │
//!    ├─ tokenizer ──────────── zona terlindungi disingkirkan lebih dulu
//!    │
//!    ├─ T0  tabel ganti ────── `yg`→`yang`, `recieve`→`receive`   (~0 ms)
//!    │
//!    ├─ T1  kamus + SymSpell ─ typo bukan-kata: `mkaan`→`makan`   (<1 ms)
//!    │        └─ morfologi ─── "mengerjakannya" itu sah, jangan diutak-atik
//!    │
//!    └─ ranker ─────────────── memilih kandidat + menghitung keyakinan
//!             └─ T2 nanti mencolok di sini tanpa mengubah apa pun di atas
//! ```
//!
//! Setiap koreksi keluar dengan angka keyakinan. Yang tinggi diterapkan
//! otomatis; yang sedang hanya diberi garis bergelombang; yang rendah
//! dibuang diam-diam. Ambangnya diatur lewat [`Aggressiveness`].

pub mod bigram;
pub mod distance;
pub mod morphology;
pub mod qwerty;
pub mod ranker;
pub mod resources;
pub mod symspell;
pub mod table;
pub mod tokenizer;
pub mod userdict;

use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use bigram::BigramModel;
use ranker::{Context, FrequencyRanker, NgramRanker, Ranker, Suggestion};
use symspell::SymSpell;
use table::{match_case, Table};
use tokenizer::tokenize;
use userdict::UserDict;

/// Jumlah kata minimum sebelum sebuah kamus boleh dipakai untuk **menandai**
/// kata sebagai typo.
///
/// Ini katup pengaman yang tidak boleh dihapus. Dengan kamus rintisan yang
/// cuma berisi beberapa ratus kata, hampir setiap kata yang ditulis pengguna
/// akan dianggap "tidak dikenal" — dan aplikasi berubah jadi mesin perusak
/// teks. Selama kamus sungguhan belum dipasang, hanya lapisan T0 (tabel
/// ganti langsung) yang bekerja. Itu tetap berguna dan tidak pernah berbahaya.
pub const MIN_DICT_FOR_FLAGGING: usize = 5_000;

/// Frekuensi minimum sebuah kata agar boleh dipakai sebagai **kata dasar**
/// dalam analisis imbuhan.
///
/// Pengupas imbuhan menghasilkan banyak kemungkinan kata dasar, dan sebagian
/// di antaranya omong kosong. Kalau salah satu kebetulan cocok dengan sampah
/// korpus, seluruh kata dinyatakan sah dan typo-nya lolos.
///
/// Kasus nyata yang memicu ambang ini: `berlai` (typo dari `berlari`) dikupas
/// jadi `ber` + `lai`. Ternyata `lai` ada di korpus dengan frekuensi 187 —
/// potongan nama atau kata asing dari subtitle. Akibatnya `berlai` dinyatakan
/// sah dan tidak pernah diperbaiki.
///
/// Pemisahannya jelas di data: sampah tertinggi `lai` 187 dan `apu` 67,
/// sedangkan kata dasar sungguhan paling langka `gali` 526, `sapu` 551,
/// `karang` 576. Ambang 400 duduk di tengah jurang itu.
///
/// Catatan: ambang ini **hanya** berlaku untuk kata dasar hasil kupasan.
/// Kata yang persis ada di kamus tetap sah berapa pun frekuensinya.
///
/// Kalau nanti kamus KBBI terkurasi dipasang, ambang ini bisa diturunkan
/// jauh — masalahnya memang lahir dari memakai korpus mentah sebagai kamus.
pub const MIN_ROOT_FREQ: u32 = 400;

/// Batas frekuensi yang memisahkan kata "sah kuat" dari "sah lemah".
///
/// # Masalah yang diselesaikan: typo yang kebetulan kata nyata
///
/// `haru` ada di kamus (freq 199, artinya "terharu"), jadi dulu ia langsung
/// dinyatakan sah dan tidak pernah ditandai — padahal pengguna sering
/// memaksudkan `hari` (105.663) atau `harus` (260.853). Begitu pula `ino`
/// (sampah subtitle "Ino" di korpus Inggris, freq 194) menutup jalan koreksi
/// ke `ini`.
///
/// Kata yang frekuensinya di atas ambang ini dianggap **sah kuat** — jangan
/// diganggu. Yang di bawahnya **sah lemah**: masih bolih ditandai, tapi hanya
/// kalau ada tetangga yang **jauh** lebih umum (lihat [`REAL_WORD_RATIO`]),
/// dan tidak pernah diterapkan otomatis (lihat [`REAL_WORD_MAX_CONF`]).
///
/// # Kenapa ambangnya asimetris antar bahasa
///
/// Kedua kamus punya peran berbeda, jadi tingkat kepercayaannya beda:
///
/// * **Indonesia** adalah bahasa utama aplikasi. Kata yang ada di kamus ID
///   diperlakukan sah walau jarang — `sarat` (56), `basa` (125), `haru` (199),
///   `busa` (221) semuanya kata KBBI yang benar. Menandainya sebagai typo
///   adalah kesalahan yang langsung terasa mengganggu.
/// * **Inggris** dipakai sebagai pelengkap, dan korpusnya (subtitle) penuh
///   sampah: nama karakter, transliterasi, potongan kata. `ino` muncul 194×
///   hanya karena nama tokoh anime. Kata yang HANYA ada di sisi Inggris dengan
///   frekuensi rendah pantas dicurigai.
///
/// Ambang tunggal tidak bisa melayani keduanya: cukup rendah untuk melindungi
/// `sarat` berarti ikut melindungi `ino`; cukup tinggi untuk mencurigai `ino`
/// berarti ikut menuduh `sarat`.
pub const REAL_WORD_FLOOR_ID: u32 = 25;
pub const REAL_WORD_FLOOR_EN: u32 = 1000;

/// Seberapa jauh lebih umum sebuah usulan harus dibanding kata sah-lemah yang
/// diketik, sebelum layak ditandai. `haru` (199) → `harus` (260.853) lolos
/// telak; kata langka yang cuma sedikit lebih umum dari tetangganya tidak.
pub const REAL_WORD_RATIO: u32 = 150;

/// Keyakinan maksimum untuk koreksi kata-nyata. Sengaja di bawah ambang
/// otomatis paling agresif sekalipun (0.58): mengganti kata yang **sah**
/// secara otomatis adalah cara tercepat merusak tulisan orang, jadi kata-nyata
/// **selalu** hanya ditandai, tidak pernah diterapkan sendiri.
pub const REAL_WORD_MAX_CONF: f32 = 0.55;

/// Ambang-tandai khusus kata-nyata, terpisah dari setelan agresivitas.
///
/// Kata sah-langka yang punya tetangga [`REAL_WORD_RATIO`]× lebih umum sudah
/// cukup mencurigakan untuk ditandai, **meski** kita belum yakin penggantinya
/// yang mana. Contoh: `haru` bisa jadi `hari` atau `harus` — keduanya nyaris
/// seri sehingga keyakinannya rendah (0.32), tapi tetap layak diberi garis
/// bergelombang agar pengguna sadar. Tie-breaker sesungguhnya adalah konteks
/// kalimat (Tier 2).
pub const REAL_WORD_FLAG: f32 = 0.18;

/// Keunggulan skor konteks (log) yang harus dimiliki kandidat atas kata yang
/// diketik sebelum koreksi kata-nyata berbasis konteks (Tier 2) menyala.
///
/// Dengan pelicin [`bigram::SMOOTH`] = 0.5, satu tetangga yang tak pernah
/// terlihat menyumbang ln(0.5) ≈ −0.69, sedangkan pasangan yang muncul 30×
/// menyumbang ln(30.5) ≈ 3.4. Ambang 2.5 memastikan hanya perbedaan yang
/// benar-benar tegas ("makan nasi" vs "makam nasi") yang memicu penandaan.
pub const CONTEXT_MARGIN: f32 = 2.5;

/// Frekuensi minimum kandidat pengganti pada jalur konteks. Mencegah kata sah
/// ditarik ke kata langka lain hanya karena kebetulan sekali muncul bersebelahan.
pub const CONTEXT_MIN_CAND_FREQ: u32 = 200;

/// Ambang hitungan pasangan saat hanya satu tetangga tersedia (awal/akhir
/// kalimat). Satu sisi harus sekuat ini untuk berani menandai kata sah.
pub const CONTEXT_STRONG_PAIR: u32 = 20;

/// Status sebuah kata terhadap kamus — lihat [`Corrector::known_status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Known {
    /// Sah dan cukup umum. Jangan diganggu.
    Solid,
    /// Ada di kamus tapi langka; membawa frekuensinya untuk uji rasio.
    Weak(u32),
    /// Tak dikenal.
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Aggressiveness {
    /// Hanya memperbaiki yang nyaris pasti. Sisanya cuma digarisbawahi.
    Konservatif,
    #[default]
    Seimbang,
    /// Lebih banyak diperbaiki otomatis, risiko salah lebih besar.
    Agresif,
}

impl Aggressiveness {
    /// Ambang untuk **menerapkan otomatis**.
    pub fn auto_threshold(self) -> f32 {
        match self {
            Self::Konservatif => 0.88,
            Self::Seimbang => 0.72,
            Self::Agresif => 0.58,
        }
    }

    /// Ambang untuk sekadar **menandai** (garis bergelombang).
    pub fn flag_threshold(self) -> f32 {
        match self {
            Self::Konservatif => 0.45,
            Self::Seimbang => 0.35,
            Self::Agresif => 0.25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub aggressiveness: Aggressiveness,
    /// Bentangkan singkatan seperti `yg` → `yang`. Sebagian orang sengaja
    /// menyingkat, jadi ini bisa dimatikan terpisah dari koreksi typo.
    pub slang_expansion: bool,
    pub check_indonesian: bool,
    pub check_english: bool,
    pub max_edit: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            aggressiveness: Aggressiveness::default(),
            slang_expansion: true,
            check_indonesian: true,
            check_english: true,
            max_edit: 2,
        }
    }
}

/// Dari lapisan mana koreksi ini berasal. Ditampilkan di log koreksi supaya
/// pengguna paham kenapa sesuatu berubah.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Tabel typo — salah ketik yang tidak mungkin disengaja.
    Typo,
    /// Tabel singkatan/slang.
    Slang,
    /// Kamus + SymSpell.
    Dictionary,
    /// Tier 2 — kata sah yang tampak keliru menurut kalimatnya
    /// (mis. `makam` → `makan` pada "makan nasi").
    Context,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Correction {
    /// Posisi byte di dalam teks asal.
    pub start: usize,
    pub end: usize,
    pub from: String,
    pub to: String,
    pub confidence: f32,
    pub source: Source,
    /// Cukup yakin untuk diterapkan tanpa bertanya?
    pub auto: bool,
}

pub struct Corrector {
    typo: Table,
    slang: Table,
    id: SymSpell,
    en: SymSpell,
    ranker: Box<dyn Ranker>,
    /// Model bigram Tier 2. Kosong kalau data konteks belum dipasang; dalam hal
    /// itu koreksi berjalan tanpa konteks (persis perilaku Tier 1).
    bigram: Arc<BigramModel>,
    settings: RwLock<Settings>,
    userdict: RwLock<UserDict>,
}

impl Corrector {
    pub fn new(id: SymSpell, en: SymSpell, typo: Table, slang: Table, settings: Settings) -> Self {
        Self {
            typo,
            slang,
            id,
            en,
            ranker: Box::new(FrequencyRanker::default()),
            bigram: Arc::new(BigramModel::default()),
            settings: RwLock::new(settings),
            userdict: RwLock::new(UserDict::new()),
        }
    }

    /// Ganti pemeringkat secara manual — jarang dipakai langsung; lebih sering
    /// lewat [`Corrector::enable_context`].
    pub fn set_ranker(&mut self, ranker: Box<dyn Ranker>) {
        self.ranker = ranker;
    }

    /// Nyalakan Tier 2: pasang model bigram, lalu ganti pemeringkat ke
    /// [`NgramRanker`] yang memakainya. Setelah ini, koreksi memperhitungkan
    /// kata tetangga — "makam nasi" bisa jadi "makan nasi".
    pub fn enable_context(&mut self, model: BigramModel) {
        let shared = Arc::new(model);
        self.bigram = Arc::clone(&shared);
        self.ranker = Box::new(NgramRanker::new(shared));
    }

    /// Apakah konteks Tier 2 aktif (model bigram terpasang & tak kosong)?
    pub fn has_context(&self) -> bool {
        !self.bigram.is_empty()
    }

    /// Jumlah pasangan bigram — dipakai UI menampilkan status Tier 2.
    pub fn bigram_len(&self) -> usize {
        self.bigram.len()
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().clone()
    }

    pub fn set_settings(&self, s: Settings) {
        *self.settings.write() = s;
    }

    pub fn userdict(&self) -> &RwLock<UserDict> {
        &self.userdict
    }

    /// Berapa kata di tiap kamus — dipakai UI untuk memberi tahu pengguna
    /// kalau kamus lengkap belum terpasang.
    pub fn dict_sizes(&self) -> (usize, usize) {
        (self.id.len(), self.en.len())
    }

    /// Akses kamus mentah, untuk alat diagnosis (`explain_word`).
    pub fn dict_id(&self) -> &SymSpell {
        &self.id
    }

    pub fn dict_en(&self) -> &SymSpell {
        &self.en
    }

    /// Apakah kamus sudah cukup lengkap untuk berani menandai typo?
    ///
    /// Sengaja menuntut **semua** bahasa yang diaktifkan sudah siap. Kalau
    /// kamus Indonesia lengkap tapi Inggris masih rintisan, kata Inggris yang
    /// sah akan terlihat "tidak dikenal" dan ditarik paksa ke kata Indonesia
    /// terdekat — persis kegagalan yang paling merusak pada catatan campur
    /// dua bahasa.
    pub fn dictionaries_ready(&self) -> bool {
        let s = self.settings.read();
        let id_ready = !s.check_indonesian || self.id.len() >= MIN_DICT_FOR_FLAGGING;
        let en_ready = !s.check_english || self.en.len() >= MIN_DICT_FOR_FLAGGING;
        (s.check_indonesian || s.check_english) && id_ready && en_ready
    }

    /// Periksa teks, kembalikan daftar koreksi yang diusulkan.
    /// Tidak mengubah apa pun — penerapan dilakukan lewat [`apply`].
    pub fn check(&self, text: &str) -> Vec<Correction> {
        let settings = self.settings.read().clone();
        let userdict = self.userdict.read();
        let tokens = tokenize(text);
        let ready = self.dictionaries_ready();
        let mut out = Vec::new();

        for (i, tok) in tokens.iter().enumerate() {
            let word = tok.text(text);
            let lower = word.to_lowercase();

            // --- Reduplikasi angka-2: "anak2" -> "anak-anak" --------------
            // Dicek LEBIH DULU dari penjaga token-terlindungi, karena token
            // berangka ("anak2") normalnya dilindungi. Hanya menyala kalau
            // pembentangan slang aktif dan kata dasarnya sah — jadi "mp3",
            // "s2", "v2" tidak ikut terpengaruh.
            if settings.slang_expansion && ready {
                if let Some(expanded) = self.reduplication(&lower) {
                    out.push(Correction {
                        start: tok.start,
                        end: tok.end,
                        from: word.to_string(),
                        to: match_case(word, &expanded),
                        confidence: 0.95,
                        source: Source::Slang,
                        auto: true,
                    });
                    continue;
                }
            }

            if tok.is_protected() {
                continue;
            }

            if userdict.knows(&lower) {
                continue;
            }

            // --- T0: tabel ganti langsung ---------------------------------
            if let Some(fixed) = self.typo.get(word) {
                out.push(Correction {
                    start: tok.start,
                    end: tok.end,
                    from: word.to_string(),
                    to: fixed,
                    confidence: 1.0,
                    source: Source::Typo,
                    auto: true,
                });
                continue;
            }
            if settings.slang_expansion {
                if let Some(fixed) = self.slang.get(word) {
                    out.push(Correction {
                        start: tok.start,
                        end: tok.end,
                        from: word.to_string(),
                        to: fixed,
                        confidence: 0.95,
                        source: Source::Slang,
                        auto: true,
                    });
                    continue;
                }
            }

            // --- T1: kamus + SymSpell --------------------------------------
            if !ready {
                continue;
            }
            let status = self.known_status(&lower, &settings);

            let ctx = Context {
                prev: prev_word(&tokens, i, text),
                next: next_word(&tokens, i, text),
            };

            // --- T2: deteksi kata-nyata berbasis konteks ------------------
            // Kata yang valid (`Solid`) normalnya dilewati. Tapi typo yang
            // kebetulan kata sah — `makam` untuk `makan` — hanya bisa
            // ketahuan dari kalimatnya. Kalau model bigram kuat memihak
            // tetangga, tandai (tak pernah otomatis: mengganti kata sah
            // secara diam-diam terlalu berisiko).
            if status == Known::Solid {
                if let Some((to, conf)) = self.context_correction(&lower, &ctx, &settings) {
                    if !userdict.is_rejected(&lower, &to) {
                        out.push(Correction {
                            start: tok.start,
                            end: tok.end,
                            from: word.to_string(),
                            to: match_case(word, &to),
                            confidence: conf,
                            source: Source::Context,
                            auto: false,
                        });
                    }
                }
                continue;
            }

            // Dua jalur: kata sah-langka ("kata-nyata") vs kata tak dikenal.
            // Keduanya menghasilkan usulan + parameter penandaan yang berbeda.
            let (sug, confidence, allow_auto, flag_threshold) = match status {
                // --- Jalur kata-nyata -------------------------------------
                // Kata yang diketik sebenarnya sah (cuma langka). Koreksi hanya
                // ditandai — tidak pernah otomatis — dan lewat ambang rendah
                // khususnya, sebab menandai "kata langka ini punya kembaran
                // umum" berharga walau kita belum yakin penggantinya yang mana.
                // Contoh: `haru` → `harus`/`hari`.
                Known::Weak(_) => {
                    let Some(sug) = self.real_word_suggestion(&lower, &ctx, &settings) else {
                        continue;
                    };
                    let conf = sug.confidence.min(REAL_WORD_MAX_CONF);
                    let flag = settings.aggressiveness.flag_threshold().min(REAL_WORD_FLAG);
                    (sug, conf, false, flag)
                }
                // --- Jalur normal -----------------------------------------
                Known::No => {
                    let Some(sug) = self.best_suggestion(&lower, &ctx, &settings) else {
                        continue;
                    };
                    let conf = sug.confidence;
                    (sug, conf, true, settings.aggressiveness.flag_threshold())
                }
                Known::Solid => unreachable!("Solid sudah disaring di atas"),
            };

            if confidence < flag_threshold {
                continue;
            }
            if userdict.is_rejected(&lower, &sug.word) {
                continue;
            }

            out.push(Correction {
                start: tok.start,
                end: tok.end,
                from: word.to_string(),
                to: match_case(word, &sug.word),
                confidence,
                source: Source::Dictionary,
                auto: allow_auto && confidence >= settings.aggressiveness.auto_threshold(),
            });
        }

        out
    }

    /// Klasifikasi status sebuah kata terhadap kamus.
    ///
    /// Tiga hasil yang memicu perlakuan berbeda:
    ///
    /// * [`Known::Solid`] — sah dan cukup umum (atau sah lewat imbuhan). Jangan
    ///   diganggu sama sekali.
    /// * [`Known::Weak`] — ada di kamus tapi **langka** (di bawah
    ///   [`REAL_WORD_FLOOR`]). Kemungkinan besar sah, tapi bisa jadi typo yang
    ///   kebetulan kata nyata. Masih boleh ditandai bila ada tetangga yang jauh
    ///   lebih umum. Membawa frekuensinya sendiri untuk uji rasio.
    /// * [`Known::No`] — tak dikenal. Jalur koreksi biasa.
    ///
    /// Analisis imbuhan tetap memutuskan `Solid` (kata dasarnya wajib melewati
    /// [`MIN_ROOT_FREQ`]) — tanpa pagar itu, satu kecocokan kebetulan dengan
    /// sampah korpus meloloskan typo apa pun.
    fn known_status(&self, lower: &str, s: &Settings) -> Known {
        let mut weak_freq = 0u32;

        if s.check_indonesian && !self.id.is_empty() {
            let f = self.id.freq(lower);
            if f >= REAL_WORD_FLOOR_ID {
                return Known::Solid;
            }
            if morphology::is_valid(lower, &|w: &str| self.id.freq(w) >= MIN_ROOT_FREQ) {
                return Known::Solid;
            }
            weak_freq = weak_freq.max(f);
        }
        if s.check_english {
            let f = self.en.freq(lower);
            if f >= REAL_WORD_FLOOR_EN {
                return Known::Solid;
            }
            weak_freq = weak_freq.max(f);
        }

        if weak_freq > 0 {
            Known::Weak(weak_freq)
        } else {
            Known::No
        }
    }

    /// Reduplikasi angka-2 khas Indonesia: `anak2` → `anak-anak`.
    ///
    /// Hanya menyala kalau:
    ///   * kata diakhiri tepat satu digit `2` (bukan `mp3`, bukan `x22`),
    ///   * sisanya minimal 2 huruf alfabet (menyingkirkan `s2`, `v2`),
    ///   * kata dasar itu sah menurut kamus/imbuhan (menyingkirkan `ab2`).
    ///
    /// Mengembalikan bentuk berulang, atau `None` kalau bukan reduplikasi.
    fn reduplication(&self, lower: &str) -> Option<String> {
        let base = lower.strip_suffix('2')?;
        if base.chars().count() < 2 || !base.chars().all(|c| c.is_alphabetic()) {
            return None;
        }
        let sah = self.id.contains(base)
            || morphology::is_valid(base, &|w: &str| self.id.freq(w) >= MIN_ROOT_FREQ);
        if sah {
            Some(format!("{base}-{base}"))
        } else {
            None
        }
    }

    /// Deteksi kata-nyata berbasis konteks (Tier 2).
    ///
    /// Dipanggil untuk kata yang **valid** (`Known::Solid`). Mencari apakah ada
    /// tetangga-ejaan (jarak edit 1) yang jauh lebih pas dengan kalimatnya
    /// menurut model bigram. Contoh kanonis: pengguna mengetik "saya **makam**
    /// nasi" — `makam` sah, tapi "makan nasi" muncul ribuan kali di korpus
    /// sedangkan "makam nasi" tak pernah.
    ///
    /// # Kenapa syaratnya ketat
    ///
    /// Menandai kata yang sebenarnya benar itu mengganggu. Jadi ini hanya
    /// menyala kalau **semua** terpenuhi:
    ///
    /// * Model bigram terpasang, dan ada konteks (minimal satu tetangga).
    /// * Kata yang diketik nyaris **tak** didukung konteks (bukti 0).
    /// * Ada kandidat jarak-1 yang didukung konteks nyata (bukti ≥ 1) dan
    ///   unggul [`CONTEXT_MARGIN`] dalam skala log.
    /// * Kandidat itu kata umum (bukan sesama kata langka).
    ///
    /// Hasilnya **tak pernah** otomatis — hanya garis bergelombang.
    fn context_correction(&self, lower: &str, ctx: &Context, s: &Settings) -> Option<(String, f32)> {
        if self.bigram.is_empty() || !s.check_indonesian {
            return None;
        }
        // Kata terlalu pendek terlalu berisiko (banyak tetangga sah).
        if lower.chars().count() < 4 {
            return None;
        }
        // Perlu ada konteks sama sekali.
        if ctx.prev.is_none() && ctx.next.is_none() {
            return None;
        }

        // Kalau kata yang diketik sendiri sudah punya dukungan konteks, jangan
        // ganggu — ia kemungkinan besar memang kata yang dimaksud. Ini yang
        // menjaga "makam pahlawan" tetap aman.
        let (typed_score, typed_ev) = self.bigram.context_score(ctx.prev, lower, ctx.next);
        if typed_ev > 0 {
            return None;
        }

        let typed_freq = self.id.freq(lower);
        let both_sides = ctx.prev.is_some() && ctx.next.is_some();

        // Kandidat jarak-1 dari kamus Indonesia.
        let cands = self.id.lookup_candidates(lower, 1, 12);
        let mut best: Option<(f32, &str)> = None;
        for cand in &cands {
            if cand.distance == 0 || cand.word == lower {
                continue;
            }
            // Koreksi kata-nyata bergerak dari kata langka ke kata yang lebih
            // umum. Ini memblokir `nasi`(911) → `napi`(343): mengganti kata
            // umum dengan kata yang lebih langka hampir selalu salah.
            if cand.freq < CONTEXT_MIN_CAND_FREQ || cand.freq < typed_freq {
                continue;
            }

            let left = ctx.prev.map(|p| self.bigram.cooc(p, &cand.word)).unwrap_or(0);
            let right = ctx.next.map(|n| self.bigram.cooc(&cand.word, n)).unwrap_or(0);

            // Bukti yang dituntut sengaja tinggi supaya kata benar tidak
            // dikorbankan:
            //   * kalau dua tetangga tersedia, kandidat wajib didukung KEDUANYA
            //     ("saya makan" DAN "makan nasi") — inilah yang memblokir
            //     `beli`→`beri`, sebab "beri koran" tak pernah muncul meski
            //     "tolong beri" sering;
            //   * kalau hanya satu tetangga (awal/akhir kalimat), sisi itu
            //     harus sangat kuat.
            let cukup = if both_sides {
                left > 0 && right > 0
            } else {
                left.max(right) >= CONTEXT_STRONG_PAIR
            };
            if !cukup {
                continue;
            }

            let (score, _) = self.bigram.context_score(ctx.prev, &cand.word, ctx.next);
            if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
                best = Some((score, &cand.word));
            }
        }

        let (score, word) = best?;
        let margin = score - typed_score;
        if margin < CONTEXT_MARGIN {
            return None;
        }

        // Keyakinan naik bersama margin, tapi dijaga di bawah ambang otomatis:
        // koreksi kata-nyata selalu hanya ditandai.
        let confidence = (0.35 + 0.10 * (margin - CONTEXT_MARGIN)).clamp(0.0, REAL_WORD_MAX_CONF);
        Some((word.to_string(), confidence))
    }

    /// Ambil usulan terbaik dari tiap kamus lalu pilih pemenangnya.
    ///
    /// # Kenapa dibandingkan lewat skor, bukan keyakinan
    ///
    /// Keyakinan mengukur "seberapa unggul jawaban ini atas pesaing **di
    /// kamus yang sama**". Angka itu tidak berarti apa-apa lintas kamus:
    /// kandidat buruk yang kebetulan tidak punya pesaing bisa terlihat lebih
    /// yakin daripada kandidat bagus yang pesaingnya ramai.
    ///
    /// Kasus nyata: `finsh`. Kamus Inggris menempatkan `finish` di puncak
    /// (skor 6,22), kamus Indonesia menempatkan `finch` (skor 2,16 — korpus
    /// subtitle Indonesia memang berisi kata Inggris). Karena `finch` punya
    /// margin lebih lebar di kamusnya sendiri, keyakinannya lebih tinggi, dan
    /// jawaban yang salah menang.
    ///
    /// Skor noisy-channel bisa diadu lintas kamus justru karena
    /// [`super::symspell::Candidate::log_prob`] sudah dinormalkan jadi peluang.
    fn best_suggestion(&self, lower: &str, ctx: &Context, s: &Settings) -> Option<Suggestion> {
        let mut best: Option<(f32, Suggestion)> = None;
        for (enabled, dict) in [(s.check_indonesian, &self.id), (s.check_english, &self.en)] {
            if !enabled || dict.len() < MIN_DICT_FOR_FLAGGING {
                continue;
            }
            let cands = dict.lookup_candidates(lower, s.max_edit, 8);
            let Some(top) = cands.first() else { continue };
            let score = top.score();
            let Some(sug) = self.ranker.pick(lower, &cands, ctx) else {
                continue;
            };
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, sug));
            }
        }
        best.map(|(_, sug)| sug)
    }

    /// Usulan untuk kata sah-langka (jalur kata-nyata). Berbeda dari
    /// [`Corrector::best_suggestion`] dalam dua hal penting:
    ///
    /// 1. **Prioritas Bahasa Indonesia.** Kalau kata muncul di kamus ID
    ///    (sekecil apa pun frekuensinya), hanya usulan dari kamus ID yang
    ///    diterima — Bahasa Indonesia "mengklaim" kata itu. Ini melindungi kata
    ///    ID sah yang kebetulan mirip kata Inggris umum: `surel` (email, ID 197
    ///    / EN 0) tidak boleh berubah jadi `surely` hanya karena `surely` lebih
    ///    sering di korpus Inggris. Kata yang sama sekali tidak ada di kamus ID
    ///    (mis. `ino`, ID 0 / EN 194 — sampah subtitle) baru boleh diarahkan
    ///    lintas bahasa ke kata ID umum (`ini`).
    ///
    /// 2. **Kecocokan-diri dibuang & uji rasio per-kamus.** Kandidat jarak 0
    ///    (kata itu sendiri) dilewati karena kita mencari alternatif, dan usulan
    ///    wajib [`REAL_WORD_RATIO`]× lebih umum dari kata yang diketik *di kamus
    ///    yang sama*.
    fn real_word_suggestion(&self, lower: &str, ctx: &Context, s: &Settings) -> Option<Suggestion> {
        let claimed_by_id = s.check_indonesian && self.id.freq(lower) > 0;

        for (is_english, enabled, dict) in [
            (false, s.check_indonesian, &self.id),
            (true, s.check_english, &self.en),
        ] {
            if !enabled || dict.len() < MIN_DICT_FOR_FLAGGING {
                continue;
            }
            // ID sudah mengklaim kata ini → jangan terima usulan lintas bahasa.
            if is_english && claimed_by_id {
                continue;
            }
            let typed_freq = dict.freq(lower).max(1);
            // Hanya jarak 1: typo yang kebetulan kata sah nyaris selalu meleset
            // satu huruf saja (`haru`↔`hari`/`harus`). Membuka ke jarak 2
            // memasukkan tebakan liar seperti `haru`→`dari` yang jelas keliru.
            let mut cands = dict.lookup_candidates(lower, 1, 8);
            cands.retain(|c| c.distance > 0);
            if cands.is_empty() {
                continue;
            }
            if let Some(sug) = self.ranker.pick(lower, &cands, ctx) {
                if (sug.freq as u64) >= (typed_freq as u64) * REAL_WORD_RATIO as u64 {
                    return Some(sug);
                }
            }
        }
        None
    }
}

fn prev_word<'a>(tokens: &[tokenizer::Token], i: usize, text: &'a str) -> Option<&'a str> {
    tokens[..i].iter().rev().find(|t| !t.is_protected()).map(|t| t.text(text))
}

fn next_word<'a>(tokens: &[tokenizer::Token], i: usize, text: &'a str) -> Option<&'a str> {
    tokens.get(i + 1..)?.iter().find(|t| !t.is_protected()).map(|t| t.text(text))
}

/// Terapkan koreksi ke teks. Rentang yang tumpang tindih dilewati.
pub fn apply(text: &str, corrections: &[Correction]) -> String {
    let mut sorted: Vec<&Correction> = corrections.iter().collect();
    sorted.sort_by_key(|c| c.start);

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for c in sorted {
        if c.start < cursor || c.end > text.len() {
            continue;
        }
        out.push_str(&text[cursor..c.start]);
        out.push_str(&c.to);
        cursor = c.end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Terapkan hanya koreksi yang cukup yakin — ini yang dipakai saat mengetik.
pub fn apply_auto(text: &str, corrections: &[Correction]) -> String {
    let auto: Vec<Correction> = corrections.iter().filter(|c| c.auto).cloned().collect();
    apply(text, &auto)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kamus rintisan yang sengaja dibuat melewati [`MIN_DICT_FOR_FLAGGING`]
    /// dengan kata boneka, supaya jalur T1 ikut teruji.
    fn dict_besar(nyata: &[(&str, u32)]) -> SymSpell {
        let mut kata: Vec<(String, u32)> =
            nyata.iter().map(|(w, f)| (w.to_string(), *f)).collect();
        // Pengganjal: kata acak yang tidak akan pernah mirip dengan uji coba.
        for i in 0..MIN_DICT_FOR_FLAGGING {
            kata.push((format!("qqzz{i}"), 1));
        }
        SymSpell::build(kata, 2, 7)
    }

    fn corrector() -> Corrector {
        let id = dict_besar(&[
            ("makan", 9000),
            ("makam", 200),
            ("saya", 20000),
            ("nasi", 5000),
            ("catatan", 4000),
            ("rapat", 3000),
            ("kerja", 8000),
            ("tulis", 4000),
            ("cepat", 6000),
            ("penting", 5000),
            ("ini", 30000),
        ]);
        let en = dict_besar(&[
            ("the", 50000),
            ("meeting", 4000),
            ("deadline", 2000),
            ("receive", 1500),
        ]);
        let typo = Table::from_tsv("yagn\tyang\nrecieve\treceive\nsya\tsaya\n");
        let slang = Table::from_tsv("yg\tyang\nbgt\tbanget\n");
        Corrector::new(id, en, typo, slang, Settings::default())
    }

    fn perbaiki(c: &Corrector, teks: &str) -> String {
        apply(teks, &c.check(teks))
    }

    #[test]
    fn tabel_typo_bekerja() {
        let c = corrector();
        assert_eq!(perbaiki(&c, "yagn penting"), "yang penting");
    }

    #[test]
    fn slang_dibentangkan_saat_aktif() {
        let c = corrector();
        assert_eq!(perbaiki(&c, "ini bgt"), "ini banget");
    }

    #[test]
    fn slang_dibiarkan_saat_dimatikan() {
        let c = corrector();
        c.set_settings(Settings { slang_expansion: false, ..Settings::default() });
        assert_eq!(perbaiki(&c, "ini bgt"), "ini bgt");
    }

    #[test]
    fn typo_tetap_diperbaiki_meski_slang_dimatikan() {
        let c = corrector();
        c.set_settings(Settings { slang_expansion: false, ..Settings::default() });
        assert_eq!(perbaiki(&c, "yagn penting"), "yang penting");
    }

    #[test]
    fn typo_kamus_diperbaiki() {
        let c = corrector();
        assert_eq!(perbaiki(&c, "saya mkaan nasi"), "saya makan nasi");
    }

    #[test]
    fn kata_benar_tidak_disentuh() {
        let c = corrector();
        let teks = "saya makan nasi cepat";
        assert_eq!(perbaiki(&c, teks), teks);
        assert!(c.check(teks).is_empty());
    }

    #[test]
    fn kata_berimbuhan_tidak_dirusak() {
        let c = corrector();
        // "mengerjakan" tidak ada di kamus, tapi kata dasarnya ada.
        let teks = "sedang mengerjakan catatan";
        assert!(c.check(teks).is_empty(), "koreksi: {:?}", c.check(teks));
    }

    #[test]
    fn zona_terlindungi_dilewati_sepenuhnya() {
        let c = corrector();
        let teks = "buka https://mkaan.com dan `mkaan()` lalu #mkaan";
        assert!(c.check(teks).is_empty(), "koreksi: {:?}", c.check(teks));
    }

    #[test]
    fn kapitalisasi_awal_kalimat_dipertahankan() {
        let c = corrector();
        assert_eq!(perbaiki(&c, "Mkaan nasi"), "Makan nasi");
    }

    #[test]
    fn campur_indonesia_inggris_keduanya_aman() {
        let c = corrector();
        let teks = "saya rapat the meeting deadline";
        assert!(c.check(teks).is_empty(), "koreksi: {:?}", c.check(teks));
    }

    #[test]
    fn kamus_pribadi_melindungi_kata() {
        let c = corrector();
        c.userdict().write().add_word("mkaan");
        assert_eq!(perbaiki(&c, "saya mkaan"), "saya mkaan");
    }

    #[test]
    fn koreksi_yang_dibatalkan_dua_kali_berhenti_diusulkan() {
        let c = corrector();
        assert!(!c.check("saya mkaan").is_empty());
        {
            let mut d = c.userdict().write();
            d.record_undo("mkaan", "makan");
            d.record_undo("mkaan", "makan");
        }
        assert!(c.check("saya mkaan").is_empty());
    }

    #[test]
    fn katup_pengaman_mencegah_kamus_rintisan_merusak_teks() {
        // Kamus mungil: T1 harus mati total, T0 tetap jalan.
        let kecil = Corrector::new(
            SymSpell::build([("makan".to_string(), 100u32)], 2, 7),
            SymSpell::build([("the".to_string(), 100u32)], 2, 7),
            Table::from_tsv("yagn\tyang\n"),
            Table::default(),
            Settings::default(),
        );
        assert!(!kecil.dictionaries_ready());
        // Kata asing dibiarkan, tidak ditarik paksa ke "makan".
        assert_eq!(perbaiki(&kecil, "xyzabc kata"), "xyzabc kata");
        // Tapi tabel typo tetap bekerja.
        assert_eq!(perbaiki(&kecil, "yagn itu"), "yang itu");
    }

    #[test]
    fn agresivitas_mengubah_perilaku_penerapan_otomatis() {
        let c = corrector();
        let teks = "saya mkaan";

        c.set_settings(Settings { aggressiveness: Aggressiveness::Agresif, ..Default::default() });
        let agresif = c.check(teks);

        c.set_settings(Settings { aggressiveness: Aggressiveness::Konservatif, ..Default::default() });
        let konservatif = c.check(teks);

        // Sama-sama terdeteksi, tapi yang konservatif lebih enggan menerapkan.
        assert_eq!(agresif.len(), konservatif.len());
        assert!(agresif.iter().filter(|x| x.auto).count() >= konservatif.iter().filter(|x| x.auto).count());
    }

    #[test]
    fn apply_auto_hanya_menerapkan_yang_yakin() {
        let c = corrector();
        let teks = "yagn mkaan";
        let mut koreksi = c.check(teks);
        for k in &mut koreksi {
            if k.source == Source::Dictionary {
                k.auto = false;
            }
        }
        // Hanya koreksi tabel yang diterapkan.
        assert_eq!(apply_auto(teks, &koreksi), "yang mkaan");
    }

    #[test]
    fn posisi_byte_tetap_benar_dengan_huruf_beraksen() {
        let c = corrector();
        let teks = "kafé mkaan";
        let k = c.check(teks);
        assert_eq!(k.len(), 1);
        assert_eq!(&teks[k[0].start..k[0].end], "mkaan");
        assert_eq!(apply(teks, &k), "kafé makan");
    }

    #[test]
    fn beberapa_koreksi_dalam_satu_kalimat() {
        let c = corrector();
        assert_eq!(perbaiki(&c, "yagn sya mkaan"), "yang saya makan");
    }

    #[test]
    fn teks_kosong_aman() {
        let c = corrector();
        assert!(c.check("").is_empty());
        assert_eq!(apply("", &[]), "");
    }
}
