/**
 * Alur ekspor sisi-klien: buka dialog "Save As" native, lalu minta Rust
 * menuliskan berkasnya.
 */

import { save } from "@tauri-apps/plugin-dialog";
import type { ExportFormat } from "./api";
import { api } from "./api";

const FORMAT_META: Record<ExportFormat, { label: string; ext: string; mime: string }> = {
  txt: { label: "Teks", ext: "txt", mime: "text/plain" },
  md: { label: "Markdown", ext: "md", mime: "text/markdown" },
  docx: { label: "Word", ext: "docx", mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document" },
  pdf: { label: "PDF", ext: "pdf", mime: "application/pdf" },
};

/** Ubah judul jadi nama berkas yang aman. */
function safeFileName(title: string): string {
  const cleaned = title
    .trim()
    .replace(/[\\/:*?"<>|]+/g, " ") // karakter terlarang di nama berkas Windows
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80);
  return cleaned || "catatan";
}

/**
 * Ekspor satu catatan. Mengembalikan `true` kalau berhasil disimpan, `false`
 * kalau pengguna membatalkan dialog.
 */
export async function exportNote(
  format: ExportFormat,
  title: string,
  body: string,
): Promise<boolean> {
  const meta = FORMAT_META[format];
  const path = await save({
    title: `Ekspor sebagai ${meta.label}`,
    defaultPath: `${safeFileName(title)}.${meta.ext}`,
    filters: [{ name: meta.label, extensions: [meta.ext] }],
  });
  if (!path) return false; // dibatalkan
  await api.exportNote(path, format, title, body);
  return true;
}

export const EXPORT_FORMATS: { id: ExportFormat; label: string }[] = [
  { id: "docx", label: "Word (.docx)" },
  { id: "pdf", label: "PDF (.pdf)" },
  { id: "md", label: "Markdown (.md)" },
  { id: "txt", label: "Teks (.txt)" },
];
