"use client";

import { useRouter } from "next/navigation";
import { type FormEvent, useState } from "react";

import { call, type Result } from "@/api/call";
import { createApiClient, type Schemas } from "@/api/client";
import { Button } from "@/components/ui/Button";
import { ROLE_LABELS, ROLES, type Role, saveSession } from "@/session/session";

const HOME: Record<Role, string> = { coach: "/coach", headquarters: "/headquarters" };

export type GetAccount = (token: string) => Promise<Result<Schemas["Account"]>>;

/** トークンが示す本人のアカウント（契約の getMyAccount）。トークンの中身は読まず、役割は API に聞く。 */
const getAccountByToken: GetAccount = (token) => call(() => createApiClient(token).GET("/me"));

const INVALID_TOKEN = "トークンが無効です。アカウント登録で受け取ったアクセストークンを確かめてください。";
const STUDENT_TOKEN = "このトークンは生徒のものです。生徒のトークンでは管理画面に入れません。";

function isRole(value: Schemas["AccountRole"]): value is Role {
  return (ROLES as readonly string[]).includes(value);
}

/** 選んだロールとトークンの役割が合わないときの理由。合えば null。 */
function mismatchMessage(selected: Role, actual: Schemas["AccountRole"]): string | null {
  if (!isRole(actual)) return STUDENT_TOKEN;
  if (actual === selected) return null;
  return `このトークンは${ROLE_LABELS[actual]}のものです（${ROLE_LABELS[selected]}のものではありません）。ロール「${ROLE_LABELS[actual]}」を選んでログインしてください。`;
}

/**
 * モックのトークンを入力して保持する（Gen1 は本物の認証なし）。ロールで出す画面を決める。
 * 保持する前に getMyAccount でトークンの役割を確かめ、選んだロールと違えば理由を出して入れない。
 */
export function LoginForm({ getAccount = getAccountByToken }: { getAccount?: GetAccount }) {
  const router = useRouter();
  const [role, setRole] = useState<Role>("coach");
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    const trimmed = token.trim();
    if (trimmed === "") {
      setError("トークンを入力してください。");
      return;
    }
    setError(null);
    setBusy(true);
    const account = await getAccount(trimmed);
    if (!account.ok) {
      setError(
        account.code === "unauthorized" ? INVALID_TOKEN : `ログインできませんでした。${account.message}`,
      );
      setBusy(false);
      return;
    }
    const mismatch = mismatchMessage(role, account.data.role);
    if (mismatch !== null) {
      setError(mismatch);
      setBusy(false);
      return;
    }
    saveSession({ role, token: trimmed });
    router.push(HOME[role]);
  }

  return (
    <form onSubmit={onSubmit} className="space-y-4">
      <fieldset className="space-y-1">
        <legend className="font-medium">ロール</legend>
        {ROLES.map((value) => (
          <label key={value} className="mr-4 inline-flex items-center gap-1">
            <input
              type="radio"
              name="role"
              value={value}
              checked={role === value}
              onChange={() => setRole(value)}
            />
            {ROLE_LABELS[value]}
          </label>
        ))}
      </fieldset>
      <label className="block">
        <span className="font-medium">アクセストークン</span>
        <input
          className="mt-1 block w-full rounded-md border px-2 py-1 font-mono"
          value={token}
          onChange={(event) => setToken(event.target.value)}
          placeholder="POST /accounts の access_token"
        />
      </label>
      {error && (
        <p role="alert" className="text-danger">
          {error}
        </p>
      )}
      <Button type="submit" loading={busy}>
        ログイン
      </Button>
    </form>
  );
}
