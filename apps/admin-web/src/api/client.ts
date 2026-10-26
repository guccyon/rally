import createClient from "openapi-fetch";

import type { components, paths } from "./schema";

/** admin-web の API クライアント。型は契約（specs/contracts/api/admin-web/openapi.yaml）から生成した `schema.ts` だけを使う。 */
export type ApiClient = ReturnType<typeof createClient<paths>>;

export type Schemas = components["schemas"];

/** ブラウザからは同じオリジンの `/api` を呼ぶ（next.config.ts の rewrites がバックエンドへ中継する）。 */
export const API_BASE_URL = "/api";

export function createApiClient(
  token: string,
  options: { baseUrl?: string; fetch?: typeof fetch } = {},
): ApiClient {
  return createClient<paths>({
    baseUrl: options.baseUrl ?? API_BASE_URL,
    headers: { Authorization: `Bearer ${token}` },
    ...(options.fetch ? { fetch: options.fetch } : {}),
  });
}
