"use client";

import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";

export default function HeadquartersPage() {
  return <RoleGate role="headquarters">{() => <HeadquartersHome />}</RoleGate>;
}
