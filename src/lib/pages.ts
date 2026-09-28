/**
 * Penanda halaman di dalam catatan.
 *
 * Cermin dari logika di `src-tauri/src/export.rs` — keduanya harus sepakat
 * soal bentuk penanda, jadi kalau salah satu diubah, ubah keduanya.
 *
 * Yang disimpan di catatan adalah penanda **netral tanpa nomor**. Nomor
 * halaman dihitung saat ekspor, sehingga menyisipkan halaman di tengah tidak
 * pernah membuat penomoran basi.
 */

/** Bentuk penanda yang disisipkan aplikasi. */
export const PAGE_MARKER = "=============== HALAMAN ===============";

/**
 * Batas karakter per halaman. Dipilih agar satu halaman kira-kira muat dalam
 * satu lembar A4 saat diekspor ke PDF/Word (A4 11pt ≈ 3.000–3.500 karakter).
 * Diberlakukan lewat `maxLength` textarea — mengetik berhenti saat penuh dan
 * pengguna membuat halaman baru untuk melanjutkan.
 */
export const PAGE_CHAR_LIMIT = 3000;

/** Apakah baris ini penanda halaman? Parsernya sengaja longgar. */
export function isPageMarker(line: string): boolean {
  return /^\s*={3,}\s*halaman\s*={3,}\s*$/i.test(line);
}

/**
 * Pecah teks tersimpan menjadi array halaman (untuk editor per-halaman).
 * Cermin `split_pages` di Rust. Selalu mengembalikan minimal satu halaman.
 */
export function splitPages(text: string): string[] {
  const pages: string[] = [""];
  for (const line of text.split("\n")) {
    if (isPageMarker(line)) {
      pages.push("");
    } else {
      const i = pages.length - 1;
      pages[i] = pages[i] === "" ? line : `${pages[i]}\n${line}`;
    }
  }
  return pages.map((p) => p.replace(/^\n+|\n+$/g, ""));
}

/**
 * Gabungkan array halaman kembali jadi satu teks tersimpan, dipisah penanda.
 * Round-trip dengan splitPages: split(join(pages)) === pages (untuk isi wajar).
 */
export function joinPages(pages: string[]): string {
  return pages.join(`\n${PAGE_MARKER}\n`);
}

/** Berapa halaman dalam catatan ini? Selalu minimal 1. */
export function countPages(text: string): number {
  let n = 1;
  for (const line of text.split("\n")) {
    if (isPageMarker(line)) n += 1;
  }
  return n;
}

/** Halaman ke berapa posisi kursor berada (1-indeks). */
export function pageAt(text: string, pos: number): number {
  let n = 1;
  let idx = 0;
  for (const line of text.split("\n")) {
    if (idx >= pos) break;
    if (isPageMarker(line)) n += 1;
    idx += line.length + 1;
  }
  return n;
}

/**
 * Sisipkan penanda halaman di posisi kursor.
 *
 * Penanda selalu berdiri di barisnya sendiri dengan baris kosong di atas dan
 * bawah, supaya tetap terbaca sebagai pemisah di editor mana pun.
 */
export function insertPageBreak(
  value: string,
  selStart: number,
  selEnd: number,
): { next: string; caret: number } {
  const before = value.slice(0, selStart);
  const after = value.slice(selEnd);
  // Rapatkan baris kosong berlebih di titik sambung agar tidak menumpuk.
  const kiri = before.replace(/\n+$/, "");
  const kanan = after.replace(/^\n+/, "");
  const awal = kiri.length > 0 ? `${kiri}\n\n` : "";
  const blok = `${PAGE_MARKER}\n\n`;
  const next = awal + blok + kanan;
  return { next, caret: (awal + blok).length };
}
