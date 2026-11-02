"use client";

import { type FormEvent, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

// 契約の ReturnVideoReviewRequest.comment の maxLength。
const COMMENT_MAX_LENGTH = 4000;

type CoachVideo = Schemas["CoachVideo"];

/** AI の一次フィードバックの文。ない（null・載っていない・空白だけ）ときは null。 */
function aiFeedbackComment(video: CoachVideo) {
  const comment = video.ai_feedback?.comment;
  return comment && comment.trim() !== "" ? comment : null;
}

/**
 * 1 本の動画の添削を書いて返却（returnVideoReview）する欄。「動画の添削」の一覧とチャットの添削依頼のカードで共通。
 * 欄の上には AI の一次フィードバック（glossary の ai_feedback）を参考として出す。コーチの添削とは別のもので、
 * 返却するコメントには入れない。ないときは何も出さない。返却できなければ文言を出して欄を残す。
 */
export function VideoReviewCommentForm({
  api,
  video,
  studentName,
  onReturned,
  onCancel,
}: {
  api: Pick<CoachApi, "returnVideoReview">;
  video: CoachVideo;
  studentName: string;
  onReturned: (review: Schemas["VideoReview"]) => void | Promise<void>;
  onCancel: () => void;
}) {
  const [comment, setComment] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (comment.trim() === "") {
      setError("添削コメントを入力してください。");
      return;
    }
    setBusy(true);
    setError(null);
    const response = await api.returnVideoReview(video.video_id, { comment });
    if (response.ok) {
      await onReturned(response.data);
    } else {
      setError(response.message);
      setBusy(false);
    }
  }

  const aiFeedback = aiFeedbackComment(video);

  return (
    <form onSubmit={onSubmit} aria-label="添削を書く" className="space-y-3 rounded-md border p-3">
      <p className="text-sm">
        {studentName}の動画（{formatDateTime(video.submitted_at)} に届いたもの）
      </p>
      {aiFeedback && (
        <section
          aria-label="AI の一次フィードバック（参考）"
          className="rounded-md border border-dashed bg-surface p-2 text-sm"
        >
          <h4 className="font-semibold">AI の一次フィードバック（参考）</h4>
          <p className="text-xs text-text-muted">
            AI が自動で作った文です。コーチの添削ではなく、下の添削コメントには入りません。
          </p>
          <p className="mt-1 whitespace-pre-wrap">{aiFeedback}</p>
        </section>
      )}
      <label className="block">
        添削コメント
        <textarea
          className="mt-1 block w-full rounded-md border px-2 py-1"
          rows={4}
          maxLength={COMMENT_MAX_LENGTH}
          value={comment}
          onChange={(event) => setComment(event.target.value)}
        />
      </label>
      <div className="flex gap-2">
        <button
          type="submit"
          disabled={busy}
          className="rounded-pill bg-primary px-4 py-1 text-on-primary disabled:opacity-50"
        >
          添削を返却
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={onCancel}
          className="rounded-pill border px-4 py-1 disabled:opacity-50"
        >
          やめる
        </button>
      </div>
      <ResultMessage result={error ? { ok: false, message: error } : null} />
    </form>
  );
}
