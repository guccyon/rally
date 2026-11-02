"use client";

import { RoleGate } from "@/components/RoleGate";
import { CancelledBookingsHome } from "@/components/headquarters/CancelledBookingsHome";

export default function CancelledBookingsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <CancelledBookingsHome token={session.token} />}
    </RoleGate>
  );
}
