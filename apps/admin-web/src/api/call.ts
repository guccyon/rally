import type { Schemas } from "./client";
import { errorCode, errorMessage } from "./errorMessage";

/**
 * 画面が扱う結果。失敗は表示用の文言にしてから返す。失敗の種類で振る舞いを変えるときは、文言ではなく `code`
 * （契約の ErrorCode。接続できなかったときや未知のコードのときはない）で見分ける。
 */
export type Result<T> =
  | { ok: true; data: T }
  | { ok: false; message: string; code?: Schemas["ErrorCode"] };

const NETWORK_ERROR = "サーバーに接続できません。";

/** openapi-fetch の呼び出しを `Result` にする（ロールの API で共通）。 */
export async function call<T>(
  request: () => Promise<{ data?: T; error?: unknown }>,
): Promise<Result<T>> {
  try {
    const { data, error } = await request();
    if (data !== undefined) return { ok: true, data };
    const code = errorCode(error);
    return { ok: false, message: errorMessage(error), ...(code === undefined ? {} : { code }) };
  } catch {
    return { ok: false, message: NETWORK_ERROR };
  }
}
