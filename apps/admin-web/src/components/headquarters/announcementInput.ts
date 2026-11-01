import type { Schemas } from "@/api/client";
import { lengthOf } from "@/components/coach/lessonMenuInput";

// 契約の SendAnnouncementRequest の制約（文字数は Unicode のコードポイントで数える）。
export const TITLE_MAX_LENGTH = 50;
export const BODY_MAX_LENGTH = 2000;

/** フォームの入力。対象のスポーツが "" なら生徒全員。 */
export type AnnouncementInput = {
  sport: Schemas["Sport"] | "";
  title: string;
  body: string;
};

export type AnnouncementInputErrors = { title?: string; body?: string };

export const EMPTY_INPUT: AnnouncementInput = { sport: "", title: "", body: "" };

/** サーバーが断る入力（空・空白だけ・長すぎる）を送る前に見つける。問題のない項目は返さない。 */
export function validateAnnouncement(input: AnnouncementInput): AnnouncementInputErrors {
  const errors: AnnouncementInputErrors = {};
  const titleLength = lengthOf(input.title);
  if (input.title.trim() === "") {
    errors.title = "題を入力してください。";
  } else if (titleLength > TITLE_MAX_LENGTH) {
    errors.title = `題は ${TITLE_MAX_LENGTH} 文字までです。${titleLength - TITLE_MAX_LENGTH} 文字減らしてください。`;
  }
  const bodyLength = lengthOf(input.body);
  if (input.body.trim() === "") {
    errors.body = "本文を入力してください。";
  } else if (bodyLength > BODY_MAX_LENGTH) {
    errors.body = `本文は ${BODY_MAX_LENGTH} 文字までです。${bodyLength - BODY_MAX_LENGTH} 文字減らしてください。`;
  }
  return errors;
}

/** 入力の対象を契約の形にする（生徒全員なら null）。 */
export function targetSportOf(input: AnnouncementInput): Schemas["Sport"] | null {
  return input.sport === "" ? null : input.sport;
}

/** 入力を契約の本文にする。題と本文は書いたとおりに送り（改行も含めて保存される）、生徒全員なら sport を null にする。 */
export function toAnnouncementRequest(input: AnnouncementInput): Schemas["SendAnnouncementRequest"] {
  return {
    title: input.title,
    body: input.body,
    sport: targetSportOf(input),
  };
}
