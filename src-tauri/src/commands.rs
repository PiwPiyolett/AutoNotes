//! Jembatan Tauri: perintah yang bisa dipanggil dari sisi web.
//!
//! Semua fungsi di sini adalah pembungkus tipis di atas [`crate::corrector`]
//! dan [`crate::store`]. Kesalahan diubah menjadi `String` supaya bisa
//! diserialkan ke JavaScript; jangan taruh logika bisnis di sini.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use parking_lot::RwLock;
use serde::Serialize;

use crate::corrector::{
    apply, apply_auto,
    resources::{self, build_corrector, bundled_dict_dir},
    Aggressiveness, Corrector, Correction, Settings,
};
use crate::store::{Note, NoteMeta, SearchHit, Vault};

pub struct AppState {
    pub corrector: RwLock<Arc<Corrector>>,
    pub vault: Vault,
}

impl AppState {
    /// `resource_dict_dir` adalah folder kamus hasil bundling Tauri (dipakai
    /// saat aplikasi sudah terpasang). Kalau tidak ada di sana — misalnya
    /// menjalankan lewat `tauri dev` — jatuh ke folder kamus di pohon sumber.
    pub fn init(data_dir: PathBuf, resource_dict_dir: Option<PathBuf>) -> Result<Self> {
        let dict_dir = match resource_dict_dir {
            Some(d) if d.join(resources::WORDS_ID).exists() => d,
            _ => bundled_dict_dir(),
        };
        let corrector = build_corrector(Some(&dict_dir), Settings::default());
        let vault = Vault::open(&data_dir.join("catatan"), &data_dir.join("index.db"))?;
        // Bangun ulang indeks kalau kosong (aplikasi baru dibuka atau berkas
        // ditambah dari luar). Murah kalau memang tidak ada apa-apa.
        let _ = vault.reindex();
        Ok(Self {
            corrector: RwLock::new(Arc::new(corrector)),
            vault,
        })
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ============ koreksi ==================================================

#[derive(Serialize)]
pub struct CheckResult {
    pub corrections: Vec<Correction>,
    pub auto_applied: String,
}

/// Periksa teks. Mengembalikan daftar koreksi + teks setelah koreksi
/// otomatis diterapkan. UI menampilkan `auto_applied` di editor dan
/// memakai `corrections` untuk menggambar garis bergelombang.
#[tauri::command]
pub fn check_text(state: tauri::State<'_, AppState>, text: String) -> CheckResult {
    let c = state.corrector.read().clone();
    let corrections = c.check(&text);
    let auto_applied = apply_auto(&text, &corrections);
    CheckResult { corrections, auto_applied }
}

/// Terapkan **semua** koreksi (termasuk yang cuma ditandai) — dipakai
/// tombol "Perbaiki semua" pada catatan lama.
#[tauri::command]
pub fn fix_all(state: tauri::State<'_, AppState>, text: String) -> String {
    let c = state.corrector.read().clone();
    apply(&text, &c.check(&text))
}

#[tauri::command]
pub fn corrector_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.corrector.read().settings()
}

#[tauri::command]
pub fn set_corrector_settings(state: tauri::State<'_, AppState>, settings: Settings) {
    state.corrector.read().set_settings(settings);
}

#[tauri::command]
pub fn dict_status(state: tauri::State<'_, AppState>) -> DictStatus {
    let c = state.corrector.read();
    let (id, en) = c.dict_sizes();
    DictStatus {
        id_words: id,
        en_words: en,
        ready: c.dictionaries_ready(),
        context_enabled: c.has_context(),
        bigram_pairs: c.bigram_len(),
    }
}

#[derive(Serialize)]
pub struct DictStatus {
    pub id_words: usize,
    pub en_words: usize,
    pub ready: bool,
    /// Tier 2 (koreksi sadar-konteks) aktif?
    pub context_enabled: bool,
    pub bigram_pairs: usize,
}

#[tauri::command]
pub fn add_to_user_dict(state: tauri::State<'_, AppState>, word: String) {
    state.corrector.read().userdict().write().add_word(&word);
}

#[tauri::command]
pub fn record_undo(state: tauri::State<'_, AppState>, from: String, to: String) {
    state.corrector.read().userdict().write().record_undo(&from, &to);
}

// ============ catatan ==================================================

#[tauri::command]
pub fn list_notes(state: tauri::State<'_, AppState>, limit: Option<usize>) -> Result<Vec<NoteMeta>, String> {
    state.vault.list(limit.unwrap_or(500)).map_err(err)
}

#[tauri::command]
pub fn load_note(state: tauri::State<'_, AppState>, id: String) -> Result<Option<Note>, String> {
    let _ = state.vault.touch(&id, now());
    state.vault.load(&id).map_err(err)
}

#[tauri::command]
pub fn create_note(state: tauri::State<'_, AppState>, body: String) -> Result<Note, String> {
    state.vault.create(&body, now()).map_err(err)
}

#[tauri::command]
pub fn save_note(state: tauri::State<'_, AppState>, note: Note) -> Result<(), String> {
    let mut n = note;
    n.updated = now();
    // Judul hanya mengikuti isi selama pengguna BELUM mengetik judul sendiri.
    // Kalau `title_manual` sudah true, judul manual dijaga apa adanya.
    if !n.title_manual {
        n.title = crate::store::derive_title(&n.body, "Tanpa judul");
    } else if n.title.trim().is_empty() {
        // Judul manual dikosongkan → kembali ke mode otomatis.
        n.title = crate::store::derive_title(&n.body, "Tanpa judul");
        n.title_manual = false;
    }
    state.vault.save(&n).map_err(err)
}

#[tauri::command]
pub fn trash_note(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    state.vault.trash(&id, now()).map_err(err)
}

#[tauri::command]
pub fn search_notes(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    // Kueri ikut dikoreksi supaya mengetik "anggran" tetap menemukan
    // "anggaran". Bentuk asli tetap dicari lewat OR — kalau pengguna memang
    // sengaja mencari "anggran", hasilnya tidak hilang.
    let c = state.corrector.read().clone();
    let koreksi = c.check(&query);
    let mut varian = Vec::new();
    if !koreksi.is_empty() {
        let baru = apply(&query, &koreksi);
        if baru != query {
            varian.push(baru);
        }
    }
    state.vault.search(&query, &varian, limit.unwrap_or(50)).map_err(err)
}

/// Statistik singkat untuk status bar.
#[tauri::command]
pub fn stats(state: tauri::State<'_, AppState>) -> Result<Stats, String> {
    Ok(Stats {
        total_notes: state.vault.count().map_err(err)?,
    })
}

#[derive(Serialize)]
pub struct Stats {
    pub total_notes: usize,
}

/// Ekspor catatan ke berkas. `path` sudah dipilih pengguna lewat dialog
/// "Save As" di frontend; di sini Rust hanya membangun isi & menuliskannya.
#[tauri::command]
pub fn export_note(
    path: String,
    format: String,
    title: String,
    body: String,
) -> Result<(), String> {
    let fmt = crate::export::Format::parse(&format)
        .ok_or_else(|| format!("format tak dikenal: {format}"))?;
    crate::export::export_note(std::path::Path::new(&path), fmt, &title, &body).map_err(err)
}

/// Semua `Aggressiveness` yang tersedia — dipakai UI settings.
#[tauri::command]
pub fn aggressiveness_options() -> Vec<Aggressiveness> {
    vec![
        Aggressiveness::Konservatif,
        Aggressiveness::Seimbang,
        Aggressiveness::Agresif,
    ]
}
