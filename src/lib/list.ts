/**
 * Penomoran daftar otomatis: angka, huruf (a, b, c), dan romawi (i, ii, iii).
 *
 * Tantangan utamanya ambiguitas: "i." bisa berarti huruf ke-9 ATAU romawi 1.
 * Diselesaikan dengan melihat penanda baris SEBELUMNYA. Kalau daftar berjalan
 * a, b, ... , h, lalu "i" → itu huruf (lanjut ke j). Kalau daftar berjalan
 * i, ii, iii → itu romawi. Tanpa konteks, "i" tunggal dianggap romawi (sesuai
 * harapan umum), huruf lain dianggap alfabet.
 */

const ROMAN_UNITS: [number, string][] = [
  [1000, "m"], [900, "cm"], [500, "d"], [400, "cd"],
  [100, "c"], [90, "xc"], [50, "l"], [40, "xl"],
  [10, "x"], [9, "ix"], [5, "v"], [4, "iv"], [1, "i"],
];

export function toRoman(n: number): string {
  if (n <= 0 || n > 3999) return String(n);
  let out = "";
  let rem = n;
  for (const [v, sym] of ROMAN_UNITS) {
    while (rem >= v) {
      out += sym;
      rem -= v;
    }
  }
  return out;
}

/** Kembalikan nilai romawi bila `s` adalah angka romawi kanonik, kalau tidak null. */
export function parseRoman(s: string): number | null {
  const lower = s.toLowerCase();
  if (!/^[ivxlcdm]+$/.test(lower)) return null;
  const val: Record<string, number> = { i: 1, v: 5, x: 10, l: 50, c: 100, d: 500, m: 1000 };
  let total = 0;
  for (let i = 0; i < lower.length; i++) {
    const cur = val[lower[i]];
    const nxt = val[lower[i + 1]] ?? 0;
    total += cur < nxt ? -cur : cur;
  }
  // Hanya terima bentuk kanonik ("iiii" atau "vx" ditolak).
  return toRoman(total) === lower ? total : null;
}

export type MarkerKind = "numeric" | "alpha" | "roman";

function isSingleLetter(s: string): boolean {
  return /^[a-z]$/.test(s.toLowerCase());
}

/** Tentukan jenis penanda, dengan bantuan penanda sebelumnya bila ada. */
export function markerKind(marker: string, prev?: string | null): MarkerKind {
  if (/^\d+$/.test(marker)) return "numeric";
  const lower = marker.toLowerCase();
  const roman = parseRoman(lower);

  // 1) Ikuti konteks baris sebelumnya bila membentuk urutan yang cocok.
  if (prev) {
    const prevLower = prev.toLowerCase();
    if (isSingleLetter(prevLower) && isSingleLetter(lower) &&
        lower.charCodeAt(0) === prevLower.charCodeAt(0) + 1) {
      return "alpha";
    }
    const pr = parseRoman(prevLower);
    if (pr != null && roman != null && roman === pr + 1) return "roman";
  }

  // 2) Tanpa konteks: "i" tunggal & romawi multi-huruf = romawi; sisanya huruf.
  if (roman != null && (marker.length > 1 || lower === "i")) return "roman";
  if (isSingleLetter(lower)) return "alpha";
  if (roman != null) return "roman";
  return "alpha";
}

/** Penanda berikutnya setelah `marker` (mempertahankan besar/kecil huruf). */
export function nextMarker(marker: string, prev?: string | null): string {
  const kind = markerKind(marker, prev);
  const upper = marker === marker.toUpperCase() && marker !== marker.toLowerCase();

  if (kind === "numeric") return String(parseInt(marker, 10) + 1);

  if (kind === "roman") {
    const val = parseRoman(marker.toLowerCase()) ?? 0;
    const r = toRoman(val + 1);
    return upper ? r.toUpperCase() : r;
  }

  // alpha: a → b … y → z; setelah z tetap z (kasus langka).
  const code = marker.toLowerCase().charCodeAt(0);
  const nc = code >= 122 ? "z" : String.fromCharCode(code + 1);
  return upper ? nc.toUpperCase() : nc;
}
