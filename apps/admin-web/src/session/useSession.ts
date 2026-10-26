"use client";

import { useSyncExternalStore } from "react";

import { readSession, type Session, subscribeSession } from "./session";

/** ログインの状態。サーバー描画と読み込み前は `undefined`、未ログインは `null`。 */
export function useSession(): Session | null | undefined {
  return useSyncExternalStore(subscribeSession, readSession, () => undefined);
}
