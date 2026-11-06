import type { Schemas } from "@/api/client";
import { BOOKING_STATUS_LABELS } from "@/components/labels";
import { StatusTag } from "@/components/ui/StatusTag";
import type { Status } from "@/components/ui/status";

/** 予約の状態と色の対応（specs/design/tokens.yaml の color.booking_state）。 */
const BOOKING_STATE_STATUS: Record<Schemas["BookingStatus"], Status> = {
  pending: "warning",
  confirmed: "success",
  cancelled: "neutral",
};

/** 予約の状態のタグ（components.yaml の status_tag。点と glossary の呼び名の字）。本部の予約と生徒の利用の様子で共通。 */
export function BookingStatusBadge({ status }: { status: Schemas["BookingStatus"] }) {
  return <StatusTag label={BOOKING_STATUS_LABELS[status]} status={BOOKING_STATE_STATUS[status]} />;
}
