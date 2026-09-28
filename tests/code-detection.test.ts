/**
 * Uji deteksi & pelabelan bahasa untuk paste kode.
 * Dijalankan lewat: npx esbuild --bundle ... | node
 */
import { looksLikeCode, detectLanguage, insideFence, wrapInFence } from "../src/lib/code";

type Kasus = { nama: string; teks: string; kode: boolean; lang?: string };

const KASUS: Kasus[] = [
  // ---- HTML ----
  { nama: "HTML satu baris", teks: `<div class="box">Halo</div>`, kode: true, lang: "html" },
  { nama: "HTML dokumen", teks: `<!DOCTYPE html>\n<html>\n<body>\n<h1>Judul</h1>\n</body>\n</html>`, kode: true, lang: "html" },
  { nama: "HTML self-closing", teks: `<img src="foto.jpg" alt="foto" />`, kode: true, lang: "html" },
  { nama: "HTML tag tunggal", teks: `<br>`, kode: true, lang: "html" },

  // ---- CSS ----
  { nama: "CSS aturan", teks: `body {\n  margin: 0;\n  padding: 10px;\n}`, kode: true, lang: "css" },
  { nama: "CSS satu baris", teks: `.btn { color: red; }`, kode: true, lang: "css" },
  { nama: "CSS media query", teks: `@media (max-width: 768px) {\n  .nav { display: none; }\n}`, kode: true, lang: "css" },

  // ---- JavaScript ----
  { nama: "JS fungsi", teks: `function hitung(a, b) {\n  return a + b;\n}`, kode: true, lang: "javascript" },
  { nama: "JS arrow", teks: `const total = items.reduce((a, b) => a + b, 0);`, kode: true, lang: "javascript" },
  { nama: "JS console", teks: `console.log("halo dunia");`, kode: true, lang: "javascript" },
  { nama: "JS import", teks: `import React from "react";\nexport default App;`, kode: true, lang: "javascript" },

  // ---- Python ----
  { nama: "Python def", teks: `def hitung(a, b):\n    return a + b`, kode: true, lang: "python" },
  { nama: "Python import", teks: `import pandas as pd\nfrom os import path`, kode: true, lang: "python" },
  { nama: "Python class", teks: `class Mobil:\n    def __init__(self):\n        self.roda = 4`, kode: true, lang: "python" },

  // ---- Bahasa lain ----
  { nama: "Java", teks: `public class Main {\n  public static void main(String[] args) {}\n}`, kode: true, lang: "java" },
  { nama: "C++", teks: `#include <iostream>\nint main() {\n  std::cout << "hi";\n}`, kode: true, lang: "cpp" },
  { nama: "PHP", teks: `<?php\necho "halo";\n?>`, kode: true, lang: "php" },
  { nama: "SQL", teks: `SELECT nama, umur FROM pengguna WHERE aktif = 1;`, kode: true, lang: "sql" },
  { nama: "Rust", teks: `fn main() {\n    println!("halo");\n}`, kode: true, lang: "rust" },
  { nama: "JSON", teks: `{\n  "nama": "Budi",\n  "umur": 30\n}`, kode: true },
  { nama: "shell", teks: `npm run build && node dist/index.js`, kode: true, lang: "bash" },

  // ---- BUKAN kode (tidak boleh dibungkus) ----
  { nama: "kalimat biasa", teks: `Saya mau makan nasi goreng hari ini`, kode: false },
  { nama: "kalimat perbandingan", teks: `harga 3 > 2 dan 5 < 10 saja`, kode: false },
  { nama: "alamat surel", teks: `kirim ke budi@kantor.co.id ya`, kode: false },
  { nama: "daftar belanja", teks: `1. beras\n2. gula\n3. kopi`, kode: false },
  { nama: "catatan rapat", teks: `Rapat besok jam 9 pagi di ruang meeting lantai 2`, kode: false },
  { nama: "kalimat berkurung", teks: `Tolong beli (kalau sempat) susu dan roti`, kode: false },
  { nama: "jam & tanggal", teks: `Deadline: 21 Agustus 2026 pukul 17:00`, kode: false },
  { nama: "URL biasa", teks: `Lihat di https://contoh.co.id/laporan untuk detail`, kode: false },
];

let lulus = 0;
let gagal = 0;
const salah: string[] = [];

console.log("\n=== UJI DETEKSI KODE ===\n");
console.log("status  bahasa      kasus");
console.log("-".repeat(60));

for (const k of KASUS) {
  const terdeteksi = looksLikeCode(k.teks);
  const lang = terdeteksi ? detectLanguage(k.teks) : "";
  let ok = terdeteksi === k.kode;
  if (ok && k.lang !== undefined) ok = lang === k.lang;

  if (ok) lulus++;
  else {
    gagal++;
    salah.push(`${k.nama}: kode=${terdeteksi} (harusnya ${k.kode})` +
      (k.lang !== undefined ? `, bahasa="${lang}" (harusnya "${k.lang}")` : ""));
  }
  console.log(`${ok ? " OK   " : " GAGAL"}  ${(lang || "-").padEnd(11)} ${k.nama}`);
}

console.log("\n=== UJI PEMBUNGKUSAN ===\n");
// Pembungkusan tidak boleh merusak teks di sekitarnya.
const v = "Catatan awal\n";
const { next, caret } = wrapInFence(v, v.length, v.length, `<b>x</b>`, "html");
const wrapOk = next.startsWith("Catatan awal\n") && next.includes("```html") && next.trim().endsWith("```");
console.log(`${wrapOk ? " OK   " : " GAGAL"}  bungkus menjaga teks sekitar`);
console.log(`${caret === next.length ? " OK   " : " GAGAL"}  posisi kursor setelah blok`);
if (!wrapOk) console.log(JSON.stringify(next));

// insideFence: paste di dalam blok kode tidak dibungkus ulang.
const doc = "teks\n```js\nconst a = 1;\n";
const inside = insideFence(doc, doc.length);
console.log(`${inside ? " OK   " : " GAGAL"}  deteksi "di dalam blok kode"`);
const outside = !insideFence("teks biasa saja", 5);
console.log(`${outside ? " OK   " : " GAGAL"}  deteksi "di luar blok kode"`);

console.log("\n" + "=".repeat(60));
console.log(`HASIL: ${lulus}/${KASUS.length} deteksi benar`);
if (salah.length) {
  console.log("\nGAGAL:");
  for (const s of salah) console.log("  ! " + s);
}
const semua = gagal === 0 && wrapOk && inside && outside;
console.log(`\nVERDICT: ${semua ? "LULUS" : "PERLU PERBAIKAN"}\n`);
