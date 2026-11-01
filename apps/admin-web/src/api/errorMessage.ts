import type { Schemas } from "./client";

// 契約の ErrorCode → 画面に出す日本語。Error.message は「表示用ではない」（契約）ので、文言はここ 1 か所で決める。
const MESSAGES: Record<Schemas["ErrorCode"], string> = {
  unauthorized: "トークンがないか無効です。ログインし直してください。",
  forbidden: "権限がないためアクセスできません。",
  not_found: "対象が見つかりません（ID を確かめてください）。",
  validation_failed: "入力の内容が正しくありません。",
  slot_overlaps: "公開済みの枠と時間が重なっています。",
  slot_has_active_booking: "予約が入っている枠は取り下げられません。",
  slot_already_started: "始まった枠は取り下げられません。",
  slot_already_closed: "この枠はすでに取り下げられています。",
  invalid_booking_state: "予約が確定していないか、キャンセルされています。",
  invalid_session_state: "この通話はすでに開始済みか、終了しています。",
  invalid_video_state: "この動画の添削はすでに返却済みです。",
  review_payment_not_confirmed: "ワンショットの決済が確定していないため、添削に着手できません。",
  chat_not_entitled: "この生徒はまだ月額契約がないため、チャットを送れません。",
  coach_already_suspended: "このコーチの受付はすでに停止しています。",
  coach_not_suspended: "このコーチの受付は停止していません。",
  lesson_menu_unpublished_by_headquarters:
    "本部が非公開にしたメニューのため、公開できません。非公開のままなら内容は直せます。",
  lesson_menu_already_unpublished_by_headquarters: "このメニューはすでに本部が非公開にしています。",
  lesson_menu_not_unpublished_by_headquarters: "このメニューは本部が非公開にしたものではありません。",
  lesson_review_not_low_rated: "低い評価ではないため、対応済みにできません。",
};

const UNKNOWN = "エラーが発生しました。時間をおいてやり直してください。";

/** API のエラー本文（契約の Error）から既知の ErrorCode を取り出す。未知のコード（契約は値を追加しうる）や本文がなければ undefined。 */
export function errorCode(error: unknown): Schemas["ErrorCode"] | undefined {
  if (typeof error === "object" && error !== null && "code" in error) {
    const code = (error as { code: unknown }).code;
    if (typeof code === "string" && Object.hasOwn(MESSAGES, code)) {
      return code as Schemas["ErrorCode"];
    }
  }
  return undefined;
}

/** API のエラー本文（契約の Error）を画面の文言にする。未知のコードは汎用の文言。 */
export function errorMessage(error: unknown): string {
  const code = errorCode(error);
  return code === undefined ? UNKNOWN : MESSAGES[code];
}
