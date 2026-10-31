"use client";

import { RoleGate } from "@/components/RoleGate";
import { BookingListHome } from "@/components/headquarters/BookingListHome";

export default function BookingListPage() {
  return (
    <RoleGate role="headquarters">
      {(session) => <BookingListHome token={session.token} />}
    </RoleGate>
  );
}
