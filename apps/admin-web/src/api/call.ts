import type { Schemas } from "./client";
import { errorCode, errorMessage } from "./errorMessage";
import { clockOffsetMs } from "./serverClock";

/**
 * 画面が扱う結果。失敗は表示用の文言にしてから返す。失敗の種類で振る舞いを変えるときは、文言ではなく `code`
 * （契約の ErrorCode。接続できなかったときや未知のコードのときはない）で見分ける。
 * `clockOffsetMs` は応答の Date ヘッダーから出したサーバーの時計と端末の時計のずれ（serverClock.ts）。Date がない・読めないときはない。
 */
export type Result<T> =
  | { ok: true; data: T; clockOffsetMs?: number }
  | { ok: false; message: string; code?: Schemas["ErrorCode"]; clockOffsetMs?: number };

const NETWORK_ERROR = "サーバーに接続できません。";

/** openapi-fetch の呼び出しを `Result` にする（ロールの API で共通）。 */
export async function call<T>(
  request: () => Promise<{ data?: T; error?: unknown; response?: Response }>,
): Promise<Result<T>> {
  try {
    const sentAt = Date.now();
    const { data, error, response } = await request();
    const offset = clockOffsetMs(response?.headers.get("Date"), sentAt, Date.now());
    const clock = offset === undefined ? {} : { clockOffsetMs: offset };
    if (data !== undefined) return { ok: true, data, ...clock };
    const code = errorCode(error);
    return { ok: false, message: errorMessage(error), ...(code === undefined ? {} : { code }), ...clock };
  } catch {
    return { ok: false, message: NETWORK_ERROR };
  }
}
