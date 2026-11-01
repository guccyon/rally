"use client";

import { RoleGate } from "@/components/RoleGate";
import { PendingVideoReviewsHome } from "@/components/headquarters/PendingVideoReviewsHome";

export default function PendingVideoReviewsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <PendingVideoReviewsHome token={session.token} />}
    </RoleGate>
  );
}
