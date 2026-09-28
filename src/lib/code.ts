/**
 * Deteksi "apakah teks yang di-paste ini kode?" dan tebak bahasanya, supaya
 * kode bisa dibungkus otomatis dalam blok berpagar Markdown (```lang … ```).
 *
 * Tujuannya bukan analisis sintaks sempurna — cukup heuristik yang tidak
 * salah membungkus prosa biasa, tapi mengenali kode dari HTML, CSS, dan
 * bahasa pemrograman umum.
 */

/**
 * Sinyal KUAT: kemunculan satu saja sudah cukup menandai kode, bahkan pada
 * satu baris. Nyaris tak pernah muncul di prosa biasa.
 */
const STRONG_SIGNALS: RegExp[] = [
  /<\/?[a-z][\w-]*(\s[^<>]*)?\/?>/i, // tag HTML: <div ...>, </div>, <br/>
  /<!DOCTYPE/i,
  /<\?php/i,
  /\b(function|const|let|var|class|def|import|export|public\s+static|#include|using\s+namespace|fn\s+\w+|func\s+\w+|struct|interface|package)\b/,
  /=>|::|:=|->|std::|println!|console\.(log|error|warn)|System\.out/,
  /\b(console|printf|println|System|echo|puts|fmt|print)\s*[.(]/,
  /^\s*(SELECT|INSERT\s+INTO|UPDATE|DELETE\s+FROM|CREATE\s+TABLE)\b/im,
  /[{};]\s*$/m, // baris berakhir dengan { } ;
  // Perintah terminal: nama perkakas di awal baris diikuti argumen.
  /^\s*[$#>]?\s*(npm|npx|yarn|pnpm|git|cargo|docker|kubectl|pip|apt|brew|sudo|make|chmod|mkdir|curl|wget)\s+\S/m,
];

/**
 * Sinyal LEMAH: sendirian bisa saja prosa; perlu dua, atau satu sinyal kuat.
 */
const WEAK_SIGNALS: RegExp[] = [
  /===|!==|&&|\|\||\+\+|--|\}\s*else|\)\s*\{/,
  /^\s*(@[\w.]+|#\w+|\/\/|\/\*|\*\s)/im, // dekorator, preprocessor, komentar
  /\b(if|for|while|switch|elif|foreach|return)\s*[({]/,
  /^\s{2,}\S/m, // baris berindentasi (khas kode)
  /["'][^"']*["']\s*[:=]/, // pasangan "kunci": / atribut=
];

/** Apakah teks ini kemungkinan besar kode? */
export function looksLikeCode(text: string): boolean {
  const t = text.trim();
  if (t.length < 3) return false;

  // Satu sinyal kuat sudah cukup (mis. satu tag HTML di satu baris).
  if (STRONG_SIGNALS.some((re) => re.test(t))) return true;

  // Selain itu, butuh dua sinyal lemah.
  let weak = 0;
  for (const re of WEAK_SIGNALS) {
    if (re.test(t)) weak += 1;
    if (weak >= 2) return true;
  }
  return false;
}

/** Tag HTML yang dikenal — dipakai agar `<br>` pun terlabeli html. */
const HTML_TAGS =
  "a|abbr|article|aside|audio|b|blockquote|body|br|button|canvas|caption|code|col|" +
  "datalist|dd|details|dialog|div|dl|dt|em|embed|fieldset|figcaption|figure|footer|" +
  "form|h1|h2|h3|h4|h5|h6|head|header|hr|html|i|iframe|img|input|label|legend|li|" +
  "link|main|map|mark|menu|meta|nav|noscript|object|ol|optgroup|option|output|p|" +
  "param|picture|pre|progress|q|s|samp|script|section|select|small|source|span|" +
  "strong|style|sub|summary|sup|table|tbody|td|template|textarea|tfoot|th|thead|" +
  "time|title|tr|track|u|ul|var|video|wbr";

/**
 * Tebak bahasa untuk label pagar.
 *
 * **Urutan sangat penting.** Pola beberapa bahasa saling tumpang tindih, jadi
 * yang paling khas harus diperiksa lebih dulu. Contoh nyata dari pengujian:
 * `import React from "react"` sempat terlabeli Python karena pemeriksaan
 * Python (`import\s+\w+`) berjalan sebelum JavaScript. Sekarang pola modul ES
 * (`import … from "…"`, `export default`) diperiksa duluan, dan pola `import`
 * milik Python dipersempit agar tidak menyambar bentuk JavaScript.
 */
export function detectLanguage(text: string): string {
  const t = text.trim();

  // 1. Penanda dokumen yang tak mungkin salah.
  if (/<\?php/i.test(t)) return "php";
  if (/<!DOCTYPE\s+html/i.test(t)) return "html";
  if (new RegExp(`</?(${HTML_TAGS})(\\s[^<>]*)?/?>`, "i").test(t)) return "html";

  // 2. CSS: aturan @ atau blok selector { prop: value; }
  if (/(^|\n)\s*(@media|@import|@keyframes|@font-face)\b/i.test(t)) return "css";
  if (/(^|\n)\s*[.#]?[\w-]+(\s*[,>+~]\s*[.#]?[\w-]+)*\s*\{[^{}]*:[^{}]*(;|\s*\})/.test(t)) return "css";

  // 3. Modul ES / JSX -> JavaScript. Diperiksa SEBELUM Python karena
  //    `import X from "Y"` juga cocok dengan pola import Python yang longgar.
  if (/\bimport\s+[\w{},*\s]+\s+from\s+['"]/.test(t)) return "javascript";
  if (/\bexport\s+(default|const|function|class|\{)/.test(t)) return "javascript";
  if (/\brequire\s*\(\s*['"]/.test(t)) return "javascript";

  // 4. Python: `from X import Y`, `def f(...):`, atau import berdiri sendiri.
  if (/^\s*from\s+[\w.]+\s+import\b/m.test(t)) return "python";
  if (/^\s*def\s+\w+\s*\(.*\)\s*:/m.test(t)) return "python";
  if (/^\s*import\s+[\w.]+(\s+as\s+\w+)?\s*$/m.test(t)) return "python";
  if (/^\s*(class\s+\w+.*:|if\s+__name__\s*==)/m.test(t)) return "python";

  // 5. Bahasa bertipe statis.
  if (/#include\s*[<"]|std::|int\s+main\s*\(|printf\s*\(|cout\s*<</.test(t)) return "cpp";
  if (/\b(public\s+(static\s+)?(void|class)|System\.out\.print|package\s+[\w.]+;)/.test(t)) return "java";
  if (/\bfn\s+\w+\s*\(|\blet\s+mut\b|println!|\buse\s+std::/.test(t)) return "rust";
  if (/\bfunc\s+\w+\s*\(|\bpackage\s+main\b/.test(t)) return "go";

  // 6. SQL.
  if (/^\s*(SELECT|INSERT\s+INTO|UPDATE|DELETE\s+FROM|CREATE\s+(TABLE|DATABASE)|ALTER\s+TABLE|DROP\s+TABLE)\b/im.test(t))
    return "sql";

  // 7. Perintah shell / terminal.
  if (/^\s*[$#>]?\s*(npm|npx|yarn|pnpm|git|cd|ls|dir|mkdir|rm|cp|mv|cat|echo|curl|wget|node|python3?|pip|cargo|docker|kubectl|make|sudo|apt|brew|chmod)\b/m.test(t))
    return "bash";

  // 8. JSON: objek/array dengan kunci berkutip.
  if (/^\s*[[{]/.test(t) && /"[^"]+"\s*:/.test(t)) return "json";

  // 9. JavaScript umum (paling longgar, jadi terakhir).
  if (/\b(function|const|let|var)\b|=>|console\.(log|error|warn)|document\.|window\./.test(t))
    return "javascript";

  return "";
}

/**
 * Apakah posisi `pos` berada DI DALAM blok kode berpagar? Kalau ya, paste
 * tidak perlu dibungkus lagi (sudah terlindungi).
 */
export function insideFence(text: string, pos: number): boolean {
  const before = text.slice(0, pos);
  let count = 0;
  for (const line of before.split("\n")) {
    if (/^\s*```/.test(line)) count += 1;
  }
  return count % 2 === 1;
}

/** Bungkus `code` dalam pagar Markdown, menjaga agar pagar ada di barisnya sendiri. */
export function wrapInFence(
  value: string,
  selStart: number,
  selEnd: number,
  code: string,
  lang: string,
): { next: string; caret: number } {
  const before = value.slice(0, selStart);
  const after = value.slice(selEnd);
  const body = code.replace(/\r\n/g, "\n").replace(/\s+$/, "");
  const nlBefore = before.length > 0 && !before.endsWith("\n") ? "\n" : "";
  const nlAfter = after.length > 0 && !after.startsWith("\n") ? "\n" : "";
  const block = `${nlBefore}\`\`\`${lang}\n${body}\n\`\`\`${nlAfter}`;
  return { next: before + block + after, caret: (before + block).length };
}
