import type { Schemas } from "@/api/client";
import { BOOKING_STATUS_LABELS } from "@/components/labels";

/** 状態の色分け。確定待ちは対応が要るので目に付く色、キャンセル済みは沈める。 */
const STATUS_CLASSES: Record<Schemas["BookingStatus"], string> = {
  pending: "bg-warning-container text-warning-on-container",
  confirmed: "bg-success-container text-success-on-container",
  cancelled: "bg-surface text-text-muted",
};

/** 予約の状態の札（「確定待ち」は琥珀、「確定済み」は緑、「キャンセル済み」は灰）。本部の予約と生徒の利用の様子で共通。 */
export function BookingStatusBadge({ status }: { status: Schemas["BookingStatus"] }) {
  return <span className={`rounded-sm px-2 text-xs ${STATUS_CLASSES[status]}`}>{BOOKING_STATUS_LABELS[status]}</span>;
}
