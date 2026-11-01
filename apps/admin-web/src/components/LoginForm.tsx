"use client";

import { useRouter } from "next/navigation";
import { type FormEvent, useState } from "react";

import { ROLE_LABELS, ROLES, type Role, saveSession } from "@/session/session";

const HOME: Record<Role, string> = { coach: "/coach", headquarters: "/headquarters" };

/** モックのトークンを入力して保持する（Gen1 は本物の認証なし）。ロールで出す画面を決める。 */
export function LoginForm() {
  const router = useRouter();
  const [role, setRole] = useState<Role>("coach");
  const [token, setToken] = useState("");
  const [error, setError] = useState<string | null>(null);

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmed = token.trim();
    if (trimmed === "") {
      setError("トークンを入力してください。");
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
          className="mt-1 block w-full rounded border px-2 py-1 font-mono"
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
      <button type="submit" className="rounded bg-primary px-4 py-1 text-inverse">
        ログイン
      </button>
    </form>
  );
}
