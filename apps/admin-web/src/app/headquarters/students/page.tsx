"use client";

import { RoleGate } from "@/components/RoleGate";
import { StudentDirectoryHome } from "@/components/headquarters/StudentDirectoryHome";

export default function StudentDirectoryPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <StudentDirectoryHome token={session.token} />}
    </RoleGate>
  );
}
