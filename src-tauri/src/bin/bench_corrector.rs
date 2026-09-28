//! Pengukur performa mesin koreksi.
//!
//! Jalankan dengan:
//! ```text
//! cargo run --release --bin bench_corrector
//! ```
//!
//! Angka yang penting bukan rata-rata, melainkan **p99** — satu hentakan
//! 80 ms sekali-sekali lebih terasa mengganggu saat mengetik daripada
//! rata-rata 5 ms yang stabil.

use std::time::Instant;

use catatan_anti_typo_lib::corrector::{
    apply,
    resources::{bundled_dict_dir, build_corrector},
    Settings,
};

const PARAGRAF: &str = "\
Rapat pagi ini membahas rencana peluncuran produk baru pada kuartal depan. \
Tim pemasaran diminta menyiapkan materi presentasi sebelum hari Jumat, \
sementara tim teknis harus menyelesaikan pengujian akhir dan mengirimkan \
laporan hasilnya. Anggaran tambahan sudah disetujui, tetapi penggunaannya \
tetap harus dilaporkan setiap bulan. Beberapa catatan penting lainnya akan \
dikirim menyusul lewat surel kepada seluruh peserta rapat hari ini juga.";

const PARAGRAF_TYPO: &str = "\
Rapat pagi ini membhas rencana peluncuran produk baru pda kuartal depan. \
Tim pemasaran dimnta menyiapkan materi presentasi sebelm hari Jumat, \
sementara tim teknis harus menyelesaikan penguijan akhir dan mengirimkan \
laporan hasilnya. Anggaran tambhan sudah disetujui, tetapi penggunaannya \
tetap harus dilaporkan setiap buln.";

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

fn ukur(nama: &str, iterasi: usize, mut f: impl FnMut() -> usize) {
    // Pemanasan agar cache CPU dan alokator sudah panas.
    for _ in 0..20 {
        f();
    }

    let mut waktu = Vec::with_capacity(iterasi);
    let mut total_koreksi = 0usize;
    for _ in 0..iterasi {
        let t0 = Instant::now();
        total_koreksi += f();
        waktu.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    waktu.sort_by(|a, b| a.partial_cmp(b).unwrap());

    println!(
        "  {nama:<34} rata2 {:>7.3} ms   p50 {:>7.3}   p99 {:>7.3}   (koreksi: {})",
        waktu.iter().sum::<f64>() / waktu.len() as f64,
        percentile(&waktu, 0.50),
        percentile(&waktu, 0.99),
        total_koreksi / iterasi,
    );
}

fn main() {
    println!("\n=== Pemuatan kamus (sekali saat aplikasi dibuka) ===");
    let t0 = Instant::now();
    let c = build_corrector(Some(&bundled_dict_dir()), Settings::default());
    let muat = t0.elapsed();
    let (id, en) = c.dict_sizes();
    println!("  {id} kata ID + {en} kata EN dalam {:.0} ms", muat.as_secs_f64() * 1000.0);
    println!("  kamus siap dipakai menandai typo: {}", c.dictionaries_ready());

    println!("\n=== Pemeriksaan teks ===");
    ukur("satu kata benar", 20_000, || c.check("mengerjakan").len());
    ukur("satu kata typo", 20_000, || c.check("mengerjakn").len());
    ukur("kalimat pendek (8 kata)", 10_000, || {
        c.check("saya sedang mengerjakan catatan rapat hari ini").len()
    });
    ukur("paragraf bersih (70 kata)", 2_000, || c.check(PARAGRAF).len());
    ukur("paragraf ber-typo (55 kata)", 2_000, || c.check(PARAGRAF_TYPO).len());

    let panjang = PARAGRAF.repeat(15); // ~1.000 kata, catatan yang sudah panjang
    ukur("catatan panjang (~1.000 kata)", 200, || c.check(&panjang).len());

    println!("\n=== Contoh hasil ===");
    let koreksi = c.check(PARAGRAF_TYPO);
    for k in &koreksi {
        println!(
            "  {:<12} -> {:<14} yakin {:.0}%  {}",
            k.from,
            k.to,
            k.confidence * 100.0,
            if k.auto { "otomatis" } else { "hanya ditandai" }
        );
    }
    println!("\n  hasil akhir:\n  {}", apply(PARAGRAF_TYPO, &koreksi));
    println!();
}
