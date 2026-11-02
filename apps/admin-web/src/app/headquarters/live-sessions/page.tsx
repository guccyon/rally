"use client";

import { RoleGate } from "@/components/RoleGate";
import { LiveSessionRecordsHome } from "@/components/headquarters/LiveSessionRecordsHome";

export default function LiveSessionRecordsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <LiveSessionRecordsHome token={session.token} />}
    </RoleGate>
  );
}
