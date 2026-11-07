import { Icon } from "./Icon";
import { STATUS_EDGE_CLASSES, STATUS_ICON_CLASSES, STATUS_ICONS, type Status } from "./status";

/**
 * 知らせ（specs/design/components.yaml の notice）。自動で消さない。淡い色の面は使わない（0.5.0）。
 * 失敗・注意・情報は inline（background に border の 1px と、左に状態の色の 4px の線とアイコン。文字は text）。
 * 成功は success（inverse-surface の濃い面と、完了の印のライムの丸）。
 * 失敗（danger）は role=alert、それ以外は role=status。失敗は「何が起きたか」と「どうすればよいか」の 2 文で書く。
 */
export function Notice({ message, status }: { message: string; status: Status }) {
  if (status === "success") {
    return (
      <div role="status" className="flex items-start gap-2 rounded-md bg-inverse-surface p-4 font-bold text-on-inverse-surface">
        <span
          aria-hidden="true"
          data-testid="done-mark"
          className="mt-1 inline-flex size-5 shrink-0 items-center justify-center rounded-pill bg-secondary text-on-secondary"
        >
          <Icon name="check" size={16} />
        </span>
        <p>{message}</p>
      </div>
    );
  }
  return (
    <div
      role={status === "danger" ? "alert" : "status"}
      className={`flex items-start gap-2 rounded-md border border-l-4 border-border bg-background p-4 text-text ${STATUS_EDGE_CLASSES[status]}`}
    >
      <Icon name={STATUS_ICONS[status]} size={20} className={`mt-1 shrink-0 ${STATUS_ICON_CLASSES[status]}`} />
      <p>{message}</p>
    </div>
  );
}
