import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { Correction, Note, NoteMeta } from "./lib/api";
import { api } from "./lib/api";
import { Sidebar } from "./components/Sidebar";
import { Editor } from "./components/Editor";
import { CorrectionPanel } from "./components/CorrectionPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { IconPin, IconTrash, IconSettings } from "./components/Icon";
import { debounce, deriveTitle } from "./lib/format";
import { applyCorrectionToText, correctionSpanInText } from "./lib/corrections";
import { getTypingAnim } from "./lib/prefs";
import { splitPages, joinPages, PAGE_CHAR_LIMIT } from "./lib/pages";
import "./styles/tokens.css";
import "./styles/app.css";
import "./styles/themes/clay.css";
import "./styles/themes/holo.css";

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function loadNum(key: string, fallback: number): number {
  try {
    const v = Number(localStorage.getItem(key));
    return Number.isFinite(v) && v > 0 ? v : fallback;
  } catch {
    return fallback;
  }
}
function saveNum(key: string, v: number): void {
  try {
    localStorage.setItem(key, String(Math.round(v)));
  } catch {
    /* abaikan */
  }
}

export default function App() {
  const [notes, setNotes] = useState<NoteMeta[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [activeNote, setActiveNote] = useState<Note | null>(null);
  // `draft` adalah SATU-SATUNYA sumber kebenaran untuk isi editor. Editor dan
  // panel koreksi sama-sama beroperasi di atasnya, jadi tombol "Ganti" di panel
  // mengubah teks yang persis sama dengan yang tampil di editor.
  const [draft, setDraft] = useState("");
  // Halaman aktif. Catatan disimpan sebagai satu teks berpenanda, tapi editor
  // menampilkan satu halaman pada satu waktu (dibolak-balik pakai panah).
  const [pageIdx, setPageIdx] = useState(0);
  const [corrections, setCorrections] = useState<Correction[]>([]);
  const [flash, setFlash] = useState<{ start: number; end: number } | null>(null);
  const [panelOpen, setPanelOpen] = useState(true);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const [autoApply, setAutoApply] = useState(true);
  const [totalNotes, setTotalNotes] = useState(0);
  const [typingAnim, setTypingAnim] = useState(getTypingAnim());
  const [sidebarW, setSidebarW] = useState(() => loadNum("pane-sidebar-w", 280));
  const [panelW, setPanelW] = useState(() => loadNum("pane-panel-w", 280));
  const appRef = useRef<HTMLDivElement | null>(null);

  const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Simpan lebar panel (debounce) supaya tidak menulis localStorage tiap gerakan.
  useEffect(() => {
    const t = setTimeout(() => saveNum("pane-sidebar-w", sidebarW), 400);
    return () => clearTimeout(t);
  }, [sidebarW]);
  useEffect(() => {
    const t = setTimeout(() => saveNum("pane-panel-w", panelW), 400);
    return () => clearTimeout(t);
  }, [panelW]);

  // Mulai menyeret gagang pengubah lebar sidebar/panel.
  const startResize = (which: "sidebar" | "panel") => (e: React.MouseEvent) => {
    e.preventDefault();
    const rect = appRef.current?.getBoundingClientRect();
    if (!rect) return;
    const move = (ev: MouseEvent) => {
      if (which === "sidebar") {
        setSidebarW(clamp(ev.clientX - rect.left, 190, 480));
      } else {
        setPanelW(clamp(rect.right - ev.clientX, 200, 520));
      }
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.classList.remove("resizing");
    };
    document.body.classList.add("resizing");
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  // Muat catatan aktif tiap kali id berubah.
  useEffect(() => {
    if (!activeId) {
      setActiveNote(null);
      setDraft("");
      setCorrections([]);
      return;
    }
    api
      .loadNote(activeId)
      .then((n) => {
        setActiveNote(n);
        setDraft(n?.body ?? "");
        setPageIdx(0);
        setCorrections([]);
      })
      .catch(console.error);
  }, [activeId]);

  // Halaman-halaman catatan aktif + indeks yang selalu dalam jangkauan.
  const pages = useMemo(() => splitPages(draft), [draft]);
  const safeIdx = Math.min(pageIdx, pages.length - 1);
  const pageText = pages[safeIdx] ?? "";

  useEffect(() => {
    api.stats().then((s) => setTotalNotes(s.total_notes)).catch(console.error);
  }, [refreshKey]);

  // Kilas hijau singkat di rentang teks yang baru berubah.
  const flashRange = useCallback((range: { start: number; end: number } | null) => {
    if (flashTimer.current) clearTimeout(flashTimer.current);
    setFlash(range);
    if (range) {
      flashTimer.current = setTimeout(() => setFlash(null), 650);
    }
  }, []);

  // --- aksi ---------------------------------------------------------------

  const newNote = useCallback(async () => {
    const n = await api.createNote("");
    setActiveId(n.id);
    setRefreshKey((k) => k + 1);
  }, []);

  const saveDraft = useCallback(
    debounce(async (body: string, note: Note) => {
      await api.saveNote({ ...note, body });
      setRefreshKey((k) => k + 1);
    }, 400),
    [],
  );

  // Dipanggil tiap kali isi editor berubah (ketikan ATAU koreksi diterapkan).
  const onDraftChange = useCallback(
    (body: string) => {
      setDraft(body);
      if (!activeNote) return;
      // Dalam mode otomatis, judul ikut baris pertama secara langsung supaya
      // input judul menampilkannya real-time. Mode manual tidak disentuh.
      const note = activeNote.title_manual
        ? activeNote
        : { ...activeNote, title: deriveTitle(body) };
      if (!activeNote.title_manual) setActiveNote(note);
      saveDraft(body, note);
    },
    [activeNote, saveDraft],
  );

  // Ganti isi HALAMAN aktif, lalu rakit ulang seluruh catatan & simpan.
  const setPageText = useCallback(
    (newPageText: string) => {
      const next = [...splitPages(draft)];
      const i = Math.min(pageIdx, next.length - 1);
      next[i] = newPageText;
      onDraftChange(joinPages(next));
    },
    [draft, pageIdx, onDraftChange],
  );

  // --- navigasi & pengelolaan halaman ------------------------------------
  const gotoPage = useCallback((idx: number) => {
    setPageIdx(idx);
    setCorrections([]); // koreksi bersifat per-halaman
    setFlash(null);
  }, []);

  const newPage = useCallback(() => {
    const next = splitPages(draft);
    const at = Math.min(pageIdx, next.length - 1) + 1;
    next.splice(at, 0, ""); // halaman kosong tepat setelah yang sekarang
    onDraftChange(joinPages(next));
    gotoPage(at);
  }, [draft, pageIdx, onDraftChange, gotoPage]);

  const deletePage = useCallback(() => {
    const next = splitPages(draft);
    if (next.length <= 1) return; // selalu sisakan satu halaman
    const i = Math.min(pageIdx, next.length - 1);
    next.splice(i, 1);
    onDraftChange(joinPages(next));
    gotoPage(Math.max(0, i - 1));
  }, [draft, pageIdx, onDraftChange, gotoPage]);

  // Pengguna mengetik judul sendiri → tandai manual, jangan lagi ikuti isi.
  const onTitleChange = useCallback(
    (title: string) => {
      if (!activeNote) return;
      const next = { ...activeNote, title, title_manual: true };
      setActiveNote(next);
      saveDraft(draft, next);
    },
    [activeNote, draft, saveDraft],
  );

  const togglePin = useCallback(async () => {
    if (!activeNote) return;
    const next = { ...activeNote, pinned: !activeNote.pinned };
    setActiveNote(next);
    await api.saveNote({ ...next, body: draft });
    setRefreshKey((k) => k + 1);
  }, [activeNote, draft]);

  const trashActive = useCallback(async () => {
    if (!activeNote) return;
    await api.trashNote(activeNote.id);
    setActiveId(null);
    setRefreshKey((k) => k + 1);
  }, [activeNote]);

  // "Ganti": terapkan satu koreksi ke HALAMAN aktif (offset koreksi relatif
  // ke halaman), lalu kilas hijau di kata baru dan buang saran dari daftar.
  const applyCorrection = useCallback(
    (c: Correction) => {
      const next = applyCorrectionToText(pageText, c);
      setPageText(next);
      flashRange(correctionSpanInText(next, c));
      setCorrections((cs) => cs.filter((x) => x !== c));
    },
    [pageText, setPageText, flashRange],
  );

  // "Lewati": buang saran ini sekali, dan catat ke ingatan pembatalan supaya
  // pengulangan yang sama akhirnya berhenti disarankan.
  const ignoreCorrection = useCallback(async (c: Correction) => {
    setCorrections((cs) => cs.filter((x) => x !== c));
    try {
      await api.recordUndo(c.from, c.to);
    } catch (e) {
      console.error(e);
    }
  }, []);

  // "Simpan": kata ini benar — masuk kamus pribadi, tidak pernah ditandai lagi.
  const keepWord = useCallback(async (c: Correction) => {
    setCorrections((cs) => cs.filter((x) => x.from.toLowerCase() !== c.from.toLowerCase()));
    try {
      await api.addToDict(c.from);
    } catch (e) {
      console.error(e);
    }
  }, []);

  const fixAll = useCallback(async () => {
    try {
      const fixed = await api.fixAll(pageText); // hanya halaman aktif
      setPageText(fixed);
      setCorrections([]);
    } catch (e) {
      console.error(e);
    }
  }, [pageText, setPageText]);

  // --- pintasan papan tik lokal ------------------------------------------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;

      // Ctrl+N: catatan baru
      if (mod && e.key.toLowerCase() === "n" && !e.shiftKey) {
        e.preventDefault();
        newNote();
        return;
      }
      // Ctrl+K: fokus ke kotak pencarian
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        const el = document.querySelector<HTMLInputElement>(".sidebar-search input");
        el?.focus();
        el?.select();
        return;
      }
      // Ctrl+, : pengaturan
      if (mod && e.key === ",") {
        e.preventDefault();
        setSettingsOpen((v) => !v);
        return;
      }
      // Ctrl+\: pin/lepas catatan aktif
      if (mod && e.key === "\\") {
        e.preventDefault();
        togglePin();
        return;
      }
      // Ctrl+/: buka/tutup panel koreksi
      if (mod && e.key === "/") {
        e.preventDefault();
        setPanelOpen((v) => !v);
        return;
      }
      // Ctrl+Shift+.: matikan/nyalakan auto-apply
      if (mod && e.shiftKey && e.key === ".") {
        e.preventDefault();
        setAutoApply((v) => !v);
        return;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [newNote, togglePin]);

  const panelVisible = panelOpen && !!activeNote;

  return (
    <div
      ref={appRef}
      className={`app ${panelVisible ? "panel-open" : ""}`}
      style={{ "--sidebar-w": `${sidebarW}px`, "--panel-w": `${panelW}px` } as React.CSSProperties}
    >
      <aside className="sidebar">
        <Sidebar
          notes={notes}
          activeId={activeId}
          onSelect={setActiveId}
          onNew={newNote}
          refreshKey={refreshKey}
          onNotesRefresh={setNotes}
        />
      </aside>

      <main className="workspace">
        <Editor
          value={pageText}
          fullBody={draft}
          noteId={activeNote?.id ?? null}
          title={activeNote?.title ?? ""}
          corrections={corrections}
          flash={flash}
          onChange={setPageText}
          onTitleChange={onTitleChange}
          onCorrectionsChange={setCorrections}
          onFixAll={fixAll}
          autoApplyEnabled={autoApply}
          typingAnim={typingAnim}
          pageIndex={safeIdx}
          pageCount={pages.length}
          pageLimit={PAGE_CHAR_LIMIT}
          onPrevPage={() => gotoPage(safeIdx - 1)}
          onNextPage={() => gotoPage(safeIdx + 1)}
          onNewPage={newPage}
          onDeletePage={deletePage}
          onPanelToggle={() => setPanelOpen((v) => !v)}
          panelOpen={panelOpen}
        />
      </main>

      {panelVisible && (
        <aside className="side-panel">
          <CorrectionPanel
            corrections={corrections}
            onApply={applyCorrection}
            onIgnore={ignoreCorrection}
            onAddToDict={keepWord}
          />
        </aside>
      )}

      {/* Gagang seret pengubah lebar. Absolut di atas garis batas panel. */}
      <div
        className="resizer"
        style={{ left: `calc(${sidebarW}px - 3px)` }}
        onMouseDown={startResize("sidebar")}
        role="separator"
        aria-orientation="vertical"
        aria-label="Ubah lebar sidebar"
        title="Seret untuk mengubah lebar"
      />
      {panelVisible && (
        <div
          className="resizer"
          style={{ right: `calc(${panelW}px - 3px)` }}
          onMouseDown={startResize("panel")}
          role="separator"
          aria-orientation="vertical"
          aria-label="Ubah lebar panel"
          title="Seret untuk mengubah lebar"
        />
      )}

      <footer className="statusbar">
        <div className="row gap-2">
          <StatusDot ok />
          <span>{totalNotes} catatan</span>
        </div>
        <div className="grow" />
        {activeNote && (
          <div className="row gap-1">
            <button
              className={`btn-icon ${activeNote.pinned ? "active" : ""}`}
              onClick={togglePin}
              title={activeNote.pinned ? "Lepas pin (Ctrl+\\)" : "Pin catatan (Ctrl+\\)"}
              aria-pressed={activeNote.pinned}
            >
              <IconPin size={13} />
            </button>
            <button className="btn-icon" onClick={trashActive} title="Buang ke sampah">
              <IconTrash size={13} />
            </button>
            <span className="fg-faint">·</span>
          </div>
        )}
        <button
          className="btn-icon"
          onClick={() => setSettingsOpen(true)}
          title="Pengaturan (Ctrl+,)"
          aria-label="Pengaturan"
        >
          <IconSettings size={13} />
        </button>
        <span className="text-xs fg-subtle mono">
          {autoApply ? "Auto-koreksi aktif" : "Auto-koreksi dimatikan"}
        </span>
      </footer>

      <SettingsDialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        typingAnim={typingAnim}
        onTypingAnimChange={setTypingAnim}
      />
    </div>
  );
}

function StatusDot({ ok }: { ok: boolean }) {
  return (
    <span
      style={{
        display: "inline-block",
        width: 6,
        height: 6,
        borderRadius: 999,
        background: ok ? "var(--success)" : "var(--danger)",
      }}
    />
  );
}
