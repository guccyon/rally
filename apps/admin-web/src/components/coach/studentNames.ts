import type { Schemas } from "@/api/client";

type CoachBooking = Schemas["CoachBooking"];

/** 生徒の表示名を引けないときの文言。 */
export const UNKNOWN_NAME = "（名前を表示できません）";

/**
 * コーチの自分の予約の一覧（listMyCoachBookings）から、生徒の ID → 表示名を作る。
 * スレッドは生徒の ID しか持たないので、その表示名はここから引く（スレッドは予約が確定したときに開くので、
 * 当事者の生徒は予約の一覧にいる）。表示名のない予約は入れない。
 */
export function studentNamesFromBookings(bookings: CoachBooking[]): Map<string, string> {
  const names = new Map<string, string>();
  for (const item of bookings) {
    if (item.student_display_name) names.set(item.booking.student_id, item.student_display_name);
  }
  return names;
}
