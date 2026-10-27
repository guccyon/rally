import { call } from "./call";
import type { ApiClient } from "./client";

/** 本部の操作（契約の operationId と 1 対 1）。 */
export function headquartersApi(client: ApiClient) {
  return {
    getSlotSupply: () => call(() => client.GET("/slot-supply")),
  };
}

export type HeadquartersApi = ReturnType<typeof headquartersApi>;
