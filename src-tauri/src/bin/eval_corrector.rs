//! Alat ukur mutu koreksi.
//!
//! ```text
//! cargo run --release --bin eval_corrector
//! ```
//!
//! Ambang keyakinan tidak boleh ditebak. Berkas ini menjalankan mesin koreksi
//! pada dua kumpulan data dan melaporkan dua angka yang saling bertolak
//! belakang:
//!
//! * **Recall** — dari sekian typo, berapa yang benar-benar diperbaiki?
//! * **Kesalahan pada teks bersih** — dari sekian kalimat yang sudah benar,
//!   berapa yang malah dirusak?
//!
//! Yang kedua jauh lebih mahal. Melewatkan typo hanya membuat pengguna
//! kecewa; merusak kata yang sudah benar membuat mereka berhenti percaya
//! dan mematikan fiturnya — dan sekali dimatikan, tidak pernah dinyalakan lagi.

use catatan_anti_typo_lib::corrector::{
    apply, apply_auto,
    resources::{build_corrector, bundled_dict_dir},
    Aggressiveness, Corrector, Settings,
};

/// Kalimat ber-typo dan bentuk benarnya. Typo dibuat meniru kesalahan orang
/// yang mengetik cepat: huruf hilang, huruf tertukar, huruf dobel, dan
/// meleset ke tombol tetangga.
const BER_TYPO: &[(&str, &str)] = &[
    // --- huruf hilang ---
    ("saya sedang membhas laporan", "saya sedang membahas laporan"),
    ("tolong kirim sebelm jam lima", "tolong kirim sebelum jam lima"),
    ("anggaran tambhan sudah cair", "anggaran tambahan sudah cair"),
    ("dia tidak dimnta datang", "dia tidak diminta datang"),
    ("rapat bulan dpan dibatalkan", "rapat bulan depan dibatalkan"),
    ("kami akan menyiapkan matri presentasi", "kami akan menyiapkan materi presentasi"),
    ("hasil pengujan sudah keluar", "hasil pengujian sudah keluar"),
    ("mohon konfirmasi kehadiran ada", "mohon konfirmasi kehadiran anda"),
    ("jangan lupa bwa berkasnya", "jangan lupa bawa berkasnya"),
    ("keputusan itu sanga penting", "keputusan itu sangat penting"),
    ("silakan hubungi bagian keuanga", "silakan hubungi bagian keuangan"),
    ("proyek ini memerlukan waktu lbih lama", "proyek ini memerlukan waktu lebih lama"),
    // --- huruf tertukar ---
    ("kita harus berlari lebih cepta", "kita harus berlari lebih cepat"),
    ("hasil penguijan akhir memuaskan", "hasil pengujian akhir memuaskan"),
    ("mereka sudah menerima laporna", "mereka sudah menerima laporan"),
    ("tolong perikas ulang datanya", "tolong periksa ulang datanya"),
    ("dokumen itu sudah dikrim kemarin", "dokumen itu sudah dikirim kemarin"),
    ("saya belum semapt membacanya", "saya belum sempat membacanya"),
    ("acara akan dimulai pukul delapna", "acara akan dimulai pukul delapan"),
    ("kondisi keuangan perusahan membaik", "kondisi keuangan perusahaan membaik"),
    // --- huruf dobel / kelebihan ---
    ("kami sudah menyelesaikkan tugasnya", "kami sudah menyelesaikan tugasnya"),
    ("laporan itu sangatt lengkap", "laporan itu sangat lengkap"),
    ("mohon ditunggu sebentarr lagi", "mohon ditunggu sebentar lagi"),
    ("pekerjaaan ini butuh ketelitian", "pekerjaan ini butuh ketelitian"),
    // --- meleset ke tombol tetangga ---
    ("tolong simpan fike itu", "tolong simpan file itu"),
    ("kami akan mengurim undangan", "kami akan mengirim undangan"),
    ("dia sudah menulos jawabannya", "dia sudah menulis jawabannya"),
    ("hasilnya belum tentu benat", "hasilnya belum tentu benar"),
    ("mari kita mulao sekarang", "mari kita mulai sekarang"),
    ("sudah berapa lana menunggu", "sudah berapa lama menunggu"),
    // --- kata panjang berimbuhan ---
    ("mereka sedang mengerjakn tugasnya", "mereka sedang mengerjakan tugasnya"),
    ("proses pengambilna keputusan lambat", "proses pengambilan keputusan lambat"),
    ("kegiatan itu akan dilaksanakn besok", "kegiatan itu akan dilaksanakan besok"),
    ("data tersebut belum diperbarui semuana", "data tersebut belum diperbarui semuanya"),
    ("penggunaanya harus dilaporkan", "penggunaannya harus dilaporkan"),
    // --- Inggris ---
    ("please send the meting notes", "please send the meeting notes"),
    ("we need to finsh this today", "we need to finish this today"),
    ("the deadlien is next friday", "the deadline is next friday"),
    ("i will chekc the numbers", "i will check the numbers"),
    ("this is a importent decision", "this is a important decision"),
    ("lets discuss the buget tomorrow", "lets discuss the budget tomorrow"),
    ("the projcet is almost complete", "the project is almost complete"),
    ("send me the updatde version", "send me the updated version"),
];

/// Kalimat yang sudah benar. Satu koreksi apa pun di sini adalah kegagalan.
const BERSIH: &[&str] = &[
    "saya sedang mengerjakan catatan rapat hari ini",
    "tolong kirim laporan keuangan sebelum jam tiga sore",
    "dia sedang menyapu halaman belakang rumah",
    "mereka mengerjakannya bersama sampai larut malam",
    "pengambilan keputusan itu memerlukan pertimbangan matang",
    "anggaran tambahan sudah disetujui oleh direksi",
    "jangan lupa membawa berkas asli dan fotokopinya",
    "kegiatan pelatihan akan dilaksanakan minggu depan",
    "penggunaannya harus dilaporkan setiap akhir bulan",
    "hasil pengujian menunjukkan peningkatan yang signifikan",
    "perusahaan itu membuka lowongan untuk posisi baru",
    "silakan hubungi bagian keuangan untuk keterangan lebih lanjut",
    "kami sudah menerima dokumen yang dikirim kemarin",
    "proses perpanjangan izin memakan waktu dua minggu",
    "dia belum sempat membaca seluruh isi laporannya",
    "beberapa peserta datang terlambat karena macet",
    "please send the meeting notes before the deadline",
    "we need to finish this project by next week",
    "the budget has been approved by the finance team",
    "i will check the numbers and get back to you",
    "this is an important decision for the company",
    "the updated version is available for download",
    "meeting besok jam sembilan, tolong siapkan slide",
    "deadline laporan tahunan sudah semakin dekat",
    "tim teknis akan melakukan maintenance malam ini",
    "kirim ke budi.santoso@kantor.co.id sebelum sore",
    "buka https://contoh.co.id/laporan untuk detailnya",
    "jalankan `npm run build` lalu periksa hasilnya",
    "dokumen PDF itu dikirim lewat surel oleh HRD",
];

struct Hasil {
    otomatis: usize,
    ditandai: usize,
    terlewat: usize,
    tak_tersentuh: Vec<String>,
    salah_koreksi: Vec<String>,
    rusak_bersih: Vec<String>,
}

fn evaluasi(c: &Corrector) -> Hasil {
    let mut h = Hasil {
        otomatis: 0,
        ditandai: 0,
        terlewat: 0,
        tak_tersentuh: Vec::new(),
        salah_koreksi: Vec::new(),
        rusak_bersih: Vec::new(),
    };

    for (salah, benar) in BER_TYPO {
        let koreksi = c.check(salah);
        let hasil_auto = apply_auto(salah, &koreksi);
        let hasil_penuh = apply(salah, &koreksi);

        if hasil_auto == *benar {
            h.otomatis += 1;
        } else if hasil_penuh == *benar {
            h.ditandai += 1;
        } else if hasil_penuh == *salah {
            h.terlewat += 1;
            // Cari kata mana yang berbeda, supaya polanya kelihatan.
            let beda: Vec<String> = salah
                .split_whitespace()
                .zip(benar.split_whitespace())
                .filter(|(a, b)| a != b)
                .map(|(a, b)| format!("{a} (seharusnya {b})"))
                .collect();
            h.tak_tersentuh.push(beda.join(", "));
        } else {
            // Dikoreksi, tapi ke arah yang salah — kategori terburuk.
            h.terlewat += 1;
            h.salah_koreksi.push(format!("{salah}  ->  {hasil_penuh}"));
        }
    }

    for teks in BERSIH {
        let koreksi = c.check(teks);
        if !koreksi.is_empty() {
            let auto: Vec<String> = koreksi
                .iter()
                .map(|k| format!("{}->{}{}", k.from, k.to, if k.auto { "*" } else { "" }))
                .collect();
            h.rusak_bersih.push(format!("{teks}   [{}]", auto.join(", ")));
        }
    }

    h
}

fn main() {
    let c = build_corrector(Some(&bundled_dict_dir()), Settings::default());
    let (id, en) = c.dict_sizes();
    println!("\nKamus: {id} kata ID, {en} kata EN");
    println!("Data uji: {} kalimat ber-typo, {} kalimat bersih\n", BER_TYPO.len(), BERSIH.len());

    for tingkat in [
        Aggressiveness::Konservatif,
        Aggressiveness::Seimbang,
        Aggressiveness::Agresif,
    ] {
        c.set_settings(Settings { aggressiveness: tingkat, ..Settings::default() });
        let h = evaluasi(&c);
        let total = BER_TYPO.len();
        let persen = |n: usize| n as f64 * 100.0 / total as f64;

        println!("── {tingkat:?} (auto ≥ {:.2}, tandai ≥ {:.2})",
            tingkat.auto_threshold(), tingkat.flag_threshold());
        println!("   diperbaiki otomatis : {:>2}/{total}  ({:.0}%)", h.otomatis, persen(h.otomatis));
        println!("   hanya ditandai      : {:>2}/{total}  ({:.0}%)", h.ditandai, persen(h.ditandai));
        println!("   terlewat / salah    : {:>2}/{total}  ({:.0}%)", h.terlewat, persen(h.terlewat));
        println!("   teks bersih dirusak : {:>2}/{}", h.rusak_bersih.len(), BERSIH.len());
        if !h.tak_tersentuh.is_empty() {
            println!("   ── tidak terdeteksi sama sekali:");
            for s in &h.tak_tersentuh {
                println!("      {s}");
            }
        }
        if !h.salah_koreksi.is_empty() {
            println!("   ── dikoreksi ke arah salah:");
            for s in &h.salah_koreksi {
                println!("      {s}");
            }
        }
        if !h.rusak_bersih.is_empty() {
            println!("   ── teks bersih yang tersentuh (* = otomatis):");
            for s in &h.rusak_bersih {
                println!("      {s}");
            }
        }
        println!();
    }
}
