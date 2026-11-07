"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { Button } from "@/components/ui/Button";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime } from "@/components/labels";

import { videoLessonMenuLabel } from "./videoLessonMenu";
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

/** 生徒の欄。動画を送った時点のレッスンメニューがあれば、名前の下に「メニュー: 〇〇」を出す。 */
function StudentCell({ video }: { video: CoachVideo }) {
  const menu = videoLessonMenuLabel(video.lesson_menu);
  return (
    <td className="py-1">
      {studentName(video)}
      {menu && <span className="block text-text-muted">{menu}</span>}
    </td>
  );
}

/**
 * 添削の返却。開いたときに自分に届いた動画の一覧（listMyCoachVideos）を出し、添削待ちの行を選ぶと
 * 添削コメントの欄（VideoReviewCommentForm。AI の一次フィードバックも欄の上に出す）を開いて返却（returnVideoReview）する。
 * 添削待ち・返却済みの行には、動画を送った時点のレッスンメニューがあれば「メニュー: 〇〇」を出す。
 * 返却したら一覧を取り直す。`initialVideoId`（対応待ちから移ったときの動画）が添削待ちにあれば、その欄を開いた状態で出す。
 * 「返却待ちだけ」は API の `pending_only=true` に渡す。開いたときは条件なしで読み、切り替えたときと返却後の取り直しは選んだ条件で読む。
 */
export function ReturnVideoReviewForm({
  api,
  initialVideoId,
}: {
  api: Pick<CoachApi, "listMyCoachVideos" | "returnVideoReview">;
  initialVideoId?: string;
}) {
  const [pendingOnly, setPendingOnly] = useState(false);
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [selected, setSelected] = useState<CoachVideo | null>(null);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 切り替えを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchVideos = useCallback(
    async (onlyPending: boolean): Promise<ListState> => {
      const response = await api.listMyCoachVideos(onlyPending ? { pending_only: true } : {});
      if (!response.ok) return { kind: "failed", message: response.message };
      return { kind: "loaded", ...splitVideos(response.data.videos) };
    },
    [api],
  );

  // 開いたときの読み込み（条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchVideos(false).then((next) => {
      if (!active || request !== latestRequest.current) return;
      setList(next);
      if (next.kind === "loaded") {
        setSelected(next.waiting.find((video) => video.video_id === initialVideoId) ?? null);
      }
    });
    return () => {
      active = false;
    };
  }, [fetchVideos, initialVideoId]);

  async function reload(onlyPending: boolean) {
    const request = ++latestRequest.current;
    const loaded = await fetchVideos(onlyPending);
    if (request === latestRequest.current) setList(loaded);
  }

  async function changePendingOnly(next: boolean) {
    setPendingOnly(next);
    setResult(null);
    setSelected(null);
    setList({ kind: "loading" });
    await reload(next);
  }

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
    await reload(pendingOnly);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒から届いた動画です。添削待ちの動画の「添削する」を押すと、コメントを書いて返却できます。
      </p>
      <label className="flex items-center gap-1 text-sm">
        <input
          type="checkbox"
          checked={pendingOnly}
          onChange={(event) => void changePendingOnly(event.target.checked)}
        />
        返却待ちだけ
      </label>
      {list.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {list.kind === "failed" && <ResultMessage result={{ ok: false, message: list.message }} />}
      {list.kind === "loaded" && list.waiting.length === 0 && list.returned.length === 0 && (
        <p className="text-sm">
          {pendingOnly ? "返却待ちの動画はありません。" : "まだ動画は届いていません。"}
        </p>
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
                      <StudentCell video={video} />
                      <td className="py-1">{formatDateTime(video.submitted_at)}</td>
                      <td className="py-1">
                        {selected?.video_id === video.video_id ? (
                          <span className="text-text-muted">選択中</span>
                        ) : (
                          <Button variant="secondary" onClick={() => onSelect(video)}>
                            添削する
                          </Button>
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
                      <StudentCell video={video} />
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
