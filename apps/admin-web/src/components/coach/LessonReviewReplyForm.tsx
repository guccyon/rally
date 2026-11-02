"use client";

import { type FormEvent, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

// 契約の ReplyToLessonReviewRequest.comment の maxLength（Unicode のコードポイントで数える）。
const REPLY_MAX_LENGTH = 200;

/** 契約と同じくコードポイントで数える（textarea の maxLength は UTF-16 の単位で数えるので使わない）。 */
function lengthOf(text: string): number {
  return Array.from(text).length;
}

/**
 * 評価 1 件への返信（glossary の lesson_review_reply）の欄。コーチの「自分の評価」の各行の下に出す。
 * 保存済みの返信（評価の `reply`）があれば文と返信した時刻を出して「返信を書き直す」、なければ「返信する」で欄を開く。
 * 書き直すときは今の返信を欄に入れておき、保存（replyToLessonReview）すると丸ごと置き換わる（削除はない）。
 * 空白だけ・200 文字を超える返信は送らずに理由を出す。保存できなければ文言を出して欄を残す。
 */
export function LessonReviewReplyForm({
  api,
  review,
  onSaved,
}: {
  api: Pick<CoachApi, "replyToLessonReview">;
  review: Schemas["CoachReview"];
  onSaved: (reply: Schemas["LessonReviewReply"]) => void;
}) {
  const reply = review.reply ?? null;
  const [editing, setEditing] = useState(false);
  const [comment, setComment] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);

  function open() {
    setComment(reply?.comment ?? "");
    setError(null);
    setSaved(false);
    setEditing(true);
  }

  const remaining = REPLY_MAX_LENGTH - lengthOf(comment);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (comment.trim() === "") {
      setError("返信を入力してください。");
      return;
    }
    if (remaining < 0) {
      setError(`返信は ${REPLY_MAX_LENGTH} 文字までです。${-remaining} 文字減らしてください。`);
      return;
    }
    setBusy(true);
    setError(null);
    const response = await api.replyToLessonReview(review.review_id, { comment });
    setBusy(false);
    if (response.ok) {
      onSaved(response.data);
      setEditing(false);
      setSaved(true);
    } else {
      setError(response.message);
    }
  }

  return (
    <div className="space-y-2 pt-1">
      {reply && !editing && (
        <section aria-label="あなたの返信" className="rounded-md bg-surface p-2">
          <h4 className="font-semibold">あなたの返信</h4>
          <p className="whitespace-pre-wrap">{reply.comment}</p>
          <p className="text-text-muted">{formatDateTime(reply.replied_at)}</p>
        </section>
      )}
      {editing ? (
        <form onSubmit={onSubmit} aria-label="評価に返信する" className="space-y-2 rounded-md border p-2">
          <label className="block">
            返信
            <textarea
              className="mt-1 block w-full rounded-md border px-2 py-1"
              rows={3}
              value={comment}
              onChange={(event) => setComment(event.target.value)}
            />
          </label>
          <p className={remaining < 0 ? "text-danger" : "text-text-muted"}>
            {remaining < 0
              ? `${REPLY_MAX_LENGTH} 文字を ${-remaining} 文字超えています`
              : `あと ${remaining} 文字`}
          </p>
          <div className="flex gap-2">
            <button
              type="submit"
              disabled={busy}
              className="rounded-pill bg-primary px-4 py-1 text-on-primary disabled:opacity-50"
            >
              保存する
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setEditing(false)}
              className="rounded-pill border px-4 py-1 disabled:opacity-50"
            >
              やめる
            </button>
          </div>
          <ResultMessage result={error ? { ok: false, message: error } : null} />
        </form>
      ) : (
        <button type="button" onClick={open} className="rounded-pill border px-3 py-0.5">
          {reply ? "返信を書き直す" : "返信する"}
        </button>
      )}
      <ResultMessage result={saved ? { ok: true, message: "返信を保存しました。" } : null} />
    </div>
  );
}
