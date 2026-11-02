"use client";

import { type FormEvent, useEffect, useState } from "react";

import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";

// 契約の UpdateCoachProfileRequest.bio の maxLength（Unicode のコードポイントで数える）。
const BIO_MAX_LENGTH = 400;

const NOT_COACH = "コーチのアカウントでログインしたときだけ、自己紹介を書けます。";

type State =
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "failed"; message: string };

/** 契約と同じくコードポイントで数える（textarea の maxLength は UTF-16 の単位で数えるので使わない）。 */
function lengthOf(text: string): number {
  return Array.from(text).length;
}

/**
 * コーチの「自己紹介」。開いたときにログイン中の本人のアカウント（getMyAccount）の bio を出し、
 * 編集して updateMyCoachProfile で丸ごと置き換える。空のまま保存すると消える。
 */
export function CoachProfileForm({
  api,
}: {
  api: Pick<CoachApi, "getMyAccount" | "updateMyCoachProfile">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [bio, setBio] = useState("");
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void api.getMyAccount().then((account) => {
      if (!active) return;
      if (!account.ok) {
        setState({ kind: "failed", message: account.message });
      } else if (account.data.role !== "coach") {
        setState({ kind: "failed", message: NOT_COACH });
      } else {
        setBio(account.data.bio ?? "");
        setState({ kind: "ready" });
      }
    });
    return () => {
      active = false;
    };
  }, [api]);

  const remaining = BIO_MAX_LENGTH - lengthOf(bio);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (remaining < 0) {
      setResult({
        ok: false,
        message: `自己紹介は ${BIO_MAX_LENGTH} 文字までです。${-remaining} 文字減らしてください。`,
      });
      return;
    }
    setBusy(true);
    const response = await api.updateMyCoachProfile({ bio });
    setBusy(false);
    if (response.ok) {
      setBio(response.data.bio ?? bio);
      setResult({ ok: true, message: "自己紹介を保存しました。" });
    } else {
      setResult(response);
    }
  }

  if (state.kind === "loading") return <p className="text-sm text-text-muted">読み込み中…</p>;
  if (state.kind === "failed") return <ResultMessage result={{ ok: false, message: state.message }} />;

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒に向けたあなたの紹介文です（{BIO_MAX_LENGTH} 文字まで）。空にして保存すると消えます。
      </p>
      <form onSubmit={onSubmit} className="space-y-3">
        <label className="block">
          自己紹介文
          <textarea
            className="mt-1 block w-full rounded-md border px-2 py-1"
            rows={6}
            value={bio}
            onChange={(event) => setBio(event.target.value)}
          />
        </label>
        <p className={remaining < 0 ? "text-sm text-danger" : "text-sm text-text-muted"}>
          {remaining < 0
            ? `${BIO_MAX_LENGTH} 文字を ${-remaining} 文字超えています`
            : `あと ${remaining} 文字`}
        </p>
        <button
          type="submit"
          disabled={busy}
          className="rounded-pill bg-primary px-4 py-1 text-on-primary disabled:opacity-50"
        >
          保存する
        </button>
      </form>
      <ResultMessage result={result} />
    </div>
  );
}
