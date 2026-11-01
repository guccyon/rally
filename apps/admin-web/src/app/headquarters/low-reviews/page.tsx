"use client";

import { RoleGate } from "@/components/RoleGate";
import { LowRatedReviewsHome } from "@/components/headquarters/LowRatedReviewsHome";

export default function LowRatedReviewsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <LowRatedReviewsHome token={session.token} />}
    </RoleGate>
  );
}
