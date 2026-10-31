"use client";

import { useState } from "react";

import type { Schemas } from "@/api/client";
import { lengthOf } from "@/components/coach/lessonMenuInput";

/** 本部が非公開にするときの理由の上限（契約の UnpublishLessonMenuRequest.reason。コードポイントで数える）。 */
export const UNPUBLISH_REASON_MAX_LENGTH = 200;

type Props = {
  menu: Schemas["CoachLessonMenu"];
  onUnpublish: (reason: string) => Promise<void>;
  onRepublish: () => Promise<void>;
};

/**
 * 本部のレッスンメニューの行の「非公開にする」/「公開に戻す」。押したら行の中で確かめ（ブラウザの confirm は使わない）、
 * 確定で送る。本部が非公開にした行（`headquarters_unpublish` がある）は「公開に戻す」、それ以外（公開中・コーチが自分で
 * 非公開にしたもの）は「非公開にする」。送る処理と結果の表示は呼び出し側が持つ。
 */
export function LessonMenuPublicationAction({ menu, onUnpublish, onRepublish }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const unpublishedByHeadquarters = menu.headquarters_unpublish !== undefined;
  const remaining = UNPUBLISH_REASON_MAX_LENGTH - lengthOf(reason);

  async function confirm() {
    setBusy(true);
    await (unpublishedByHeadquarters ? onRepublish() : onUnpublish(reason));
    setBusy(false);
    setConfirming(false);
    setReason("");
  }

  function cancel() {
    setConfirming(false);
    setReason("");
  }

  if (!confirming) {
    return (
      <button
        type="button"
        aria-label={
          unpublishedByHeadquarters
            ? `「${menu.name}」を公開に戻す`
            : `「${menu.name}」を非公開にする`
        }
        className={
          unpublishedByHeadquarters
            ? "rounded border px-2 py-0.5"
            : "rounded border border-red-600 px-2 py-0.5 text-red-700"
        }
        onClick={() => setConfirming(true)}
      >
        {unpublishedByHeadquarters ? "公開に戻す" : "非公開にする"}
      </button>
    );
  }

  if (unpublishedByHeadquarters) {
    return (
      <div role="group" aria-label={`「${menu.name}」の公開に戻す確認`} className="space-y-1">
        <p className="text-xs">
          このメニューを公開に戻しますか？生徒の一覧にまた出て、コーチが自分で公開・非公開を決められるようになります。
        </p>
        <div className="flex gap-2">
          <button
            type="button"
            disabled={busy}
            className="rounded bg-blue-600 px-2 py-0.5 text-white disabled:opacity-50"
            onClick={() => void confirm()}
          >
            {busy ? "公開に戻しています…" : "公開に戻すのを確定"}
          </button>
          <button
            type="button"
            disabled={busy}
            className="rounded border px-2 py-0.5 disabled:opacity-50"
            onClick={cancel}
          >
            やめる
          </button>
        </div>
      </div>
    );
  }

  return (
    <div role="group" aria-label={`「${menu.name}」の非公開の確認`} className="space-y-1">
      <p className="text-xs">
        このメニューを非公開にしますか？生徒の一覧に出なくなり、本部が公開に戻すまでコーチは公開し直せません。
      </p>
      <label className="block text-xs">
        理由（任意）
        <textarea
          className="mt-1 block w-full rounded border px-2 py-1"
          rows={2}
          value={reason}
          disabled={busy}
          aria-invalid={remaining < 0 ? true : undefined}
          onChange={(event) => setReason(event.target.value)}
        />
      </label>
      <span className={remaining < 0 ? "text-xs text-red-600" : "text-xs text-gray-500"}>
        {remaining < 0
          ? `${UNPUBLISH_REASON_MAX_LENGTH} 文字を ${-remaining} 文字超えています`
          : `あと ${remaining} 文字`}
      </span>
      <div className="flex gap-2">
        <button
          type="button"
          disabled={busy || remaining < 0}
          className="rounded bg-red-600 px-2 py-0.5 text-white disabled:opacity-50"
          onClick={() => void confirm()}
        >
          {busy ? "非公開にしています…" : "非公開を確定"}
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded border px-2 py-0.5 disabled:opacity-50"
          onClick={cancel}
        >
          やめる
        </button>
      </div>
    </div>
  );
}
