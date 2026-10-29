"use client";

import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

import { VideoReviewCommentForm } from "./VideoReviewCommentForm";

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

function studentName(video: CoachVideo) {
  return video.student_display_name ?? "（名前を表示できません）";
}

/**
 * 添削の返却。開いたときに自分に届いた動画の一覧（listMyCoachVideos）を出し、添削待ちの行を選ぶと
 * 添削コメントの欄（VideoReviewCommentForm。AI の一次フィードバックも欄の上に出す）を開いて返却（returnVideoReview）する。
 * 返却したら一覧を取り直す。
 */
export function ReturnVideoReviewForm({
  api,
}: {
  api: Pick<CoachApi, "listMyCoachVideos" | "returnVideoReview">;
}) {
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [selected, setSelected] = useState<CoachVideo | null>(null);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);

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
    setResult(null);
  }

  async function onReturned(review: Schemas["VideoReview"]) {
    if (!selected) return;
    const returnedAt = review.returned_at;
    setResult({
      ok: true,
      message: returnedAt
        ? `${studentName(selected)}に添削を返却しました（${formatDateTime(returnedAt)}）。`
        : `${studentName(selected)}に添削を返却しました。`,
    });
    setSelected(null);
    setList(await fetchVideos());
  }

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
            <VideoReviewCommentForm
              key={selected.video_id}
              api={api}
              video={selected}
              studentName={studentName(selected)}
              onReturned={onReturned}
              onCancel={() => setSelected(null)}
            />
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
