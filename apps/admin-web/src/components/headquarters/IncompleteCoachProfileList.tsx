"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { ResultMessage } from "@/components/ui/ResultMessage";
import {
  COACH_ACCEPTANCE_STATUS_LABELS,
  COACH_PROFILE_FIELD_LABELS,
  SPORT_LABELS,
  formatDateTime,
} from "@/components/labels";

import { ACCEPTANCE_STATUSES, SPORTS, coachNameQuery, coachNameQueryFailureMessage } from "./CoachAcceptanceList";
import { coachOverviewPath } from "./CoachOverviewHome";

type IncompleteCoachProfile = Schemas["IncompleteCoachProfile"];
type Sport = Schemas["Sport"];
type AcceptanceStatus = Schemas["CoachAcceptanceStatus"];

/** 絞り込みの条件。空なら絞らない。`q` は表示名の検索語（前後の空白を除いた値）。 */
export type IncompleteCoachProfileFilter = { sport: Sport | ""; acceptanceStatus: AcceptanceStatus | ""; q: string };

const NO_FILTER: IncompleteCoachProfileFilter = { sport: "", acceptanceStatus: "", q: "" };

/** 0 件のときの文言。表示名で探しているときは、その検索語を文言に含める。 */
function emptyMessage(filter: IncompleteCoachProfileFilter): string {
  const others = filter.sport !== "" || filter.acceptanceStatus !== "";
  if (filter.q !== "") {
    return others
      ? `表示名に「${filter.q}」を含み、条件に合うコーチはいません`
      : `表示名に「${filter.q}」を含む、自己紹介の足りないコーチはいません`;
  }
  return others ? "条件に合うコーチはいません" : "自己紹介の足りないコーチはいません";
}

type State =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: IncompleteCoachProfile[] }
  | { kind: "failed"; message: string };

/** 最後に更新した日時。契約は記録がなければ null で、その間は登録した時刻を目安にする。 */
function updatedAtLabel(coach: IncompleteCoachProfile): string {
  return coach.updated_at === null
    ? `記録なし（登録: ${formatDateTime(coach.registered_at)}）`
    : formatDateTime(coach.updated_at);
}

/** 自己紹介の足りないコーチの一覧の列。 */
const INCOMPLETE_COACH_PROFILE_COLUMNS: DataTableColumn<IncompleteCoachProfile>[] = [
  {
    header: "コーチ",
    cell: (coach) => (
      <Link href={coachOverviewPath(coach.coach_id)} className="text-primary underline">
        {coach.display_name}
      </Link>
    ),
  },
  {
    header: "空の項目",
    cell: (coach) => coach.missing_fields.map((field) => COACH_PROFILE_FIELD_LABELS[field]).join("、"),
  },
  { header: "最後に更新した日時", cell: updatedAtLabel },
];

/**
 * 本部の「自己紹介の足りないコーチ」。開いたときにプロフィールに空の項目があるコーチの一覧
 * （listIncompleteCoachProfiles）を読み、コーチ（そのコーチの様子へのリンク）・空の項目・最後に更新した日時を出す。
 * 「足りない」の判定と並べ方は契約の正本どおり API が済ませているので、返った順（登録の古い順）のまま出す。
 * 照会だけで、コーチへの連絡はしない。
 * 一覧の上の絞り込み（スポーツ・受付の状態）は API の `sport` / `acceptance_status` に渡して読み直す（組み合わせられる）。
 * 受付の状態の選択肢と言葉はコーチの受付の一覧と同じ。
 * 表示名の検索欄はコーチの受付の一覧と同じ作り: 「探す」（または Enter）で確定し、前後の空白を除いた語を `q` に渡して読み直す
 * （空なら絞らない。上限を超える語は送らずに理由を出す）。スポーツ・受付の状態を変えたときも、検索欄に入っている語を一緒に確定する。
 * 探して 0 件なら、検索語を含めた文言を出す。`initialFilter`（URL のクエリ）の条件で開き、
 * 変えたら `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function IncompleteCoachProfileList({
  api,
  initialFilter = NO_FILTER,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listIncompleteCoachProfiles">;
  initialFilter?: IncompleteCoachProfileFilter;
  onFilterChange?: (filter: IncompleteCoachProfileFilter) => void;
}) {
  const [filter, setFilter] = useState<IncompleteCoachProfileFilter>(initialFilter);
  // 検索欄の入力中の値。確定する（探す・Enter）までは一覧を読み直さない。
  const [queryDraft, setQueryDraft] = useState(initialFilter.q);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchCoaches = useCallback(
    async (next: IncompleteCoachProfileFilter): Promise<State> => {
      const response = await api.listIncompleteCoachProfiles({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.acceptanceStatus === "" ? {} : { acceptance_status: next.acceptanceStatus }),
        ...(next.q === "" ? {} : { q: next.q }),
      });
      return response.ok
        ? { kind: "loaded", coaches: response.data.coaches }
        : { kind: "failed", message: coachNameQueryFailureMessage(response) };
    },
    [api],
  );

  // 開いたときの読み込み（渡された条件）。初期の状態が loading なので、ここでは結果だけを入れる。
  const { sport: initialSport, acceptanceStatus: initialAcceptanceStatus, q: initialQ } = initialFilter;
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchCoaches({ sport: initialSport, acceptanceStatus: initialAcceptanceStatus, q: initialQ }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchCoaches, initialSport, initialAcceptanceStatus, initialQ]);

  async function changeFilter(next: IncompleteCoachProfileFilter) {
    setFilter(next);
    setQueryError(null);
    onFilterChange?.(next);
    setState({ kind: "loading" });
    const request = ++latestRequest.current;
    const loaded = await fetchCoaches(next);
    if (request === latestRequest.current) setState(loaded);
  }

  // 検索欄の語を確定し、条件 change と合わせて読み直す。語が上限を超えていれば理由を出し、条件は変えない。
  async function commitFilter(change: Partial<Pick<IncompleteCoachProfileFilter, "sport" | "acceptanceStatus">>) {
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

  return (
    <div className="space-y-4">
      <p className="text-sm">
        自己紹介に空の項目があるコーチを、登録の古い順に見られます。書きかけのままのコーチを見つけて声をかけるための一覧です。
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
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.sport}
            onChange={(event) => void commitFilter({ sport: event.target.value as Sport | "" })}
          >
            <option value="">すべて</option>
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          受付の状態
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.acceptanceStatus}
            onChange={(event) =>
              void commitFilter({ acceptanceStatus: event.target.value as AcceptanceStatus | "" })
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
      </form>
      {queryError !== null && <ResultMessage result={{ ok: false, message: queryError }} />}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.coaches.length === 0 && (
        <p className="text-sm">{emptyMessage(filter)}</p>
      )}
      {state.kind === "loaded" && state.coaches.length > 0 && (
        <DataTable
          label="自己紹介の足りないコーチの一覧"
          columns={INCOMPLETE_COACH_PROFILE_COLUMNS}
          rows={state.coaches}
          rowKey={(coach) => coach.coach_id}
        />
      )}
    </div>
  );
}
