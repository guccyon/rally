"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime, formatDay } from "@/components/labels";

import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";
import { coachReviewSummaryPath } from "./CoachReviewSummaryHome";
import { LowRatedReviewHandlingAction } from "./LowRatedReviewHandlingAction";

type LowRatedReview = Schemas["LowRatedLessonReview"];

/** period が null なら期間で絞らない（契約で `from`・`to` を省いたときと同じく、直近の期間のすべて）。 */
type Filter = { unhandledOnly: boolean; period: Period | null };

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["LowRatedLessonReviewList"]; filter: Filter }
  | { kind: "failed"; message: string };

const NO_COACH_NAME = "（名前を表示できません）";

const NO_PERIOD: Period = { from: "", to: "" };

// 期間で絞っているとき（送る前の確かめ periodError をすり抜けた 422）の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED_WITH_PERIOD = `期間の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;
// 生徒で絞っているとき（生徒の ID は URL のクエリから来るので、書き換えられて UUID でないこともある）に添える。返事のないメッセージの一覧と同じ。
const VALIDATION_HINT_WITH_STUDENT = "生徒は生徒の一覧から選び直してください。";

/** 422 の理由。期間・生徒のうち、いま絞っている条件の直す場所を伝える。どちらでも絞っていなければ ErrorCode の文言のまま。 */
function validationFailedMessageOf({ period }: Filter, studentFiltered: boolean, message: string): string {
  if (period === null) return studentFiltered ? `${message}${VALIDATION_HINT_WITH_STUDENT}` : message;
  return studentFiltered ? `${VALIDATION_FAILED_WITH_PERIOD}${VALIDATION_HINT_WITH_STUDENT}` : VALIDATION_FAILED_WITH_PERIOD;
}

/** 一覧（listLowRatedLessonReviews）に付けるクエリ。「未対応だけ」を外したとき・期間なしの条件は付けない。 */
function queryOf({ unhandledOnly, period }: Filter) {
  return {
    ...(unhandledOnly ? { handled: false } : {}),
    ...(period ?? {}),
  };
}

/** 0 件のときの文言。期間・生徒で絞っていれば、その期間・その生徒のことだと言う。 */
function emptyMessageOf({ unhandledOnly, period }: Filter, studentFiltered: boolean): string {
  const reviews = unhandledOnly ? "未対応の低い評価" : "低い評価";
  const student = studentFiltered ? "この生徒の" : "";
  if (period === null) return studentFiltered || unhandledOnly ? `${student}${reviews}はありません` : "低い評価はまだありません";
  return `この期間に${student}${reviews}はありません`;
}

function HandlingStatus({ review }: { review: LowRatedReview }) {
  const handling = review.handling;
  if (handling === undefined || handling === null) return <p>対応: 未対応</p>;
  return <p>対応: 対応済み（{formatDateTime(handling.handled_at)}）</p>;
}

/**
 * 本部の「低い評価」。開いたときに低い評価の一覧（listLowRatedLessonReviews）を読み、1 件ずつ点・コーチ・
 * メニュー（あれば）・一言・送った時刻・対応の状況を出す。しきい値・期間・並べ方は契約の正本どおり API が済ませているので、
 * 返った順のまま出し、しきい値と期間は応答の値を添える。コーチの名前からそのコーチの評価の画面へ移れる。
 * 「未対応だけ」は API の `handled=false` に渡す。未対応の行は確かめてから対応済みにする（markLowRatedLessonReviewHandled）。
 * 「開始日」「終了日」で送った日付（日本時間）の期間に絞る（API の `from`・`to`）。期間を省ける「コーチの評価」の期間の欄と同じく、
 * 期間を付けずに開き（直近の期間のすべて）、送る前に確かめて「この期間で見る」で読み直し、「期間を外す」で戻す。
 * 「未対応だけ」と期間は組み合わせられる（どちらを変えても、もう一方の確定した条件は残す）。
 * `studentId` があれば、その生徒がつけた評価だけを読む（API の `student_id`。「未対応だけ」・期間と合わせて付け、どちらを変えても残す）。
 * 生徒は欄では選ばず、ほかの一覧と同じく URL のクエリ（生徒の利用の様子の画面からの入口）で受け取る。0 件と 422 の文言は生徒で絞っていることを添える。
 */
export function LowRatedReviewList({
  api,
  studentId,
}: {
  api: Pick<HeadquartersApi, "listLowRatedLessonReviews" | "markLowRatedLessonReviewHandled">;
  studentId?: string;
}) {
  // 今の一覧を読んだ条件（取り直しにも使う）。期間は「この期間で見る」を押したときだけ変わる。
  const [filter, setFilter] = useState<Filter>({ unhandledOnly: false, period: null });
  // 期間の欄の値（「この期間で見る」を押すまでは一覧に効かない）。
  const [draftPeriod, setDraftPeriod] = useState<Period>(NO_PERIOD);
  const [periodDraftError, setPeriodDraftError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 切り替えを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchReviews = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listLowRatedLessonReviews({
        ...queryOf(next),
        ...(studentId ? { student_id: studentId } : {}),
      });
      if (response.ok) return { kind: "loaded", list: response.data, filter: next };
      return {
        kind: "failed",
        message:
          response.code === "validation_failed"
            ? validationFailedMessageOf(next, Boolean(studentId), response.message)
            : response.message,
      };
    },
    [api, studentId],
  );

  // 開いたときの読み込み（条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchReviews({ unhandledOnly: false, period: null }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchReviews]);

  async function reload(next: Filter) {
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchReviews(next);
    if (request === latestRequest.current) setState(loaded);
  }

  async function changeFilter(next: Filter) {
    setFilter(next);
    setResult(null);
    await reload(next);
  }

  async function submitPeriod(event: FormEvent) {
    event.preventDefault();
    const error = periodError(draftPeriod);
    setPeriodDraftError(error);
    if (error !== null) return;
    await changeFilter({ ...filter, period: draftPeriod });
  }

  async function clearPeriod() {
    setDraftPeriod(NO_PERIOD);
    setPeriodDraftError(null);
    await changeFilter({ ...filter, period: null });
  }

  async function markHandled(review: LowRatedReview) {
    const response = await api.markLowRatedLessonReviewHandled(review.review_id);
    if (response.ok) {
      setResult({ ok: true, message: "評価を対応済みにしました。" });
      // 「未対応だけ」でも、押した行はすぐには消さずに対応済みと出す（次に読み直したときに一覧から外れる）。
      const handling = response.data;
      setState((current) =>
        current.kind === "loaded"
          ? {
              ...current,
              list: {
                ...current.list,
                reviews: current.list.reviews.map((row) =>
                  row.review_id === review.review_id ? { ...row, handling } : row,
                ),
              },
            }
          : current,
      );
      return;
    }
    // 409（低い評価でない）・404（評価がない）などは ErrorCode の文言を出し、一覧を今の状態に取り直す。
    setResult(response);
    if (response.code !== undefined) await reload(filter);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けた評価のうち、点の低いものを送った時刻の新しい順に見られます。コーチの名前から、そのコーチの評価へ移れます。期間を選ぶと、その期間（日本時間）に送られた評価だけにします（期間は{" "}
        {MAX_PERIOD_DAYS} 日まで。「未対応だけ」と組み合わせられます）。
      </p>
      <label className="flex items-center gap-1 text-sm">
        <input
          type="checkbox"
          checked={filter.unhandledOnly}
          onChange={(event) => void changeFilter({ ...filter, unhandledOnly: event.target.checked })}
        />
        未対応だけ
      </label>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submitPeriod(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={draftPeriod.from}
            onChange={(event) => setDraftPeriod({ ...draftPeriod, from: event.target.value })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={draftPeriod.to}
            onChange={(event) => setDraftPeriod({ ...draftPeriod, to: event.target.value })}
          />
        </label>
        <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
          この期間で見る
        </button>
        {filter.period !== null && (
          <button type="button" className="rounded-pill border px-4 py-1" onClick={() => void clearPeriod()}>
            期間を外す
          </button>
        )}
      </form>
      {periodDraftError !== null && <ResultMessage result={{ ok: false, message: periodDraftError }} />}
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <p className="text-sm text-text-muted">
          直近 {state.list.period_days} 日に送られた、{state.list.max_rating} 点以下の評価
          {state.filter.period === null
            ? "です。"
            : `のうち、${formatDay(state.filter.period.from)} 〜 ${formatDay(state.filter.period.to)} に送られたものです。`}
        </p>
      )}
      {state.kind === "loaded" && state.list.reviews.length === 0 && (
        <p className="text-sm">{emptyMessageOf(state.filter, Boolean(studentId))}</p>
      )}
      {state.kind === "loaded" && state.list.reviews.length > 0 && (
        <ul aria-label="低い評価の一覧" className="divide-y rounded-md border text-sm">
          {state.list.reviews.map((review) => {
            const coachName = review.coach_display_name ?? NO_COACH_NAME;
            const handled = review.handling !== undefined && review.handling !== null;
            return (
              <li key={review.review_id} className="space-y-1 px-3 py-2">
                <p>
                  <span className="font-semibold">{review.rating} 点</span>
                  <span className="text-text-muted">（5 点満点）</span>
                </p>
                <p>
                  コーチ:{" "}
                  <Link
                    href={coachReviewSummaryPath(review.coach_id)}
                    className="text-primary underline"
                  >
                    {coachName}
                  </Link>
                </p>
                {review.lesson_menu && <p>メニュー: {review.lesson_menu.name}</p>}
                <p>{review.comment}</p>
                <p className="text-text-muted">{formatDateTime(review.submitted_at)}</p>
                <HandlingStatus review={review} />
                {!handled && (
                  <LowRatedReviewHandlingAction
                    label={`${coachName}への${formatDateTime(review.submitted_at)}の評価`}
                    onConfirm={() => markHandled(review)}
                  />
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
