"use client";

import { RoleGate } from "@/components/RoleGate";
import { IncompleteCoachProfilesHome } from "@/components/headquarters/IncompleteCoachProfilesHome";

export default function IncompleteCoachProfilesPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <IncompleteCoachProfilesHome token={session.token} />}
    </RoleGate>
  );
}
