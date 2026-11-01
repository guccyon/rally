"use client";

import { RoleGate } from "@/components/RoleGate";
import { CoachAcceptanceHome } from "@/components/headquarters/CoachAcceptanceHome";

export default function CoachAcceptancePage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <CoachAcceptanceHome token={session.token} />}
    </RoleGate>
  );
}
