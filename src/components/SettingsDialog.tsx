import { useEffect, useState } from "react";

import type { Aggressiveness, DictStatus, Settings } from "../lib/api";
import { api } from "../lib/api";
import type { Appearance, Skin } from "../lib/theme";
import {
  SKINS,
  getSkin,
  isDarkOnly,
  setSkin as applySkin,
  getAppearance,
  setAppearance as applyAppearance,
} from "../lib/theme";
import type { TypingAnim } from "../lib/prefs";
import { TYPING_ANIMS, setTypingAnim as applyTypingAnim } from "../lib/prefs";
import { IconX, IconSettings } from "./Icon";
import "./SettingsDialog.css";

type Props = {
  open: boolean;
  onClose: () => void;
  typingAnim: TypingAnim;
  onTypingAnimChange: (v: TypingAnim) => void;
};

const APPEARANCES: { key: Appearance; label: string }[] = [
  { key: "system", label: "Ikut sistem" },
  { key: "light", label: "Terang" },
  { key: "dark", label: "Gelap" },
];

const LEVELS: { key: Aggressiveness; label: string; blurb: string }[] = [
  { key: "konservatif", label: "Konservatif", blurb: "Hanya perbaiki yang nyaris pasti. Sisanya cuma diberi garis." },
  { key: "seimbang",    label: "Seimbang",    blurb: "Rekomendasi untuk hampir semua orang." },
  { key: "agresif",     label: "Agresif",     blurb: "Lebih banyak diperbaiki otomatis. Risiko salah lebih besar." },
];

export function SettingsDialog({ open, onClose, typingAnim, onTypingAnimChange }: Props) {
  const chooseTypingAnim = (v: TypingAnim) => {
    applyTypingAnim(v);
    onTypingAnimChange(v);
  };
  const [settings, setSettings] = useState<Settings | null>(null);
  const [dict, setDict] = useState<DictStatus | null>(null);
  const [skin, setSkinState] = useState<Skin>(getSkin());
  const [appearance, setAppearanceState] = useState<Appearance>(getAppearance());

  const chooseSkin = (s: Skin) => {
    applySkin(s);
    setSkinState(s);
  };
  const chooseAppearance = (a: Appearance) => {
    applyAppearance(a);
    setAppearanceState(a);
  };

  useEffect(() => {
    if (!open) return;
    Promise.all([api.getSettings(), api.dictStatus()])
      .then(([s, d]) => {
        setSettings(s);
        setDict(d);
      })
      .catch(console.error);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open || !settings || !dict) return null;

  const update = (patch: Partial<Settings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    api.setSettings(next).catch(console.error);
  };

  return (
    <div className="dialog-scrim" role="dialog" aria-modal="true" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <div className="row gap-2">
            <IconSettings size={16} />
            <h2 className="dialog-title">Pengaturan</h2>
          </div>
          <button className="btn btn-icon" onClick={onClose} aria-label="Tutup">
            <IconX size={16} />
          </button>
        </div>

        <div className="dialog-body scroll-y">
          <Section title="Tema" description="Bahasa visual aplikasi. Bisa ditambah tema unik lain nanti.">
            <div className="level-picker">
              {SKINS.map((s) => (
                <button
                  key={s.id}
                  className={`level-card ${skin === s.id ? "active" : ""}`}
                  onClick={() => chooseSkin(s.id)}
                  aria-pressed={skin === s.id}
                >
                  <span className="level-name">{s.label}</span>
                  <span className="level-blurb text-xs fg-muted">{s.blurb}</span>
                </button>
              ))}
            </div>
            <div className="appearance-row">
              <span className="appearance-label">
                Mode warna
                {isDarkOnly(skin) && (
                  <span className="fg-subtle text-xs" style={{ display: "block", fontWeight: 400 }}>
                    Tema ini selalu gelap
                  </span>
                )}
              </span>
              <div className={`appearance-seg ${isDarkOnly(skin) ? "disabled" : ""}`}>
                {APPEARANCES.map((a) => (
                  <button
                    key={a.key}
                    className={`seg-btn ${appearance === a.key ? "on" : ""}`}
                    onClick={() => chooseAppearance(a.key)}
                    disabled={isDarkOnly(skin)}
                    aria-pressed={appearance === a.key}
                  >
                    {a.label}
                  </button>
                ))}
              </div>
            </div>
          </Section>

          <div className="divider" />

          <Section title="Animasi mengetik" description="Efek saat huruf baru muncul di editor.">
            <div className="level-picker anim-picker">
              {TYPING_ANIMS.map((t) => (
                <button
                  key={t.id}
                  className={`level-card ${typingAnim === t.id ? "active" : ""}`}
                  onClick={() => chooseTypingAnim(t.id)}
                  aria-pressed={typingAnim === t.id}
                >
                  <span className="level-name">{t.label}</span>
                  <span className="level-blurb text-xs fg-muted">{t.blurb}</span>
                </button>
              ))}
            </div>
          </Section>

          <div className="divider" />

          <Section title="Tingkat agresivitas" description="Menentukan seberapa yakin koreksi harus sebelum diterapkan otomatis.">
            <div className="level-picker">
              {LEVELS.map((l) => (
                <button
                  key={l.key}
                  className={`level-card ${settings.aggressiveness === l.key ? "active" : ""}`}
                  onClick={() => update({ aggressiveness: l.key })}
                  aria-pressed={settings.aggressiveness === l.key}
                >
                  <span className="level-name">{l.label}</span>
                  <span className="level-blurb text-xs fg-muted">{l.blurb}</span>
                </button>
              ))}
            </div>
          </Section>

          <div className="divider" />

          <Section title="Bahasa">
            <Toggle
              label="Bahasa Indonesia"
              hint="Kamus 40 rb kata + kupasan imbuhan (peluluhan me-, di-, ber-…)"
              checked={settings.check_indonesian}
              onChange={(v) => update({ check_indonesian: v })}
            />
            <Toggle
              label="Bahasa Inggris"
              hint="Kamus 50 rb kata + tabel typo klasik."
              checked={settings.check_english}
              onChange={(v) => update({ check_english: v })}
            />
          </Section>

          <div className="divider" />

          <Section title="Perilaku">
            <Toggle
              label="Bentangkan singkatan sehari-hari"
              hint={<>Ubah <code>yg</code> → <code>yang</code>, <code>bgt</code> → <code>banget</code>. Matikan kalau kamu suka menyingkat.</>}
              checked={settings.slang_expansion}
              onChange={(v) => update({ slang_expansion: v })}
            />
          </Section>

          <div className="divider" />

          <Section title="Status kamus">
            <div className="dict-status">
              <StatusRow label="Kata Indonesia" value={dict.id_words.toLocaleString("id-ID")} />
              <StatusRow label="Kata Inggris" value={dict.en_words.toLocaleString("id-ID")} />
              <StatusRow
                label="Deteksi typo aktif"
                value={dict.ready ? "Ya" : "Belum, kamus terlalu kecil"}
                positive={dict.ready}
              />
              <StatusRow
                label="Koreksi sadar-konteks (T2)"
                value={
                  dict.context_enabled
                    ? `Aktif · ${dict.bigram_pairs.toLocaleString("id-ID")} pasangan`
                    : "Tidak aktif"
                }
                positive={dict.context_enabled}
              />
            </div>
            <p className="fg-subtle text-xs" style={{ marginTop: "var(--space-2)" }}>
              Koreksi sadar-konteks membedakan kata sah yang keliru menurut
              kalimatnya, mis. "makam nasi" jadi "makan nasi".
            </p>
          </Section>
        </div>
      </div>
    </div>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="dialog-section">
      <div className="dialog-section-head">
        <h3>{title}</h3>
        {description && <p className="fg-muted text-sm">{description}</p>}
      </div>
      <div className="stack gap-2">{children}</div>
    </section>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: React.ReactNode;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="toggle">
      <div className="toggle-text">
        <span className="toggle-label">{label}</span>
        {hint && <span className="toggle-hint text-xs fg-muted">{hint}</span>}
      </div>
      <span className={`toggle-switch ${checked ? "on" : ""}`} onClick={() => onChange(!checked)}>
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          className="sr-only"
        />
        <span className="toggle-thumb" />
      </span>
    </label>
  );
}

function StatusRow({ label, value, positive }: { label: string; value: string; positive?: boolean }) {
  return (
    <div className="status-row">
      <span className="fg-muted text-sm">{label}</span>
      <span className={`mono text-sm ${positive === true ? "status-good" : positive === false ? "status-warn" : ""}`}>
        {value}
      </span>
    </div>
  );
}
