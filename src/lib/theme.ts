/**
 * Sistem tema dua-sumbu.
 *
 * - **skin** — bahasa visual keseluruhan: `default` (teal/slate) atau `clay`
 *   (Claymorphism), dst. Diterapkan sebagai `data-ui-theme` di <html>.
 * - **appearance** — terang/gelap/ikut-sistem, sebagai `data-theme`.
 *
 * Keduanya independen: tema Clay pun punya versi gelapnya. Nilai disimpan di
 * localStorage dan diterapkan lebih awal oleh skrip inline di index.html agar
 * tidak berkedip saat memuat.
 *
 * Menambah tema baru cukup: (1) tambah slug di `SKINS`, (2) buat berkas CSS
 * `src/styles/themes/<slug>.css` yang di-scope `:root[data-ui-theme="<slug>"]`,
 * (3) impor di main.tsx. Komponen tidak perlu diubah — semuanya lewat token.
 */

export type Skin = "default" | "clay" | "holo";
export type Appearance = "system" | "light" | "dark";

export const SKINS: { id: Skin; label: string; blurb: string; darkOnly?: boolean }[] = [
  {
    id: "default",
    label: "Bawaan",
    blurb: "Bersih, teal + oranye. Ringan untuk mengetik lama.",
  },
  {
    id: "clay",
    label: "Clay",
    blurb: "Claymorphism: lembut, membulat, terasa bisa dipencet.",
  },
  {
    id: "holo",
    label: "Prisma",
    blurb: "Holografik: gradien pelangi & kaca. Selalu gelap.",
    darkOnly: true,
  },
];

const SKIN_KEY = "ui-skin";
const APPEARANCE_KEY = "ui-appearance";

export function getSkin(): Skin {
  const v = safeGet(SKIN_KEY);
  return v === "clay" || v === "holo" ? v : "default";
}

/** Apakah skin ini mengunci tampilan gelap (mengabaikan setelan mode warna)? */
export function isDarkOnly(skin: Skin): boolean {
  return SKINS.find((s) => s.id === skin)?.darkOnly ?? false;
}

export function setSkin(skin: Skin): void {
  safeSet(SKIN_KEY, skin);
  if (skin === "default") {
    document.documentElement.removeAttribute("data-ui-theme");
  } else {
    document.documentElement.setAttribute("data-ui-theme", skin);
  }
}

export function getAppearance(): Appearance {
  const v = safeGet(APPEARANCE_KEY);
  return v === "light" || v === "dark" ? v : "system";
}

export function setAppearance(mode: Appearance): void {
  safeSet(APPEARANCE_KEY, mode);
  if (mode === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", mode);
  }
}

function safeGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}

function safeSet(k: string, v: string): void {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* localStorage tidak tersedia — abaikan */
  }
}
