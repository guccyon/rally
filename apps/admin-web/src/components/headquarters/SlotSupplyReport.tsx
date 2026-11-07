"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { ResultMessage } from "@/components/ui/ResultMessage";
import {
  formatDateTime,
  formatDay,
  SLOT_SUPPLY_STATUS_LABELS,
  SPORT_LABELS,
  toJstDate,
} from "@/components/labels";

import { BookingLessonMenuFilter } from "./BookingLessonMenuFilter";
import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";
import { lessonMenuOptionLabel } from "./CancelledBookingList";
import { SPORTS } from "./PendingVideoReviewList";
import { Button } from "@/components/ui/Button";

type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | {
      kind: "loaded";
      report: Schemas["SlotSupplyReport"];
      coachId: string;
      sport: Sport | "";
      lessonMenuId: string;
    }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

type LessonMenusState =
  | { kind: "loading" }
  | { kind: "loaded"; lessonMenus: Schemas["CoachLessonMenu"][] }
  | { kind: "failed"; message: string };

/**
 * period が null なら API の既定の期間（開いたときだけ）。coachId が空ならすべてのコーチ、sport が空ならすべての競技、
 * lessonMenuId が空ならすべてのレッスンメニュー（メニューのない枠も数える）。
 */
type Filter = { period: Period | null; coachId: string; sport: Sport | ""; lessonMenuId: string };

const ALL_COACHES: Filter = { period: null, coachId: "", sport: "", lessonMenuId: "" };

const NO_NAME = "（名前を表示できません）";

// 判定の見た目。足りていないを目立たせ、数からは言えない・対象外は判定ではないので控えめに出す。
const STATUS_CLASS_NAMES: Record<Schemas["SlotSupplyStatus"], string> = {
  sufficient: "",
  insufficient: "font-semibold text-danger",
  no_activity: "text-text-muted",
  not_applicable: "text-text-muted",
};

/** 需給の表の列。 */
const SUPPLY_COLUMNS: DataTableColumn<Schemas["SlotSupply"]>[] = [
  { header: "スポーツ", cell: (supply) => SPORT_LABELS[supply.sport] },
  {
    header: "判定",
    cell: (supply) => (
      <span className={STATUS_CLASS_NAMES[supply.status]}>{SLOT_SUPPLY_STATUS_LABELS[supply.status]}</span>
    ),
  },
  { header: "公開した枠の数", variant: "numeric", cell: (supply) => supply.published_slots },
  { header: "予約しようとした生徒の数", variant: "numeric", cell: (supply) => supply.booking_students },
];

// 対象外の理由（契約の `getSlotSupply` の判定の 1）。
const NOT_APPLICABLE_NOTE = "対象外: 月額の契約の対象でないスポーツ（ゴルフ。ワンショットだけ）は、需給を判定しません。";

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間・コーチ・スポーツ・レッスンメニューの指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチ・スポーツ・レッスンメニューは一覧から選び直してください。`;

/**
 * スポーツごとの予約枠の需給（getSlotSupply）。判定と数え方は契約の説明が正本で、画面は返った値を出すだけ。
 * 開いたときは期間とコーチを付けずに読み（API の既定: 集計した時刻までの 30 日・すべてのコーチ）、数えた期間の日付（日本時間）を入力欄に入れる。
 * 「この条件で見る」で選んだ期間（日本時間の日付）を `from` / `to` に、選んだコーチを `coach_id` に、選んだスポーツを `sport` に渡して
 * 読み直し、「再集計」は最後に読んだ条件で読み直す。コーチの選択肢はコーチの一覧（listCoaches）の返った順（キャンセルされた予約と同じ）、
 * スポーツの選択肢は契約の `Sport` の順（返却待ちの動画添削と同じ）。スポーツで絞ると、その競技を提供するコーチの枠と予約だけを数え、
 * 表は今までどおり 6 種類すべてを出す（契約）。コーチ・スポーツで絞った結果には、表の上に「どのコーチ・どの競技の需給か」の見出しを出す。
 * 選んだレッスンメニューは `lesson_menu_id` に渡し、そのメニューに結びついた枠と予約だけを数える（契約）。選択肢と表示はキャンセルされた
 * 予約の絞り込みに揃え、レッスンメニューの一覧（listLessonMenus。絞り込みなしで、公開をやめたメニューも含む）の返った順に出し、
 * 絞った結果には表の上に「どのレッスンメニューで絞っているか」（BookingLessonMenuFilter）を出す。
 */
export function SlotSupplyReport({
  api,
}: {
  api: Pick<HeadquartersApi, "getSlotSupply" | "listCoaches" | "listLessonMenus">;
}) {
  const [draft, setDraft] = useState<Filter>(ALL_COACHES);
  const [draftError, setDraftError] = useState<string | null>(null);
  const [applied, setApplied] = useState<Filter>(ALL_COACHES);
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [lessonMenus, setLessonMenus] = useState<LessonMenusState>({ kind: "loading" });
  const [state, setState] = useState<State>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchReport = useCallback(
    async (filter: Filter): Promise<State> => {
      const response = await api.getSlotSupply({
        ...(filter.period ?? {}),
        ...(filter.coachId === "" ? {} : { coach_id: filter.coachId }),
        ...(filter.sport === "" ? {} : { sport: filter.sport }),
        ...(filter.lessonMenuId === "" ? {} : { lesson_menu_id: filter.lessonMenuId }),
      });
      if (response.ok) {
        return {
          kind: "loaded",
          report: response.data,
          coachId: filter.coachId,
          sport: filter.sport,
          lessonMenuId: filter.lessonMenuId,
        };
      }
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      };
    },
    [api],
  );

  // 最初の集計。初期の状態が loading なので、ここでは結果だけを入れる。読めたら数えた期間の日付を入力欄に入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchReport(ALL_COACHES).then((next) => {
      if (!active || request !== latestRequest.current) return;
      setState(next);
      if (next.kind === "loaded") {
        const used = usedPeriodOf(next.report);
        setDraft((current) => (current.period === null ? { ...current, period: used } : current));
      }
    });
    return () => {
      active = false;
    };
  }, [fetchReport]);

  // コーチの選択肢。読めなくても需給は期間だけで見られるようにし、コーチの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listCoaches().then((response) => {
      if (!active) return;
      setCoaches(
        response.ok
          ? { kind: "loaded", coaches: response.data.coaches }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  // レッスンメニューの選択肢。コーチと同じく、読めなくても需給は出し、レッスンメニューの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listLessonMenus({}).then((response) => {
      if (!active) return;
      setLessonMenus(
        response.ok
          ? { kind: "loaded", lessonMenus: response.data.lesson_menus }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  async function load(filter: Filter) {
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchReport(filter);
    if (request === latestRequest.current) setState(result);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    const period = draft.period ?? { from: "", to: "" };
    const error = periodError(period);
    setDraftError(error);
    if (error !== null) return;
    const filter = { ...draft, period };
    setApplied(filter);
    await load(filter);
  }

  const period = draft.period ?? { from: "", to: "" };

  return (
    <div className="space-y-4">
      <p className="text-sm">
        枠の開始の日付（日本時間）で期間を選んで、繁忙期の前に枠が足りるかを見られます。コーチを選ぶと、そのコーチの枠と、それらの枠への予約だけを数えます。スポーツを選ぶと、そのスポーツを提供するコーチの枠と、それらの枠への予約だけを数えます。レッスンメニューを選ぶと、コーチがそのメニューに結びつけて公開した枠と、それらの枠への予約だけを数えます。期間は{" "}
        {MAX_PERIOD_DAYS} 日まで選べます。開いたときは集計した時刻までの 30 日・すべてのコーチ・すべてのスポーツ・すべてのレッスンメニューです。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={period.from}
            onChange={(event) => setDraft({ ...draft, period: { ...period, from: event.target.value } })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={period.to}
            onChange={(event) => setDraft({ ...draft, period: { ...period, to: event.target.value } })}
          />
        </label>
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.coachId}
            disabled={coaches.kind !== "loaded"}
            onChange={(event) => setDraft({ ...draft, coachId: event.target.value })}
          >
            <option value="">すべて</option>
            {coaches.kind === "loaded" &&
              coaches.coaches.map((coach) => (
                <option key={coach.coach_id} value={coach.coach_id}>
                  {coach.display_name}
                </option>
              ))}
          </select>
        </label>
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.sport}
            onChange={(event) => setDraft({ ...draft, sport: event.target.value as Sport | "" })}
          >
            <option value="">すべて</option>
            {SPORTS.map((sport) => (
              <option key={sport} value={sport}>
                {SPORT_LABELS[sport]}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          レッスンメニュー
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.lessonMenuId}
            disabled={lessonMenus.kind !== "loaded"}
            onChange={(event) => setDraft({ ...draft, lessonMenuId: event.target.value })}
          >
            <option value="">すべて</option>
            {lessonMenus.kind === "loaded" &&
              lessonMenus.lessonMenus.map((menu) => (
                <option key={menu.lesson_menu_id} value={menu.lesson_menu_id}>
                  {lessonMenuOptionLabel(menu)}
                </option>
              ))}
          </select>
        </label>
        <Button type="submit" variant="secondary">
          この条件で見る
        </Button>
        <Button variant="secondary" disabled={state.kind === "loading"} onClick={() => void load(applied)}>
          再集計
        </Button>
      </form>
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      {lessonMenus.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `レッスンメニューの一覧を読めませんでした。${lessonMenus.message}` }}
        />
      )}
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-text-muted">集計中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <LoadedReport
          api={api}
          report={state.report}
          coachName={coachNameOf(state.coachId, coaches)}
          sport={state.sport}
          lessonMenuId={state.lessonMenuId}
        />
      )}
    </div>
  );
}

/** 数えた期間の日付（日本時間）。日付で指定していれば応答の `from` / `to`、既定の 30 日なら `window_start` / `window_end` の日付。 */
function usedPeriodOf(report: Schemas["SlotSupplyReport"]): Period {
  if (report.from !== undefined && report.to !== undefined) return { from: report.from, to: report.to };
  // window_end は含まないので、その直前の時刻の日付を終わりにする。
  return { from: toJstDate(report.window_start), to: toJstDate(Date.parse(report.window_end) - 1) };
}

/** どのスポーツにも期間に開始する枠がなく、予約しようとした生徒もいない（数えるものが 0 件）。 */
function isEmpty(report: Schemas["SlotSupplyReport"]): boolean {
  return report.sports.every((supply) => supply.published_slots === 0 && supply.booking_students === 0);
}

/**
 * 絞ったコーチの名前（絞っていなければ null）。応答には名前が載らないので選択肢のコーチの一覧から引く（コーチは一覧を読めてから
 * しか選べない）。一覧にない ID のときは名前の代わりに「名前を表示できません」を出す。
 */
function coachNameOf(coachId: string, coaches: CoachesState): string | null {
  if (coachId === "") return null;
  const coach = coaches.kind === "loaded" ? coaches.coaches.find((each) => each.coach_id === coachId) : undefined;
  return coach?.display_name ?? NO_NAME;
}

/** 絞った条件の見出し（絞っていなければ null）。スポーツはその競技を提供するコーチで絞る（契約）ので、コーチの言い方で書く。 */
function headingOf(coachName: string | null, sport: Sport | ""): string | null {
  if (sport === "") return coachName === null ? null : `${coachName} の予約枠の需給`;
  const sportCoaches = `${SPORT_LABELS[sport]}を提供するコーチ`;
  return coachName === null
    ? `${sportCoaches}の予約枠の需給`
    : `${coachName} の予約枠の需給（${sportCoaches}に限る）`;
}

/**
 * 0 件のときの文言。コーチだけ・スポーツだけ・レッスンメニューだけならそのコーチ・その競技のコーチ・そのメニューと言い、
 * 2 つ以上なら「条件に合う」と言う（どのメニューかは表の上の BookingLessonMenuFilter が名前で出す）。
 */
function emptyMessageOf(coachName: string | null, sport: Sport | "", lessonMenuId: string): string {
  const conditions = [coachName !== null, sport !== "", lessonMenuId !== ""].filter(Boolean).length;
  if (conditions >= 2) return "この期間に開始する条件に合う予約枠はなく、予約しようとした生徒もいません";
  if (lessonMenuId !== "") return "この期間に開始するこのレッスンメニューの予約枠はなく、予約しようとした生徒もいません";
  if (coachName !== null) return "この期間に開始するこのコーチの予約枠はなく、予約しようとした生徒もいません";
  if (sport !== "") return "この期間に開始するこのスポーツのコーチの予約枠はなく、予約しようとした生徒もいません";
  return "この期間に開始する予約枠はなく、予約しようとした生徒もいません";
}

function LoadedReport({
  api,
  report,
  coachName,
  sport,
  lessonMenuId,
}: {
  api: Pick<HeadquartersApi, "listLessonMenus">;
  report: Schemas["SlotSupplyReport"];
  coachName: string | null;
  sport: Sport | "";
  lessonMenuId: string;
}) {
  const heading = headingOf(coachName, sport);
  return (
    <div className="space-y-2">
      {heading !== null && <h3 className="font-semibold">{heading}</h3>}
      {lessonMenuId !== "" && <BookingLessonMenuFilter key={lessonMenuId} api={api} lessonMenuId={lessonMenuId} />}
      {report.from !== undefined && report.to !== undefined ? (
        <p className="text-sm">
          集計の期間: {formatDay(report.from)} 〜 {formatDay(report.to)}（この期間の日付に開始する枠を数える）
        </p>
      ) : (
        <p className="text-sm">
          集計の期間: {formatDateTime(report.window_start)} 〜 {formatDateTime(report.window_end)}
          （終わりは含まない。この期間に開始する枠を数える）
        </p>
      )}
      {isEmpty(report) && (
        <p className="text-sm">{emptyMessageOf(coachName, sport, lessonMenuId)}</p>
      )}
      {!isEmpty(report) && (
        <DataTable
          label="スポーツ別の予約枠の需給"
          columns={SUPPLY_COLUMNS}
          rows={report.sports}
          rowKey={(supply) => supply.sport}
        />
      )}
      {!isEmpty(report) && report.sports.some((supply) => supply.status === "not_applicable") && (
        <p className="text-sm text-text-muted">{NOT_APPLICABLE_NOTE}</p>
      )}
    </div>
  );
}
