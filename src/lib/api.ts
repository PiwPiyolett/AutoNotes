/**
 * Jembatan tipis ke perintah Tauri di sisi Rust.
 *
 * Fungsi di sini hanya membungkus `invoke()` dan menambahkan tipe.
 * Kalau Rust-nya berubah tanda tangan, TypeScript ikut menyala merah di sini.
 */

import { invoke } from "@tauri-apps/api/core";

// ---------------- tipe ---------------------------------------------------

export type Aggressiveness = "konservatif" | "seimbang" | "agresif";

export type Settings = {
  aggressiveness: Aggressiveness;
  slang_expansion: boolean;
  check_indonesian: boolean;
  check_english: boolean;
  max_edit: number;
};

export type Source = "typo" | "slang" | "dictionary" | "context";

export type Correction = {
  start: number;
  end: number;
  from: string;
  to: string;
  confidence: number;
  source: Source;
  auto: boolean;
};

export type CheckResult = {
  corrections: Correction[];
  auto_applied: string;
};

export type DictStatus = {
  id_words: number;
  en_words: number;
  ready: boolean;
  context_enabled: boolean;
  bigram_pairs: number;
};

export type NoteMeta = {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  pinned: boolean;
  archived: boolean;
  tags: string[];
};

export type Note = {
  id: string;
  title: string;
  tags: string[];
  created: number;
  updated: number;
  pinned: boolean;
  archived: boolean;
  /** true = judul diketik manual, jangan ikuti isi. */
  title_manual: boolean;
  body: string;
};

export type SearchHit = {
  id: string;
  title: string;
  updated_at: number;
  pinned: boolean;
  snippet: string;
  score: number;
};

// ---------------- panggilan ---------------------------------------------

export const api = {
  // koreksi
  checkText: (text: string) => invoke<CheckResult>("check_text", { text }),
  fixAll: (text: string) => invoke<string>("fix_all", { text }),
  getSettings: () => invoke<Settings>("corrector_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_corrector_settings", { settings }),
  dictStatus: () => invoke<DictStatus>("dict_status"),
  addToDict: (word: string) => invoke<void>("add_to_user_dict", { word }),
  recordUndo: (from: string, to: string) => invoke<void>("record_undo", { from, to }),
  aggressivenessOptions: () => invoke<Aggressiveness[]>("aggressiveness_options"),

  // catatan
  listNotes: (limit?: number) => invoke<NoteMeta[]>("list_notes", { limit }),
  loadNote: (id: string) => invoke<Note | null>("load_note", { id }),
  createNote: (body: string) => invoke<Note>("create_note", { body }),
  saveNote: (note: Note) => invoke<void>("save_note", { note }),
  trashNote: (id: string) => invoke<void>("trash_note", { id }),
  searchNotes: (query: string, limit?: number) =>
    invoke<SearchHit[]>("search_notes", { query, limit }),

  stats: () => invoke<{ total_notes: number }>("stats"),

  // ekspor
  exportNote: (path: string, format: ExportFormat, title: string, body: string) =>
    invoke<void>("export_note", { path, format, title, body }),
};

export type ExportFormat = "txt" | "md" | "docx" | "pdf";
