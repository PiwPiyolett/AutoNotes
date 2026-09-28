/**
 * Preferensi tampilan sisi-klien (disimpan di localStorage, seperti tema).
 * Untuk sekarang: gaya animasi mengetik.
 */

export type TypingAnim = "off" | "pop" | "fade" | "rise" | "blur";

export const TYPING_ANIMS: { id: TypingAnim; label: string; blurb: string }[] = [
  { id: "off", label: "Mati", blurb: "Mengetik polos, tanpa animasi." },
  { id: "pop", label: "Pop", blurb: "Huruf membesar lalu mengecil saat muncul." },
  { id: "fade", label: "Fade", blurb: "Huruf memudar masuk. Paling kalem." },
  { id: "rise", label: "Naik", blurb: "Huruf naik sedikit sambil memudar." },
  { id: "blur", label: "Blur", blurb: "Huruf menajam dari kabur." },
];

const KEY = "typing-anim";

export function getTypingAnim(): TypingAnim {
  try {
    const v = localStorage.getItem(KEY);
    if (v && TYPING_ANIMS.some((t) => t.id === v)) return v as TypingAnim;
  } catch {
    /* localStorage tak tersedia */
  }
  return "pop"; // baku: Pop aktif
}

export function setTypingAnim(v: TypingAnim): void {
  try {
    localStorage.setItem(KEY, v);
  } catch {
    /* abaikan */
  }
}
