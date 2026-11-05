"use client";

import { RoleGate } from "@/components/RoleGate";
import { SlotSupplyHome } from "@/components/headquarters/SlotSupplyHome";

export default function SlotSupplyPage() {
  return (
    <RoleGate role="headquarters" width="wide">{(session) => <SlotSupplyHome token={session.token} />}</RoleGate>
  );
}
