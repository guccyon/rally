"use client";

import { RoleGate } from "@/components/RoleGate";
import { CoachReviewSummaryHome } from "@/components/headquarters/CoachReviewSummaryHome";

export default function CoachReviewSummaryPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <CoachReviewSummaryHome token={session.token} />}
    </RoleGate>
  );
}
