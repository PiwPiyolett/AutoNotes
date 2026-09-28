//! Audit menyeluruh mesin koreksi: kosakata sehari-hari, kalimat nyata,
//! typo umum, dan kasus tepi.
//!
//! ```text
//! cargo run --release --bin audit
//! ```
//!
//! Dua jenis kegagalan dilaporkan terpisah, karena bobotnya sangat berbeda:
//!
//! * **FALSE POSITIVE** — kata/kalimat BENAR yang malah dikoreksi. Ini yang
//!   paling merusak kepercayaan; targetnya nol.
//! * **MISS** — typo yang tidak terdeteksi. Mengecewakan tapi tidak merusak.

use std::collections::BTreeMap;

use catatan_anti_typo_lib::corrector::{
    apply, resources::{build_corrector, bundled_dict_dir}, Aggressiveness, Corrector, Settings,
};

/// Kata yang HARUS dibiarkan (semuanya kata Indonesia/Inggris yang sah).
const KOSAKATA_BENAR: &[(&str, &[&str])] = &[
    ("kata dasar sehari-hari", &[
        "makan", "minum", "tidur", "bangun", "mandi", "jalan", "lari", "duduk",
        "beli", "jual", "bayar", "kirim", "terima", "buka", "tutup", "ambil",
        "simpan", "hapus", "tulis", "baca", "lihat", "dengar", "bicara", "diam",
        "rumah", "kantor", "sekolah", "pasar", "toko", "jalan", "mobil", "motor",
        "meja", "kursi", "pintu", "jendela", "lampu", "buku", "pena", "kertas",
        "nasi", "roti", "susu", "kopi", "teh", "gula", "garam", "minyak",
        "hari", "malam", "pagi", "siang", "sore", "besok", "kemarin", "lusa",
        "satu", "dua", "tiga", "empat", "lima", "enam", "tujuh", "delapan",
    ]),
    ("kata berimbuhan", &[
        "mengerjakan", "mengirimkan", "membacakan", "menuliskan", "membersihkan",
        "memperbaiki", "mempersiapkan", "mempertimbangkan", "memberitahukan",
        "dikerjakan", "dikirimkan", "dibacakan", "diselesaikan", "diperlukan",
        "pekerjaan", "pengiriman", "pembacaan", "penyelesaian", "perbaikan",
        "berlari", "berjalan", "bermain", "belajar", "bekerja", "berbicara",
        "terbaik", "terbesar", "tercepat", "terakhir", "terlambat", "terharu",
        "kebersihan", "keamanan", "kesehatan", "keuangan", "kegiatan", "keputusan",
        "penggunaannya", "mengerjakannya", "menyelesaikannya", "memberikannya",
        "sebaiknya", "seharusnya", "sebenarnya", "sesungguhnya", "setidaknya",
    ]),
    ("kata kantor & formal", &[
        "rapat", "laporan", "anggaran", "proyek", "jadwal", "agenda", "notulen",
        "presentasi", "dokumen", "berkas", "arsip", "surat", "memo", "undangan",
        "atasan", "bawahan", "rekan", "klien", "vendor", "mitra", "pelanggan",
        "target", "capaian", "kinerja", "evaluasi", "penilaian", "peninjauan",
        "keuangan", "pemasaran", "penjualan", "produksi", "operasional", "logistik",
        "perusahaan", "organisasi", "divisi", "departemen", "bagian", "unit",
    ]),
    ("kata Inggris umum", &[
        "meeting", "deadline", "project", "report", "email", "file", "folder",
        "update", "upload", "download", "backup", "server", "client", "database",
        "please", "thanks", "sorry", "hello", "morning", "today", "tomorrow",
        "team", "manager", "office", "budget", "schedule", "review", "approve",
        "the", "and", "with", "from", "that", "this", "have", "will", "would",
    ]),
    ("kata rawan (mirip kata lain)", &[
        "makam", "malam", "salam", "dalam", "alam", "diam", "siam",
        "hari", "harus", "haru", "baru", "biru", "buru", "guru",
        "surel", "surat", "sarat", "serat", "sirat",
        "rusa", "rasa", "rusak", "risau", "resah",
        "bisa", "basa", "busa", "beda", "bela", "bila",
        "kata", "kota", "kita", "kuda",
        "pesan", "pasan", "pisah", "pusat", "peran",
        "tahu", "tahun", "taruh", "tanah", "tanam",
    ]),
];

/// Kalimat sehari-hari yang HARUS bersih (nol koreksi).
const KALIMAT_BENAR: &[(&str, &[&str])] = &[
    ("percakapan kantor", &[
        "Tolong kirim laporan keuangan sebelum jam tiga sore",
        "Rapat besok pagi dipindah ke ruang meeting lantai dua",
        "Saya sudah mengirimkan berkas yang diminta kemarin",
        "Mohon konfirmasi kehadiran anda paling lambat hari Jumat",
        "Anggaran tambahan sudah disetujui oleh direksi minggu lalu",
        "Tim pemasaran sedang menyiapkan materi presentasi untuk klien",
        "Jangan lupa membawa dokumen asli dan fotokopinya besok",
        "Hasil evaluasi kinerja akan dibagikan setelah rapat selesai",
        "Silakan hubungi bagian keuangan untuk keterangan lebih lanjut",
        "Proses perpanjangan izin memakan waktu sekitar dua minggu",
    ]),
    ("catatan pribadi", &[
        "Beli beras gula kopi dan sabun cuci di pasar",
        "Antar adik ke sekolah jam tujuh pagi setiap hari",
        "Bayar listrik dan air sebelum tanggal dua puluh",
        "Jangan lupa minum obat setelah makan siang nanti",
        "Cuci mobil hari Minggu kalau cuacanya cerah",
        "Telepon ibu malam ini untuk menanyakan kabarnya",
        "Simpan struk belanja untuk catatan pengeluaran bulanan",
        "Siapkan pakaian kerja untuk besok sebelum tidur",
    ]),
    ("kalimat dengan kata rawan", &[
        "Mereka pergi ziarah ke makam pahlawan pagi tadi",
        "Saya makan nasi goreng di warung dekat kantor",
        "Acara itu penuh haru dan banyak yang menangis",
        "Kami harus menyelesaikan pekerjaan ini hari ini",
        "Kirim undangan lewat surel ke seluruh peserta rapat",
        "Ada rusa berkeliaran di hutan dekat desa itu",
        "Dia merasa terharu melihat kejutan dari teman-temannya",
        "Berita terbaru menyebutkan harga bahan pokok naik lagi",
    ]),
    ("campur Indonesia-Inggris", &[
        "Meeting besok jam sembilan tolong siapkan slide presentasi",
        "Deadline laporan tahunan sudah semakin dekat minggu ini",
        "Tim teknis akan melakukan maintenance server malam ini",
        "Please send the meeting notes before the deadline tomorrow",
        "Saya sudah upload file laporan ke folder shared drive",
        "Tolong review draft proposal ini sebelum dikirim ke client",
    ]),
    ("zona terlindungi", &[
        "Kirim ke budi.santoso@kantor.co.id sebelum sore ini",
        "Buka https://laporan.internal.co.id/dashboard untuk detailnya",
        "Jangan lupa mention @tim-keuangan di channel #proyek-q3",
        "Jalankan `npm run build` lalu periksa hasilnya di folder dist",
        "Dokumen PDF itu dikirim lewat surel oleh HRD kemarin",
        "Versi aplikasi v2 build A1B2 sudah rilis kemarin sore",
    ]),
];

/// Typo yang SEHARUSNYA terdeteksi (minimal ditandai).
const TYPO_HARUS_KENA: &[(&str, &str)] = &[
    ("yagn", "yang"), ("dnegan", "dengan"), ("tidka", "tidak"),
    ("adlah", "adalah"), ("seprti", "seperti"), ("untk", "untuk"),
    ("mkaan", "makan"), ("banayk", "banyak"), ("sekarng", "sekarang"),
    ("sudh", "sudah"), ("belm", "belum"), ("masi", "masih"),
    ("membhas", "membahas"), ("sebelm", "sebelum"), ("tambhan", "tambahan"),
    ("pengujan", "pengujian"), ("buln", "bulan"), ("cepta", "cepat"),
    ("recieve", "receive"), ("seperate", "separate"), ("definately", "definitely"),
    ("tommorow", "tomorrow"), ("becuase", "because"), ("thier", "their"),
    ("disini", "di sini"), ("terimakasih", "terima kasih"),
    ("resiko", "risiko"), ("praktek", "praktik"), ("ijin", "izin"),
    ("ino", "ini"), ("inu", "ini"), ("nii", "ini"), ("iin", "ini"),
    ("sangatt", "sangat"), ("sebentarr", "sebentar"), ("pekerjaaan", "pekerjaan"),
];

struct Hasil {
    total: usize,
    gagal: Vec<String>,
}

fn uji_kosakata(c: &Corrector) -> BTreeMap<&'static str, Hasil> {
    let mut out = BTreeMap::new();
    for (kategori, kata_kata) in KOSAKATA_BENAR {
        let mut h = Hasil { total: kata_kata.len(), gagal: Vec::new() };
        for kata in *kata_kata {
            let koreksi = c.check(kata);
            if let Some(k) = koreksi.first() {
                h.gagal.push(format!("{kata} -> {} ({:.0}%{})", k.to, k.confidence * 100.0,
                    if k.auto { ", OTOMATIS" } else { "" }));
            }
        }
        out.insert(*kategori, h);
    }
    out
}

fn uji_kalimat(c: &Corrector) -> BTreeMap<&'static str, Hasil> {
    let mut out = BTreeMap::new();
    for (kategori, kalimat_kalimat) in KALIMAT_BENAR {
        let mut h = Hasil { total: kalimat_kalimat.len(), gagal: Vec::new() };
        for kalimat in *kalimat_kalimat {
            let koreksi = c.check(kalimat);
            if !koreksi.is_empty() {
                let detail: Vec<String> = koreksi.iter()
                    .map(|k| format!("{}->{}{}", k.from, k.to, if k.auto { "*" } else { "" }))
                    .collect();
                h.gagal.push(format!("\"{kalimat}\"\n        [{}]", detail.join(", ")));
            }
        }
        out.insert(*kategori, h);
    }
    out
}

fn uji_typo(c: &Corrector) -> (usize, usize, Vec<String>, Vec<String>) {
    let mut kena = 0;
    let mut lewat = Vec::new();
    let mut salah_arah = Vec::new();
    for (salah, benar) in TYPO_HARUS_KENA {
        let koreksi = c.check(salah);
        match koreksi.first() {
            Some(k) if k.to.eq_ignore_ascii_case(benar) => kena += 1,
            Some(k) => salah_arah.push(format!("{salah} -> {} (harusnya {benar})", k.to)),
            None => lewat.push(format!("{salah} (harusnya {benar})")),
        }
    }
    (kena, TYPO_HARUS_KENA.len(), lewat, salah_arah)
}

fn main() {
    let c = build_corrector(Some(&bundled_dict_dir()), Settings::default());
    let (id, en) = c.dict_sizes();
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  AUDIT MESIN KOREKSI                                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("Kamus: {id} kata ID, {en} kata EN | Konteks T2: {} ({} bigram)\n",
        if c.has_context() { "aktif" } else { "MATI" }, c.bigram_len());

    // ---- 1. Kosakata benar (false positive paling berbahaya) ----
    println!("─── 1. KOSAKATA BENAR (tidak boleh dikoreksi) ───");
    let mut fp_total = 0;
    let mut kata_total = 0;
    for (kategori, h) in uji_kosakata(&c) {
        kata_total += h.total;
        fp_total += h.gagal.len();
        let status = if h.gagal.is_empty() { "OK  " } else { "GAGAL" };
        println!("  [{status}] {kategori:<28} {}/{} bersih", h.total - h.gagal.len(), h.total);
        for g in &h.gagal {
            println!("        ! {g}");
        }
    }

    // ---- 2. Kalimat sehari-hari ----
    println!("\n─── 2. KALIMAT SEHARI-HARI (harus nol koreksi) ───");
    let mut kal_fp = 0;
    let mut kal_total = 0;
    for (kategori, h) in uji_kalimat(&c) {
        kal_total += h.total;
        kal_fp += h.gagal.len();
        let status = if h.gagal.is_empty() { "OK  " } else { "GAGAL" };
        println!("  [{status}] {kategori:<28} {}/{} bersih", h.total - h.gagal.len(), h.total);
        for g in &h.gagal {
            println!("        ! {g}");
        }
    }

    // ---- 3. Typo yang harus terdeteksi ----
    println!("\n─── 3. TYPO (harus terdeteksi & benar) ───");
    let (kena, total, lewat, salah) = uji_typo(&c);
    println!("  Terkoreksi benar : {kena}/{total} ({:.0}%)", kena as f64 * 100.0 / total as f64);
    if !lewat.is_empty() {
        println!("  Tidak terdeteksi ({}):", lewat.len());
        for l in &lewat { println!("        - {l}"); }
    }
    if !salah.is_empty() {
        println!("  Salah arah ({}):", salah.len());
        for s in &salah { println!("        ! {s}"); }
    }

    // ---- 3b. Batasan yang diketahui & diterima ----
    //
    // Kata sah tapi sangat khusus (nama tempat, istilah keagamaan) yang
    // kebetulan berjarak satu ketukan dari kata yang jauh lebih umum.
    // Menambahkannya ke kamus akan mematikan koreksi typo yang lebih sering
    // berguna, jadi sengaja dibiarkan. Pengguna punya tombol "Simpan" untuk
    // memasukkannya ke kamus pribadi.
    println!("\n─── 3b. BATASAN DIKETAHUI (bukan kegagalan) ───");
    for kata in ["kuta", "kada", "sana", "sini"] {
        match c.check(kata).first() {
            Some(k) => println!("  ~ {kata:<8} -> {:<8} {:.0}%{}", k.to, k.confidence * 100.0,
                if k.auto { "  OTOMATIS <-- perlu ditinjau" } else { "  (hanya ditandai, aman)" }),
            None => println!("  ~ {kata:<8} dibiarkan"),
        }
    }

    // ---- 4. Kasus tepi ----
    println!("\n─── 4. KASUS TEPI ───");
    let tepi: &[(&str, &str)] = &[
        ("teks kosong", ""),
        ("hanya spasi", "   \n\t  "),
        ("satu huruf", "a"),
        ("angka saja", "12345"),
        ("tanda baca", "!!!???..."),
        ("emoji", "halo 👋 dunia 🌍"),
        ("beraksen", "kafé naïve résumé"),
        ("baris sangat panjang", "kata "),
        ("karakter kontrol", "a\tb\rc"),
    ];
    let mut panik = 0;
    for (nama, teks) in tepi {
        let teks_full = if *nama == "baris sangat panjang" { teks.repeat(5000) } else { teks.to_string() };
        let hasil = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let k = c.check(&teks_full);
            let _ = apply(&teks_full, &k);
            k.len()
        }));
        match hasil {
            Ok(n) => println!("  [OK  ] {nama:<24} {n} koreksi"),
            Err(_) => { println!("  [PANIK] {nama}"); panik += 1; }
        }
    }

    // ---- 5. Konsistensi antar tingkat agresivitas ----
    println!("\n─── 5. AGRESIVITAS (teks bersih di semua tingkat) ───");
    let contoh_bersih = "Saya sudah mengirimkan laporan keuangan ke bagian administrasi kemarin sore";
    for t in [Aggressiveness::Konservatif, Aggressiveness::Seimbang, Aggressiveness::Agresif] {
        c.set_settings(Settings { aggressiveness: t, ..Settings::default() });
        let k = c.check(contoh_bersih);
        let status = if k.is_empty() { "OK  " } else { "GAGAL" };
        println!("  [{status}] {:<14} {} koreksi", format!("{t:?}"), k.len());
        for x in &k { println!("        ! {}->{}", x.from, x.to); }
    }
    c.set_settings(Settings::default());

    // ---- Ringkasan ----
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  RINGKASAN                                                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  Kosakata benar    : {}/{} bersih  ({} false positive)",
        kata_total - fp_total, kata_total, fp_total);
    println!("  Kalimat benar     : {}/{} bersih  ({} false positive)",
        kal_total - kal_fp, kal_total, kal_fp);
    println!("  Typo terdeteksi   : {kena}/{total}");
    println!("  Kasus tepi panik  : {panik}");
    let verdict = if fp_total == 0 && kal_fp == 0 && panik == 0 { "LULUS" } else { "PERLU PERHATIAN" };
    println!("\n  VERDICT: {verdict}\n");
}
