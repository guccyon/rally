// ログインの状態（モックのトークンとロール）を localStorage に保持する。Gen1 は本物の認証がない。
// トークンは不透明なもの（契約の bearerAuth）として扱い、中身は解釈しない。ロールは利用者がログイン時に選ぶ。

export const ROLES = ["coach", "headquarters"] as const;
export type Role = (typeof ROLES)[number];

export const ROLE_LABELS: Record<Role, string> = {
  coach: "コーチ",
  headquarters: "本部",
};

export type Session = { role: Role; token: string };

const STORAGE_KEY = "rally.admin-web.session";

const listeners = new Set<() => void>();
let cachedRaw: string | null = null;
let cached: Session | null = null;

function parse(raw: string | null): Session | null {
  if (raw === null) return null;
  try {
    const value: unknown = JSON.parse(raw);
    if (typeof value !== "object" || value === null) return null;
    const { role, token } = value as Record<string, unknown>;
    if (typeof token !== "string" || token === "") return null;
    if (!ROLES.includes(role as Role)) return null;
    return { role: role as Role, token };
  } catch {
    return null;
  }
}

/** 同じ保存内容には同じオブジェクトを返す（useSyncExternalStore のスナップショット）。 */
export function readSession(): Session | null {
  const raw = window.localStorage.getItem(STORAGE_KEY);
  if (raw !== cachedRaw) {
    cachedRaw = raw;
    cached = parse(raw);
  }
  return cached;
}

export function saveSession(session: Session): void {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
  listeners.forEach((listener) => listener());
}

export function clearSession(): void {
  window.localStorage.removeItem(STORAGE_KEY);
  listeners.forEach((listener) => listener());
}

export function subscribeSession(listener: () => void): () => void {
  listeners.add(listener);
  window.addEventListener("storage", listener);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", listener);
  };
}
