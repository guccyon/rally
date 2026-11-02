"use client";

import { RoleGate } from "@/components/RoleGate";
import { OverdueLiveSessionsHome } from "@/components/headquarters/OverdueLiveSessionsHome";

export default function OverdueLiveSessionsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <OverdueLiveSessionsHome token={session.token} />}
    </RoleGate>
  );
}
