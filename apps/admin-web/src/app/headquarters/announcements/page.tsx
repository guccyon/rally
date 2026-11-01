"use client";

import { RoleGate } from "@/components/RoleGate";
import { AnnouncementsHome } from "@/components/headquarters/AnnouncementsHome";

export default function AnnouncementsPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <AnnouncementsHome token={session.token} />}
    </RoleGate>
  );
}
