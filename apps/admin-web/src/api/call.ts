import { errorMessage } from "./errorMessage";

/** 画面が扱う結果。失敗は表示用の文言にしてから返す。 */
export type Result<T> = { ok: true; data: T } | { ok: false; message: string };

const NETWORK_ERROR = "サーバーに接続できません。";

/** openapi-fetch の呼び出しを `Result` にする（ロールの API で共通）。 */
export async function call<T>(
  request: () => Promise<{ data?: T; error?: unknown }>,
): Promise<Result<T>> {
  try {
    const { data, error } = await request();
    if (data !== undefined) return { ok: true, data };
    return { ok: false, message: errorMessage(error) };
  } catch {
    return { ok: false, message: NETWORK_ERROR };
  }
}
