// ロールから入口（ログインの画面）とホームへの対応。LoginForm・RoleGate・入口の画面が同じものを使う。
// ADR 0011 の /coach/login（#1418）ができたら、コーチの入口をここで差し替える。

import type { Role } from "./session";

/** ロールごとのログインの入口。ログインしていないとき・ログアウトしたときの戻り先。 */
export const ROLE_ENTRY_PATHS: Record<Role, string> = {
  coach: "/",
  headquarters: "/",
};

/** ロールごとのホーム。ログインのあとと、ログインしたまま入口を開いたときの行き先。 */
export const ROLE_HOME_PATHS: Record<Role, string> = {
  coach: "/coach",
  headquarters: "/headquarters",
};
