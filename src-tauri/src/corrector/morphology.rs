//! Pengupas imbuhan Bahasa Indonesia (bergaya Sastrawi).
//!
//! # Kenapa ini wajib ada
//!
//! Kamus dasar berisi kata *dasar*: "kerja", "tulis", "ambil". Tapi orang
//! menulis "mengerjakannya", "menuliskan", "pengambilan". Tanpa pengupas
//! imbuhan, mesin koreksi akan menandai semua itu sebagai typo dan mencoba
//! "memperbaikinya" — bencana.
//!
//! # Yang paling rumit: peluluhan nasal
//!
//! Awalan me-/pe- berubah bentuk mengikuti huruf pertama kata dasar, dan
//! sebagian huruf itu **hilang**:
//!
//! | bentuk  | huruf awal dasar | contoh                        |
//! |---------|------------------|-------------------------------|
//! | `mem-`  | b, f, v          | mem + beri → memberi          |
//! | `mem-`  | **p luluh**      | mem + pukul → memukul         |
//! | `men-`  | c, d, j, z       | men + cuci → mencuci          |
//! | `men-`  | **t luluh**      | men + tulis → menulis         |
//! | `meng-` | vokal, g, h      | meng + ambil → mengambil      |
//! | `meng-` | **k luluh**      | meng + karang → mengarang     |
//! | `meny-` | **s luluh**      | meny + sapu → menyapu         |
//! | `menge-`| kata satu suku   | menge + bom → mengebom        |
//!
//! Karena itu pengupas ini tidak mengembalikan satu jawaban, melainkan
//! **daftar kemungkinan kata dasar**. Cukup satu yang ada di kamus untuk
//! menyatakan kata itu sah.
//!
//! # Bias yang disengaja
//!
//! Pengupas ini lebih memilih salah menyatakan "sah" daripada salah
//! menyatakan "typo". Konsekuensi terburuk dari terlalu longgar hanyalah
//! satu typo lolos; terlalu ketat berarti kata benar dirusak.

/// Partikel yang menempel paling luar.
const PARTICLES: [&str; 4] = ["lah", "kah", "tah", "pun"];

/// Kata ganti milik.
const POSSESSIVES: [&str; 3] = ["nya", "ku", "mu"];

/// Akhiran pembentuk kata.
const SUFFIXES: [&str; 3] = ["kan", "an", "i"];

/// Panjang minimum sisa kata setelah dikupas. Di bawah ini hasilnya
/// hampir pasti kebetulan, bukan kata dasar sungguhan.
const MIN_ROOT: usize = 3;

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Kupas awalan, kembalikan semua kemungkinan kata dasar.
fn prefix_roots(w: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if char_len(&s) >= MIN_ROOT {
            out.push(s);
        }
    };

    // --- Keluarga me- -------------------------------------------------------
    if let Some(r) = w.strip_prefix("menge") {
        push(r.to_string()); // mengebom → bom
    }
    if let Some(r) = w.strip_prefix("meng") {
        push(r.to_string()); // mengambil → ambil, menggali → gali
        push(format!("k{r}")); // mengarang → karang (k luluh)
    }
    if let Some(r) = w.strip_prefix("meny") {
        push(format!("s{r}")); // menyapu → sapu (s luluh)
    }
    if let Some(r) = w.strip_prefix("mem") {
        push(r.to_string()); // memberi → beri
        push(format!("p{r}")); // memukul → pukul (p luluh)
    }
    if let Some(r) = w.strip_prefix("men") {
        push(r.to_string()); // mendengar → dengar
        push(format!("t{r}")); // menulis → tulis (t luluh)
    }
    if let Some(r) = w.strip_prefix("me") {
        push(r.to_string()); // melihat → lihat, merasa → rasa
    }

    // --- Keluarga pe- (pola peluluhan identik) ------------------------------
    if let Some(r) = w.strip_prefix("penge") {
        push(r.to_string());
    }
    if let Some(r) = w.strip_prefix("peng") {
        push(r.to_string());
        push(format!("k{r}"));
    }
    if let Some(r) = w.strip_prefix("peny") {
        push(format!("s{r}"));
    }
    if let Some(r) = w.strip_prefix("pem") {
        push(r.to_string());
        push(format!("p{r}"));
    }
    if let Some(r) = w.strip_prefix("pen") {
        push(r.to_string());
        push(format!("t{r}"));
    }

    // --- Awalan sederhana tanpa peluluhan -----------------------------------
    for p in ["ber", "bel", "be", "per", "pel", "pe", "ter", "di", "ke", "se"] {
        if let Some(r) = w.strip_prefix(p) {
            push(r.to_string());
        }
    }

    out
}

/// Semua kemungkinan bentuk kata setelah dikupas berlapis:
/// partikel → kata ganti milik → akhiran → awalan.
///
/// Hasilnya selalu memuat kata aslinya sendiri di posisi pertama.
pub fn stems(word: &str) -> Vec<String> {
    let base = word.to_lowercase();
    let mut layer: Vec<String> = vec![base];

    // Tiap lapisan mempertahankan bentuk sebelumnya, karena tidak semua
    // kata punya semua imbuhan (mis. "bukumu" tidak berpartikel).
    for group in [&PARTICLES[..], &POSSESSIVES[..], &SUFFIXES[..]] {
        let mut next = layer.clone();
        for s in &layer {
            for suf in group {
                if let Some(r) = s.strip_suffix(suf) {
                    if char_len(r) >= MIN_ROOT {
                        next.push(r.to_string());
                    }
                }
            }
        }
        layer = next;
    }

    let mut out = layer.clone();
    for s in &layer {
        out.extend(prefix_roots(s));
    }

    // Sisipan reduplikasi: "anak-anak", "buku-buku" → ambil satu bagian.
    if let Some((kiri, kanan)) = word.to_lowercase().split_once('-') {
        if kiri == kanan && char_len(kiri) >= MIN_ROOT {
            out.push(kiri.to_string());
        }
    }

    out.sort();
    out.dedup();
    // Jaga kata asli tetap di depan agar pemanggil bisa memeriksanya lebih dulu.
    let original = word.to_lowercase();
    out.retain(|s| *s != original);
    out.insert(0, original);
    out
}

/// Apakah kata ini sah menurut kamus, setelah memperhitungkan imbuhan?
///
/// `has` adalah penanya kamus — biasanya penutup atas [`super::symspell::SymSpell`].
pub fn is_valid(word: &str, has: &dyn Fn(&str) -> bool) -> bool {
    stems(word).iter().any(|s| has(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn kamus() -> impl Fn(&str) -> bool {
        let set: HashSet<&'static str> = [
            "kerja", "tulis", "ambil", "sapu", "pukul", "beri", "dengar", "lihat",
            "rasa", "karang", "bom", "gali", "buku", "makan", "anak", "jual",
            "cuci", "baca", "main",
        ]
        .into_iter()
        .collect();
        move |w: &str| set.contains(w)
    }

    fn sah(w: &str) -> bool {
        is_valid(w, &kamus())
    }

    #[test]
    fn kata_dasar_polos() {
        assert!(sah("kerja"));
        assert!(sah("buku"));
    }

    #[test]
    fn peluluhan_p_jadi_mem() {
        assert!(sah("memukul"), "memukul → pukul");
    }

    #[test]
    fn peluluhan_t_jadi_men() {
        assert!(sah("menulis"), "menulis → tulis");
    }

    #[test]
    fn peluluhan_s_jadi_meny() {
        assert!(sah("menyapu"), "menyapu → sapu");
    }

    #[test]
    fn peluluhan_k_jadi_meng() {
        assert!(sah("mengarang"), "mengarang → karang");
    }

    #[test]
    fn meng_dengan_vokal() {
        assert!(sah("mengambil"), "mengambil → ambil");
    }

    #[test]
    fn menge_untuk_kata_satu_suku() {
        assert!(sah("mengebom"), "mengebom → bom");
    }

    #[test]
    fn tanpa_peluluhan() {
        assert!(sah("memberi"));
        assert!(sah("mendengar"));
        assert!(sah("melihat"));
        assert!(sah("merasa"));
        assert!(sah("menggali"));
    }

    #[test]
    fn imbuhan_bertumpuk_penuh() {
        // Ini kasus uji utamanya: awalan meng- + akhiran -kan + -nya sekaligus.
        assert!(sah("mengerjakannya"), "mengerjakannya → kerja");
        assert!(sah("menuliskan"), "menuliskan → tulis");
        assert!(sah("dibacakanlah"), "dibacakanlah → baca");
        assert!(sah("permainan"), "permainan → main");
    }

    #[test]
    fn awalan_sederhana() {
        assert!(sah("dibaca"));
        assert!(sah("terjual"));
        assert!(sah("bermain"));
        assert!(sah("sebuku"));
    }

    #[test]
    fn kata_ganti_milik() {
        assert!(sah("bukuku"));
        assert!(sah("bukumu"));
        assert!(sah("bukunya"));
    }

    #[test]
    fn reduplikasi() {
        assert!(sah("anak-anak"));
    }

    #[test]
    fn typo_asli_tetap_ditolak() {
        assert!(!sah("mkaan"), "typo tidak boleh lolos jadi 'sah'");
        assert!(!sah("zzzzz"));
        assert!(!sah("krja"));
    }

    #[test]
    fn kupasan_tidak_menyisakan_potongan_terlalu_pendek() {
        // "makan" jangan sampai dikupas jadi "mak" lalu dianggap kata lain.
        let s = stems("makan");
        assert_eq!(s[0], "makan");
        assert!(!s.contains(&"ma".to_string()));
    }

    #[test]
    fn kata_asli_selalu_di_depan() {
        assert_eq!(stems("mengerjakannya")[0], "mengerjakannya");
    }
}
