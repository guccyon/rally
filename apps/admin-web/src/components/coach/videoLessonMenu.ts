import type { Schemas } from "@/api/client";

/**
 * 動画を送った時点のレッスンメニュー（契約の `CoachVideo.lesson_menu`）の表示。「メニュー: 名前」の形で
 * （student-app のメニューの表示と同じ形）、メニューを選ばずに送られた動画は null（何も出さない）。
 */
export function videoLessonMenuLabel(
  menu: Schemas["VideoLessonMenu"] | null | undefined,
): string | null {
  return menu ? `メニュー: ${menu.name}` : null;
}
