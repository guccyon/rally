import { STATUS_DOT_CLASSES, type Status } from "./status";

/**
 * 状態のタグ（specs/design/components.yaml の status_tag）。8px の状態の色の点と、glossary の呼び名の文字（small 700）で出す。
 * 淡い色の面の札にしない（0.5.0）。neutral は点が border-strong で文字が text-muted。
 * urgent は data_table の急ぎの行だけで、文字も danger にする。
 */
export function StatusTag({ label, status, urgent = false }: { label: string; status: Status; urgent?: boolean }) {
  const text = urgent ? "text-danger" : status === "neutral" ? "text-text-muted" : "text-text";
  return (
    <span className={`inline-flex items-center gap-1 whitespace-nowrap text-sm font-bold ${text}`}>
      <span
        aria-hidden="true"
        data-testid="status-dot"
        className={`inline-block size-2 shrink-0 rounded-pill ${urgent ? "bg-danger" : STATUS_DOT_CLASSES[status]}`}
      />
      {label}
    </span>
  );
}
