"use client";

import { RoleGate } from "@/components/RoleGate";
import { LessonMenuCatalogHome } from "@/components/headquarters/LessonMenuCatalogHome";

export default function LessonMenuCatalogPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <LessonMenuCatalogHome token={session.token} />}
    </RoleGate>
  );
}
