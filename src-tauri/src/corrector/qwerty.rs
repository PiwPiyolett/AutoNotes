//! Bobot kedekatan tombol QWERTY.
//!
//! Orang yang mengetik cepat hampir selalu meleset ke tombol *tetangga*,
//! bukan ke huruf acak. Jadi substitusi `s`→`a` (bersebelahan) harus jauh
//! lebih murah daripada `s`→`p` (beda ujung papan ketik). Tanpa bobot ini,
//! "gaji" dan "gadi" terlihat sama-sama berjarak 1 dari "gali" — padahal
//! secara praktik yang satu jauh lebih mungkin.

use once_cell::sync::Lazy;
use std::collections::HashMap;

const ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm"];

/// Pergeseran horizontal tiap baris papan ketik fisik, dalam satuan lebar tombol.
const OFFSETS: [f32; 4] = [0.0, 0.25, 0.50, 0.85];

static KEYPOS: Lazy<HashMap<char, (f32, f32)>> = Lazy::new(|| {
    let mut m = HashMap::with_capacity(40);
    for (r, row) in ROWS.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            m.insert(ch, (c as f32 + OFFSETS[r], r as f32));
        }
    }
    m
});

/// Jarak euclidean antar dua tombol. `None` kalau salah satu bukan huruf/angka
/// QWERTY standar (mis. karakter beraksen).
pub fn key_distance(a: char, b: char) -> Option<f32> {
    let (ax, ay) = *KEYPOS.get(&a)?;
    let (bx, by) = *KEYPOS.get(&b)?;
    let (dx, dy) = (ax - bx, ay - by);
    Some((dx * dx + dy * dy).sqrt())
}

/// Biaya mengganti satu huruf dengan huruf lain.
///
/// # Kenapa potongannya kecil (0.7, bukan 0.5)
///
/// Diskon yang terlalu besar membuat *substitusi* selalu mengalahkan
/// *huruf hilang*, padahal keduanya sama-sama umum. Versi awal memakai 0.55
/// dan hasilnya `dpan` dikoreksi jadi `span` (`d`→`s` bertetangga, 0,55)
/// alih-alih `depan` (menyisipkan `e`, 1,0) — meski `depan` ribuan kali
/// lebih sering.
///
/// Sekarang kedekatan tombol hanya menggeser peringkat sedikit, dan
/// keputusan sesungguhnya diserahkan ke frekuensi lewat model noisy-channel
/// di [`super::symspell::LAMBDA`].
pub fn substitution_cost(a: char, b: char) -> f32 {
    if a == b {
        return 0.0;
    }
    let la = lower(a);
    let lb = lower(b);
    if la == lb {
        return 0.10; // hanya beda besar/kecil huruf
    }
    match key_distance(la, lb) {
        Some(d) if d <= 1.15 => 0.70, // bersebelahan langsung
        Some(d) if d <= 2.10 => 0.90, // satu tombol di antaranya
        _ => 1.0,
    }
}

fn lower(c: char) -> char {
    if c.is_ascii() {
        c.to_ascii_lowercase()
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tetangga_lebih_murah_dari_yang_jauh() {
        // 'a' dan 's' bersebelahan; 'a' dan 'p' berjauhan.
        assert!(substitution_cost('a', 's') < substitution_cost('a', 'p'));
    }

    #[test]
    fn huruf_sama_gratis() {
        assert_eq!(substitution_cost('k', 'k'), 0.0);
    }

    #[test]
    fn beda_kapital_hampir_gratis() {
        assert!(substitution_cost('k', 'K') < 0.2);
    }

    #[test]
    fn karakter_asing_kena_biaya_penuh() {
        assert_eq!(substitution_cost('é', 'ß'), 1.0);
    }
}
