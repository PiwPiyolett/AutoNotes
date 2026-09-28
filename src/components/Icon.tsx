/**
 * Ikon SVG inline. Lucide-style, semua stroke 1.75 supaya beratnya seragam.
 * Bukan emoji — emoji tampil beda di tiap OS dan tidak bisa dikontrol
 * warnanya oleh tema.
 */

type Props = { size?: number; className?: string; strokeWidth?: number };

const base = (children: React.ReactNode, { size = 16, className, strokeWidth = 1.75 }: Props) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={strokeWidth}
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
    aria-hidden="true"
  >
    {children}
  </svg>
);

export const IconPlus = (p: Props = {}) =>
  base(<><path d="M12 5v14" /><path d="M5 12h14" /></>, p);

export const IconSearch = (p: Props = {}) =>
  base(<><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></>, p);

export const IconPin = (p: Props = {}) =>
  base(
    <><path d="M12 17v5" /><path d="M9 10.76V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v6.76a2 2 0 0 0 .48 1.3l2 2.55A1 1 0 0 1 16.72 17H7.28a1 1 0 0 1-.76-1.39l2-2.55A2 2 0 0 0 9 10.76Z" /></>,
    p,
  );

export const IconTrash = (p: Props = {}) =>
  base(
    <><path d="M3 6h18" /><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" /></>,
    p,
  );

export const IconSettings = (p: Props = {}) =>
  base(
    <>
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z" />
      <circle cx="12" cy="12" r="3" />
    </>,
    p,
  );

export const IconPanelRight = (p: Props = {}) =>
  base(<><rect width="18" height="18" x="3" y="3" rx="2" /><path d="M15 3v18" /></>, p);

export const IconCheck = (p: Props = {}) =>
  base(<path d="M20 6 9 17l-5-5" />, p);

export const IconWand = (p: Props = {}) =>
  base(
    <>
      <path d="M15 4V2" /><path d="M15 16v-2" /><path d="M8 9h2" /><path d="M20 9h2" />
      <path d="M17.8 11.8 19 13" /><path d="M15 9h.01" /><path d="M17.8 6.2 19 5" />
      <path d="m3 21 9-9" /><path d="M12.2 6.2 11 5" />
    </>,
    p,
  );

export const IconCircle = (p: Props = {}) =>
  base(<circle cx="12" cy="12" r="9" />, p);

export const IconSpark = (p: Props = {}) =>
  base(
    <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.582a.5.5 0 0 1 0 .962L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" />,
    p,
  );

export const IconChevronLeft = (p: Props = {}) =>
  base(<path d="m15 18-6-6 6-6" />, p);

export const IconChevronRight = (p: Props = {}) =>
  base(<path d="m9 18 6-6-6-6" />, p);

export const IconChevronDown = (p: Props = {}) =>
  base(<path d="m6 9 6 6 6-6" />, p);

export const IconX = (p: Props = {}) =>
  base(<><path d="M18 6 6 18" /><path d="m6 6 12 12" /></>, p);

export const IconDownload = (p: Props = {}) =>
  base(
    <><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><path d="M7 10l5 5 5-5" /><path d="M12 15V3" /></>,
    p,
  );

export const IconPageBreak = (p: Props = {}) =>
  base(
    <>
      <path d="M4 4h10l6 6v0" /><path d="M14 4v6h6" />
      <path d="M3 14h18" strokeDasharray="3 2" />
      <path d="M6 20h12" /><path d="M6 17h12" />
    </>,
    p,
  );

export const IconBook = (p: Props = {}) =>
  base(
    <>
      <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20" />
    </>,
    p,
  );
