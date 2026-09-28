//! Pemuat kamus & tabel.
//!
//! Tabel T0 **ditanam langsung ke dalam biner** lewat `include_str!` — ukurannya
//! hanya puluhan kilobyte dan dengan begitu aplikasi tidak akan pernah gagal
//! mengoreksi typo dasar hanya karena sebuah berkas hilang.
//!
//! Kamus besar (ratusan ribu kata) sebaliknya dibaca dari disk saat aplikasi
//! dibuka, supaya bisa diperbarui tanpa mengompilasi ulang dan supaya
//! pengguna bisa memasang kamus KBBI/Hunspell lengkap sendiri.

use std::fs;
use std::path::Path;

use super::symspell::SymSpell;
use super::table::Table;
use super::{Corrector, Settings};

pub const TYPO_ID: &str = include_str!("../../resources/dict/typo_id.tsv");
pub const TYPO_EN: &str = include_str!("../../resources/dict/typo_en.tsv");
pub const SLANG_ID: &str = include_str!("../../resources/dict/slang_id.tsv");

/// Nama berkas kamus yang dicari di dalam direktori kamus.
pub const WORDS_ID: &str = "words_id.txt";
pub const WORDS_EN: &str = "words_en.txt";

/// Kamus pelengkap Bahasa Indonesia: kosakata administratif, akademik, dan
/// teknologi yang tidak tercakup korpus subtitle. Opsional — kalau berkasnya
/// tidak ada, aplikasi tetap jalan dengan kamus utama saja.
pub const WORDS_ID_EXTRA: &str = "words_id_extra.txt";

/// Tabel typo gabungan (Indonesia + Inggris) — selalu aktif.
pub fn typo_table() -> Table {
    let mut t = Table::from_tsv(TYPO_ID);
    t.extend_tsv(TYPO_EN);
    t
}

/// Tabel singkatan/slang — bisa dimatikan pengguna.
pub fn slang_table() -> Table {
    Table::from_tsv(SLANG_ID)
}

/// Frekuensi minimum agar sebuah kata diterima masuk kamus.
///
/// Daftar frekuensi dibuat dari korpus nyata, bukan kamus terkurasi — jadi
/// ekornya penuh sampah: salah ketik penulis subtitle, potongan nama, dan
/// kata asing yang muncul sekali dua kali. Sampah itu berbahaya bukan karena
/// ia dianggap sah, melainkan karena ia menjadi **sasaran koreksi**: kata
/// aneh berfrekuensi 17 bisa mengalahkan kata benar yang jaraknya sedikit
/// lebih jauh.
///
/// Nilai 25 menyisakan sekitar 44rb dari 50rb kata — masih jauh di atas
/// [`super::MIN_DICT_FOR_FLAGGING`], tapi memangkas ekor yang paling berisik.
pub const MIN_WORD_FREQ: u32 = 25;

/// Baca daftar kata dari berkas teks.
///
/// Format yang diterima:
///   * `kata<TAB>frekuensi` atau `kata<SPASI>frekuensi` — dipakai apa adanya
///   * `kata` saja — frekuensi diturunkan dari urutan baris, dengan asumsi
///     berkas sudah terurut dari yang paling sering
///
/// Baris kosong dan baris diawali `#` diabaikan.
pub fn parse_wordlist(src: &str) -> Vec<(String, u32)> {
    let total = src.lines().count() as u32;
    let mut out = Vec::new();
    for (rank, line) in src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Peringkat jadi cadangan kalau kolom frekuensi tidak ada / rusak.
        let by_rank = total.saturating_sub(rank as u32);
        let mut parts = line.split_whitespace();
        let Some(word) = parts.next() else { continue };
        let freq = parts
            .next()
            .and_then(|f| f.parse::<u32>().ok())
            .unwrap_or(by_rank);
        out.push((word.to_string(), freq.max(1)));
    }
    out
}

/// Buang kata yang tidak layak menghuni kamus.
///
/// Dua saringan:
///
/// 1. **Frekuensi minimum** — memangkas ekor korpus yang penuh sampah.
/// 2. **Daftar cekal dari tabel typo** — kalau kita sudah menyatakan `yagn`
///    dan `disini` itu bentuk salah, keduanya tidak boleh ada di kamus.
///    Tanpa ini, keduanya jadi sasaran koreksi: `yagnn` akan "diperbaiki"
///    menjadi `yagn` (jarak 1) alih-alih `yang` (jarak 2). Ini bukan kasus
///    hipotetis — `disini` muncul 55.392 kali di korpus.
///
/// Kunci tabel **slang** sengaja dibiarkan. `yg` memang kata yang orang tulis
/// dengan sadar; kalau pembentangan slang dimatikan, `yg` harus tetap
/// terhitung sah, bukan malah ditandai typo.
pub fn filter_wordlist(
    words: Vec<(String, u32)>,
    min_freq: u32,
    banned: &Table,
) -> Vec<(String, u32)> {
    words
        .into_iter()
        .filter(|(w, f)| *f >= min_freq && !banned.has_key(w))
        .collect()
}

fn load_wordlist(dir: &Path, name: &str, banned: &Table) -> Vec<(String, u32)> {
    match fs::read_to_string(dir.join(name)) {
        Ok(src) => filter_wordlist(parse_wordlist(&src), MIN_WORD_FREQ, banned),
        Err(_) => Vec::new(),
    }
}

/// Rakit mesin koreksi lengkap.
///
/// `dict_dir` boleh menunjuk ke direktori yang belum berisi kamus apa pun —
/// aplikasi tetap jalan, hanya lapisan T1 yang tidur sampai kamus terpasang
/// (lihat [`super::MIN_DICT_FOR_FLAGGING`]).
/// Nama berkas model bigram Tier 2 (opsional).
pub const BIGRAMS_ID: &str = "bigrams_id.tsv";

pub fn build_corrector(dict_dir: Option<&Path>, settings: Settings) -> Corrector {
    let typo = typo_table();
    let (id_words, en_words) = match dict_dir {
        Some(dir) => {
            let mut id = load_wordlist(dir, WORDS_ID, &typo);
            // Gabung kamus pelengkap. SymSpell::build sudah menangani kata
            // ganda dengan mengambil frekuensi tertinggi, jadi entri yang
            // sudah ada di kamus utama tidak tertimpa nilai sintetis.
            id.extend(load_wordlist(dir, WORDS_ID_EXTRA, &typo));
            (id, load_wordlist(dir, WORDS_EN, &typo))
        }
        None => (Vec::new(), Vec::new()),
    };

    let mut corrector = Corrector::new(
        SymSpell::build(id_words, 2, 7),
        SymSpell::build(en_words, 2, 7),
        typo,
        slang_table(),
        settings,
    );

    // Tier 2: nyalakan konteks kalau model bigram ada di direktori kamus.
    // Kalau tidak ada, koreksi tetap berjalan tanpa konteks (Tier 1).
    if let Some(dir) = dict_dir {
        if let Ok(src) = fs::read_to_string(dir.join(BIGRAMS_ID)) {
            let model = super::bigram::BigramModel::from_tsv(&src);
            if !model.is_empty() {
                corrector.enable_context(model);
            }
        }
    }

    corrector
}

/// Direktori kamus bawaan di dalam pohon sumber. Dipakai oleh uji integrasi
/// dan sebagai cadangan saat berjalan lewat `cargo run`.
pub fn bundled_dict_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/dict")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn tabel_tertanam_terbaca() {
        let typo = typo_table();
        let slang = slang_table();
        assert!(typo.len() > 200, "tabel typo cuma {} entri", typo.len());
        assert!(slang.len() > 100, "tabel slang cuma {} entri", slang.len());
    }

    #[test]
    fn koreksi_typo_kunci_ada() {
        let t = typo_table();
        assert_eq!(t.get("yagn").as_deref(), Some("yang"));
        assert_eq!(t.get("recieve").as_deref(), Some("receive"));
        assert_eq!(t.get("disini").as_deref(), Some("di sini"));
        assert_eq!(t.get("terimakasih").as_deref(), Some("terima kasih"));
    }

    #[test]
    fn slang_terpisah_dari_typo() {
        // "yg" hanya boleh ada di tabel slang, bukan di tabel typo —
        // kalau tercampur, pengguna tidak bisa mematikannya.
        assert!(typo_table().get("yg").is_none());
        assert_eq!(slang_table().get("yg").as_deref(), Some("yang"));
    }

    /// Kata yang sah dalam salah satu bahasa tidak boleh jadi *sumber*
    /// koreksi otomatis — itu jalan tercepat merusak catatan orang.
    #[test]
    fn tidak_mengoreksi_kata_yang_sebenarnya_sah() {
        let t = typo_table();
        for kata in [
            "form", "from", "si", "its", "lets", "wont", "then", "than", "keluar",
            "rubah", "makna", "semu", "tidal", "cm", "no", "hari", "teknik",
        ] {
            assert!(
                t.get(kata).is_none(),
                "'{kata}' adalah kata sah tapi ada di tabel typo — berbahaya"
            );
        }
    }

    #[test]
    fn tidak_ada_entri_yang_bentrok_antar_berkas() {
        let mut asal: HashMap<String, String> = HashMap::new();
        let mut bentrok = Vec::new();
        for src in [TYPO_ID, TYPO_EN, SLANG_ID] {
            for line in src.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let Some((from, to)) = line.split_once('\t') else { continue };
                let (from, to) = (from.trim().to_lowercase(), to.trim().to_string());
                if let Some(lama) = asal.insert(from.clone(), to.clone()) {
                    if lama != to {
                        bentrok.push(format!("{from}: '{lama}' vs '{to}'"));
                    }
                }
            }
        }
        assert!(bentrok.is_empty(), "entri bentrok: {bentrok:?}");
    }

    #[test]
    fn daftar_kata_dengan_kolom_frekuensi() {
        let w = parse_wordlist("makan\t9000\nminum\t8000\n");
        assert_eq!(w, vec![("makan".into(), 9000), ("minum".into(), 8000)]);
    }

    #[test]
    fn daftar_kata_tanpa_frekuensi_memakai_urutan_baris() {
        let w = parse_wordlist("yang\nsaya\nmakan\n");
        assert_eq!(w.len(), 3);
        assert!(w[0].1 > w[1].1, "kata di baris awal harus lebih sering");
        assert!(w[1].1 > w[2].1);
    }

    #[test]
    fn daftar_kata_melewati_komentar_dan_baris_kosong() {
        let w = parse_wordlist("# catatan\n\nmakan\t10\n\n# lagi\nminum\t5\n");
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn frekuensi_rusak_tidak_membuat_gagal() {
        let w = parse_wordlist("makan\tbukanangka\n");
        assert_eq!(w.len(), 1);
        assert!(w[0].1 >= 1);
    }

    #[test]
    fn tanpa_direktori_kamus_tetap_bisa_dirakit() {
        let c = build_corrector(None, Settings::default());
        assert_eq!(c.dict_sizes(), (0, 0));
        assert!(!c.dictionaries_ready(), "tanpa kamus, T1 harus tidur");
        // T0 tetap bekerja.
        let teks = "yagn penting";
        assert_eq!(super::super::apply(teks, &c.check(teks)), "yang penting");
    }

    #[test]
    fn direktori_kamus_tidak_ada_tidak_membuat_panik() {
        let c = build_corrector(Some(Path::new("Z:/tidak/ada/sama/sekali")), Settings::default());
        assert_eq!(c.dict_sizes(), (0, 0));
    }

    #[test]
    fn saringan_membuang_kata_di_bawah_ambang_frekuensi() {
        let w = vec![("sering".into(), 100u32), ("jarang".into(), 3)];
        let hasil = filter_wordlist(w, 25, &Table::default());
        assert_eq!(hasil, vec![("sering".to_string(), 100)]);
    }

    #[test]
    fn saringan_mencekal_kunci_tabel_typo() {
        let banned = Table::from_tsv("yagn\tyang\n");
        let w = vec![("yang".into(), 100u32), ("yagn".into(), 99)];
        let hasil = filter_wordlist(w, 1, &banned);
        assert_eq!(hasil, vec![("yang".to_string(), 100)]);
    }

    // ---- Uji terhadap kamus sungguhan yang terpasang ----------------------

    fn kamus_nyata() -> Corrector {
        build_corrector(Some(&bundled_dict_dir()), Settings::default())
    }

    #[test]
    fn kamus_terpasang_dan_cukup_besar() {
        let c = kamus_nyata();
        let (id, en) = c.dict_sizes();
        assert!(id > 20_000, "kamus ID cuma {id} kata");
        assert!(en > 20_000, "kamus EN cuma {en} kata");
        assert!(c.dictionaries_ready());
    }

    /// Ini uji regresi untuk masalah yang paling berbahaya: korpus mentah
    /// memuat typo sebagai "kata sah", lalu typo itu menarik koreksi ke
    /// dirinya sendiri.
    #[test]
    fn typo_dari_korpus_tidak_ikut_masuk_kamus() {
        let c = kamus_nyata();
        let teks = "yagn disini recieve";
        let hasil = super::super::apply(teks, &c.check(teks));
        assert_eq!(hasil, "yang di sini receive");
    }

    #[test]
    fn kunci_slang_tetap_dianggap_kata_sah() {
        // Dengan pembentangan slang dimatikan, "yg" tidak boleh ditandai
        // sebagai typo yang tidak dikenal.
        let c = kamus_nyata();
        c.set_settings(Settings { slang_expansion: false, ..Settings::default() });
        assert!(c.check("yg penting").is_empty(), "{:?}", c.check("yg penting"));
    }

    #[test]
    fn typo_nyata_diperbaiki_dengan_kamus_sungguhan() {
        let c = kamus_nyata();
        for (salah, benar) in [
            ("saya mkaan nasi", "saya makan nasi"),
            ("kita berlai cepat", "kita berlari cepat"),
            ("tolong tuils ulang", "tolong tulis ulang"),
        ] {
            let hasil = super::super::apply(salah, &c.check(salah));
            assert_eq!(hasil, benar, "masukan: {salah}");
        }
    }

    #[test]
    fn kalimat_yang_sudah_benar_tidak_disentuh_sama_sekali() {
        let c = kamus_nyata();
        for teks in [
            "saya sedang mengerjakan catatan rapat hari ini",
            "tolong kirim laporan keuangan sebelum jam tiga sore",
            "please send the meeting notes before the deadline",
            "aku mau beli buku baru di toko dekat rumah",
        ] {
            let koreksi = c.check(teks);
            assert!(koreksi.is_empty(), "teks '{teks}' malah dikoreksi: {koreksi:?}");
        }
    }

    /// Regresi: analisis imbuhan yang keliru pernah meloloskan typo.
    ///
    /// `berlai` bisa dikupas jadi `ber` + `lai`, dan `lai` betul-betul ada
    /// di korpus dengan frekuensi 187 — potongan nama dari subtitle. Sebelum
    /// [`super::super::MIN_ROOT_FREQ`] ada, `berlai` dinyatakan sah dan tidak
    /// pernah diperbaiki.
    #[test]
    fn analisis_imbuhan_keliru_tidak_meloloskan_typo() {
        let c = kamus_nyata();
        let teks = "kita berlai cepat";
        assert_eq!(
            super::super::apply(teks, &c.check(teks)),
            "kita berlari cepat"
        );
    }

    /// Sisi sebaliknya dari ambang di atas: jangan sampai kata berimbuhan
    /// yang sah malah ikut tersapu.
    #[test]
    fn ambang_kata_dasar_tidak_merusak_kata_berimbuhan_sah() {
        let c = kamus_nyata();
        for teks in [
            "dia sedang menyapu halaman",
            "tolong dibacakan sekali lagi",
            "mereka mengerjakannya bersama",
            "pengambilan keputusan itu penting",
            "jangan memukul adikmu",
        ] {
            let koreksi = c.check(teks);
            assert!(koreksi.is_empty(), "teks '{teks}' malah dikoreksi: {koreksi:?}");
        }
    }

    /// Regresi: kamus yang salah tidak boleh mengalahkan kamus yang benar.
    ///
    /// `finsh` → kamus Inggris menempatkan `finish` di puncak, kamus Indonesia
    /// menempatkan `finch` (korpus subtitle Indonesia memuat kata Inggris).
    /// Dulu pemilihan lintas kamus memakai *keyakinan*, sehingga `finch` yang
    /// margin-nya lebih lebar di kamusnya sendiri menang. Sekarang dipilih
    /// lewat skor noisy-channel yang sudah dinormalkan jadi peluang.
    #[test]
    fn kamus_yang_benar_menang_lintas_bahasa() {
        let c = kamus_nyata();
        for (salah, benar) in [("finsh", "finish"), ("membhas", "membahas")] {
            let k = c.check(salah);
            assert_eq!(k.len(), 1, "'{salah}' tidak menghasilkan koreksi tunggal: {k:?}");
            assert_eq!(k[0].to, benar, "'{salah}' dikoreksi ke arah salah");
        }
    }

    /// Regresi: melewatkan huruf jauh lebih umum daripada menambah huruf.
    ///
    /// `dpan` → `dan` berarti pengetik menyisipkan `p` entah dari mana;
    /// `dpan` → `depan` berarti ia melewatkan `e`. Tanpa biaya asimetris,
    /// `dan` menang telak semata karena frekuensinya 30 kali lipat.
    #[test]
    fn huruf_hilang_lebih_dipercaya_daripada_huruf_berlebih() {
        let c = kamus_nyata();
        let k = c.check("dpan");
        if let Some(x) = k.first() {
            assert_ne!(x.to, "dan", "regresi: huruf berlebih dianggap terlalu murah");
        }
    }

    /// Tombol yang tertekan dua kali adalah pengecualian dari aturan di atas:
    /// huruf berlebih yang merupakan kembaran tetangganya justru sangat umum.
    #[test]
    fn huruf_kembar_berlebih_tetap_murah() {
        let c = kamus_nyata();
        for (salah, benar) in [
            ("sangatt", "sangat"),
            ("sebentarr", "sebentar"),
            ("pekerjaaan", "pekerjaan"),
        ] {
            let k = c.check(salah);
            assert_eq!(k.len(), 1, "'{salah}' tidak terdeteksi");
            assert_eq!(k[0].to, benar);
            assert!(k[0].auto, "'{salah}' seharusnya cukup yakin untuk otomatis");
        }
    }

    /// Kata yang persis ada di kamus tidak boleh dikalahkan tetangganya yang
    /// jauh lebih sering. `makam` sah meski `makan` 60 kali lebih umum.
    #[test]
    fn kata_sah_tapi_jarang_tidak_ditarik_ke_kata_umum() {
        let c = kamus_nyata();
        for teks in ["makam pahlawan", "sapu tangan", "kaki lima"] {
            let k = c.check(teks);
            assert!(k.is_empty(), "'{teks}' malah dikoreksi: {k:?}");
        }
    }

    #[test]
    fn catatan_campur_dua_bahasa_aman() {
        let c = kamus_nyata();
        let teks = "meeting besok jam 9, tolong siapkan slide dan laporan";
        let koreksi = c.check(teks);
        assert!(koreksi.is_empty(), "koreksi tak diinginkan: {koreksi:?}");
    }

    // ---- Reduplikasi angka-2 -----------------------------------------------

    #[test]
    fn reduplikasi_angka_dua_dibentangkan() {
        let c = kamus_nyata();
        for (dari, ke) in [
            ("anak2", "anak-anak"),
            ("jalan2", "jalan-jalan"),
            ("buku2", "buku-buku"),
            ("kata2", "kata-kata"),
        ] {
            let hasil = crate::corrector::apply(dari, &c.check(dari));
            assert_eq!(hasil, ke, "'{dari}' semestinya jadi '{ke}'");
        }
    }

    #[test]
    fn reduplikasi_menjaga_kapitalisasi() {
        let c = kamus_nyata();
        assert_eq!(crate::corrector::apply("Anak2", &c.check("Anak2")), "Anak-anak");
    }

    /// Token berangka yang BUKAN reduplikasi tidak boleh tersentuh:
    /// format berkas, singkatan gelar, versi, angka biasa, dan kata dasar
    /// yang tidak sah.
    ///
    /// Catatan: `bab2`/`poin2` SENGAJA tidak diuji di sini — keduanya kata
    /// sah, jadi memang ditafsirkan sebagai reduplikasi (`bab-bab`,
    /// `poin-poin`), sesuai konvensi. Kalau pengguna memaksudkan "bab 2",
    /// ia menuliskannya dengan spasi.
    #[test]
    fn bukan_reduplikasi_dibiarkan() {
        let c = kamus_nyata();
        for teks in ["mp3", "s2", "v2", "covid19", "3d", "xyz2", "a2"] {
            let koreksi = c.check(teks);
            assert!(
                !koreksi.iter().any(|k| k.to.contains('-') && k.source == crate::corrector::Source::Slang),
                "'{teks}' salah dianggap reduplikasi: {koreksi:?}"
            );
        }
    }

    #[test]
    fn reduplikasi_mati_saat_slang_dimatikan() {
        let c = kamus_nyata();
        c.set_settings(Settings { slang_expansion: false, ..Settings::default() });
        assert!(c.check("anak2").is_empty(), "reduplikasi harus ikut sakelar slang");
        c.set_settings(Settings::default());
    }

    // ---- Jalur kata-nyata (typo yang kebetulan kata sah) ------------------

    /// Kata Indonesia sah yang JARANG tetap dilindungi.
    ///
    /// # Riwayat: perilaku ini pernah dibalik, lalu diperbaiki
    ///
    /// Versi sebelumnya sengaja **menandai** `haru` (freq 199) dengan alasan
    /// "kata langka yang punya kembaran jauh lebih umum (`harus`, 260.853)
    /// pantas dicurigai". Audit kosakata membuktikan alasan itu keliru: aturan
    /// yang sama ikut menuduh `sarat` (56), `basa` (125), `busa` (221) — semua
    /// kata KBBI yang benar — dan merusak kalimat wajar seperti
    /// "acara itu penuh haru".
    ///
    /// Sekarang kata yang ada di kamus Indonesia dihormati apa pun
    /// frekuensinya ([`super::super::REAL_WORD_FLOOR_ID`]). Typo kata-nyata
    /// tetap tertangkap, tapi lewat **konteks kalimat** (Tier 2) yang jauh
    /// lebih presisi — lihat `kata_sah_yang_keliru_menurut_konteks_ditandai`.
    #[test]
    fn kata_indonesia_sah_walau_langka_tetap_dilindungi() {
        let c = kamus_nyata();
        for kata in ["haru", "sarat", "basa", "busa", "surel", "rusa"] {
            let koreksi = c.check(kata);
            assert!(
                koreksi.is_empty(),
                "'{kata}' kata Indonesia sah, tidak boleh dikoreksi: {koreksi:?}"
            );
        }
    }

    /// Sisi lain dari aturan di atas: kata yang **tidak ada** di kamus
    /// Indonesia dan hanya muncul sebagai sampah di korpus Inggris tetap
    /// boleh dikoreksi. Ambang kedua bahasa memang sengaja berbeda.
    #[test]
    fn sampah_korpus_inggris_tetap_bisa_dikoreksi() {
        let c = kamus_nyata();
        let koreksi = c.check("ino");
        assert!(
            koreksi.iter().any(|k| k.to == "ini"),
            "'ino' (bukan kata ID, hanya sampah subtitle EN) semestinya disarankan jadi 'ini': {koreksi:?}"
        );
    }

    /// Regresi: `ino`/`inu` (upaya mengetik `ini`) harus sampai jadi saran.
    /// `ino` khususnya hanya ada di korpus Inggris (sampah subtitle) sehingga
    /// dulu dianggap kata Inggris sah dan tak pernah dikoreksi.
    #[test]
    fn varian_ini_dikenali() {
        let c = kamus_nyata();
        for salah in ["ino", "inu", "inp", "nii", "iin"] {
            let koreksi = c.check(salah);
            assert!(
                koreksi.iter().any(|k| k.to == "ini"),
                "'{salah}' semestinya menyarankan 'ini': {koreksi:?}"
            );
        }
    }

    /// Regresi paling penting: kata Indonesia sah yang kebetulan mirip kata
    /// Inggris umum TIDAK boleh ditarik lintas bahasa. `surel` (email, ID 197 /
    /// EN 0) pernah "diperbaiki" jadi `surely` hanya karena `surely` lebih
    /// sering di korpus Inggris. Prioritas kamus ID melindunginya.
    #[test]
    fn kata_indonesia_sah_tidak_ditarik_ke_kata_inggris() {
        let c = kamus_nyata();
        for teks in ["surel", "rusa", "email", "terharu", "haus", "tahu"] {
            let koreksi = c.check(teks);
            assert!(
                !koreksi.iter().any(|k| k.from.eq_ignore_ascii_case(teks)),
                "'{teks}' (kata sah) malah dikoreksi: {koreksi:?}"
            );
        }
    }

    // ---- Tier 2: koreksi sadar-konteks ------------------------------------

    #[test]
    fn model_bigram_terpasang() {
        let c = kamus_nyata();
        assert!(c.has_context(), "model bigram semestinya termuat");
        assert!(c.bigram_len() > 100_000, "bigram cuma {}", c.bigram_len());
    }

    /// Inti Tier 2: typo yang kebetulan kata sah, ketahuan dari kalimatnya.
    /// `makam` valid, tapi "makan nasi" jauh lebih masuk akal dari "makam nasi".
    #[test]
    fn kata_sah_yang_keliru_menurut_konteks_ditandai() {
        let c = kamus_nyata();
        let teks = "saya makam nasi di warung";
        let koreksi = c.check(teks);
        let makam = koreksi.iter().find(|k| k.from == "makam");
        assert!(makam.is_some(), "makam→makan semestinya ditandai: {koreksi:?}");
        let makam = makam.unwrap();
        assert_eq!(makam.to, "makan");
        assert_eq!(makam.source, crate::corrector::Source::Context);
        assert!(!makam.auto, "koreksi kata-nyata tak pernah otomatis");
    }

    /// Sisi pengaman: kata sah yang MEMANG pas dengan konteksnya tidak boleh
    /// diganggu. "makam pahlawan" lazim, jadi makam dibiarkan.
    #[test]
    fn kata_sah_yang_pas_dengan_konteks_dibiarkan() {
        let c = kamus_nyata();
        for teks in [
            "mereka pergi ke makam pahlawan",
            "kita akan makan nasi bersama",
            "ini adalah versi terbaru aplikasi",
            "tolong beli koran hari ini",
        ] {
            let koreksi = c.check(teks);
            assert!(koreksi.is_empty(), "'{teks}' malah dikoreksi: {koreksi:?}");
        }
    }

    /// Konteks tidak boleh menarik kata umum ke kata yang lebih langka
    /// (`nasi` 911 → `napi` 343), meski kebetulan ada pasangan bigram.
    #[test]
    fn konteks_tidak_menarik_ke_kata_lebih_langka() {
        let c = kamus_nyata();
        let koreksi = c.check("saya makam nasi di warung");
        assert!(
            !koreksi.iter().any(|k| k.from == "nasi"),
            "nasi (kata umum) tidak boleh dikoreksi: {koreksi:?}"
        );
    }

    /// Kata-nyata hanya ditandai kalau tetangganya JAUH lebih umum. Kata langka
    /// yang tidak punya kembaran dominan dibiarkan.
    #[test]
    fn kata_nyata_tanpa_kembaran_dominan_dibiarkan() {
        let c = kamus_nyata();
        // `gnome` (EN 1007, ID 90) — tidak ada kata ID/EN yang 150x lebih umum
        // dan berjarak dekat.
        let koreksi = c.check("gnome");
        assert!(
            !koreksi.iter().any(|k| k.from == "gnome"),
            "gnome semestinya dibiarkan: {koreksi:?}"
        );
    }
}
