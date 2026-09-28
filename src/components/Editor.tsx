import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import type { Correction } from "../lib/api";
import { api } from "../lib/api";
import type { TypingAnim } from "../lib/prefs";
import { debounce } from "../lib/format";
import { nextMarker } from "../lib/list";
import { looksLikeCode, detectLanguage, insideFence, wrapInFence } from "../lib/code";
import { EXPORT_FORMATS, exportNote } from "../lib/export";
import {
  IconWand, IconCheck, IconBook, IconDownload, IconPlus, IconTrash,
  IconChevronLeft, IconChevronRight,
} from "./Icon";
import "./Editor.css";

type Props = {
  /** Teks HALAMAN aktif — bukan seluruh catatan (App yang memecah halaman). */
  value: string;
  /** Seluruh isi catatan (semua halaman + penanda) — untuk ekspor. */
  fullBody: string;
  /** Id catatan aktif; berubahnya menandai pindah catatan. */
  noteId: string | null;
  title: string;
  corrections: Correction[];
  /** Kilas hijau di rentang ini (satuan UTF-16 di `value`), lalu hilang. */
  flash: { start: number; end: number } | null;
  onChange: (text: string) => void;
  /** Judul diedit manual oleh pengguna. */
  onTitleChange: (title: string) => void;
  onCorrectionsChange: (c: Correction[]) => void;
  onFixAll: () => void;
  autoApplyEnabled: boolean;
  /** Gaya animasi huruf yang baru diketik. */
  typingAnim: TypingAnim;
  /** Navigasi & pengelolaan halaman. */
  pageIndex: number;
  pageCount: number;
  pageLimit: number;
  onPrevPage: () => void;
  onNextPage: () => void;
  onNewPage: () => void;
  onDeletePage: () => void;
  onPanelToggle: () => void;
  panelOpen: boolean;
};

/** Rentang huruf yang baru diketik (indeks UTF-16), dianimasikan sekali. */
type AnimRange = { start: number; end: number; id: number };

/** Berapa lama animRange dipertahankan sebelum dibersihkan (ms). Harus lebih
 *  lama dari durasi animasi CSS terpanjang (fade 320ms). */
const ANIM_CLEAR_MS = 360;

/** Jeda pemeriksaan typo. Sengaja > ANIM_CLEAR_MS supaya regenerasi backdrop
 *  akibat koreksi tidak jatuh saat animasi masih berjalan. */
const CHECK_DEBOUNCE_MS = 450;

/**
 * Editor dua-lapis, **controlled**:
 *
 *   <textarea>       ← lapisan tulis, teksnya transparan
 *   <div .backdrop>  ← lapisan tampil, teks + <mark> untuk garis bergelombang
 *
 * Teks bukan milik Editor melainkan milik App, sehingga tombol di panel
 * koreksi (yang juga hidup di App) mengubah teks yang sama persis dengan yang
 * ditampilkan di sini. Dulu Editor menyimpan salinan teksnya sendiri, itulah
 * sebabnya tombol "Terapkan" seolah tidak bekerja — ia mengubah satu salinan,
 * layar menampilkan salinan lain.
 *
 * Editor tetap bertanggung jawab atas pemeriksaan ejaan (debounce) dan
 * penerapan otomatis, karena itu erat dengan interaksi mengetik.
 */
export function Editor({
  value,
  fullBody,
  noteId,
  title,
  corrections,
  flash,
  onChange,
  onTitleChange,
  onCorrectionsChange,
  onFixAll,
  autoApplyEnabled,
  typingAnim,
  pageIndex,
  pageCount,
  pageLimit,
  onPrevPage,
  onNextPage,
  onNewPage,
  onDeletePage,
  onPanelToggle,
  panelOpen,
}: Props) {
  const [checking, setChecking] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const backdropRef = useRef<HTMLDivElement | null>(null);

  // --- Animasi huruf baru ------------------------------------------------
  //
  // Kunci menghindari "kedip dua kali":
  //   1. animRange di-set SINKRON di dalam handler ketik (bukan useEffect
  //      setelah render), supaya huruf tak sempat tampil polos satu frame
  //      sebelum dianimasikan.
  //   2. animRange dibersihkan setelah animasi selesai, dan pemeriksaan typo
  //      (yang me-regenerasi backdrop) sengaja ditunda lebih lama dari durasi
  //      animasi, supaya tidak jatuh di tengah animasi dan memicu pop ulang.
  const prevRef = useRef(value);
  const animCounter = useRef(0);
  const clearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Seleksi/kursor yang harus dipulihkan setelah perubahan teks terprogram.
  const pendingSel = useRef<{ start: number; end: number } | null>(null);
  const [animRange, setAnimRange] = useState<AnimRange | null>(null);
  const animOn = typingAnim !== "off";

  // Reset animasi saat berpindah catatan.
  useEffect(() => {
    setAnimRange(null);
  }, [noteId]);

  // Jaga prevRef tetap sinkron saat teks berubah BUKAN karena ketikan
  // (koreksi otomatis, tempel, pindah catatan) supaya diff huruf berikutnya
  // tetap benar dan perubahan itu tidak ikut dianimasikan. Berjalan setelah
  // handleType (yang juga men-set prevRef), jadi idempoten untuk ketikan.
  useEffect(() => {
    prevRef.current = value;
  }, [value]);

  // Dipanggil hanya oleh ketikan asli di textarea.
  const handleType = (next: string) => {
    const prev = prevRef.current;
    prevRef.current = next;
    if (animOn && next.length > prev.length && next.length - prev.length <= 3) {
      let p = 0;
      while (p < prev.length && prev[p] === next[p]) p++;
      let s = 0;
      while (s < prev.length - p && prev[prev.length - 1 - s] === next[next.length - 1 - s]) s++;
      const start = p;
      const end = next.length - s;
      if (end > start && end - start <= 3) {
        animCounter.current += 1;
        setAnimRange({ start, end, id: animCounter.current });
        if (clearTimer.current) clearTimeout(clearTimer.current);
        clearTimer.current = setTimeout(() => setAnimRange(null), ANIM_CLEAR_MS);
      } else {
        setAnimRange(null);
      }
    } else if (animOn) {
      setAnimRange(null);
    }
    onChange(next);
  };

  // Ubah teks secara terprogram (auto-daftar / indentasi) tanpa memicu animasi
  // huruf, lalu tempatkan seleksi setelah React memperbarui nilainya.
  const commitRange = (next: string, start: number, end: number) => {
    prevRef.current = next;
    setAnimRange(null);
    pendingSel.current = { start, end };
    onChange(next);
  };
  const commit = (next: string, caret: number) => commitRange(next, caret, caret);

  // --- Paste kode: bungkus otomatis dalam blok berpagar ------------------
  // Kalau isi clipboard tampak seperti kode (HTML/CSS/bahasa pemrograman),
  // ia dibungkus ```lang … ``` supaya terlindungi dari autocorrect dan tampil
  // sebagai blok kode. Prosa biasa dibiarkan menempel normal.
  const handlePaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const text = e.clipboardData.getData("text");
    if (!text) return;
    const ta = e.currentTarget;
    const pos = ta.selectionStart;
    // Sudah di dalam blok kode → tempel apa adanya (sudah terlindungi).
    if (insideFence(value, pos)) return;
    if (!looksLikeCode(text)) return; // bukan kode → paste normal
    e.preventDefault();
    const lang = detectLanguage(text);
    const { next, caret } = wrapInFence(value, ta.selectionStart, ta.selectionEnd, text, lang);
    commit(next, caret);
  };

  // --- Tab / Shift+Tab: indentasi & auto-lanjut daftar -------------------
  const INDENT = "  "; // dua spasi per tingkat

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.nativeEvent as { isComposing?: boolean }).isComposing) return;
    const ta = e.currentTarget;

    // ===== Ctrl+Enter: buat halaman baru (kolom ketik kosong) =====
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      onNewPage();
      return;
    }

    // ===== Ctrl+Shift+C: bungkus teks terpilih jadi blok kode =====
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "c") {
      const { selectionStart: s, selectionEnd: en } = ta;
      if (s !== en) {
        e.preventDefault();
        const sel = value.slice(s, en);
        const { next, caret } = wrapInFence(value, s, en, sel, detectLanguage(sel));
        commit(next, caret);
      }
      return;
    }

    // ===== TAB: indentasi =====
    if (e.key === "Tab") {
      e.preventDefault();
      const start = ta.selectionStart;
      const end = ta.selectionEnd;

      if (start === end && !e.shiftKey) {
        // Sisipkan satu tingkat indentasi di kursor.
        commit(value.slice(0, start) + INDENT + value.slice(start), start + INDENT.length);
        return;
      }

      // Operasikan pada semua baris yang tersentuh seleksi.
      const blockStart = value.lastIndexOf("\n", start - 1) + 1;
      const blockEnd = end; // cukup sampai akhir seleksi
      const block = value.slice(blockStart, blockEnd);
      const lines = block.split("\n");

      if (e.shiftKey) {
        // Outdent: buang hingga 2 spasi (atau satu tab) di awal tiap baris.
        let removedFirst = 0;
        const outdented = lines.map((l, i) => {
          const m = /^( {1,2}|\t)/.exec(l);
          if (m && i === 0) removedFirst = m[0].length;
          return m ? l.slice(m[0].length) : l;
        });
        const newBlock = outdented.join("\n");
        const newVal = value.slice(0, blockStart) + newBlock + value.slice(blockEnd);
        const removedTotal = block.length - newBlock.length;
        commitRange(newVal, Math.max(blockStart, start - removedFirst), end - removedTotal);
      } else {
        // Indent: tambahkan satu tingkat di awal tiap baris.
        const newBlock = lines.map((l) => INDENT + l).join("\n");
        const newVal = value.slice(0, blockStart) + newBlock + value.slice(blockEnd);
        const added = INDENT.length;
        commitRange(newVal, start + added, end + added * lines.length);
      }
      return;
    }

    // ===== ENTER: auto-lanjut daftar =====
    if (e.key !== "Enter" || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return;
    if (ta.selectionStart !== ta.selectionEnd) return;

    const pos = ta.selectionStart;
    const lineStart = value.lastIndexOf("\n", pos - 1) + 1;
    const lineToCaret = value.slice(lineStart, pos);

    // Bullet: indentasi, -/*/+, SPASI wajib, isi.
    const bul = /^(\s*)([-*+])[ \t]+(.*)$/.exec(lineToCaret);
    // Bernomor/huruf/romawi: indentasi, token, pemisah (. atau )), SPASI, isi.
    const ord = /^(\s*)([0-9]+|[A-Za-z]+)([.)])[ \t]+(.*)$/.exec(lineToCaret);

    if (bul) {
      const [, indent, marker, content] = bul;
      e.preventDefault();
      if (content.trim() === "") {
        commit(value.slice(0, lineStart) + value.slice(pos), lineStart);
      } else {
        const ins = `\n${indent}${marker} `;
        commit(value.slice(0, pos) + ins + value.slice(pos), pos + ins.length);
      }
      return;
    }

    if (ord) {
      const [, indent, marker, delim, content] = ord;
      e.preventDefault();
      if (content.trim() === "") {
        commit(value.slice(0, lineStart) + value.slice(pos), lineStart);
        return;
      }
      // Penanda baris sebelumnya (indentasi sama) untuk memutus ambiguitas
      // huruf vs romawi (mis. h→i huruf, tapi i→ii romawi).
      const prevMarker = previousOrdinalMarker(value, lineStart, indent, delim);
      const ins = `\n${indent}${nextMarker(marker, prevMarker)}${delim} `;
      commit(value.slice(0, pos) + ins + value.slice(pos), pos + ins.length);
    }
  };

  // Terapkan seleksi tertunda (dari auto-daftar / indentasi) setelah nilai berubah.
  useLayoutEffect(() => {
    if (pendingSel.current == null) return;
    const ta = textareaRef.current;
    if (ta) {
      ta.selectionStart = pendingSel.current.start;
      ta.selectionEnd = pendingSel.current.end;
    }
    pendingSel.current = null;
  }, [value]);

  useEffect(() => () => {
    if (clearTimer.current) clearTimeout(clearTimer.current);
  }, []);

  // Pemeriksaan koreksi setelah berhenti mengetik. Jedanya sengaja lebih lama
  // dari durasi animasi huruf (lihat ANIM_CLEAR_MS) supaya regenerasi backdrop
  // akibat hasil koreksi tidak jatuh di tengah animasi dan memicu pop ulang.
  useEffect(() => {
    if (noteId === null) return;
    setChecking(true);
    const run = debounce(async () => {
      try {
        const res = await api.checkText(value);
        if (autoApplyEnabled && res.auto_applied !== value) {
          // Terapkan koreksi berkeyakinan-tinggi di tempat, lalu periksa ulang
          // supaya sisa saran (yang hanya ditandai) tetap tampil.
          onChange(res.auto_applied);
          const r2 = await api.checkText(res.auto_applied);
          onCorrectionsChange(r2.corrections);
        } else {
          onCorrectionsChange(res.corrections);
        }
      } catch (e) {
        console.error(e);
      } finally {
        setChecking(false);
      }
    }, CHECK_DEBOUNCE_MS);
    run();
    return () => {
      run.cancel();
      setChecking(false);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, autoApplyEnabled, noteId]);

  const highlighted = useMemo(
    () => renderBackdrop(value, corrections, flash, animOn ? animRange : null, typingAnim),
    [value, corrections, flash, animRange, animOn, typingAnim],
  );

  const stats = useMemo(() => wordAndChar(value), [value]);

  const nearLimit = value.length >= pageLimit * 0.9;
  const atLimit = value.length >= pageLimit;

  if (noteId === null) return <EmptyState />;

  return (
    <>
      <div className="editor-toolbar">
        <div className="row gap-2 grow" style={{ minWidth: 0 }}>
          <input
            className="editor-title-input"
            value={title}
            onChange={(e) => onTitleChange(e.target.value)}
            placeholder="Tanpa judul"
            aria-label="Judul catatan"
            spellCheck={false}
          />
          {checking && <span className="chip text-xs" title="Memeriksa ejaan…">memeriksa…</span>}
          {!checking && corrections.length > 0 && (
            <span className="chip chip-orange text-xs">{corrections.length} saran</span>
          )}
          {!checking && corrections.length === 0 && value.length > 20 && (
            <span className="chip chip-teal text-xs">
              <IconCheck size={11} /> bersih
            </span>
          )}
        </div>
        <div className="row gap-2">
          {corrections.length > 0 && (
            <button className="btn btn-accent" onClick={onFixAll} title="Terapkan semua saran sekaligus">
              <IconWand size={14} /> Perbaiki semua
            </button>
          )}
          <ExportMenu title={title} body={fullBody} />
          <button
            className={`btn btn-icon ${panelOpen ? "active" : ""}`}
            onClick={onPanelToggle}
            title={panelOpen ? "Sembunyikan panel koreksi" : "Tampilkan panel koreksi"}
            aria-pressed={panelOpen}
          >
            <IconBook size={16} />
          </button>
        </div>
      </div>

      <div className={`editor-canvas ${animOn ? "anim-active" : ""}`}>
        <div
          ref={backdropRef}
          className="editor-backdrop"
          aria-hidden
          dangerouslySetInnerHTML={{ __html: highlighted }}
        />
        <textarea
          ref={textareaRef}
          className="editor-textarea"
          value={value}
          maxLength={pageLimit}
          onChange={(e) => handleType(e.target.value)}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          onScroll={(e) => {
            if (backdropRef.current) {
              backdropRef.current.scrollTop = e.currentTarget.scrollTop;
              backdropRef.current.scrollLeft = e.currentTarget.scrollLeft;
            }
          }}
          placeholder="Ketik apa saja. Autocorrect Bahasa Indonesia + Inggris berjalan otomatis."
          spellCheck={false}
          autoFocus
        />
      </div>

      <div className="editor-footer">
        <div className="pager">
          <button
            className="btn btn-icon"
            onClick={onPrevPage}
            disabled={pageIndex <= 0}
            title="Halaman sebelumnya"
            aria-label="Halaman sebelumnya"
          >
            <IconChevronLeft size={16} />
          </button>
          <span className="pager-label text-xs">
            Halaman {pageIndex + 1} / {pageCount}
          </span>
          <button
            className="btn btn-icon"
            onClick={onNextPage}
            disabled={pageIndex >= pageCount - 1}
            title="Halaman berikutnya"
            aria-label="Halaman berikutnya"
          >
            <IconChevronRight size={16} />
          </button>
          <button
            className="btn btn-icon"
            onClick={onNewPage}
            title="Halaman baru (Ctrl+Enter)"
            aria-label="Halaman baru"
          >
            <IconPlus size={16} />
          </button>
          {pageCount > 1 && (
            <button
              className="btn btn-icon"
              onClick={onDeletePage}
              title="Hapus halaman ini"
              aria-label="Hapus halaman ini"
            >
              <IconTrash size={16} />
            </button>
          )}
        </div>
        <span className="text-xs fg-subtle">
          {stats.words} kata ·{" "}
          <span className={atLimit ? "fg-danger" : nearLimit ? "fg-warning" : undefined}>
            {value.length} / {pageLimit}
          </span>{" "}
          karakter
        </span>
      </div>
    </>
  );
}

/**
 * Menu ekspor: tombol yang membuka daftar format. Memilih format akan
 * membuka dialog "Save As" native lalu menulis berkasnya lewat Rust.
 */
function ExportMenu({ title, body }: { title: string; body: string }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onDoc);
    return () => window.removeEventListener("mousedown", onDoc);
  }, [open]);

  const run = async (format: (typeof EXPORT_FORMATS)[number]["id"], label: string) => {
    setBusy(format);
    try {
      const saved = await exportNote(format, title || "Tanpa judul", body);
      if (saved) {
        setDone(label);
        setTimeout(() => setDone(null), 2200);
      }
    } catch (e) {
      console.error(e);
      alert(`Gagal mengekspor: ${e}`);
    } finally {
      setBusy(null);
      setOpen(false);
    }
  };

  return (
    <div className="export-menu" ref={ref}>
      <button
        className="btn btn-icon"
        onClick={() => setOpen((v) => !v)}
        title="Ekspor catatan (Word, PDF, Markdown, teks)"
        aria-haspopup="menu"
        aria-expanded={open}
      >
        <IconDownload size={16} />
      </button>
      {open && (
        <div className="export-dropdown" role="menu">
          <div className="export-heading">Ekspor sebagai</div>
          {EXPORT_FORMATS.map((f) => (
            <button
              key={f.id}
              className="export-item"
              role="menuitem"
              disabled={busy !== null}
              onClick={() => run(f.id, f.label)}
            >
              {busy === f.id ? "Menyimpan…" : f.label}
            </button>
          ))}
        </div>
      )}
      {done && (
        <span className="export-toast" role="status">
          <IconCheck size={12} /> Tersimpan sebagai {done}
        </span>
      )}
    </div>
  );
}

function EmptyState() {
  return (
    <div className="editor-empty">
      <div className="editor-empty-inner">
        <h2>Pilih catatan atau buat yang baru</h2>
        <p>Autocorrect Bahasa Indonesia + Inggris berjalan otomatis saat kamu mengetik.</p>
        <div className="editor-empty-shortcuts">
          <div><kbd>Ctrl</kbd>+<kbd>N</kbd><span>Catatan baru</span></div>
          <div><kbd>Ctrl</kbd>+<kbd>K</kbd><span>Cari</span></div>
          <div><kbd>Ctrl</kbd>+<kbd>Enter</kbd><span>Halaman baru</span></div>
          <div><kbd>Ctrl</kbd>+<kbd>,</kbd><span>Pengaturan</span></div>
        </div>
      </div>
    </div>
  );
}

/**
 * Susun backdrop HTML: teks apa adanya + `<mark>` untuk tiap kata bertyp.
 * Rentang koreksi (offset byte dari Rust) diubah ke indeks UTF-16 lewat
 * penghitungan panjang byte per potongan, supaya garis duduk tepat di bawah
 * kata yang benar meski ada huruf beraksen.
 */
function renderBackdrop(
  text: string,
  corrections: Correction[],
  flash: { start: number; end: number } | null,
  animRange: AnimRange | null,
  animStyle: TypingAnim,
): string {
  if (text === "") return "&nbsp;";

  const enc = new TextEncoder();
  // Peta: offset byte → indeks UTF-16. Dihitung sekali.
  const byteToChar = buildByteToCharMap(text, enc);

  type Span = { start: number; end: number; html: (inner: string) => string };
  const spans: Span[] = [];

  for (const c of corrections) {
    const s = byteToChar.get(c.start);
    const e = byteToChar.get(c.end);
    if (s === undefined || e === undefined) continue;
    const kind = c.confidence >= 0.7 ? "typo" : "typo weak";
    spans.push({ start: s, end: e, html: (inner) => `<mark class="${kind}">${inner}</mark>` });
  }
  if (flash) {
    spans.push({ start: flash.start, end: flash.end, html: (inner) => `<mark class="flash">${inner}</mark>` });
  }
  if (animRange) {
    // Tiap huruf baru dibungkus span tersendiri + jeda bertahap, supaya
    // mengetik beberapa huruf sekaligus (mis. dari IME) muncul beruntun.
    spans.push({
      start: animRange.start,
      end: animRange.end,
      html: (inner) => wrapPerChar(inner, animStyle, animRange.id),
    });
  }
  spans.sort((a, b) => a.start - b.start);

  let out = "";
  let cursor = 0;
  for (const s of spans) {
    if (s.start < cursor) continue; // lewati rentang yang tumpang tindih
    out += escape(text.slice(cursor, s.start));
    out += s.html(escape(text.slice(s.start, s.end)));
    cursor = s.end;
  }
  out += escape(text.slice(cursor));
  return out + "\n&nbsp;";
}

/**
 * Bungkus tiap karakter (yang sudah di-escape) dalam span animasi. `key`
 * memaksa DOM membuat elemen baru sehingga animasi CSS berjalan sekali.
 */
function wrapPerChar(escaped: string, style: TypingAnim, key: number): string {
  // Pisah per entitas HTML atau karakter tunggal supaya `&amp;` tetap utuh.
  const parts = escaped.match(/&[a-z]+;|[\s\S]/gi) ?? [];
  return parts
    .map((ch, i) => `<span class="tc tc-${style}" data-k="${key}-${i}" style="animation-delay:${i * 45}ms">${ch}</span>`)
    .join("");
}

/** Bangun peta offset-byte → indeks-char untuk seluruh batas karakter. */
/**
 * Ambil token penanda daftar dari baris tepat SEBELUM `lineStart`, asalkan
 * indentasi dan pemisahnya sama. Dipakai untuk memutus ambiguitas huruf vs
 * romawi saat melanjutkan daftar (lihat lib/list).
 */
function previousOrdinalMarker(
  text: string,
  lineStart: number,
  indent: string,
  delim: string,
): string | null {
  if (lineStart === 0) return null;
  const prevEnd = lineStart - 1; // posisi '\n' pemisah
  const prevStart = text.lastIndexOf("\n", prevEnd - 1) + 1;
  const prevLine = text.slice(prevStart, prevEnd);
  const m = /^(\s*)([0-9]+|[A-Za-z]+)([.)])[ \t]+/.exec(prevLine);
  if (!m) return null;
  if (m[1] !== indent || m[3] !== delim) return null;
  return m[2];
}

function buildByteToCharMap(text: string, enc: TextEncoder): Map<number, number> {
  const map = new Map<number, number>();
  let byte = 0;
  let char = 0;
  map.set(0, 0);
  for (const ch of text) {
    byte += enc.encode(ch).length;
    char += ch.length; // surrogate pair menyumbang 2 unit UTF-16
    map.set(byte, char);
  }
  return map;
}

function escape(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function wordAndChar(text: string) {
  // `text` sudah berupa satu halaman (tanpa penanda), jadi hitung apa adanya.
  const chars = text.length;
  const words = text.trim() === "" ? 0 : text.trim().split(/\s+/u).length;
  return { chars, words };
}
