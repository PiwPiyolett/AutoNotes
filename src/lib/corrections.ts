/**
 * Penerapan koreksi di sisi klien.
 *
 * # Kenapa ada TextEncoder di sini
 *
 * Rust mengembalikan `start`/`end` sebagai **offset byte UTF-8**. JavaScript
 * mengindeks string dengan **satuan UTF-16**. Untuk teks ASCII keduanya sama,
 * tapi begitu ada huruf beraksen ("kafé"), emoji, atau karakter non-latin,
 * `text.slice(c.start, c.end)` akan meleset dan merusak catatan. Jadi
 * penggantian dilakukan di tingkat byte, lalu didekode kembali.
 *
 * Selain itu offset bisa **basi**: pengguna mungkin mengetik lagi setelah
 * pemeriksaan terakhir. Karena itu kita verifikasi dulu bahwa byte di rentang
 * itu memang kata `from`; kalau tidak cocok, jatuh ke pencarian kemunculan
 * pertama `from`. Dengan begitu tombol "Terapkan" tidak pernah mengganti
 * potongan teks yang salah.
 */

import type { Correction } from "./api";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function applyCorrectionToText(text: string, c: Correction): string {
  const bytes = encoder.encode(text);

  // Jalur cepat & tepat: offset masih valid dan cocok dengan `from`.
  if (c.end <= bytes.length && c.start <= c.end) {
    const segment = decoder.decode(bytes.slice(c.start, c.end));
    if (segment === c.from) {
      const before = decoder.decode(bytes.slice(0, c.start));
      const after = decoder.decode(bytes.slice(c.end));
      return before + c.to + after;
    }
  }

  // Jalur cadangan: offset basi — ganti kemunculan pertama `from` yang utuh.
  const idx = text.indexOf(c.from);
  if (idx >= 0) {
    return text.slice(0, idx) + c.to + text.slice(idx + c.from.length);
  }

  // `from` sudah tidak ada (mungkin sudah diperbaiki) — biarkan apa adanya.
  return text;
}

/** Panjang rentang koreksi dalam satuan UTF-16, untuk menandai kilas. */
export function correctionSpanInText(text: string, c: Correction): { start: number; end: number } | null {
  const idx = text.indexOf(c.to);
  if (idx < 0) return null;
  return { start: idx, end: idx + c.to.length };
}
