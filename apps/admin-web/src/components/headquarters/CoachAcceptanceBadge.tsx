import type { Schemas } from "@/api/client";
import { COACH_ACCEPTANCE_STATUS_LABELS } from "@/components/labels";

const STATUS_CLASSES: Record<Schemas["CoachAcceptanceStatus"], string> = {
  accepting: "bg-success-subtle text-success-strong",
  suspended: "bg-inactive-subtle text-inactive-strong",
};

/** コーチの受付の状態の札（「受付中」は緑、「停止中」は灰）。本部のコーチの受付とコーチの様子で共通。 */
export function CoachAcceptanceBadge({ status }: { status: Schemas["CoachAcceptanceStatus"] }) {
  return (
    <span className={`rounded px-2 py-0.5 text-xs ${STATUS_CLASSES[status]}`}>
      {COACH_ACCEPTANCE_STATUS_LABELS[status]}
    </span>
  );
}
