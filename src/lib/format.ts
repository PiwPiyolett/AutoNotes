/** Pemformatan waktu ala manusia: "baru saja", "12 menit", "kemarin". */
export function relativeTime(unixSecs: number): string {
  const now = Date.now() / 1000;
  const diff = Math.max(0, now - unixSecs);

  if (diff < 45) return "baru saja";
  if (diff < 90) return "1 menit";
  const minutes = Math.floor(diff / 60);
  if (minutes < 60) return `${minutes} menit`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} jam`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "kemarin";
  if (days < 7) return `${days} hari`;
  if (days < 30) return `${Math.floor(days / 7)} minggu`;
  if (days < 365) return `${Math.floor(days / 30)} bulan`;
  return `${Math.floor(days / 365)} tahun`;
}

/**
 * Turunkan judul dari isi — cermin `derive_title` di Rust, untuk menampilkan
 * judul otomatis secara real-time di input. Baris tak-kosong pertama, tanpa
 * tanda `#`, maksimal 120 karakter. Kosong kalau isi kosong (biar placeholder
 * "Tanpa judul" muncul).
 */
export function deriveTitle(body: string): string {
  for (const raw of body.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const bersih = line.replace(/^#+/, "").trim();
    if (!bersih) continue;
    return Array.from(bersih).slice(0, 120).join("");
  }
  return "";
}

/** debounce sederhana untuk pencarian & pemeriksaan. */
export function debounce<T extends (...a: never[]) => unknown>(fn: T, ms: number) {
  let t: ReturnType<typeof setTimeout> | null = null;
  const wrapped = ((...args: Parameters<T>) => {
    if (t) clearTimeout(t);
    t = setTimeout(() => fn(...args), ms);
  }) as T & { cancel: () => void };
  wrapped.cancel = () => {
    if (t) clearTimeout(t);
    t = null;
  };
  return wrapped;
}
