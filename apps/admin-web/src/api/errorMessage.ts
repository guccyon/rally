import type { Schemas } from "./client";

// 契約の ErrorCode → 画面に出す日本語。Error.message は「表示用ではない」（契約）ので、文言はここ 1 か所で決める。
const MESSAGES: Record<Schemas["ErrorCode"], string> = {
  unauthorized: "トークンがないか無効です。ログインし直してください。",
  forbidden: "権限がないためアクセスできません。",
  not_found: "対象が見つかりません（ID を確かめてください）。",
  validation_failed: "入力の内容が正しくありません。",
  slot_overlaps: "公開済みの枠と時間が重なっています。",
  invalid_booking_state: "予約が確定していないか、キャンセルされています。",
  invalid_session_state: "この通話はすでに開始済みか、終了しています。",
  invalid_video_state: "この動画の添削はすでに返却済みです。",
  review_payment_not_confirmed: "ワンショットの決済が確定していないため、添削に着手できません。",
  chat_not_entitled: "この生徒はまだ月額契約がないため、チャットを送れません。",
};

const UNKNOWN = "エラーが発生しました。時間をおいてやり直してください。";

/** API のエラー本文（契約の Error）を画面の文言にする。未知のコード（契約は値を追加しうる）は汎用の文言。 */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    const code = (error as { code: unknown }).code;
    if (typeof code === "string" && Object.hasOwn(MESSAGES, code)) {
      return MESSAGES[code as Schemas["ErrorCode"]];
    }
  }
  return UNKNOWN;
}
