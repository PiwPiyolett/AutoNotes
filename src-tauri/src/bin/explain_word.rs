//! Bedah keputusan mesin koreksi untuk satu kata.
//!
//! ```text
//! cargo run --release --bin explain_word -- dpan membhas finsh
//! ```
//!
//! Menampilkan seluruh kandidat dari kedua kamus lengkap dengan biaya edit,
//! frekuensi, dan skor noisy-channel — supaya kesalahan peringkat bisa
//! dilacak ke angkanya, bukan ditebak.

use catatan_anti_typo_lib::corrector::{
    resources::{build_corrector, bundled_dict_dir},
    Settings,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("pakai: explain_word <kata> [kata...]");
        std::process::exit(1);
    }

    let c = build_corrector(Some(&bundled_dict_dir()), Settings::default());

    for kata in &args {
        println!("\n═══ {kata} ═══");
        for (nama, dict) in [("ID", c.dict_id()), ("EN", c.dict_en())] {
            let cands = dict.lookup_candidates(kata, 2, 8);
            println!("  [{nama}]  ada di kamus: {}", dict.contains(kata));
            if cands.is_empty() {
                println!("      (tidak ada kandidat)");
                continue;
            }
            for (i, x) in cands.iter().enumerate() {
                println!(
                    "      {}{:<16} jarak {}  biaya {:.2}  freq {:>7}  skor {:>7.2}",
                    if i == 0 { "► " } else { "  " },
                    x.word,
                    x.distance,
                    x.cost,
                    x.freq,
                    x.score()
                );
            }
        }
        let koreksi = c.check(kata);
        match koreksi.first() {
            Some(k) => println!(
                "  KEPUTUSAN: {} -> {}  (yakin {:.0}%, {})",
                k.from,
                k.to,
                k.confidence * 100.0,
                if k.auto { "otomatis" } else { "hanya ditandai" }
            ),
            None => println!("  KEPUTUSAN: dibiarkan"),
        }
    }
    println!();
}
