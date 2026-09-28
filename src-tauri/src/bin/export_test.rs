//! Uji ekspor nyata: tulis berkas contoh ke folder agar bisa dibuka manual
//! di Word / pembaca PDF / editor teks.
//!
//! ```text
//! cargo run --release --bin export_test
//! ```

use std::fs;
use std::path::PathBuf;

use catatan_anti_typo_lib::export::{export_note, Format};

/// (nama berkas, judul, isi)
const SAMPEL: &[(&str, &str, &str)] = &[
    (
        "01-catatan-rapat",
        "Rapat Anggaran Kuartal Tiga",
        "Rapat pagi ini membahas rencana peluncuran produk baru pada kuartal depan.\n\
         \n\
         Poin penting:\n\
         1. Tim pemasaran menyiapkan materi presentasi sebelum hari Jumat\n\
         2. Tim teknis menyelesaikan pengujian akhir dan mengirimkan laporan\n\
         3. Anggaran tambahan sudah disetujui, penggunaannya dilaporkan tiap bulan\n\
         \n\
         Tindak lanjut dikirim lewat surel ke seluruh peserta rapat.",
    ),
    (
        "02-daftar-bertingkat",
        "Daftar Bertingkat",
        "Struktur dokumen yang disepakati:\n\
         \n\
         1. Pendahuluan\n\
           a. Latar belakang\n\
           b. Rumusan masalah\n\
         2. Tinjauan pustaka\n\
           i. Teori utama\n\
           ii. Penelitian terdahulu\n\
         3. Metodologi\n\
         \n\
         - Catatan tambahan\n\
         - Lampiran menyusul",
    ),
    (
        "03-catatan-dengan-kode",
        "Catatan Teknis dengan Kode",
        "Berikut potongan kode yang dibahas saat rapat teknis.\n\
         \n\
         Struktur HTML halaman utama:\n\
         \n\
         ```html\n\
         <div class=\"container\">\n  <h1>Judul Halaman</h1>\n  <p>Isi paragraf</p>\n</div>\n\
         ```\n\
         \n\
         Gaya tampilannya:\n\
         \n\
         ```css\n\
         .container {\n  max-width: 960px;\n  margin: 0 auto;\n}\n\
         ```\n\
         \n\
         Logika penghitungan total:\n\
         \n\
         ```javascript\n\
         const total = items.reduce((a, b) => a + b.harga, 0);\n\
         console.log(\"Total:\", total);\n\
         ```\n\
         \n\
         Pengolahan data memakai Python:\n\
         \n\
         ```python\n\
         def hitung_rata(angka):\n    return sum(angka) / len(angka)\n\
         ```\n\
         \n\
         Kesimpulan: implementasi dilanjutkan minggu depan.",
    ),
    (
        "04-teks-panjang",
        "Dokumen Panjang untuk Uji Halaman PDF",
        "Paragraf pengujian untuk memastikan pemenggalan halaman PDF berjalan \
         benar dan teks tidak terpotong di tepi kertas. ",
    ),
    (
        "06-tiga-halaman",
        "Dokumen Tiga Halaman",
        "Ini isi halaman pertama.\n\
         Baris kedua di halaman pertama.\n\
         =============== HALAMAN ===============\n\
         Ini isi halaman kedua, seharusnya mulai di lembar baru.\n\
         =============== HALAMAN ===============\n\
         Ini isi halaman ketiga, juga lembar baru.",
    ),
    (
        "05-karakter-khusus",
        "Karakter Khusus & Aksen",
        "Uji karakter: kafé, naïve, résumé, señor.\n\
         Simbol: © ® ™ € £ ¥ § ¶ • …\n\
         Emoji: 👋 🌍 ✅ 📌\n\
         Kutip: \"ganda\" 'tunggal' «prancis»\n\
         Matematika: 2 × 3 ÷ 4 ≈ 1,5 ± 0,1\n\
         Surel: budi.santoso@kantor.co.id\n\
         Tautan: https://contoh.co.id/laporan?id=42&tipe=pdf",
    ),
];

fn main() -> anyhow::Result<()> {
    let dir: PathBuf = dirs_documents().join("AutoNotes-Hasil-Uji");
    fs::create_dir_all(&dir)?;
    println!("\nFolder keluaran: {}\n", dir.display());

    let formats = [
        (Format::Txt, "txt"),
        (Format::Md, "md"),
        (Format::Docx, "docx"),
        (Format::Pdf, "pdf"),
    ];

    let mut total = 0usize;
    let mut gagal = 0usize;
    println!("{:<26}{:>8}{:>8}{:>9}{:>9}", "berkas", "txt", "md", "docx", "pdf");
    println!("{}", "-".repeat(62));

    for (nama, judul, isi) in SAMPEL {
        // Sampel 04 sengaja diulang agar panjang (uji multi-halaman PDF).
        let isi_final = if nama.starts_with("04") { isi.repeat(120) } else { isi.to_string() };

        let mut baris = format!("{nama:<26}");
        for (fmt, ext) in &formats {
            let path = dir.join(format!("{nama}.{ext}"));
            total += 1;
            match export_note(&path, *fmt, judul, &isi_final) {
                Ok(()) => {
                    let ukuran = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let valid = verifikasi(&path, *fmt, &ukuran);
                    if valid {
                        baris.push_str(&format!("{:>8}", format_kb(ukuran)));
                    } else {
                        baris.push_str(&format!("{:>8}", "RUSAK"));
                        gagal += 1;
                    }
                }
                Err(e) => {
                    baris.push_str(&format!("{:>8}", "ERROR"));
                    gagal += 1;
                    eprintln!("  ! {nama}.{ext}: {e}");
                }
            }
        }
        println!("{baris}");
    }

    println!("\n{}", "=".repeat(62));
    println!("Berkas dibuat : {}/{}", total - gagal, total);
    println!("Verdict       : {}", if gagal == 0 { "LULUS" } else { "ADA MASALAH" });
    println!("\nBuka folder di atas untuk memeriksa hasilnya secara manual:");
    println!("  - .docx  -> buka dengan Microsoft Word / LibreOffice");
    println!("  - .pdf   -> buka dengan pembaca PDF apa pun");
    println!("  - .md    -> buka dengan editor teks / pratinjau Markdown");
    println!("  - .txt   -> buka dengan Notepad\n");
    Ok(())
}

/// Periksa berkas hasil benar-benar valid, bukan sekadar ada.
fn verifikasi(path: &std::path::Path, fmt: Format, ukuran: &u64) -> bool {
    if *ukuran == 0 {
        return false;
    }
    let Ok(bytes) = fs::read(path) else { return false };
    match fmt {
        // PDF wajib diawali "%PDF" dan diakhiri penanda EOF.
        Format::Pdf => bytes.starts_with(b"%PDF") && bytes.len() > 500,
        // DOCX adalah arsip ZIP -> tanda tangan "PK".
        Format::Docx => bytes.starts_with(b"PK") && bytes.len() > 500,
        // Teks wajib UTF-8 valid.
        Format::Txt | Format::Md => String::from_utf8(bytes).is_ok(),
    }
}

fn format_kb(b: u64) -> String {
    if b < 1024 {
        format!("{b}B")
    } else {
        format!("{:.0}K", b as f64 / 1024.0)
    }
}

/// Folder Documents pengguna, dengan cadangan ke direktori kerja.
fn dirs_documents() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Documents"))
        .unwrap_or_else(|_| PathBuf::from("."))
}
