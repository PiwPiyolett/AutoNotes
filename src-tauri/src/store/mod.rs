//! Penyimpanan hibrida: berkas Markdown sebagai sumber kebenaran,
//! SQLite sebagai indeks yang boleh dibuang kapan saja.
//!
//! ```text
//!   catatan/                        <- SUMBER KEBENARAN
//!     rapat-anggaran-k3m4n5.md
//!     daftar-belanja-p6q7r8.md
//!     .trash/
//!   index.db                        <- INDEKS, bisa dihapus & dibangun ulang
//! ```
//!
//! # Kenapa dibagi begini
//!
//! Menaruh catatan di dalam basis data membuat pencarian cepat tapi menyandera
//! datanya: kalau aplikasi ini berhenti dikembangkan, catatan pengguna ikut
//! terkubur. Menaruhnya sebagai berkas polos membebaskan datanya tapi membuat
//! pencarian melambat begitu catatan mencapai ribuan.
//!
//! Pembagian ini mengambil keduanya. Berkas `.md` bisa dibuka Notepad,
//! di-`git commit`, dan disinkronkan lewat Google Drive. `index.db` hanya
//! cache: hapus, dan [`Vault::reindex`] membangunnya ulang dari disk.
//! Tidak ada satu pun data yang hanya hidup di basis data.

pub mod frontmatter;
pub mod search;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use ulid::Ulid;

pub use frontmatter::{derive_title, Note};
pub use search::SearchHit;

const SKEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS notes (
  id             TEXT    NOT NULL PRIMARY KEY,
  path           TEXT    NOT NULL UNIQUE,
  title          TEXT    NOT NULL,
  body           TEXT    NOT NULL,
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  pinned         INTEGER NOT NULL DEFAULT 0,
  archived       INTEGER NOT NULL DEFAULT 0,
  trashed_at     INTEGER,
  content_hash   TEXT    NOT NULL,
  opened_count   INTEGER NOT NULL DEFAULT 0,
  last_opened_at INTEGER
);

CREATE INDEX IF NOT EXISTS notes_updated ON notes(updated_at DESC);
CREATE INDEX IF NOT EXISTS notes_trashed ON notes(trashed_at);

CREATE TABLE IF NOT EXISTS note_tags (
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  tag     TEXT NOT NULL,
  PRIMARY KEY (note_id, tag)
);
CREATE INDEX IF NOT EXISTS note_tags_tag ON note_tags(tag);

-- Pencarian kata kunci. `prefix` memungkinkan pencarian sambil mengetik:
-- "ang" sudah menemukan "anggaran" tanpa menunggu kata selesai.
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
  title, body,
  tokenize = 'unicode61 remove_diacritics 2',
  prefix = '2 3 4'
);

-- Indeks dijaga lewat pemicu, bukan lewat kode Rust, supaya tidak mungkin
-- ada jalur penulisan yang lupa memperbaruinya.
CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
  INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;
CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
  DELETE FROM notes_fts WHERE rowid = old.rowid;
END;
CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
  DELETE FROM notes_fts WHERE rowid = old.rowid;
  INSERT INTO notes_fts(rowid, title, body) VALUES (new.rowid, new.title, new.body);
END;

-- Kamus pribadi & ingatan pembatalan koreksi (lihat corrector::userdict).
CREATE TABLE IF NOT EXISTS user_dict (
  word     TEXT NOT NULL PRIMARY KEY,
  added_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS undo_memory (
  from_word TEXT NOT NULL,
  to_word   TEXT NOT NULL,
  count     INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (from_word, to_word)
);

-- Riwayat versi. Murah, dan sekali waktu menyelamatkan catatan penting.
CREATE TABLE IF NOT EXISTS versions (
  note_id  TEXT    NOT NULL,
  saved_at INTEGER NOT NULL,
  body     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS versions_note ON versions(note_id, saved_at DESC);
"#;

/// Ringkasan catatan untuk daftar, tanpa isi, supaya memuat 10.000 catatan
/// tetap ringan.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NoteMeta {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub pinned: bool,
    pub archived: bool,
    pub tags: Vec<String>,
}

pub struct Vault {
    dir: PathBuf,
    db: Mutex<Connection>,
}

impl Vault {
    /// Buka (atau buat) brankas catatan.
    ///
    /// `dir` adalah folder berkas `.md`; `db_path` adalah berkas indeks.
    /// Keduanya dibuat kalau belum ada.
    pub fn open(dir: &Path, db_path: &Path) -> Result<Self> {
        fs::create_dir_all(dir).with_context(|| format!("membuat folder catatan {dir:?}"))?;
        fs::create_dir_all(dir.join(".trash"))?;
        if let Some(induk) = db_path.parent() {
            fs::create_dir_all(induk)?;
        }

        let db = Connection::open(db_path).with_context(|| format!("membuka indeks {db_path:?}"))?;
        db.execute_batch(SKEMA).context("menyiapkan skema indeks")?;

        Ok(Self { dir: dir.to_path_buf(), db: Mutex::new(db) })
    }

    /// Brankas dengan indeks di memori, untuk pengujian.
    #[cfg(test)]
    fn open_memori(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        fs::create_dir_all(dir.join(".trash"))?;
        let db = Connection::open_in_memory()?;
        db.execute_batch(SKEMA)?;
        Ok(Self { dir: dir.to_path_buf(), db: Mutex::new(db) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    // ---- tulis ----------------------------------------------------------

    /// Buat catatan baru dari isi mentah. Judul diturunkan dari baris pertama.
    pub fn create(&self, body: &str, now: i64) -> Result<Note> {
        let id = Ulid::generate().to_string();
        let title = derive_title(body, "Tanpa judul");
        let note = Note::new(id, title, body.to_string(), now);
        let path = self.nama_berkas(&note);
        self.tulis_berkas(&path, &note)?;
        self.rekam(&note, &path)?;
        Ok(note)
    }

    /// Simpan perubahan. Berkasnya **tidak** diganti nama meski judul berubah.
    ///
    /// Judul diturunkan dari baris pertama, jadi ia berubah tiap kali pengguna
    /// mengetik. Mengganti nama berkas setiap kali akan membuat folder sinkron
    /// (Drive/OneDrive) sibuk menghapus-membuat berkas tanpa henti, dan riwayat
    /// `git` jadi tidak terbaca. Identitas catatan ada pada `id` di
    /// frontmatter, bukan pada nama berkasnya.
    pub fn save(&self, note: &Note) -> Result<()> {
        let path = match self.path_dari_id(&note.id)? {
            Some(p) => p,
            None => self.nama_berkas(note),
        };
        self.tulis_berkas(&path, note)?;
        self.rekam(note, &path)?;
        Ok(())
    }

    /// Simpan potret isi ke riwayat versi.
    pub fn snapshot(&self, id: &str, body: &str, now: i64) -> Result<()> {
        self.db.lock().execute(
            "INSERT INTO versions (note_id, saved_at, body) VALUES (?1, ?2, ?3)",
            params![id, now, body],
        )?;
        Ok(())
    }

    /// Pindahkan ke tempat sampah. Berkasnya **tidak dihapus**, hanya dipindah
    /// ke `.trash/`. Menghapus permanen adalah tindakan terpisah yang harus
    /// diminta pengguna secara sadar.
    pub fn trash(&self, id: &str, now: i64) -> Result<()> {
        let Some(path) = self.path_dari_id(id)? else {
            return Ok(());
        };
        if path.exists() {
            let nama = path.file_name().unwrap_or_default().to_owned();
            let tujuan = self.dir.join(".trash").join(nama);
            if fs::rename(&path, &tujuan).is_err() {
                fs::copy(&path, &tujuan)?;
                let _ = fs::remove_file(&path);
            }
        }
        self.db.lock().execute(
            "UPDATE notes SET trashed_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    // ---- baca -----------------------------------------------------------

    /// Muat catatan **dari disk**, bukan dari indeks.
    ///
    /// Indeks bisa saja basi kalau pengguna menyunting berkas dari aplikasi
    /// lain. Berkas selalu jadi acuan.
    pub fn load(&self, id: &str) -> Result<Option<Note>> {
        let Some(path) = self.path_dari_id(id)? else {
            return Ok(None);
        };
        if !path.exists() {
            return Ok(None);
        }
        let src = fs::read_to_string(&path)?;
        Ok(Some(frontmatter::parse(&src, id, 0)))
    }

    /// Daftar catatan, terbaru dulu, yang disematkan di paling atas.
    pub fn list(&self, limit: usize) -> Result<Vec<NoteMeta>> {
        let db = self.db.lock();
        let mut stmt = db.prepare(
            "SELECT id, title, created_at, updated_at, pinned, archived
             FROM notes
             WHERE trashed_at IS NULL AND archived = 0
             ORDER BY pinned DESC, updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(NoteMeta {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
                pinned: r.get::<_, i64>(4)? != 0,
                archived: r.get::<_, i64>(5)? != 0,
                tags: Vec::new(),
            })
        })?;
        let mut out = Vec::new();
        for m in rows {
            let mut m = m?;
            m.tags = tags_dari(&db, &m.id)?;
            out.push(m);
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<usize> {
        let db = self.db.lock();
        let n: i64 = db.query_row(
            "SELECT COUNT(*) FROM notes WHERE trashed_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Catat bahwa catatan dibuka, bahan untuk peringkat *frecency*.
    pub fn touch(&self, id: &str, now: i64) -> Result<()> {
        self.db.lock().execute(
            "UPDATE notes SET opened_count = opened_count + 1, last_opened_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    // ---- indeks ---------------------------------------------------------

    /// Bangun ulang seluruh indeks dari berkas di disk.
    ///
    /// Ini jaring pengaman yang membuat pembagian berkas/indeks bermakna:
    /// indeks rusak, terhapus, atau ketinggalan karena pengguna menyunting
    /// berkas dari aplikasi lain, jalankan ini, semuanya pulih.
    pub fn reindex(&self) -> Result<usize> {
        let berkas = self.berkas_markdown()?;
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        tx.execute("DELETE FROM notes", [])?;

        let mut jumlah = 0usize;
        for path in berkas {
            let Ok(src) = fs::read_to_string(&path) else {
                continue;
            };
            let cadangan_id = Ulid::generate().to_string();
            let waktu = waktu_ubah(&path);
            let note = frontmatter::parse(&src, &cadangan_id, waktu);
            simpan_baris(&tx, &note, &path_relatif(&self.dir, &path), &hash(&src))?;
            jumlah += 1;
        }
        tx.commit()?;
        Ok(jumlah)
    }

    // ---- pencarian -------------------------------------------------------

    /// Cari catatan. `varian` adalah bentuk alternatif kueri, biasanya hasil
    /// koreksi typo pada kata kunci, sehingga mengetik "anggran" tetap
    /// menemukan catatan berisi "anggaran".
    pub fn search(&self, query: &str, varian: &[String], limit: usize) -> Result<Vec<SearchHit>> {
        let Some(fts) = search::build_query(query, varian) else {
            return Ok(Vec::new());
        };
        let db = self.db.lock();
        search::jalankan(&db, &fts, limit)
    }

    // ---- pembantu internal ----------------------------------------------

    fn nama_berkas(&self, note: &Note) -> PathBuf {
        let slug = search::slug(&note.title);
        let ekor: String = note.id.chars().rev().take(6).collect();
        self.dir.join(format!("{slug}-{ekor}.md"))
    }

    fn path_dari_id(&self, id: &str) -> Result<Option<PathBuf>> {
        let db = self.db.lock();
        let hasil: Option<String> = db
            .query_row("SELECT path FROM notes WHERE id = ?1", params![id], |r| r.get(0))
            .ok();
        Ok(hasil.map(|p| self.dir.join(p)))
    }

    fn tulis_berkas(&self, path: &Path, note: &Note) -> Result<()> {
        if let Some(induk) = path.parent() {
            fs::create_dir_all(induk)?;
        }
        // Tulis ke berkas sementara lalu ganti nama: kalau listrik mati di
        // tengah penulisan, catatan lama tetap utuh alih-alih jadi separuh.
        let sementara = path.with_extension("md.tmp");
        fs::write(&sementara, frontmatter::render(note))?;
        if fs::rename(&sementara, path).is_err() {
            fs::copy(&sementara, path)?;
            let _ = fs::remove_file(&sementara);
        }
        Ok(())
    }

    fn rekam(&self, note: &Note, path: &Path) -> Result<()> {
        let rel = path_relatif(&self.dir, path);
        let h = hash(&frontmatter::render(note));
        let db = self.db.lock();
        simpan_baris(&db, note, &rel, &h)
    }

    fn berkas_markdown(&self) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        kumpulkan_md(&self.dir, &mut out)?;
        out.sort();
        Ok(out)
    }
}

fn path_relatif(dir: &Path, p: &Path) -> String {
    p.strip_prefix(dir)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

fn tags_dari(db: &Connection, id: &str) -> Result<Vec<String>> {
    let mut stmt = db.prepare("SELECT tag FROM note_tags WHERE note_id = ?1 ORDER BY tag")?;
    let rows = stmt.query_map(params![id], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|t| t.ok()).collect())
}

fn kumpulkan_md(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for e in entries.flatten() {
        let p = e.path();
        let nama = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        // Folder tersembunyi (.trash, .git, .obsidian) tidak ikut diindeks.
        if nama.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            kumpulkan_md(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    Ok(())
}

fn simpan_baris(db: &Connection, note: &Note, rel: &str, hash: &str) -> Result<()> {
    db.execute(
        "INSERT INTO notes (id, path, title, body, created_at, updated_at, pinned, archived, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
           path = excluded.path, title = excluded.title, body = excluded.body,
           updated_at = excluded.updated_at, pinned = excluded.pinned,
           archived = excluded.archived, content_hash = excluded.content_hash,
           trashed_at = NULL",
        params![
            note.id, rel, note.title, note.body,
            note.created, note.updated,
            note.pinned as i64, note.archived as i64, hash
        ],
    )?;
    db.execute("DELETE FROM note_tags WHERE note_id = ?1", params![note.id])?;
    for tag in &note.tags {
        db.execute(
            "INSERT OR IGNORE INTO note_tags (note_id, tag) VALUES (?1, ?2)",
            params![note.id, tag.to_lowercase()],
        )?;
    }
    Ok(())
}

fn waktu_ubah(p: &Path) -> i64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// FNV-1a 64-bit dalam heksadesimal. Cukup untuk mendeteksi "berkas ini
/// berubah sejak terakhir diindeks", bukan untuk keperluan kriptografi.
fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sementara(PathBuf);
    impl Sementara {
        fn baru(nama: &str) -> Self {
            let p = std::env::temp_dir().join(format!("catatan-uji-{nama}-{}", Ulid::generate()));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Sementara {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn vault(nama: &str) -> (Sementara, Vault) {
        let t = Sementara::baru(nama);
        let v = Vault::open_memori(&t.0).unwrap();
        (t, v)
    }

    #[test]
    fn membuat_catatan_menulis_berkas_md() {
        let (_t, v) = vault("buat");
        let n = v.create("# Rapat anggaran\n\nisi catatan", 1000).unwrap();

        assert_eq!(n.title, "Rapat anggaran");
        let berkas = v.berkas_markdown().unwrap();
        assert_eq!(berkas.len(), 1);
        let isi = fs::read_to_string(&berkas[0]).unwrap();
        assert!(isi.contains("id: "), "berkas harus punya frontmatter:\n{isi}");
        assert!(isi.contains("isi catatan"));
        assert!(berkas[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("rapat-anggaran-"));
    }

    #[test]
    fn memuat_kembali_menghasilkan_catatan_yang_sama() {
        let (_t, v) = vault("muat");
        let n = v.create("# Judul\n\nisi", 1000).unwrap();
        let dimuat = v.load(&n.id).unwrap().unwrap();
        assert_eq!(dimuat.id, n.id);
        assert_eq!(dimuat.title, "Judul");
        assert_eq!(dimuat.body, "# Judul\n\nisi");
    }

    #[test]
    fn menyimpan_perubahan_tidak_mengganti_nama_berkas() {
        let (_t, v) = vault("simpan");
        let mut n = v.create("# Judul awal\n\nisi", 1000).unwrap();
        let nama_awal = v.berkas_markdown().unwrap()[0].file_name().unwrap().to_owned();

        n.body = "# Judul sudah berubah total\n\nisi lain".into();
        n.title = derive_title(&n.body, "-");
        n.updated = 2000;
        v.save(&n).unwrap();

        let berkas = v.berkas_markdown().unwrap();
        assert_eq!(berkas.len(), 1, "tidak boleh ada berkas kembar");
        assert_eq!(berkas[0].file_name().unwrap(), nama_awal, "nama berkas harus tetap");
        assert_eq!(
            v.load(&n.id).unwrap().unwrap().title,
            "Judul sudah berubah total"
        );
    }

    #[test]
    fn daftar_menyematkan_yang_dipin_di_atas() {
        let (_t, v) = vault("daftar");
        v.create("# Biasa satu\nisi", 1000).unwrap();
        let mut penting = v.create("# Penting\nisi", 1001).unwrap();
        v.create("# Biasa dua\nisi", 1002).unwrap();

        penting.pinned = true;
        v.save(&penting).unwrap();

        let daftar = v.list(10).unwrap();
        assert_eq!(daftar.len(), 3);
        assert_eq!(daftar[0].title, "Penting");
        assert!(daftar[0].pinned);
        assert_eq!(daftar[1].title, "Biasa dua", "sisanya terbaru dulu");
    }

    #[test]
    fn tempat_sampah_memindah_bukan_menghapus() {
        let (_t, v) = vault("sampah");
        let n = v.create("# Akan dibuang\nisi", 1000).unwrap();
        v.trash(&n.id, 2000).unwrap();

        assert_eq!(v.list(10).unwrap().len(), 0, "tidak boleh muncul di daftar");
        let di_sampah: Vec<_> = fs::read_dir(v.dir().join(".trash")).unwrap().flatten().collect();
        assert_eq!(di_sampah.len(), 1, "berkasnya harus pindah ke .trash, bukan lenyap");
    }

    #[test]
    fn tag_tersimpan_dan_dinormalkan() {
        let (_t, v) = vault("tag");
        let mut n = v.create("# Bertag\nisi", 1000).unwrap();
        n.tags = vec!["Kerja".into(), "rapat".into()];
        v.save(&n).unwrap();

        let daftar = v.list(10).unwrap();
        assert_eq!(daftar[0].tags, vec!["kerja", "rapat"]);
    }

    /// Uji yang membuktikan pembagian berkas/indeks benar-benar berarti:
    /// indeks boleh musnah tanpa satu pun catatan ikut hilang.
    #[test]
    fn indeks_bisa_dibangun_ulang_sepenuhnya_dari_disk() {
        let (_t, v) = vault("reindex");
        v.create("# Catatan satu\n\nisi pertama", 1000).unwrap();
        v.create("# Catatan dua\n\nisi kedua", 1001).unwrap();
        v.create("# Catatan tiga\n\nisi ketiga", 1002).unwrap();

        v.db.lock().execute("DELETE FROM notes", []).unwrap();
        assert_eq!(v.list(10).unwrap().len(), 0, "indeks sudah dihanguskan");

        let dipulihkan = v.reindex().unwrap();
        assert_eq!(dipulihkan, 3);
        let daftar = v.list(10).unwrap();
        assert_eq!(daftar.len(), 3);
        assert!(daftar.iter().any(|m| m.title == "Catatan dua"));
    }

    #[test]
    fn berkas_md_buatan_pengguna_ikut_terindeks() {
        let (_t, v) = vault("asing");
        // Pengguna menaruh berkas Markdown biasa, tanpa frontmatter.
        fs::write(
            v.dir().join("dari-obsidian.md"),
            "# Catatan luar\n\nditulis di aplikasi lain",
        )
        .unwrap();
        assert_eq!(v.reindex().unwrap(), 1);
        assert_eq!(v.list(10).unwrap()[0].title, "Catatan luar");
    }

    #[test]
    fn folder_tersembunyi_tidak_ikut_terindeks() {
        let (_t, v) = vault("sembunyi");
        v.create("# Nyata\nisi", 1000).unwrap();
        fs::create_dir_all(v.dir().join(".obsidian")).unwrap();
        fs::write(v.dir().join(".obsidian/config.md"), "# Bukan catatan").unwrap();
        assert_eq!(v.reindex().unwrap(), 1);
    }

    #[test]
    fn riwayat_versi_tersimpan() {
        let (_t, v) = vault("versi");
        let n = v.create("# Judul\nversi satu", 1000).unwrap();
        v.snapshot(&n.id, "versi satu", 1000).unwrap();
        v.snapshot(&n.id, "versi dua", 2000).unwrap();

        let db = v.db.lock();
        let jumlah: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM versions WHERE note_id = ?1",
                params![n.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(jumlah, 2);
    }

    // ---- pencarian -------------------------------------------------------

    fn vault_terisi(nama: &str) -> (Sementara, Vault) {
        let (t, v) = vault(nama);
        v.create("# Rapat anggaran kuartal tiga\n\npembahasan alokasi dana pemasaran", 1000).unwrap();
        v.create("# Daftar belanja mingguan\n\nberas gula kopi dan sabun cuci", 1001).unwrap();
        v.create("# Meeting notes\n\ndiscuss the project deadline with the team", 1002).unwrap();
        v.create("# Ide produk baru\n\naplikasi pencatat yang memperbaiki typo", 1003).unwrap();
        (t, v)
    }

    #[test]
    fn mencari_kata_di_judul() {
        let (_t, v) = vault_terisi("cari-judul");
        let h = v.search("anggaran", &[], 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].title, "Rapat anggaran kuartal tiga");
    }

    #[test]
    fn mencari_kata_di_isi() {
        let (_t, v) = vault_terisi("cari-isi");
        let h = v.search("sabun", &[], 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].title, "Daftar belanja mingguan");
    }

    /// Ini yang membuat pencarian terasa instan: hasil sudah muncul sebelum
    /// kata selesai diketik.
    #[test]
    fn mencari_sambil_mengetik_lewat_awalan() {
        let (_t, v) = vault_terisi("cari-awalan");
        for potongan in ["a", "ang", "angg", "anggar"] {
            let h = v.search(potongan, &[], 10).unwrap();
            assert!(
                h.iter().any(|x| x.title.contains("anggaran")),
                "'{potongan}' seharusnya sudah menemukan catatan anggaran"
            );
        }
    }

    #[test]
    fn beberapa_kata_harus_semuanya_cocok() {
        let (_t, v) = vault_terisi("cari-and");
        assert_eq!(v.search("rapat anggaran", &[], 10).unwrap().len(), 1);
        // "rapat" ada, "kopi" ada, tapi tidak di catatan yang sama.
        assert_eq!(v.search("rapat kopi", &[], 10).unwrap().len(), 0);
    }

    /// Orang yang buru-buru juga salah ketik saat mencari. Pencarian yang
    /// mengembalikan nol hasil karena satu huruf meleset membuat orang menyerah.
    #[test]
    fn kueri_ber_typo_tetap_menemukan_lewat_varian() {
        let (_t, v) = vault_terisi("cari-typo");
        assert_eq!(v.search("anggran", &[], 10).unwrap().len(), 0, "tanpa varian memang nihil");

        let h = v.search("anggran", &["anggaran".to_string()], 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].title, "Rapat anggaran kuartal tiga");
    }

    #[test]
    fn cuplikan_menandai_kata_yang_cocok() {
        let (_t, v) = vault_terisi("cuplikan");
        let h = v.search("pemasaran", &[], 10).unwrap();
        assert!(h[0].snippet.contains("pemasaran"), "cuplikan: {}", h[0].snippet);
    }

    #[test]
    fn pencarian_mengabaikan_catatan_di_tempat_sampah() {
        let (_t, v) = vault_terisi("cari-sampah");
        let target = v.search("anggaran", &[], 10).unwrap()[0].id.clone();
        v.trash(&target, 3000).unwrap();
        assert_eq!(v.search("anggaran", &[], 10).unwrap().len(), 0);
    }

    #[test]
    fn pencarian_menemukan_catatan_berbahasa_inggris() {
        let (_t, v) = vault_terisi("cari-en");
        assert_eq!(v.search("deadline", &[], 10).unwrap().len(), 1);
    }

    #[test]
    fn kueri_kosong_atau_tanda_baca_menghasilkan_nol_bukan_galat() {
        let (_t, v) = vault_terisi("cari-kosong");
        assert!(v.search("", &[], 10).unwrap().is_empty());
        assert!(v.search("   ", &[], 10).unwrap().is_empty());
        assert!(v.search("!!! ???", &[], 10).unwrap().is_empty());
    }

    #[test]
    fn sintaks_fts_dari_pengguna_tidak_membuat_galat() {
        let (_t, v) = vault_terisi("cari-sintaks");
        // Tidak boleh panik atau mengembalikan Err.
        assert!(v.search("\"rapat\" OR NEAR(", &[], 10).is_ok());
        assert!(v.search("rapat*", &[], 10).is_ok());
        assert!(v.search("^rapat", &[], 10).is_ok());
    }

    #[test]
    fn catatan_hilang_tidak_membuat_panik() {
        let (_t, v) = vault("hilang");
        assert!(v.load("id-yang-tidak-ada").unwrap().is_none());
        assert!(v.trash("id-yang-tidak-ada", 0).is_ok());
    }

    #[test]
    fn brankas_kosong_aman() {
        let (_t, v) = vault("kosong");
        assert_eq!(v.list(10).unwrap().len(), 0);
        assert_eq!(v.count().unwrap(), 0);
        assert_eq!(v.reindex().unwrap(), 0);
        assert!(v.search("apa saja", &[], 10).unwrap().is_empty());
    }
}
