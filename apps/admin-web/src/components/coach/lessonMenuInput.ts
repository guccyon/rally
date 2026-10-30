import type { Schemas } from "@/api/client";

// 契約の CreateLessonMenuRequest / UpdateLessonMenuRequest の制約（文字数は Unicode のコードポイントで数える）。
export const NAME_MAX_LENGTH = 40;
export const DESCRIPTION_MAX_LENGTH = 400;
/** 同期レッスン 1 回の時間の選択肢（15〜120 分の 15 分刻み）。 */
export const DURATIONS = [15, 30, 45, 60, 75, 90, 105, 120] as const;
export const DEFAULT_DURATION = 60;

/** フォームの入力。時間は同期のときだけ使う（非同期では送らない）。 */
export type LessonMenuInput = {
  sport: Schemas["Sport"];
  lessonType: Schemas["LessonType"];
  name: string;
  description: string;
  durationMinutes: number;
  isPublished: boolean;
};

export type InputErrors = { name?: string; description?: string; durationMinutes?: string };

/** 契約と同じくコードポイントで数える（input の maxLength は UTF-16 の単位で数えるので使わない）。 */
export function lengthOf(text: string): number {
  return Array.from(text).length;
}

/** 新しく作るときの入力の初期値。 */
export function emptyInput(sport: Schemas["Sport"]): LessonMenuInput {
  return {
    sport,
    lessonType: "live_lesson",
    name: "",
    description: "",
    durationMinutes: DEFAULT_DURATION,
    isPublished: true,
  };
}

/** 今のメニューを編集するときの入力の初期値（非同期のメニューの時間は、同期に切り替えたときの既定にしておく）。 */
export function inputOf(menu: Schemas["LessonMenu"]): LessonMenuInput {
  return {
    sport: menu.sport,
    lessonType: menu.lesson_type,
    name: menu.name,
    description: menu.description,
    durationMinutes: menu.duration_minutes ?? DEFAULT_DURATION,
    isPublished: menu.is_published,
  };
}

/** サーバーが断る入力を送る前に見つける。問題のない項目は返さない。 */
export function validateInput(input: LessonMenuInput): InputErrors {
  const errors: InputErrors = {};
  const nameLength = lengthOf(input.name);
  if (input.name.trim() === "") {
    errors.name = "名前を入力してください。";
  } else if (nameLength > NAME_MAX_LENGTH) {
    errors.name = `名前は ${NAME_MAX_LENGTH} 文字までです。${nameLength - NAME_MAX_LENGTH} 文字減らしてください。`;
  }
  const descriptionLength = lengthOf(input.description);
  if (descriptionLength > DESCRIPTION_MAX_LENGTH) {
    errors.description = `説明は ${DESCRIPTION_MAX_LENGTH} 文字までです。${descriptionLength - DESCRIPTION_MAX_LENGTH} 文字減らしてください。`;
  }
  if (
    input.lessonType === "live_lesson" &&
    !(DURATIONS as readonly number[]).includes(input.durationMinutes)
  ) {
    errors.durationMinutes = "1 回の時間は 15〜120 分の 15 分刻みで選んでください。";
  }
  return errors;
}

export function hasErrors(errors: InputErrors): boolean {
  return Object.keys(errors).length > 0;
}

/**
 * 入力を契約の本文にする（作成と変更で同じ形。変更は `description` と `is_published` も省けないので常に送る）。
 * 名前と説明は書いたとおりに送り、非同期のときは時間を null にする。
 */
export function toRequest(input: LessonMenuInput): Schemas["UpdateLessonMenuRequest"] {
  return {
    sport: input.sport,
    lesson_type: input.lessonType,
    name: input.name,
    description: input.description,
    duration_minutes: input.lessonType === "live_lesson" ? input.durationMinutes : null,
    is_published: input.isPublished,
  };
}
