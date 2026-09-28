//! Kamus pribadi & ingatan pembatalan.
//!
//! # Kenapa "ingatan pembatalan" sama pentingnya dengan kamus
//!
//! Pengguna jarang mau repot membuka menu "tambah ke kamus". Yang mereka
//! lakukan secara alami adalah menekan Ctrl+Z ketika koreksi kita salah.
//! Sinyal itu gratis dan sangat jujur — jadi kita panen.
//!
//! Kalau pengguna membatalkan koreksi `nabila` → `nabil` dua kali, aplikasi
//! berhenti mengusulkannya **selamanya**. Nama orang, istilah teknis, dan
//! singkatan internal kantor terurus sendiri tanpa pengguna pernah
//! mengonfigurasi apa pun.

use std::collections::{HashMap, HashSet};

/// Berapa kali pengguna harus membatalkan koreksi yang sama sebelum kita
/// menyerah. Satu kali bisa saja salah pencet; dua kali sudah pernyataan.
const REJECT_THRESHOLD: u32 = 2;

#[derive(Default)]
pub struct UserDict {
    known: HashSet<Box<str>>,
    /// (kata asal, kata usulan) → berapa kali pengguna menolaknya.
    rejected: HashMap<(Box<str>, Box<str>), u32>,
}

impl UserDict {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tandai sebuah kata sebagai benar — jangan pernah dikoreksi lagi.
    pub fn add_word(&mut self, word: &str) {
        self.known.insert(word.to_lowercase().into_boxed_str());
    }

    pub fn remove_word(&mut self, word: &str) {
        self.known.remove(word.to_lowercase().as_str());
    }

    pub fn knows(&self, word: &str) -> bool {
        self.known.contains(word.to_lowercase().as_str())
    }

    pub fn words(&self) -> impl Iterator<Item = &str> {
        self.known.iter().map(|w| &**w)
    }

    /// Catat bahwa pengguna membatalkan koreksi `from` → `to`.
    pub fn record_undo(&mut self, from: &str, to: &str) {
        let key = (
            from.to_lowercase().into_boxed_str(),
            to.to_lowercase().into_boxed_str(),
        );
        *self.rejected.entry(key).or_insert(0) += 1;
    }

    /// Apakah pasangan koreksi ini sudah cukup sering ditolak?
    pub fn is_rejected(&self, from: &str, to: &str) -> bool {
        let key = (
            from.to_lowercase().into_boxed_str(),
            to.to_lowercase().into_boxed_str(),
        );
        self.rejected.get(&key).is_some_and(|&n| n >= REJECT_THRESHOLD)
    }

    /// Untuk disimpan ke basis data.
    pub fn undo_entries(&self) -> impl Iterator<Item = (&str, &str, u32)> {
        self.rejected.iter().map(|((f, t), n)| (&**f, &**t, *n))
    }

    /// Muat ulang dari basis data saat aplikasi dibuka.
    pub fn restore_undo(&mut self, from: &str, to: &str, count: u32) {
        self.rejected.insert(
            (
                from.to_lowercase().into_boxed_str(),
                to.to_lowercase().into_boxed_str(),
            ),
            count,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kata_yang_ditambahkan_dikenali() {
        let mut d = UserDict::new();
        d.add_word("Nabila");
        assert!(d.knows("nabila"));
        assert!(d.knows("NABILA"));
        assert!(!d.knows("nabil"));
    }

    #[test]
    fn kata_bisa_dicabut_lagi() {
        let mut d = UserDict::new();
        d.add_word("kubernetes");
        d.remove_word("kubernetes");
        assert!(!d.knows("kubernetes"));
    }

    #[test]
    fn satu_pembatalan_belum_cukup() {
        let mut d = UserDict::new();
        d.record_undo("nabila", "nabil");
        assert!(!d.is_rejected("nabila", "nabil"));
    }

    #[test]
    fn dua_pembatalan_menghentikan_usulan_selamanya() {
        let mut d = UserDict::new();
        d.record_undo("nabila", "nabil");
        d.record_undo("nabila", "nabil");
        assert!(d.is_rejected("nabila", "nabil"));
    }

    #[test]
    fn pembatalan_hanya_berlaku_untuk_pasangan_yang_sama() {
        let mut d = UserDict::new();
        d.record_undo("nabila", "nabil");
        d.record_undo("nabila", "nabil");
        assert!(!d.is_rejected("nabila", "nabilah"));
    }

    #[test]
    fn pemulihan_dari_basis_data() {
        let mut d = UserDict::new();
        d.restore_undo("dhika", "dika", 3);
        assert!(d.is_rejected("dhika", "dika"));
    }
}
