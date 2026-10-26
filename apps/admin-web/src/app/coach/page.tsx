"use client";

import { RoleGate } from "@/components/RoleGate";
import { CoachHome } from "@/components/coach/CoachHome";

export default function CoachPage() {
  return <RoleGate role="coach">{(session) => <CoachHome token={session.token} />}</RoleGate>;
}
