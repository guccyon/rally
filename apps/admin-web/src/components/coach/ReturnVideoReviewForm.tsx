"use client";

import { type FormEvent, useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

// 契約の ReturnVideoReviewRequest.comment の maxLength。
const COMMENT_MAX_LENGTH = 4000;

type CoachVideo = Schemas["CoachVideo"];

type ListState =
  | { kind: "loading" }
  | { kind: "loaded"; waiting: CoachVideo[]; returned: CoachVideo[] }
  | { kind: "failed"; message: string };

/**
 * API の並び（添削待ちを送られた時刻の古い順、その後に返却済みを返却した時刻の新しい順）を、
 * 添削待ちと返却済みに分ける。どちらも API の順のまま。
 */
function splitVideos(videos: CoachVideo[]) {
  const waiting = videos.filter((video) => video.review_status === "in_progress");
  const returned = videos.filter((video) => video.review_status === "returned");
  return { waiting, returned };
}

/** AI の一次フィードバックの文。ない（null・載っていない・空白だけ）ときは null。 */
function aiFeedbackComment(video: CoachVideo) {
  const comment = video.ai_feedback?.comment;
  return comment && comment.trim() !== "" ? comment : null;
}

function studentName(video: CoachVideo) {
  return video.student_display_name ?? "（名前を表示できません）";
}

/**
 * 添削の返却。開いたときに自分に届いた動画の一覧（listMyCoachVideos）を出し、添削待ちの行を選ぶと
 * 添削コメントの欄を開いて返却（returnVideoReview）する。返却したら一覧を取り直す。
 * 欄の上には AI の一次フィードバック（glossary の ai_feedback）を参考として出す。コーチの添削とは別のもので、
 * 返却するコメントには入れない。ないときは何も出さない。
 */
export function ReturnVideoReviewForm({
  api,
}: {
  api: Pick<CoachApi, "listMyCoachVideos" | "returnVideoReview">;
}) {
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [selected, setSelected] = useState<CoachVideo | null>(null);
  const [comment, setComment] = useState("");
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const fetchVideos = useCallback(async (): Promise<ListState> => {
    const response = await api.listMyCoachVideos();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", ...splitVideos(response.data.videos) };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchVideos().then((next) => {
      if (active) setList(next);
    });
    return () => {
      active = false;
    };
  }, [fetchVideos]);

  function onSelect(video: CoachVideo) {
    setSelected(video);
    setComment("");
    setResult(null);
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (!selected) return;
    if (comment.trim() === "") {
      setResult({ ok: false, message: "添削コメントを入力してください。" });
      return;
    }
    setBusy(true);
    const response = await api.returnVideoReview(selected.video_id, { comment });
    if (response.ok) {
      const returnedAt = response.data.returned_at;
      setResult({
        ok: true,
        message: returnedAt
          ? `${studentName(selected)}に添削を返却しました（${formatDateTime(returnedAt)}）。`
          : `${studentName(selected)}に添削を返却しました。`,
      });
      setSelected(null);
      setComment("");
      setList(await fetchVideos());
    } else {
      setResult(response);
    }
    setBusy(false);
  }

  const selectedAiFeedback = selected ? aiFeedbackComment(selected) : null;

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒から届いた動画です。添削待ちの動画の「添削する」を押すと、コメントを書いて返却できます。
      </p>
      {list.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {list.kind === "failed" && <ResultMessage result={{ ok: false, message: list.message }} />}
      {list.kind === "loaded" && list.waiting.length === 0 && list.returned.length === 0 && (
        <p className="text-sm">まだ動画は届いていません。</p>
      )}
      {list.kind === "loaded" && (list.waiting.length > 0 || list.returned.length > 0) && (
        <>
          <div className="space-y-2">
            <h3 className="font-semibold">添削待ち</h3>
            {list.waiting.length === 0 ? (
              <p className="text-sm">添削待ちの動画はありません。</p>
            ) : (
              <table aria-label="添削待ち" className="w-full text-left text-sm">
                <thead>
                  <tr>
                    <th className="py-1">生徒</th>
                    <th className="py-1">送られた時刻</th>
                    <th className="py-1">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {list.waiting.map((video) => (
                    <tr key={video.video_id} className="border-t">
                      <td className="py-1">{studentName(video)}</td>
                      <td className="py-1">{formatDateTime(video.submitted_at)}</td>
                      <td className="py-1">
                        {selected?.video_id === video.video_id ? (
                          <span className="text-gray-500">選択中</span>
                        ) : (
                          <button
                            type="button"
                            disabled={busy}
                            onClick={() => onSelect(video)}
                            className="rounded bg-blue-600 px-3 py-1 text-white disabled:opacity-50"
                          >
                            添削する
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
          {selected && (
            <form
              onSubmit={onSubmit}
              aria-label="添削を書く"
              className="space-y-3 rounded border p-3"
            >
              <p className="text-sm">
                {studentName(selected)}の動画（{formatDateTime(selected.submitted_at)} に届いたもの）
              </p>
              {selectedAiFeedback && (
                <section
                  aria-label="AI の一次フィードバック（参考）"
                  className="rounded border border-dashed bg-gray-50 p-2 text-sm"
                >
                  <h4 className="font-semibold">AI の一次フィードバック（参考）</h4>
                  <p className="text-xs text-gray-500">
                    AI が自動で作った文です。コーチの添削ではなく、下の添削コメントには入りません。
                  </p>
                  <p className="mt-1 whitespace-pre-wrap">{selectedAiFeedback}</p>
                </section>
              )}
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
              <div className="flex gap-2">
                <button
                  type="submit"
                  disabled={busy}
                  className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50"
                >
                  添削を返却
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => setSelected(null)}
                  className="rounded border px-4 py-1 disabled:opacity-50"
                >
                  やめる
                </button>
              </div>
            </form>
          )}
          {list.returned.length > 0 && (
            <details>
              <summary className="cursor-pointer font-semibold">
                返却済み（{list.returned.length} 件）
              </summary>
              <table aria-label="返却済み" className="w-full text-left text-sm">
                <thead>
                  <tr>
                    <th className="py-1">生徒</th>
                    <th className="py-1">送られた時刻</th>
                    <th className="py-1">返却した時刻</th>
                  </tr>
                </thead>
                <tbody>
                  {list.returned.map((video) => (
                    <tr key={video.video_id} className="border-t">
                      <td className="py-1">{studentName(video)}</td>
                      <td className="py-1">{formatDateTime(video.submitted_at)}</td>
                      <td className="py-1">
                        {video.returned_at ? formatDateTime(video.returned_at) : ""}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </details>
          )}
        </>
      )}
      <ResultMessage result={result} />
    </div>
  );
}
