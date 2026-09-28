//! Jarak edit Damerau-Levenshtein (varian OSA — *optimal string alignment*).
//!
//! Ada dua versi di sini dan keduanya memang dibutuhkan:
//!
//! * [`osa`] — bilangan bulat, dipakai untuk **menyaring**: "apakah kata ini
//!   masih dalam radius 2 edit?" Punya pemutus dini supaya cepat.
//! * [`weighted_osa`] — pecahan berbobot QWERTY, dipakai untuk **memeringkat**
//!   kandidat yang lolos saringan.
//!
//! Transposisi (huruf tertukar, mis. `tahn` → `tahu`... atau lebih tepat
//! `nahu`→`tahu`) diberi biaya lebih rendah dari substitusi biasa karena
//! itulah kesalahan nomor satu orang yang mengetik cepat: jari kanan-kiri
//! saling mendahului.

use super::qwerty::substitution_cost;

/// Biaya transposisi. Sengaja jauh di bawah 1.0 — dua huruf tertukar adalah
/// pola salah ketik paling umum saat buru-buru.
const TRANSPOSE_COST: f32 = 0.50;

/// Pengetik **melewatkan** huruf (kata benar lebih panjang dari yang diketik).
///
/// Ini kesalahan paling umum saat mengejar waktu: jari bergerak ke huruf
/// berikutnya sebelum yang sekarang sempat tertekan.
const MISSING_CHAR: f32 = 0.85;

/// Pengetik **menambah** huruf yang tidak ada (kata benar lebih pendek).
///
/// Jauh lebih jarang: perlu menekan tombol yang memang tidak diniatkan sama
/// sekali. Asimetri inilah yang memisahkan `dpan` → `depan` (melewatkan `e`)
/// dari `dpan` → `dan` (menambah `p` entah dari mana) — tanpa ini, `dan`
/// menang telak hanya karena frekuensinya 30 kali lipat.
const EXTRA_CHAR: f32 = 1.50;

/// Kecuali kalau huruf berlebih itu kembaran tetangganya: `sangatt`,
/// `pekerjaaan`, `sebentarr`. Tombol yang tertekan dua kali sangat umum.
const DOUBLED_CHAR: f32 = 0.75;

/// Jarak OSA versi bilangan bulat, dengan pemutus dini.
///
/// Mengembalikan `None` kalau jaraknya sudah pasti melebihi `max` — ini
/// membuat penyaringan ribuan kandidat tetap murah.
pub fn osa(a: &[char], b: &[char], max: usize) -> Option<usize> {
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max {
        return None;
    }
    if n == 0 {
        return (m <= max).then_some(m);
    }
    if m == 0 {
        return (n <= max).then_some(n);
    }

    // Tiga baris berjalan: i-2, i-1, dan i. Baris i-2 diperlukan transposisi.
    let mut prev2 = vec![0usize; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];

    for i in 1..=n {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=m {
            let sub = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut v = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + sub);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(prev2[j - 2] + 1);
            }
            cur[j] = v;
            row_min = row_min.min(v);
        }
        // Seluruh baris sudah melampaui ambang: tidak mungkin membaik lagi.
        if row_min > max {
            return None;
        }
        // Geser jendela: prev2 <- prev, prev <- cur.
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }

    let d = prev[m];
    (d <= max).then_some(d)
}

/// Jarak OSA berbobot, **asimetris**.
///
/// `a` adalah yang **diketik pengguna**, `b` adalah **calon kata benar**.
/// Urutan argumen bermakna dan tidak boleh dibalik: biaya melewatkan huruf
/// berbeda dari biaya menambah huruf, karena kedua kesalahan itu memang tidak
/// sama seringnya.
pub fn weighted_osa(a: &[char], b: &[char]) -> f32 {
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m as f32 * MISSING_CHAR;
    }
    if m == 0 {
        return n as f32 * EXTRA_CHAR;
    }

    let mut prev2 = vec![0f32; m + 1];
    let mut prev: Vec<f32> = (0..=m).map(|j| j as f32 * MISSING_CHAR).collect();
    let mut cur = vec![0f32; m + 1];

    for i in 1..=n {
        // Huruf ke-i yang diketik dibuang: berarti pengguna kelebihan huruf.
        // Murah kalau huruf itu sekadar kembaran tetangganya (tombol
        // tertekan dua kali), mahal kalau muncul entah dari mana.
        let extra = if i > 1 && a[i - 1] == a[i - 2] {
            DOUBLED_CHAR
        } else {
            EXTRA_CHAR
        };
        cur[0] = prev[0] + extra;

        for j in 1..=m {
            let sub = substitution_cost(a[i - 1], b[j - 1]);
            let mut v = (prev[j] + extra)
                .min(cur[j - 1] + MISSING_CHAR)
                .min(prev[j - 1] + sub);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(prev2[j - 2] + TRANSPOSE_COST);
            }
            cur[j] = v;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }

    prev[m]
}

/// Pembantu: ubah `&str` jadi `Vec<char>` sekali saja agar tidak
/// menghitung ulang di dalam loop panas.
pub fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(a: &str, b: &str, max: usize) -> Option<usize> {
        osa(&chars(a), &chars(b), max)
    }

    #[test]
    fn kata_identik_berjarak_nol() {
        assert_eq!(d("makan", "makan", 2), Some(0));
    }

    #[test]
    fn satu_huruf_hilang() {
        assert_eq!(d("makn", "makan", 2), Some(1));
    }

    #[test]
    fn satu_huruf_kelebihan() {
        assert_eq!(d("makkan", "makan", 2), Some(1));
    }

    #[test]
    fn transposisi_dihitung_satu_bukan_dua() {
        // "mkaan" -> "makan" hanya tukar posisi 'm','k'... tepatnya 'k' dan 'a'.
        assert_eq!(d("mkaan", "makan", 2), Some(1));
    }

    #[test]
    fn pemutus_dini_menolak_yang_terlalu_jauh() {
        assert_eq!(d("makan", "berlari", 2), None);
    }

    #[test]
    fn beda_panjang_ekstrem_langsung_ditolak() {
        assert_eq!(d("a", "abcdefgh", 2), None);
    }

    #[test]
    fn transposisi_lebih_murah_dari_substitusi_jauh() {
        // "mkaan" (tertukar) harus lebih murah daripada "mzkan" (huruf asing).
        let tertukar = weighted_osa(&chars("mkaan"), &chars("makan"));
        let salah_huruf = weighted_osa(&chars("mzkan"), &chars("makan"));
        assert!(
            tertukar < salah_huruf,
            "tertukar={tertukar} salah_huruf={salah_huruf}"
        );
    }

    #[test]
    fn bobot_qwerty_mempengaruhi_peringkat() {
        // 'j' bersebelahan dengan 'k'; 'q' berjauhan.
        let dekat = weighted_osa(&chars("makan"), &chars("majan"));
        let jauh = weighted_osa(&chars("makan"), &chars("maqan"));
        assert!(dekat < jauh, "dekat={dekat} jauh={jauh}");
    }

    #[test]
    fn kosong_ditangani() {
        assert_eq!(d("", "", 2), Some(0));
        assert_eq!(d("", "ab", 2), Some(2));
        assert_eq!(d("abc", "", 2), None);
    }
}
