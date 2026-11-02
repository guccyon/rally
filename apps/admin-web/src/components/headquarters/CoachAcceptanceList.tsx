"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
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

/** 絞り込みの条件。空なら絞らない。`q` は表示名の検索語（前後の空白を除いた値）。 */
export type CoachAcceptanceFilter = { acceptanceStatus: AcceptanceStatus | ""; sport: Sport | ""; q: string };
type Filter = CoachAcceptanceFilter;

const NO_FILTER: Filter = { acceptanceStatus: "", sport: "", q: "" };

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
 */
function failureMessage(response: Extract<Result<unknown>, { ok: false }>): string {
  return response.code === "validation_failed" ? `${response.message}${QUERY_TOO_LONG}` : response.message;
}

/** 絞り込みの選択肢（契約の値）。URL のクエリの確かめにも使うので、凍結して公開する。 */
export const ACCEPTANCE_STATUSES: readonly AcceptanceStatus[] = Object.freeze(
  Object.keys(COACH_ACCEPTANCE_STATUS_LABELS) as AcceptanceStatus[],
);
export const SPORTS: readonly Sport[] = Object.freeze(Object.keys(SPORT_LABELS) as Sport[]);

/**
 * 本部の「コーチの受付」。開いたときにコーチの一覧（listCoaches）を読み、各行に表示名と受付の状態を出す。
 * 受付中の行は「受付を止める」（suspendCoach。確かめるときに書いた理由（任意）を付ける）、停止中の行は「再開する」
 * （resumeCoach。理由は付けない）を行の中で確かめてから送り、
 * 応答の状態を行に入れる。並びは契約の正本どおり返った順（表示名の順）のまま出す。表示名からそのコーチの様子へ移る。
 * 一覧の上の絞り込み（受付の状態・スポーツ）は API の `acceptance_status` / `sport` に渡して読み直す。
 * 表示名の検索欄は「探す」（または Enter）で確定し、前後の空白を除いた語を `q` に渡して読み直す（空なら絞らない）。
 * 上限を超える語は送らずに理由を出す。受付の状態・スポーツを変えたときも、検索欄に入っている語を一緒に確定して読み直す
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
      });
      return response.ok
        ? { kind: "loaded", coaches: response.data.coaches }
        : { kind: "failed", message: failureMessage(response) };
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
  const { acceptanceStatus: initialAcceptanceStatus, sport: initialSport, q: initialQ } = initialFilter;
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchCoaches({ acceptanceStatus: initialAcceptanceStatus, sport: initialSport, q: initialQ }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchCoaches, initialAcceptanceStatus, initialSport, initialQ]);

  async function changeFilter(next: Filter) {
    setFilter(next);
    setQueryError(null);
    onFilterChange?.(next);
    setResult(null);
    setState({ kind: "loading" });
    await reload(next);
  }

  // 検索欄の語を確定し、条件 change と合わせて読み直す。語が上限を超えていれば理由を出し、条件は変えない。
  async function commitFilter(change: Partial<Pick<Filter, "acceptanceStatus" | "sport">>) {
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

  const filtered = filter.acceptanceStatus !== "" || filter.sport !== "" || filter.q !== "";

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
      </form>
      {queryError !== null && <ResultMessage result={{ ok: false, message: queryError }} />}
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.coaches.length === 0 && (
        <p className="text-sm">{filtered ? "条件に合うコーチはいません" : "コーチがまだいません"}</p>
      )}
      {state.kind === "loaded" && state.coaches.length > 0 && (
        <table aria-label="コーチの一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">表示名</th>
              <th className="py-1">受付</th>
              <th className="py-1">操作</th>
            </tr>
          </thead>
          <tbody>
            {state.coaches.map((coach) => (
              <tr key={coach.coach_id} className="border-t align-top">
                <td className="py-1">
                  <Link href={coachOverviewPath(coach.coach_id)} className="text-primary underline">
                    {coach.display_name}
                  </Link>
                </td>
                <td className="py-1">
                  <CoachAcceptanceBadge status={coach.acceptance_status} />
                </td>
                <td className="py-1">
                  <CoachAcceptanceAction
                    // 状態が変わったら確かめ中の表示を捨て、新しい状態のボタンから出し直す。
                    key={coach.acceptance_status}
                    status={coach.acceptance_status}
                    onConfirm={(reason) => changeAcceptance(coach, reason)}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
