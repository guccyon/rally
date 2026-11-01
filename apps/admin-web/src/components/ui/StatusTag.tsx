import { STATUS_CONTAINER_CLASSES, STATUS_ICON_CLASSES, STATUS_ICONS, type Status } from "./status";

/** 状態のタグ（specs/design/components.yaml の status_tag）。色だけでなく glossary の呼び名の文字を出す。 */
export function StatusTag({ label, status }: { label: string; status: Status }) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-sm px-2 text-xs font-bold ${STATUS_CONTAINER_CLASSES[status]}`}
    >
      <span aria-hidden="true" className={STATUS_ICON_CLASSES[status]}>
        {STATUS_ICONS[status]}
      </span>
      {label}
    </span>
  );
}
