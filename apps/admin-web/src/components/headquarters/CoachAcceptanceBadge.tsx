import type { Schemas } from "@/api/client";
import { COACH_ACCEPTANCE_STATUS_LABELS } from "@/components/labels";
import { StatusTag } from "@/components/ui/StatusTag";
import type { Status } from "@/components/ui/status";

const ACCEPTANCE_STATUS: Record<Schemas["CoachAcceptanceStatus"], Status> = {
  accepting: "success",
  suspended: "neutral",
};

/** コーチの受付の状態のタグ（components.yaml の status_tag。「受付中」は success、「停止中」は neutral）。本部のコーチの受付とコーチの様子で共通。 */
export function CoachAcceptanceBadge({ status }: { status: Schemas["CoachAcceptanceStatus"] }) {
  return <StatusTag label={COACH_ACCEPTANCE_STATUS_LABELS[status]} status={ACCEPTANCE_STATUS[status]} />;
}
