<div align="center">

# 📝 AutoNotes — Catatan Anti-Typo

**Aplikasi pencatat desktop yang memperbaiki salah ketik secara otomatis — sepenuhnya offline.**

Koreksi typo Indonesia + Inggris dalam **< 1 ms**, jalan di setiap ketukan tombol tanpa terasa.

![Tauri](https://img.shields.io/badge/Tauri_2-24C8DB?style=flat-square&logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React-20232A?style=flat-square&logo=react&logoColor=61DAFB)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white)
![Vite](https://img.shields.io/badge/Vite-646CFF?style=flat-square&logo=vite&logoColor=white)
![SQLite](https://img.shields.io/badge/SQLite_FTS5-003B57?style=flat-square&logo=sqlite&logoColor=white)

<img src="Screenshot%202026-09-02%20140400.png" alt="AutoNotes" width="720">

</div>

---

## ✨ Fitur

- ⚡ **Koreksi typo < 1 ms** — cukup ringan untuk berjalan di setiap ketukan tombol.
- 🇮🇩 **Sadar konteks (T2 bigram)** — 279 rb pasangan kata: `makam` → `makan`, `haru` → `harus`, `ino` → `ini`.
- 📴 **100% offline** — tidak ada data yang keluar dari perangkatmu.
- 🗂️ **Penyimpanan Markdown + indeks SQLite** — catatan tersimpan rapi & portabel.
- 🔎 **Pencarian FTS5** — pencarian awalan yang toleran terhadap typo.
- 🔗 **Jembatan Tauri** — 15 command Rust ⇄ React untuk performa native.

## 🛠️ Tech Stack

**Frontend:** React · TypeScript · Vite
**Backend/native:** Tauri 2 · Rust
**Data:** Markdown + SQLite (FTS5)

## 🚀 Menjalankan

```bash
# Pasang dependency
npm install

# Mode pengembangan (buka jendela desktop)
npm run tauri dev

# Build installer aplikasi
npm run tauri build
```

> Butuh [Rust](https://rustup.rs) dan prasyarat [Tauri](https://tauri.app/start/prerequisites/) terpasang. Folder build `src-tauri/target/` sengaja diabaikan dari repo karena berukuran besar.

---

<div align="center">

**Ariqo Banyusila Abrar** · [@PiwPiyolett](https://github.com/PiwPiyolett)

</div>
