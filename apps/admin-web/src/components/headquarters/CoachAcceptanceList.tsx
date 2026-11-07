"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { COACH_ACCEPTANCE_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";

import { CoachAcceptanceAction } from "./CoachAcceptanceAction";
import { CoachAcceptanceBadge } from "./CoachAcceptanceBadge";
import { coachOverviewPath } from "./CoachOverviewHome";

type Coach = Schemas["Coach"];
type AcceptanceStatus = Schemas["CoachAcceptanceStatus"];
type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Coach[] }
  | { kind: "failed"; message: string };

/**
 * 絞り込みの条件。空なら絞らない。`q` は表示名の検索語（前後の空白を除いた値）。
 * `inactive` が true なら直近 30 日にレッスンのないコーチだけ（契約の `inactive`。false なら絞らない）。
 * `hasUnhandledLowRated` が true なら未対応の低い評価のあるコーチだけ（契約の `has_unhandled_low_rated`。false なら絞らない）。
 * `hasUnanswered` が true なら返事のないメッセージのあるコーチだけ（契約の `has_unanswered`。false なら絞らない）。
 * `hasOverdueVideo` が true なら返却の遅れている動画のあるコーチだけ（契約の `has_overdue_video`。false なら絞らない）。
 */
export type CoachAcceptanceFilter = {
  acceptanceStatus: AcceptanceStatus | "";
  sport: Sport | "";
  q: string;
  inactive: boolean;
  hasUnhandledLowRated: boolean;
  hasUnanswered: boolean;
  hasOverdueVideo: boolean;
};
type Filter = CoachAcceptanceFilter;

const NO_FILTER: Filter = {
  acceptanceStatus: "",
  sport: "",
  q: "",
  inactive: false,
  hasUnhandledLowRated: false,
  hasUnanswered: false,
  hasOverdueVideo: false,
};

/** 切り替えの条件の言い方。途中に並べるときは「…がなく」「…があり」、最後は「…のない」「…のある」でコーチにかける。 */
const TOGGLE_PHRASES: readonly { key: "inactive" | "hasUnhandledLowRated" | "hasUnanswered" | "hasOverdueVideo"; joined: string; last: string }[] = [
  { key: "inactive", joined: "直近 30 日にレッスンがなく", last: "直近 30 日にレッスンのない" },
  { key: "hasUnhandledLowRated", joined: "未対応の低い評価があり", last: "未対応の低い評価のある" },
  { key: "hasUnanswered", joined: "返事のないメッセージがあり", last: "返事のないメッセージのある" },
  { key: "hasOverdueVideo", joined: "返却の遅れている動画があり", last: "返却の遅れている動画のある" },
];

/** 切り替えで絞っているときの、0 件の文言の主語（どれも外していれば null）。 */
function toggledSubject(filter: Filter): string | null {
  const phrases = TOGGLE_PHRASES.filter(({ key }) => filter[key]);
  const last = phrases.at(-1);
  if (last === undefined) return null;
  return [...phrases.slice(0, -1).map(({ joined }) => joined), `${last.last}コーチ`].join("、");
}

/**
 * 0 件のときの文言。直近 30 日にレッスンのないコーチだけ・未対応の低い評価のあるコーチだけ・返事のないメッセージのあるコーチだけ・
 * 返却の遅れている動画のあるコーチだけに絞っているときは、その条件を文言に含める。
 */
function emptyMessage(filter: Filter): string {
  const others = filter.acceptanceStatus !== "" || filter.sport !== "" || filter.q !== "";
  const subject = toggledSubject(filter);
  if (subject !== null) {
    return others ? `${subject}のうち、条件に合うコーチはいません` : `${subject}はいません`;
  }
  return others ? "条件に合うコーチはいません" : "コーチがまだいません";
}

/** 契約の `listCoaches` の `q` の上限（前後の空白を除いた Unicode のコードポイントの数。表示名の上限と同じ）。 */
const QUERY_MAX_LENGTH = 50;
const QUERY_TOO_LONG = `表示名は ${QUERY_MAX_LENGTH} 文字までで探してください。`;

/** 検索欄の値を送る形にする。前後の空白（全角を含む）を除き、上限を超えていれば理由を返す。 */
export function coachNameQuery(input: string): { ok: true; q: string } | { ok: false; message: string } {
  const q = input.trim();
  // 契約と同じくコードポイントで数える（input の maxLength は UTF-16 の単位で数えるので使わない）。
  return [...q].length > QUERY_MAX_LENGTH ? { ok: false, message: QUERY_TOO_LONG } : { ok: true, q };
}

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件（検索語の長さ）を添えて直す場所を伝える。
 * 検索欄からは送る前に確かめているので、ふつうは URL のクエリに長い `q` を書いて開いたときだけここに来る。
 * 自己紹介の足りないコーチの一覧（`listIncompleteCoachProfiles` の `q` も同じ上限）でも使う。
 */
export function coachNameQueryFailureMessage(response: Extract<Result<unknown>, { ok: false }>): string {
  return response.code === "validation_failed" ? `${response.message}${QUERY_TOO_LONG}` : response.message;
}

/** 「直近 30 日のレッスン」の数え方（契約の `listCoaches` の `recent_lesson_count` の説明を短くしたもの）。 */
const RECENT_LESSON_COUNT_NOTE =
  "直近 30 日のレッスンは、今日までの 30 日に完了したビデオ通話（同期レッスン）と動画添削（非同期レッスン）の合計です。";

/** 絞り込みの選択肢（契約の値）。URL のクエリの確かめにも使うので、凍結して公開する。 */
export const ACCEPTANCE_STATUSES: readonly AcceptanceStatus[] = Object.freeze(
  Object.keys(COACH_ACCEPTANCE_STATUS_LABELS) as AcceptanceStatus[],
);
export const SPORTS: readonly Sport[] = Object.freeze(Object.keys(SPORT_LABELS) as Sport[]);

/** コーチの一覧の列（操作の列は送る処理が要るので、一覧の中で足す）。 */
const COACH_COLUMNS: DataTableColumn<Coach>[] = [
  {
    header: "表示名",
    cell: (coach) => (
      <Link href={coachOverviewPath(coach.coach_id)} className="text-primary underline">
        {coach.display_name}
      </Link>
    ),
  },
  { header: "受付", cell: (coach) => <CoachAcceptanceBadge status={coach.acceptance_status} /> },
  { header: "直近 30 日のレッスン", variant: "numeric", cell: (coach) => `${coach.recent_lesson_count} 回` },
];

/**
 * 本部の「コーチの受付」。開いたときにコーチの一覧（listCoaches）を読み、各行に表示名と受付の状態と直近 30 日のレッスンの数
 * （`recent_lesson_count`。数え方は契約が正本で、画面は返った値を「n 回」で出すだけ。0 も「0 回」）を出す。
 * 受付中の行は「受付を止める」（suspendCoach。確かめるときに書いた理由（任意）を付ける）、停止中の行は「再開する」
 * （resumeCoach。理由は付けない）を行の中で確かめてから送り、
 * 応答の状態を行に入れる。並びは契約の正本どおり返った順（表示名の順）のまま出す。表示名からそのコーチの様子へ移る。
 * 一覧の上の絞り込み（受付の状態・スポーツ）は API の `acceptance_status` / `sport` に渡して読み直す。
 * 「直近 30 日にレッスンのないコーチだけ」を入れたら `inactive=true` を付けて読み直し、外したら付けずに読み直す。
 * 「未対応の低い評価のあるコーチだけ」も同じく、入れたら `has_unhandled_low_rated=true` を付けて読み直し、外したら付けずに読み直す。
 * 「返事のないメッセージのあるコーチだけ」も同じく、入れたら `has_unanswered=true` を付けて読み直し、外したら付けずに読み直す。
 * 「返却の遅れている動画のあるコーチだけ」も同じく、入れたら `has_overdue_video=true` を付けて読み直し、外したら付けずに読み直す。
 * 表示名の検索欄は「探す」（または Enter）で確定し、前後の空白を除いた語を `q` に渡して読み直す（空なら絞らない）。
 * 上限を超える語は送らずに理由を出す。受付の状態・スポーツ・直近 30 日にレッスンのないコーチだけ・未対応の低い評価のあるコーチだけ・返事のないメッセージのあるコーチだけ・
 * 返却の遅れている動画のあるコーチだけを変えたときも、検索欄に入っている語を一緒に確定して読み直す
 * （欄に見えている語と一覧に効いている条件を常に一致させる。語が上限を超えていれば理由を出し、条件は変えない）。
 * `initialFilter`（URL のクエリ）の条件で開き、変えたら `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 * 止めた・再開したあとは、今の条件で一覧を読み直す（受付の状態で絞っていれば、変わった行は一覧から外れる）。
 */
export function CoachAcceptanceList({
  api,
  initialFilter = NO_FILTER,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listCoaches" | "suspendCoach" | "resumeCoach">;
  initialFilter?: Filter;
  onFilterChange?: (filter: Filter) => void;
}) {
  const [filter, setFilter] = useState<Filter>(initialFilter);
  // 検索欄の入力中の値。確定する（探す・Enter）までは一覧を読み直さない。
  const [queryDraft, setQueryDraft] = useState(initialFilter.q);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchCoaches = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listCoaches({
        ...(next.acceptanceStatus === "" ? {} : { acceptance_status: next.acceptanceStatus }),
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.q === "" ? {} : { q: next.q }),
        ...(next.inactive ? { inactive: true } : {}),
        ...(next.hasUnhandledLowRated ? { has_unhandled_low_rated: true } : {}),
        ...(next.hasUnanswered ? { has_unanswered: true } : {}),
        ...(next.hasOverdueVideo ? { has_overdue_video: true } : {}),
      });
      return response.ok
        ? { kind: "loaded", coaches: response.data.coaches }
        : { kind: "failed", message: coachNameQueryFailureMessage(response) };
    },
    [api],
  );

  // 条件 next で読み直し、最後に頼んだ条件の結果だけを入れる。
  const reload = useCallback(
    async (next: Filter) => {
      const request = ++latestRequest.current;
      const loaded = await fetchCoaches(next);
      if (request === latestRequest.current) setState(loaded);
    },
    [fetchCoaches],
  );

  // 開いたときの読み込み（渡された条件）。初期の状態が loading なので、ここでは結果だけを入れる。
  const {
    acceptanceStatus: initialAcceptanceStatus,
    sport: initialSport,
    q: initialQ,
    inactive: initialInactive,
    hasUnhandledLowRated: initialHasUnhandledLowRated,
    hasUnanswered: initialHasUnanswered,
    hasOverdueVideo: initialHasOverdueVideo,
  } = initialFilter;
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchCoaches({
      acceptanceStatus: initialAcceptanceStatus,
      sport: initialSport,
      q: initialQ,
      inactive: initialInactive,
      hasUnhandledLowRated: initialHasUnhandledLowRated,
      hasUnanswered: initialHasUnanswered,
      hasOverdueVideo: initialHasOverdueVideo,
    }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [
    fetchCoaches,
    initialAcceptanceStatus,
    initialSport,
    initialQ,
    initialInactive,
    initialHasUnhandledLowRated,
    initialHasUnanswered,
    initialHasOverdueVideo,
  ]);

  async function changeFilter(next: Filter) {
    setFilter(next);
    setQueryError(null);
    onFilterChange?.(next);
    setResult(null);
    setState({ kind: "loading" });
    await reload(next);
  }

  // 検索欄の語を確定し、条件 change と合わせて読み直す。語が上限を超えていれば理由を出し、条件は変えない。
  async function commitFilter(
    change: Partial<
      Pick<Filter, "acceptanceStatus" | "sport" | "inactive" | "hasUnhandledLowRated" | "hasUnanswered" | "hasOverdueVideo">
    >,
  ) {
    const query = coachNameQuery(queryDraft);
    if (!query.ok) {
      setQueryError(query.message);
      return;
    }
    setQueryDraft(query.q);
    await changeFilter({ ...filter, ...change, q: query.q });
  }

  async function submitQuery(event: FormEvent) {
    event.preventDefault();
    await commitFilter({});
  }

  async function changeAcceptance(coach: Coach, reason: string) {
    const suspending = coach.acceptance_status === "accepting";
    const response = suspending
      ? await api.suspendCoach(coach.coach_id, reason === "" ? {} : { reason })
      : await api.resumeCoach(coach.coach_id);
    if (response.ok) {
      setResult({
        ok: true,
        message: suspending
          ? `${coach.display_name}の受付を止めました。`
          : `${coach.display_name}の受付を再開しました。`,
      });
      const status = response.data.acceptance_status;
      setState((current) =>
        current.kind === "loaded"
          ? {
              ...current,
              coaches: current.coaches.map((row) =>
                row.coach_id === coach.coach_id ? { ...row, acceptance_status: status } : row,
              ),
            }
          : current,
      );
      // 今の絞り込みのまま読み直す（行は応答の状態にしてあるので、読み込み中には戻さない）。
      await reload(filter);
      return;
    }
    // 409（読み込んだ後に他で止めた・再開した）や 404 などはサーバーの ErrorCode の文言を出し、一覧を今の状態に取り直す。
    setResult(response);
    if (response.code !== undefined) await reload(filter);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        受付を止めたコーチの枠は生徒の検索に出ず、新しい予約を受け付けません。入っている予約と枠はそのままです。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submitQuery(event)}>
        <label className="block">
          表示名
          <input
            type="text"
            className="mt-1 block rounded-md border px-2 py-1"
            value={queryDraft}
            onChange={(event) => setQueryDraft(event.target.value)}
          />
        </label>
        <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
          探す
        </button>
        <label className="block">
          受付の状態
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.acceptanceStatus}
            onChange={(event) =>
              void commitFilter({
                acceptanceStatus: event.target.value as AcceptanceStatus | "",
              })
            }
          >
            <option value="">すべて</option>
            {ACCEPTANCE_STATUSES.map((value) => (
              <option key={value} value={value}>
                {COACH_ACCEPTANCE_STATUS_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.sport}
            onChange={(event) =>
              void commitFilter({
                sport: event.target.value as Sport | "",
              })
            }
          >
            <option value="">すべて</option>
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1 py-1">
          <input
            type="checkbox"
            checked={filter.inactive}
            onChange={(event) => void commitFilter({ inactive: event.target.checked })}
          />
          直近 30 日にレッスンのないコーチだけ
        </label>
        <label className="flex items-center gap-1 py-1">
          <input
            type="checkbox"
            checked={filter.hasUnhandledLowRated}
            onChange={(event) => void commitFilter({ hasUnhandledLowRated: event.target.checked })}
          />
          未対応の低い評価のあるコーチだけ
        </label>
        <label className="flex items-center gap-1 py-1">
          <input
            type="checkbox"
            checked={filter.hasUnanswered}
            onChange={(event) => void commitFilter({ hasUnanswered: event.target.checked })}
          />
          返事のないメッセージのあるコーチだけ
        </label>
        <label className="flex items-center gap-1 py-1">
          <input
            type="checkbox"
            checked={filter.hasOverdueVideo}
            onChange={(event) => void commitFilter({ hasOverdueVideo: event.target.checked })}
          />
          返却の遅れている動画のあるコーチだけ
        </label>
      </form>
      {queryError !== null && <ResultMessage result={{ ok: false, message: queryError }} />}
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.coaches.length === 0 && (
        <p className="text-sm">{emptyMessage(filter)}</p>
      )}
      {state.kind === "loaded" && state.coaches.length > 0 && (
        <>
          <p className="text-sm">{RECENT_LESSON_COUNT_NOTE}</p>
          <DataTable
            label="コーチの一覧"
            columns={[
              ...COACH_COLUMNS,
              {
                header: "操作",
                // 確かめの文と理由の欄が出るので、文の列として折り返す。
                variant: "long_text",
                cell: (coach) => (
                  <CoachAcceptanceAction
                    // 状態が変わったら確かめ中の表示を捨て、新しい状態のボタンから出し直す。
                    key={coach.acceptance_status}
                    status={coach.acceptance_status}
                    onConfirm={(reason) => changeAcceptance(coach, reason)}
                  />
                ),
              },
            ]}
            rows={state.coaches}
            rowKey={(coach) => coach.coach_id}
          />
        </>
      )}
    </div>
  );
}
