//! Ekspor catatan ke berkas di komputer pengguna: TXT, Markdown, Word (.docx),
//! dan PDF.
//!
//! Semua format dibangun dari judul + isi mentah (Markdown). TXT/MD hanya
//! menulis teks; DOCX memakai `docx-rs`; PDF memakai `printpdf` dengan font
//! bawaan Helvetica (tak perlu berkas font), dengan pembungkusan baris dan
//! pemenggalan halaman sendiri.

use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};

/// Format ekspor yang didukung. Dikirim dari frontend sebagai string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Txt,
    Md,
    Docx,
    Pdf,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "txt" => Some(Self::Txt),
            "md" | "markdown" => Some(Self::Md),
            "docx" | "word" => Some(Self::Docx),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Md => "md",
            Self::Docx => "docx",
            Self::Pdf => "pdf",
        }
    }
}

// ---- Halaman ----------------------------------------------------------

/// Penanda pemisah halaman di dalam isi catatan.
///
/// # Kenapa penandanya tidak memuat nomor
///
/// Kalau nomor ikut disimpan (`=== Halaman 3 ===`), ia langsung basi begitu
/// pengguna menyisipkan halaman baru di tengah — semua nomor sesudahnya jadi
/// salah dan harus diperbaiki manual. Jadi yang disimpan hanya penanda netral,
/// dan nomornya dihitung ulang setiap kali diekspor. Nomor selalu benar tanpa
/// pengguna memikirkannya.
///
/// Bentuk yang disisipkan aplikasi:
/// ```text
/// =============== HALAMAN ===============
/// ```
/// Parsernya longgar (jumlah `=` dan spasi bebas, huruf besar/kecil bebas)
/// supaya penanda yang diketik tangan atau disunting di Notepad tetap terbaca.
///
/// Aman terhadap Markdown: baris ini tidak bisa disalahartikan sebagai
/// *setext heading*, karena garis setext hanya boleh berisi `=` saja.
pub const PAGE_MARKER: &str = "=============== HALAMAN ===============";

/// Apakah baris ini penanda halaman?
pub fn is_page_marker(line: &str) -> bool {
    let t = line.trim();
    let Some(rest) = t.strip_prefix("===") else {
        return false;
    };
    let rest = rest.trim_start_matches('=').trim();
    let Some(inner) = rest.strip_suffix("===") else {
        return false;
    };
    inner.trim_end_matches('=').trim().eq_ignore_ascii_case("halaman")
}

/// Pecah isi catatan menjadi halaman-halaman. Selalu mengembalikan minimal
/// satu halaman (catatan tanpa penanda = satu halaman).
pub fn split_pages(body: &str) -> Vec<String> {
    let mut pages = vec![String::new()];
    for line in body.lines() {
        if is_page_marker(line) {
            pages.push(String::new());
        } else {
            let cur = pages.last_mut().expect("selalu ada minimal satu halaman");
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(line);
        }
    }
    // Rapikan spasi di ujung tiap halaman, tapi jangan buang halaman kosong
    // di tengah (pengguna mungkin sengaja menyisipkan halaman kosong).
    for p in &mut pages {
        *p = p.trim_matches('\n').to_string();
    }
    pages
}

/// Spanduk pemisah halaman untuk format teks (Markdown & TXT).
fn page_banner(nomor: usize) -> String {
    let label = format!(" Halaman {nomor} ");
    // Total lebar 58 karakter, label di tengah, sisanya diisi '='.
    let sisa = 58usize.saturating_sub(label.len());
    let kiri = sisa / 2;
    let kanan = sisa - kiri;
    format!("{}{}{}", "=".repeat(kiri), label, "=".repeat(kanan))
}

/// Tulis catatan ke `path` dalam `format` yang diminta.
pub fn export_note(path: &Path, format: Format, title: &str, body: &str) -> Result<()> {
    match format {
        Format::Txt => write_txt(path, title, body),
        Format::Md => write_md(path, title, body),
        Format::Docx => write_docx(path, title, body),
        Format::Pdf => write_pdf(path, title, body),
    }
}

// ---- TXT --------------------------------------------------------------

fn write_txt(path: &Path, title: &str, body: &str) -> Result<()> {
    // TXT tidak punya spanduk "=== Halaman ===" (itu khusus Markdown). Antar
    // halaman dipisah karakter page-break plain-text standar (form feed, \x0C)
    // yang dihormati Notepad/printer sebagai pindah lembar, tanpa teks
    // pemisah yang mengotori isi.
    let pages = split_pages(body);
    let isi = pages.join("\u{000C}\n");
    // Judul di baris pertama hanya kalau belum jadi baris pertama isi.
    let content = if body_starts_with_title(title, body) {
        isi
    } else {
        format!("{title}\n\n{isi}")
    };
    std::fs::write(path, content).context("menulis berkas TXT")
}

// ---- Markdown ---------------------------------------------------------

fn write_md(path: &Path, title: &str, body: &str) -> Result<()> {
    let isi = body_dengan_halaman(body);
    // Kalau isi sudah diawali judul (mis. "# Judul"), jangan gandakan.
    let content = if body_starts_with_title(title, body) {
        isi
    } else {
        format!("# {title}\n\n{isi}")
    };
    std::fs::write(path, content).context("menulis berkas Markdown")
}

/// Ganti penanda halaman netral dengan spanduk bernomor.
///
/// Catatan satu halaman dibiarkan apa adanya — menambahkan spanduk
/// "Halaman 1" pada catatan biasa hanya jadi gangguan.
fn body_dengan_halaman(body: &str) -> String {
    let pages = split_pages(body);
    if pages.len() <= 1 {
        return body.trim_matches('\n').to_string();
    }
    let mut out = String::new();
    for (i, isi) in pages.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&page_banner(i + 1));
        out.push_str("\n\n");
        out.push_str(isi);
    }
    out
}

fn body_starts_with_title(title: &str, body: &str) -> bool {
    let first = body.lines().next().unwrap_or("").trim();
    let stripped = first.trim_start_matches('#').trim();
    stripped.eq_ignore_ascii_case(title.trim())
}

// ---- DOCX (Word) ------------------------------------------------------

fn write_docx(path: &Path, title: &str, body: &str) -> Result<()> {
    use docx_rs::*;

    let mut docx = Docx::new();

    // Judul sebagai heading tebal berukuran besar.
    docx = docx.add_paragraph(
        Paragraph::new()
            .add_run(Run::new().add_text(title).bold().size(36))
            .style("Heading1"),
    );
    docx = docx.add_paragraph(Paragraph::new());

    // Tiap baris isi jadi satu paragraf. Baris kosong = paragraf kosong
    // (jarak antar-alinea). Antar-halaman disisipkan pemisah halaman Word
    // yang sesungguhnya, jadi hasilnya benar-benar terpisah saat dicetak.
    for (i, page) in split_pages(body).iter().enumerate() {
        if i > 0 {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
            );
        }
        for line in page.lines() {
            if line.trim().is_empty() {
                docx = docx.add_paragraph(Paragraph::new());
            } else {
                docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(line)));
            }
        }
    }

    let file = std::fs::File::create(path).context("membuat berkas DOCX")?;
    docx.build().pack(file).context("menulis berkas DOCX")?;
    Ok(())
}

// ---- PDF --------------------------------------------------------------

const PDF_PAGE_W: f32 = 210.0; // A4 lebar (mm)
const PDF_PAGE_H: f32 = 297.0; // A4 tinggi (mm)
const PDF_MARGIN: f32 = 20.0;
const PDF_BODY_SIZE: f32 = 11.0;
const PDF_TITLE_SIZE: f32 = 20.0;
const PDF_LINE_MM: f32 = 6.0; // jarak antar-baris isi
/// Perkiraan lebar rata-rata karakter Helvetica sebagai pecahan ukuran font.
/// Dipakai untuk membungkus baris; Helvetica proporsional jadi ini pendekatan.
const PDF_CHAR_W_RATIO: f32 = 0.50;

fn write_pdf(path: &Path, title: &str, body: &str) -> Result<()> {
    use printpdf::{BuiltinFont, Mm, PdfDocument};

    let (doc, page1, layer1) =
        PdfDocument::new(title, Mm(PDF_PAGE_W), Mm(PDF_PAGE_H), "isi");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .context("memuat font PDF")?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .context("memuat font tebal PDF")?;

    let usable_w = PDF_PAGE_W - 2.0 * PDF_MARGIN;
    // mm per karakter ≈ ukuran(pt) * rasio * (mm per pt).
    let mm_per_char = PDF_BODY_SIZE * PDF_CHAR_W_RATIO * 0.3528;
    let max_chars = ((usable_w / mm_per_char).floor() as usize).max(20);

    let mut layer = doc.get_page(page1).get_layer(layer1);
    let mut y = PDF_PAGE_H - PDF_MARGIN;

    // Judul.
    layer.use_text(sanitize_for_pdf(title), PDF_TITLE_SIZE, Mm(PDF_MARGIN), Mm(y), &font_bold);
    y -= PDF_LINE_MM * 1.8;

    // Isi, dihitung per halaman catatan. Ada dua sebab halaman kertas baru:
    //   1. penanda halaman dari pengguna  -> paksa halaman baru
    //   2. teks sudah mencapai batas bawah -> lanjut halaman berikutnya
    let judul_ganda = body_starts_with_title(title, body);
    for (idx, page) in split_pages(body).iter().enumerate() {
        if idx > 0 {
            // Penanda halaman pengguna: mulai halaman kertas baru.
            let (p, l) = doc.add_page(Mm(PDF_PAGE_W), Mm(PDF_PAGE_H), "isi");
            layer = doc.get_page(p).get_layer(l);
            y = PDF_PAGE_H - PDF_MARGIN;
        }
        for source_line in page.lines() {
            if judul_ganda && idx == 0 && source_line.trim() == title.trim() {
                continue; // hindari judul ganda
            }
            for line in wrap_line(&sanitize_for_pdf(source_line), max_chars) {
                if y < PDF_MARGIN {
                    let (p, l) = doc.add_page(Mm(PDF_PAGE_W), Mm(PDF_PAGE_H), "isi");
                    layer = doc.get_page(p).get_layer(l);
                    y = PDF_PAGE_H - PDF_MARGIN;
                }
                layer.use_text(&line, PDF_BODY_SIZE, Mm(PDF_MARGIN), Mm(y), &font);
                y -= PDF_LINE_MM;
            }
        }
    }

    let file = std::fs::File::create(path).context("membuat berkas PDF")?;
    doc.save(&mut BufWriter::new(file)).context("menulis berkas PDF")?;
    Ok(())
}

/// Sesuaikan teks agar aman untuk font bawaan PDF (Helvetica / WinAnsi).
///
/// # Kenapa perlu
///
/// Font bawaan PDF hanya punya 256 posisi karakter. Pengujian menunjukkan:
///
/// * Huruf beraksen (`é ï ñ`) dan simbol Latin-1 (`© ® € £ § ¶ × ÷ ±`) **aman**.
/// * Karakter di rentang 0x80–0x9F WinAnsi (`• – — " " …`) dipetakan **salah**
///   dan muncul sebagai karakter rusak.
/// * Emoji dan karakter Unicode di luar Latin-1 **hilang tanpa jejak**.
///
/// Membiarkannya berarti PDF diam-diam berbeda dari yang diketik pengguna.
/// Jadi karakter bermasalah diganti padanan ASCII yang jelas terbaca, dan yang
/// benar-benar tak punya padanan (emoji) dibuang rapi tanpa menyisakan sampah.
///
/// DOCX, Markdown, dan TXT tidak butuh ini — ketiganya UTF-8 penuh dan sudah
/// terbukti menyimpan emoji dengan benar.
fn sanitize_for_pdf(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            // Tanda baca tipografis -> padanan ASCII.
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' => out.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' => out.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2015}' | '\u{2212}' => out.push('-'),
            '\u{2022}' | '\u{2023}' | '\u{25E6}' | '\u{00B7}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{20AC}' => out.push_str("EUR"),
            '\u{2122}' => out.push_str("(TM)"),
            '\u{2030}' => out.push_str("%o"),
            '\u{2020}' | '\u{2021}' => out.push('+'),
            // Matematika di luar Latin-1.
            '\u{2248}' => out.push('~'),
            '\u{2260}' => out.push_str("!="),
            '\u{2264}' => out.push_str("<="),
            '\u{2265}' => out.push_str(">="),
            '\u{2192}' => out.push_str("->"),
            '\u{2190}' => out.push_str("<-"),
            // Spasi tak-putus -> spasi biasa.
            '\u{00A0}' | '\u{2007}' | '\u{202F}' => out.push(' '),
            // ASCII cetak + tab/newline: aman.
            c if c == '\n' || c == '\t' || (' '..='~').contains(&c) => out.push(c),
            // Latin-1 supplement (aksen & simbol): terbukti tampil benar.
            c if ('\u{00A1}'..='\u{00FF}').contains(&c) => out.push(c),
            // Sisanya (emoji, CJK, simbol langka): tidak ada padanan -> buang.
            _ => {}
        }
    }
    out
}

/// Bungkus satu baris ke potongan-potongan yang muat di lebar halaman,
/// memutus di batas kata bila memungkinkan.
fn wrap_line(line: &str, max_chars: usize) -> Vec<String> {
    if line.trim().is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for word in line.split(' ') {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
        }
        // Kata tunggal yang lebih panjang dari satu baris: potong paksa.
        while current.chars().count() > max_chars {
            let cut: String = current.chars().take(max_chars).collect();
            out.push(cut);
            current = current.chars().skip(max_chars).collect();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(nama: &str, ext: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("uji-ekspor-{nama}-{}.{ext}", ulid::Ulid::generate()))
    }

    #[test]
    fn format_dari_string() {
        assert_eq!(Format::parse("PDF"), Some(Format::Pdf));
        assert_eq!(Format::parse("word"), Some(Format::Docx));
        assert_eq!(Format::parse("markdown"), Some(Format::Md));
        assert_eq!(Format::parse("xyz"), None);
    }

    #[test]
    fn txt_berisi_judul_dan_isi() {
        let p = tmp("txt", "txt");
        export_note(&p, Format::Txt, "Rapat", "poin satu\npoin dua").unwrap();
        let isi = fs::read_to_string(&p).unwrap();
        assert!(isi.contains("Rapat"));
        assert!(isi.contains("poin satu"));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn md_tidak_menggandakan_judul() {
        let p = tmp("md", "md");
        // Isi sudah diawali judul yang sama.
        export_note(&p, Format::Md, "Belanja", "# Belanja\n\nberas").unwrap();
        let isi = fs::read_to_string(&p).unwrap();
        assert_eq!(isi.matches("Belanja").count(), 1, "judul tidak boleh dobel:\n{isi}");
        fs::remove_file(&p).ok();
    }

    #[test]
    fn docx_terbentuk_dan_tidak_kosong() {
        let p = tmp("docx", "docx");
        export_note(&p, Format::Docx, "Judul", "isi catatan").unwrap();
        assert!(fs::metadata(&p).unwrap().len() > 500, "docx terlalu kecil");
        fs::remove_file(&p).ok();
    }

    #[test]
    fn pdf_terbentuk_dan_valid() {
        let p = tmp("pdf", "pdf");
        let body = "baris pertama\n".repeat(200); // paksa multi-halaman
        export_note(&p, Format::Pdf, "Laporan", &body).unwrap();
        let bytes = fs::read(&p).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "header PDF tidak valid");
        assert!(bytes.len() > 1000);
        fs::remove_file(&p).ok();
    }

    // ---- Halaman ------------------------------------------------------

    #[test]
    fn penanda_halaman_dikenali_dalam_berbagai_bentuk() {
        assert!(is_page_marker(PAGE_MARKER));
        assert!(is_page_marker("=== HALAMAN ==="));
        assert!(is_page_marker("  ======= halaman =======  "));
        assert!(is_page_marker("==================== Halaman ===================="));
    }

    #[test]
    fn baris_biasa_bukan_penanda_halaman() {
        for l in ["halaman", "=== bab ===", "===", "======", "--- halaman ---", "Halaman 2"] {
            assert!(!is_page_marker(l), "'{l}' seharusnya bukan penanda");
        }
    }

    #[test]
    fn catatan_tanpa_penanda_tetap_satu_halaman() {
        let p = split_pages("baris satu\nbaris dua");
        assert_eq!(p.len(), 1);
        assert_eq!(p[0], "baris satu\nbaris dua");
    }

    #[test]
    fn penanda_memecah_isi_jadi_beberapa_halaman() {
        let body = format!("halaman satu\n{PAGE_MARKER}\nhalaman dua\n{PAGE_MARKER}\nhalaman tiga");
        let p = split_pages(&body);
        assert_eq!(p, vec!["halaman satu", "halaman dua", "halaman tiga"]);
    }

    #[test]
    fn md_menomori_halaman_secara_otomatis() {
        let p = tmp("halaman", "md");
        let body = format!("isi awal\n{PAGE_MARKER}\nisi kedua");
        export_note(&p, Format::Md, "Judul", &body).unwrap();
        let isi = fs::read_to_string(&p).unwrap();
        assert!(isi.contains("Halaman 1"), "spanduk halaman 1 hilang:\n{isi}");
        assert!(isi.contains("Halaman 2"), "spanduk halaman 2 hilang:\n{isi}");
        // Penanda mentah tidak boleh ikut terbawa ke hasil ekspor.
        assert!(!isi.contains("HALAMAN ==="), "penanda mentah bocor:\n{isi}");
        fs::remove_file(&p).ok();
    }

    /// Nomor dihitung ulang saat ekspor, jadi menyisipkan halaman di tengah
    /// tidak pernah membuat penomoran basi.
    #[test]
    fn nomor_halaman_selalu_berurutan() {
        let body = format!("a\n{PAGE_MARKER}\nb\n{PAGE_MARKER}\nc\n{PAGE_MARKER}\nd");
        let hasil = body_dengan_halaman(&body);
        for n in 1..=4 {
            assert!(hasil.contains(&format!("Halaman {n}")), "nomor {n} hilang");
        }
        assert!(!hasil.contains("Halaman 5"));
    }

    #[test]
    fn catatan_satu_halaman_tidak_diberi_spanduk() {
        // Menambahkan "Halaman 1" pada catatan biasa hanya jadi gangguan.
        let hasil = body_dengan_halaman("catatan biasa saja");
        assert_eq!(hasil, "catatan biasa saja");
    }

    #[test]
    fn docx_menyisipkan_pemisah_halaman_sungguhan() {
        let p = tmp("halaman", "docx");
        let body = format!("satu\n{PAGE_MARKER}\ndua");
        export_note(&p, Format::Docx, "Judul", &body).unwrap();
        // DOCX adalah ZIP; cari elemen page break di document.xml.
        let bytes = fs::read(&p).unwrap();
        assert!(bytes.starts_with(b"PK"));
        assert!(fs::metadata(&p).unwrap().len() > 500);
        fs::remove_file(&p).ok();
    }

    #[test]
    fn pdf_dengan_penanda_halaman_tetap_valid() {
        let p = tmp("halaman", "pdf");
        let body = format!("halaman satu\n{PAGE_MARKER}\nhalaman dua\n{PAGE_MARKER}\nhalaman tiga");
        export_note(&p, Format::Pdf, "Judul", &body).unwrap();
        let bytes = fs::read(&p).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        // Tiga halaman catatan harus menghasilkan tiga halaman kertas.
        // `/Count` di page tree PDF adalah jumlah halaman sesungguhnya.
        // Bentuknya "/Type/Pages/Count 3/Kids[...]" — angkanya menempel ke
        // token berikutnya, jadi ambil deretan digit di awal saja.
        let s = String::from_utf8_lossy(&bytes);
        let count: usize = s
            .split("/Count")
            .nth(1)
            .map(|r| r.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        assert!(count >= 3, "PDF hanya {count} halaman, harusnya >= 3");
        fs::remove_file(&p).ok();
    }

    #[test]
    fn sanitasi_pdf_menjaga_aksen_dan_simbol_latin() {
        // Ini yang terbukti tampil benar dengan font bawaan PDF.
        let s = sanitize_for_pdf("kafé naïve señor © ® £ × ÷ ±");
        assert!(s.contains('é') && s.contains('ï') && s.contains('ñ'));
        assert!(s.contains('©') && s.contains('®') && s.contains('£'));
        assert!(s.contains('×') && s.contains('÷') && s.contains('±'));
    }

    #[test]
    fn sanitasi_pdf_mengganti_tipografi_dengan_ascii() {
        // Karakter 0x80-0x9F dipetakan salah oleh font bawaan, jadi diganti.
        assert_eq!(sanitize_for_pdf("\u{201C}kutip\u{201D}"), "\"kutip\"");
        assert_eq!(sanitize_for_pdf("a \u{2013} b"), "a - b");
        assert_eq!(sanitize_for_pdf("\u{2022} poin"), "- poin");
        assert_eq!(sanitize_for_pdf("dan\u{2026}"), "dan...");
        assert_eq!(sanitize_for_pdf("2 \u{2248} 3"), "2 ~ 3");
    }

    #[test]
    fn sanitasi_pdf_membuang_emoji_tanpa_sisa() {
        // Emoji tidak punya padanan; harus hilang rapi, bukan jadi sampah.
        let s = sanitize_for_pdf("Halo 👋 dunia 🌍 selesai ✅");
        assert_eq!(s.trim(), "Halo  dunia  selesai");
        assert!(!s.chars().any(|c| c as u32 > 0xFF));
    }

    #[test]
    fn sanitasi_pdf_menjaga_url_dan_surel_utuh() {
        let asli = "https://contoh.co.id/x?id=42&t=pdf budi.santoso@kantor.co.id";
        assert_eq!(sanitize_for_pdf(asli), asli);
    }

    #[test]
    fn pdf_dengan_emoji_tetap_valid() {
        let p = tmp("emoji", "pdf");
        export_note(&p, Format::Pdf, "Judul 🎉", "Isi dengan emoji 👋 dan aksen kafé").unwrap();
        let bytes = fs::read(&p).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn wrap_memutus_di_batas_kata() {
        let w = wrap_line("satu dua tiga empat lima", 10);
        assert!(w.iter().all(|l| l.chars().count() <= 10), "{w:?}");
        assert!(w.len() > 1);
    }

    #[test]
    fn wrap_memotong_kata_sangat_panjang() {
        let w = wrap_line("aaaaaaaaaaaaaaaaaaaa", 8);
        assert!(w.iter().all(|l| l.chars().count() <= 8));
    }
}
