# Catatan Anti-Typo

Aplikasi pencatat desktop yang memperbaiki salah ketik secara otomatis
(Indonesia + Inggris), berjalan **sepenuhnya offline**, dan dibuat untuk orang
yang mencatat sambil mengejar waktu.

Tauri 2 + Rust + React. Koreksi typo memakan **< 1 ms**, jadi bisa berjalan di
setiap ketukan tombol tanpa terasa.

---

## Status

| Bagian | Keadaan |
|---|---|
| Mesin koreksi typo (T0 + T1) | ✅ selesai, terkalibrasi |
| Kata-nyata langka (haru→harus, ino→ini) | ✅ selesai, prioritas kamus ID |
| **Koreksi sadar-konteks (T2 bigram)** | ✅ selesai, 279 rb pasangan, makam→makan |
| Penyimpanan Markdown + indeks SQLite | ✅ selesai |
| Pencarian FTS5 (awalan + toleran typo) | ✅ selesai |
| Jembatan Tauri (command + state) | ✅ selesai, 15 command |
| Antarmuka (editor + panel + pengaturan) | ✅ selesai |
| Quick capture window + global hotkey | ⬜ belum |
| Pemahaman topik penuh (LLM lokal) | ⬜ opsional, belum |

**180 tes lulus.** Mutu (eval 43 typo + 29 kalimat bersih, mode Seimbang): **74% diperbaiki otomatis, 0/29 teks bersih dirusak.**

---

## Menjalankan

```bash
npm install
```

```bash
npm run tauri dev
```

Uji, ukur performa, dan ukur mutu koreksi:

```bash
cd src-tauri && cargo test
```

```bash
cd src-tauri && cargo run --release --bin bench_corrector
```

```bash
cd src-tauri && cargo run --release --bin eval_corrector
```

Membedah kenapa satu kata dikoreksi begitu:

```bash
cd src-tauri && cargo run --release --bin explain_word -- dpan finsh membhas
```

---

## Mesin koreksi bertingkat

```
teks masuk
   │
   ├─ tokenizer ────────── ZONA TERLINDUNGI disingkirkan lebih dulu
   │                       URL, surel, @sebutan, #tagar, blok kode,
   │                       frontmatter, kata berangka, AKRONIM
   │
   ├─ T0  tabel ganti ──── yg→yang, recieve→receive        (~0 ms)
   │      576 entri, ditanam di dalam biner
   │
   ├─ T1  kamus SymSpell ─ mkaan→makan                      (<1 ms)
   │      └─ morfologi ─── "mengerjakannya" itu sah, jangan diutak-atik
   │
   └─ ranker ───────────── memilih kandidat + menghitung keyakinan
          └─ T2 mencolok di sini tanpa mengubah apa pun di atasnya
```

Tiap koreksi keluar dengan angka keyakinan. Yang tinggi diterapkan otomatis,
yang sedang hanya diberi garis bergelombang, yang rendah dibuang diam-diam.

### Mutu terukur

Diukur pada 43 kalimat ber-typo dan 29 kalimat yang sudah benar
(`cargo run --release --bin eval_corrector`):

| Tingkat | Diperbaiki otomatis | Terdeteksi | **Teks bersih dirusak** |
|---|---|---|---|
| Konservatif | 28% | 75% | **0 / 29** |
| Seimbang | 60% | 74% | **0 / 29** |
| Agresif | 74% | 74% | **0 / 29** |

Kolom terakhir yang paling penting. Melewatkan typo hanya membuat pengguna
kecewa; merusak kata yang sudah benar membuat mereka mematikan fiturnya —
dan sekali dimatikan, tidak pernah dinyalakan lagi.

Sisa ~26% yang lolos hampir seluruhnya **typo berupa kata sah**
(`kehadiran ada` untuk `kehadiran anda`). Kamus + jarak edit secara definisi
tidak bisa menangkap itu; perlu konteks kalimat. Itulah pekerjaan T2.

---

## Keputusan desain yang perlu diketahui

**Katup pengaman kamus.** T1 tidur total sampai kamus punya ≥ 5.000 kata
(`corrector::MIN_DICT_FOR_FLAGGING`). Dengan kamus rintisan, hampir setiap kata
terlihat "tidak dikenal" dan aplikasi berubah jadi mesin perusak teks. Tanpa
kamus pun aplikasi tetap aman dipakai — T0 jalan, T1 diam.

**Model noisy-channel.** Kandidat diperingkat dengan `ln(peluang) − λ·biaya`,
bukan "biaya dulu, frekuensi belakangan". Versi lama mengoreksi `dpan` jadi
`span` karena `d` dan `s` bertetangga di QWERTY. Lihat `symspell::LAMBDA`.

**Frekuensi dinormalkan jadi peluang.** Korpus ID dan EN berbeda ukuran, jadi
frekuensi mentahnya tidak sebanding. Tanpa normalisasi, `finsh` dikoreksi jadi
`finch` (dari korpus ID) alih-alih `finish` (dari korpus EN).

**Biaya edit asimetris.** Melewatkan huruf (0,85) jauh lebih murah daripada
menambah huruf (1,50), karena pengetik cepat memang lebih sering melewatkan.
Kecuali huruf kembar (`sangatt`, 0,75) yang justru sangat umum.

**Ambang kata dasar.** Analisis imbuhan hanya diterima kalau kata dasarnya
berfrekuensi ≥ 400 (`corrector::MIN_ROOT_FREQ`). Tanpa ini, `berlai` dikupas
jadi `ber` + `lai`, dan karena `lai` kebetulan ada di korpus (187×), typo-nya
dinyatakan sah. Ambang ini bisa diturunkan jauh kalau kamus KBBI terkurasi
dipasang — masalahnya lahir dari memakai korpus mentah sebagai kamus.

**Typo tidak boleh menghuni kamus.** Korpus subtitle memuat `disini` sebanyak
55.392 kali. Kalau dibiarkan, ia jadi sasaran koreksi yang menarik. Semua kunci
tabel typo dicekal dari kamus saat pemuatan. Kunci tabel *slang* sengaja
dibiarkan — `yg` memang ditulis orang dengan sadar.

**Tabel typo dan slang dipisah.** `yagn` tidak mungkin disengaja; `yg` mungkin
disengaja. Menyatukannya berarti pengguna tidak bisa mematikan yang satu tanpa
mematikan yang lain.

---

## Penyimpanan

```
catatan/                      <- SUMBER KEBENARAN
  rapat-anggaran-k3m4n5.md
  .trash/
index.db                      <- INDEKS, boleh dihapus kapan saja
```

Berkas `.md` biasa dengan frontmatter — bisa dibuka Notepad, di-`git commit`,
disinkronkan lewat Google Drive. `index.db` hanya cache; hapus, lalu
`Vault::reindex()` membangunnya ulang dari disk. Tidak ada satu pun data yang
hanya hidup di basis data.

Nama berkas **tidak** berubah meski judul berubah. Judul diturunkan dari baris
pertama sehingga berubah tiap ketukan tombol; mengganti nama berkas terus-menerus
akan membuat folder sinkron sibuk tanpa henti. Identitas ada pada `id` di
frontmatter.

Penulisan lewat berkas sementara lalu ganti nama, jadi listrik mati di tengah
penyimpanan tidak menghasilkan catatan separuh.

---

## Kamus

`src-tauri/resources/dict/`

| Berkas | Isi | Sumber |
|---|---|---|
| `typo_id.tsv` | 247 salah ketik ID | ditulis tangan |
| `typo_en.tsv` | 199 salah ketik EN | ditulis tangan |
| `slang_id.tsv` | 130 singkatan ID | ditulis tangan |
| `words_id.txt` | 50.000 kata + frekuensi | [FrequencyWords](https://github.com/hermitdave/FrequencyWords) (OpenSubtitles, CC-BY-SA 4.0) |
| `words_en.txt` | 50.000 kata + frekuensi | idem |

Format daftar kata: `kata<spasi/TAB>frekuensi`, atau `kata` saja (frekuensi
diambil dari urutan baris). Mau pakai kamus lain — KBBI, Hunspell id_ID,
`count_1w` Norvig — tinggal ganti berkasnya.

> **Peringatan soal tabel typo.** Jangan pernah memasukkan kata yang sah ke
> kolom kiri. `form→from` dan `si→is` sempat masuk lalu dicabut: `form` kata
> sah, dan `si` kata sandang Indonesia. Ada tes yang menguncinya
> (`tidak_mengoreksi_kata_yang_sebenarnya_sah`).

---

## Tier 2 — koreksi sadar-konteks

Aktif otomatis kalau `resources/dict/bigrams_id.tsv` ada. Dibangun dari
[Leipzig Corpora Collection](https://wortschatz.uni-leipzig.de/en/download/Indonesian)
(`ind_mixed_2013_1M`, CC-BY 4.0): **279 rb pasangan kata bersebelahan** dari
teks Indonesia sungguhan.

**Dua peran:**

1. **Menajamkan pilihan typo biasa.** `NgramRanker` memilih di antara kandidat
   dengan mempertimbangkan tetangga. Ini menaikkan auto-fix dari 60% → 74%.
2. **Menangkap typo yang kebetulan kata sah** (`corrector::context_correction`).
   "saya **makam** nasi" → "makan": `makam` valid, tapi "makan nasi" ada di
   korpus sedangkan "makam nasi" tidak pernah.

**Pengaman ketat** supaya kata benar tidak dikorbankan:

- Hanya menyala kalau kata yang diketik **tak** didukung konteks (bukti 0) —
  "makam pahlawan" aman karena "makam pahlawan" memang lazim.
- Kandidat wajib didukung **kedua** tetangga saat keduanya ada — inilah yang
  memblokir "beli koran" → "beri koran" (meski "tolong beri" sering, "beri
  koran" tak pernah).
- Kandidat wajib **lebih umum** dari kata yang diketik — memblokir "nasi"(911)
  → "napi"(343).
- **Tak pernah otomatis** — mengganti kata sah secara diam-diam terlalu berisiko.

**Batasnya (jujur):** bigram hanya paham **konteks lokal** (kata tetangga). Ia
menangani `makam`↔`makan`, `hari`↔`haru`. Tapi "paragraf ini soal perasaan →
pakai *terharu*" adalah pemahaman **topik**, dan `terharu` (145×) kalah telak
dari `harus` (260.853×) di tingkat kata. Itu butuh model bahasa besar — rencana
tombol "Rapikan" (LLM lokal opsional), bukan bigram.

Uji manual: `cargo run --release --bin explain_context`

### Arsitektur titik-colok

`trait Ranker` tetap jadi satu-satunya sambungan. Menaikkan/menurunkan T2:

```rust
corrector.enable_context(BigramModel::from_tsv(&src)); // pasang model + NgramRanker
```

`Context` membawa kata sebelum/sesudah ke `Ranker::pick`. Tokenizer, kamus,
morfologi, dan penyimpanan tidak tersentuh.
