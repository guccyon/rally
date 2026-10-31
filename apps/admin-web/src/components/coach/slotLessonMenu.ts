import type { Schemas } from "@/api/client";

type LessonMenu = Schemas["LessonMenu"];

/** 時間（分）が決まっている同期のメニュー。枠に結びつけられるのはこれだけ。 */
export type SelectableLessonMenu = LessonMenu & { duration_minutes: number };

/**
 * 枠に結びつけられる自分のメニュー（契約の `publishSlot` の条件のうち、画面で確かめられるもの）:
 * 公開中・同期レッスン・枠と同じスポーツ。時間が枠の長さと同じことは、終了を時間から決めて守る。
 * 並びは API の順（作った時刻の古い順）のまま。
 */
export function selectableMenus(
  menus: LessonMenu[],
  sport: Schemas["Sport"],
): SelectableLessonMenu[] {
  return menus.filter(
    (menu): menu is SelectableLessonMenu =>
      menu.is_published &&
      menu.lesson_type === "live_lesson" &&
      menu.sport === sport &&
      menu.duration_minutes !== null,
  );
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/**
 * datetime-local の値（ブラウザのタイムゾーンの時刻）に分を足し、同じ形で返す。
 * 開始が空か読めない値なら空文字列（終了は決まらない）。
 */
export function addMinutesToLocal(value: string, minutes: number): string {
  if (value === "") return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  date.setMinutes(date.getMinutes() + minutes);
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}` +
    `T${pad(date.getHours())}:${pad(date.getMinutes())}`
  );
}
