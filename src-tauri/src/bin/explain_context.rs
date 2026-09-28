//! Uji koreksi sadar-konteks (Tier 2) pada kalimat penuh.
//!
//! ```text
//! cargo run --release --bin explain_context
//! ```

use catatan_anti_typo_lib::corrector::{
    apply,
    resources::{build_corrector, bundled_dict_dir},
    Settings,
};

const KALIMAT: &[&str] = &[
    "saya makam nasi di warung",
    "kita akan makan nasi bersama",
    "dia menangis karena sangat haru",
    "acara itu penuh haru dan bahagia",
    "tolong beli koran hari ini",
    "ini adalah versi terbaru aplikasi",
    "mereka pergi ke makam pahlawan",
    "aku merasa haru melihatnya",
];

fn main() {
    let c = build_corrector(Some(&bundled_dict_dir()), Settings::default());
    println!(
        "Konteks Tier 2 aktif: {}  ({} bigram)\n",
        c.has_context(),
        c.bigram_len()
    );

    for teks in KALIMAT {
        let koreksi = c.check(teks);
        let hasil = apply(teks, &koreksi);
        if koreksi.is_empty() {
            println!("  (bersih)   {teks}");
        } else {
            let detail: Vec<String> = koreksi
                .iter()
                .map(|k| {
                    format!(
                        "{}→{} [{:?} {:.0}%{}]",
                        k.from,
                        k.to,
                        k.source,
                        k.confidence * 100.0,
                        if k.auto { " auto" } else { "" }
                    )
                })
                .collect();
            println!("  {teks}");
            println!("     => {hasil}");
            println!("     {}", detail.join(", "));
        }
    }
    println!();
}
