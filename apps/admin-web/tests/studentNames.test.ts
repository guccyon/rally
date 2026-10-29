import { describe, expect, test } from "vitest";

import type { Schemas } from "@/api/client";
import { UNKNOWN_NAME, studentNamesFromBookings } from "@/components/coach/studentNames";

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const STUDENT_A = "33333333-3333-4333-8333-333333333331";
const STUDENT_B = "33333333-3333-4333-8333-333333333332";

function booking(sequence: number, studentId: string, name?: string): Schemas["CoachBooking"] {
  const slotId = `55555555-5555-4555-8555-${String(sequence).padStart(12, "0")}`;
  return {
    booking: {
      booking_id: `44444444-4444-4444-8444-${String(sequence).padStart(12, "0")}`,
      slot_id: slotId,
      student_id: studentId,
      coach_id: COACH_ID,
      is_trial: true,
      status: "confirmed",
      starts_at: "2026-10-30T01:00:00Z",
      created_at: "2026-10-27T01:00:00Z",
    },
    slot: {
      slot_id: slotId,
      coach_id: COACH_ID,
      sport: "tennis",
      starts_at: "2026-10-30T01:00:00Z",
      ends_at: "2026-10-30T02:00:00Z",
      status: "booked",
      capacity: 1,
      remaining_seats: 0,
    },
    ...(name ? { student_display_name: name } : {}),
  };
}

describe("studentNamesFromBookings", () => {
  test("予約の一覧から生徒の ID → 表示名を作り、表示名のない予約は入れない", () => {
    const names = studentNamesFromBookings([
      booking(1, STUDENT_A, "佐藤さん"),
      booking(2, STUDENT_B),
      booking(3, STUDENT_A, "佐藤さん"),
    ]);
    expect([...names]).toEqual([[STUDENT_A, "佐藤さん"]]);
    expect(names.get(STUDENT_B) ?? UNKNOWN_NAME).toBe("（名前を表示できません）");
  });

  test("予約が 0 件なら空", () => {
    expect(studentNamesFromBookings([]).size).toBe(0);
  });
});
