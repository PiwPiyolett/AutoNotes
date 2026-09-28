import { useEffect, useState } from "react";

import type { NoteMeta, SearchHit } from "../lib/api";
import { api } from "../lib/api";
import { relativeTime, debounce } from "../lib/format";
import { IconPlus, IconSearch, IconPin, IconX } from "./Icon";
import "./Sidebar.css";

type Props = {
  notes: NoteMeta[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
  refreshKey: number; // dipakai induk untuk memaksa re-fetch daftar
  onNotesRefresh: (list: NoteMeta[]) => void;
};

export function Sidebar({ notes, activeId, onSelect, onNew, refreshKey, onNotesRefresh }: Props) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [loading, setLoading] = useState(false);

  // Muat ulang daftar catatan.
  useEffect(() => {
    api.listNotes().then(onNotesRefresh).catch(console.error);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);

  // Pencarian dengan penundaan 120 ms — cukup untuk terasa instan
  // tanpa memanggil Rust di tiap ketukan.
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setHits(null);
      setLoading(false);
      return;
    }
    setLoading(true);
    const run = debounce(async () => {
      try {
        const h = await api.searchNotes(q, 30);
        setHits(h);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    }, 120);
    run();
    return () => run.cancel();
  }, [query]);

  const showingSearch = hits !== null;

  return (
    <>
      <div className="sidebar-header">
        <div className="app-brand">
          <div className="brand-mark" aria-hidden />
          <span className="brand-name">AutoNotes</span>
        </div>
        <button className="btn btn-icon" onClick={onNew} title="Catatan baru (Ctrl+N)" aria-label="Catatan baru">
          <IconPlus size={18} />
        </button>
      </div>

      <div className="sidebar-search">
        <div className="search-input">
          <IconSearch size={14} className="search-icon" />
          <input
            className="input"
            placeholder="Cari catatan… (Ctrl+K)"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Cari catatan"
          />
          {query && (
            <button className="btn-icon search-clear" onClick={() => setQuery("")} aria-label="Bersihkan pencarian">
              <IconX size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="divider" />

      <div className="note-list scroll-y" role="listbox" aria-label="Daftar catatan">
        {showingSearch ? (
          <SearchResults
            hits={hits}
            loading={loading}
            query={query}
            activeId={activeId}
            onSelect={onSelect}
          />
        ) : (
          <NoteBrowser notes={notes} activeId={activeId} onSelect={onSelect} />
        )}
      </div>
    </>
  );
}

function NoteBrowser({
  notes,
  activeId,
  onSelect,
}: {
  notes: NoteMeta[];
  activeId: string | null;
  onSelect: (id: string) => void;
}) {
  if (notes.length === 0) {
    return (
      <div className="empty-hint">
        <p>Belum ada catatan.</p>
        <p className="fg-subtle text-xs">Tekan <kbd>Ctrl</kbd>+<kbd>N</kbd> untuk memulai.</p>
      </div>
    );
  }
  const pinned = notes.filter((n) => n.pinned);
  const rest = notes.filter((n) => !n.pinned);
  return (
    <>
      {pinned.length > 0 && (
        <>
          <div className="list-heading">Disematkan</div>
          {pinned.map((n) => (
            <NoteRow key={n.id} note={n} active={n.id === activeId} onClick={() => onSelect(n.id)} />
          ))}
          <div className="list-heading">Semua catatan</div>
        </>
      )}
      {rest.map((n) => (
        <NoteRow key={n.id} note={n} active={n.id === activeId} onClick={() => onSelect(n.id)} />
      ))}
    </>
  );
}

function NoteRow({ note, active, onClick }: { note: NoteMeta; active: boolean; onClick: () => void }) {
  return (
    <button
      className={`note-row ${active ? "active" : ""}`}
      onClick={onClick}
      role="option"
      aria-selected={active}
    >
      <div className="row gap-2" style={{ minWidth: 0 }}>
        {note.pinned && <IconPin size={12} className="pin-icon" />}
        <span className="note-title truncate">{note.title || "Tanpa judul"}</span>
      </div>
      <div className="row gap-2 note-meta">
        <span className="text-xs fg-subtle">{relativeTime(note.updated_at)}</span>
        {note.tags.slice(0, 2).map((t) => (
          <span key={t} className="chip chip-teal">#{t}</span>
        ))}
      </div>
    </button>
  );
}

function SearchResults({
  hits,
  loading,
  query,
  activeId,
  onSelect,
}: {
  hits: SearchHit[];
  loading: boolean;
  query: string;
  activeId: string | null;
  onSelect: (id: string) => void;
}) {
  if (loading && hits.length === 0) {
    return <div className="empty-hint fg-subtle text-xs">Mencari…</div>;
  }
  if (hits.length === 0) {
    return (
      <div className="empty-hint">
        <p>Tidak ada catatan yang cocok.</p>
        <p className="fg-subtle text-xs">Coba kata kunci lain, atau tekan Enter untuk membuat catatan berjudul "{query}".</p>
      </div>
    );
  }
  return (
    <>
      <div className="list-heading">{hits.length} hasil</div>
      {hits.map((h) => (
        <button
          key={h.id}
          className={`note-row search-row ${h.id === activeId ? "active" : ""}`}
          onClick={() => onSelect(h.id)}
          role="option"
          aria-selected={h.id === activeId}
        >
          <span className="note-title truncate">{h.title || "Tanpa judul"}</span>
          <span
            className="snippet text-xs fg-muted"
            dangerouslySetInnerHTML={{ __html: highlightSnippet(h.snippet) }}
          />
        </button>
      ))}
    </>
  );
}

/**
 * SQLite mengembalikan cuplikan dengan penanda «…» yang aman (bukan HTML),
 * jadi kita tinggal menggantinya dengan `<mark>`. Escape tetap dilakukan
 * untuk sisanya supaya isi catatan yang kebetulan berisi tanda `<` tidak
 * diperlakukan sebagai markup.
 */
function highlightSnippet(raw: string): string {
  const escaped = raw
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  return escaped.replace(/«/g, "<mark>").replace(/»/g, "</mark>");
}
