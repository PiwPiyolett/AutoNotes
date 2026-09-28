import type { Correction, Source } from "../lib/api";
import { IconCheck, IconX, IconPlus } from "./Icon";
import "./CorrectionPanel.css";

type Props = {
  corrections: Correction[];
  /** Ganti kata dengan usulan sekarang. */
  onApply: (c: Correction) => void;
  /** Lewati saran ini sekali — kata dibiarkan apa adanya untuk saat ini. */
  onIgnore: (c: Correction) => void;
  /** Tandai kata sebagai benar selamanya — jangan pernah disarankan lagi. */
  onAddToDict: (c: Correction) => void;
};

/**
 * Panel saran koreksi. Menampilkan saran yang **belum** diterapkan (koreksi
 * berkeyakinan-tinggi yang sudah otomatis diterapkan tidak muncul di sini —
 * ia sudah ada di teks). Tiap kartu punya tiga aksi dengan makna yang sengaja
 * dibuat tidak tumpang tindih:
 *
 * - **Ganti**  : terima usulan, kata ditukar sekarang.
 * - **Simpan** : kata ini memang benar (nama orang, istilah) — masukkan ke
 *                kamus pribadi supaya tidak pernah ditandai lagi, di catatan
 *                mana pun.
 * - **Lewati** : biarkan kata apa adanya kali ini. Saran hilang; kalau kata
 *                yang sama muncul lagi nanti, ia disarankan lagi. (Melewati
 *                pasangan yang sama dua kali membuat aplikasi berhenti
 *                menyarankannya — "belajar dari kebiasaanmu".)
 */
export function CorrectionPanel({ corrections, onApply, onIgnore, onAddToDict }: Props) {
  const disarankan = corrections.filter((c) => c.auto);
  const perluKeputusan = corrections.filter((c) => !c.auto);

  return (
    <>
      <div className="panel-header">
        <h3 className="panel-title">Saran koreksi</h3>
        <span className="fg-subtle text-xs">{corrections.length} kata</span>
      </div>

      <div className="panel-body scroll-y">
        {corrections.length === 0 ? (
          <EmptyPanel />
        ) : (
          <>
            {disarankan.length > 0 && (
              <>
                <SectionHeader
                  label="Disarankan"
                  hint="Keyakinan tinggi, biasanya aman untuk diganti."
                />
                {disarankan.map((c) => (
                  <CorrectionCard
                    key={`${c.start}-${c.from}`}
                    c={c}
                    onApply={() => onApply(c)}
                    onIgnore={() => onIgnore(c)}
                    onAddToDict={() => onAddToDict(c)}
                  />
                ))}
              </>
            )}
            {perluKeputusan.length > 0 && (
              <>
                <SectionHeader
                  label="Menunggu keputusanmu"
                  hint="Bisa jadi kata yang kamu maksud memang benar. Kamu yang putuskan."
                />
                {perluKeputusan.map((c) => (
                  <CorrectionCard
                    key={`${c.start}-${c.from}`}
                    c={c}
                    onApply={() => onApply(c)}
                    onIgnore={() => onIgnore(c)}
                    onAddToDict={() => onAddToDict(c)}
                  />
                ))}
              </>
            )}
          </>
        )}
      </div>

      {corrections.length > 0 && (
        <div className="panel-legend">
          <LegendRow icon={<IconCheck size={12} />} label="Ganti: pakai usulan" />
          <LegendRow icon={<IconPlus size={12} />} label="Simpan: kata ini selalu benar" />
          <LegendRow icon={<IconX size={12} />} label="Lewati: biarkan sekali ini" />
        </div>
      )}
    </>
  );
}

function LegendRow({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <div className="legend-row">
      <span className="legend-icon">{icon}</span>
      <span>{label}</span>
    </div>
  );
}

function EmptyPanel() {
  return (
    <div className="empty-panel">
      <div className="empty-panel-mark" aria-hidden>
        <IconCheck size={20} />
      </div>
      <p>Tidak ada saran.</p>
      <p className="fg-subtle text-xs">Autocorrect terus mengawasi saat kamu mengetik.</p>
    </div>
  );
}

function SectionHeader({ label, hint }: { label: string; hint: string }) {
  return (
    <div className="section-header">
      <div className="section-label">{label}</div>
      <div className="section-hint text-xs fg-subtle">{hint}</div>
    </div>
  );
}

function CorrectionCard({
  c,
  onApply,
  onIgnore,
  onAddToDict,
}: {
  c: Correction;
  onApply: () => void;
  onIgnore: () => void;
  onAddToDict: () => void;
}) {
  const pct = Math.round(c.confidence * 100);
  return (
    <div className={`cc-card source-${c.source}`}>
      <div className="cc-diff">
        <span className="cc-from">{c.from}</span>
        <span className="cc-arrow" aria-hidden>→</span>
        <span className="cc-to">{c.to}</span>
      </div>
      <div className="cc-meta">
        <span className={`chip ${chipVariant(c.source)}`}>{sourceLabel(c.source)}</span>
        <span className="text-xs fg-subtle">{pct}% yakin</span>
      </div>
      <div className="cc-actions">
        <button className="btn btn-primary" onClick={onApply} title="Pakai usulan">
          <IconCheck size={13} /> Ganti
        </button>
        <button
          className="btn btn-ghost"
          onClick={onAddToDict}
          title="Kata ini memang benar, jangan pernah tandai lagi"
        >
          <IconPlus size={13} /> Simpan
        </button>
        <button className="btn btn-ghost" onClick={onIgnore} title="Biarkan sekali ini">
          <IconX size={13} /> Lewati
        </button>
      </div>
    </div>
  );
}

function sourceLabel(s: Source): string {
  switch (s) {
    case "typo": return "typo";
    case "slang": return "singkatan";
    case "context": return "konteks";
    default: return "kamus";
  }
}

function chipVariant(s: Source): string {
  switch (s) {
    case "typo": return "chip-orange";
    case "slang": return "chip";
    case "context": return "chip-context";
    default: return "chip-teal";
  }
}
