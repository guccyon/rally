import { Icon } from "./Icon";
import { STATUS_CONTAINER_CLASSES, STATUS_ICON_CLASSES, STATUS_ICONS, type Status } from "./status";

/**
 * 知らせの inline（specs/design/components.yaml の notice）。欄の中か画面の上に出し、自動で消さない。
 * 失敗（danger）は role=alert、それ以外は role=status。失敗は「何が起きたか」と「どうすればよいか」の 2 文で書く。
 */
export function Notice({ message, status }: { message: string; status: Status }) {
  return (
    <div
      role={status === "danger" ? "alert" : "status"}
      className={`flex items-start gap-2 rounded-md p-4 ${STATUS_CONTAINER_CLASSES[status]}`}
    >
      <Icon name={STATUS_ICONS[status]} size={20} className={STATUS_ICON_CLASSES[status]} />
      <p>{message}</p>
    </div>
  );
}
