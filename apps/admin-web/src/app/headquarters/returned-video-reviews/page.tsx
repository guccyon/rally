"use client";

import { RoleGate } from "@/components/RoleGate";
import { ReturnedVideoReviewsHome } from "@/components/headquarters/ReturnedVideoReviewsHome";

export default function ReturnedVideoReviewsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <ReturnedVideoReviewsHome token={session.token} />}
    </RoleGate>
  );
}
