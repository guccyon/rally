"use client";

import { type FormEvent, useState } from "react";

import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { NotImplemented } from "@/components/Section";
import { formatDateTime } from "@/components/labels";

// 契約の ReturnVideoReviewRequest.comment の maxLength。
const COMMENT_MAX_LENGTH = 4000;

/** 添削の返却（returnVideoReview）。届いた動画の一覧の API はまだないので、動画 ID を入力する。 */
export function ReturnVideoReviewForm({ api }: { api: Pick<CoachApi, "returnVideoReview"> }) {
  const [videoId, setVideoId] = useState("");
  const [comment, setComment] = useState("");
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (videoId.trim() === "") {
      setResult({ ok: false, message: "動画 ID を入力してください。" });
      return;
    }
    if (comment.trim() === "") {
      setResult({ ok: false, message: "添削コメントを入力してください。" });
      return;
    }
    setBusy(true);
    const response = await api.returnVideoReview(videoId.trim(), { comment });
    setBusy(false);
    if (response.ok) {
      const returnedAt = response.data.returned_at;
      setResult({
        ok: true,
        message: returnedAt
          ? `添削を返却しました（${formatDateTime(returnedAt)}）。`
          : "添削を返却しました。",
      });
      setComment("");
    } else {
      setResult(response);
    }
  }

  return (
    <div className="space-y-4">
      <NotImplemented>
        届いた動画の一覧を取る API が admin-web の契約にないため、動画 ID を入力して返却します。
      </NotImplemented>
      <form onSubmit={onSubmit} className="space-y-3">
        <label className="block">
          動画 ID
          <input
            className="mt-1 block w-full rounded border px-2 py-1 font-mono"
            value={videoId}
            onChange={(event) => setVideoId(event.target.value)}
          />
        </label>
        <label className="block">
          添削コメント
          <textarea
            className="mt-1 block w-full rounded border px-2 py-1"
            rows={4}
            maxLength={COMMENT_MAX_LENGTH}
            value={comment}
            onChange={(event) => setComment(event.target.value)}
          />
        </label>
        <button
          type="submit"
          disabled={busy}
          className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50"
        >
          添削を返却
        </button>
      </form>
      <ResultMessage result={result} />
    </div>
  );
}
