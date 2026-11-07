"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { LESSON_TYPE_LABELS, SPORT_LABELS, formatDateTime } from "@/components/labels";

import { lessonMenuBookingListPath } from "./BookingListHome";
import { LessonMenuPublicationAction } from "./LessonMenuPublicationAction";
import { lessonMenuPublicationHistoryPath } from "./LessonMenuPublicationHistoryHome";
import { Button } from "@/components/ui/Button";

type Sport = Schemas["Sport"];
type LessonType = Schemas["LessonType"];
type LessonMenu = Schemas["CoachLessonMenu"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; menus: LessonMenu[] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

/** URL に残す絞り込み（コーチと確定した名前の検索語。空なら絞らない）。 */
export type LessonMenuCatalogQuery = { coachId: string; q: string };

/**
 * 絞り込みの条件。`q` は確定した名前の検索語（前後の空白を除いた値。空なら絞らない）。
 * `unbookedOnly` が true なら直近 30 日に予約のないメニューだけ（契約の `unbooked`。false なら絞らない）。
 */
type Filter = {
  q: string;
  sport: Sport | "";
  lessonType: LessonType | "";
  publishedOnly: boolean;
  coachId: string;
  unbookedOnly: boolean;
};

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];
const LESSON_TYPES = Object.keys(LESSON_TYPE_LABELS) as LessonType[];

/** 名前の検索語の上限（契約の `listLessonMenus` の `q`。前後の空白を除いて数える）。 */
const MAX_QUERY_LENGTH = 40;

const QUERY_TOO_LONG = `名前の検索語は ${MAX_QUERY_LENGTH} 文字までにしてください（前後の空白は数えません）。`;

// 送る前の確かめをすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
// URL の `?coach_id=` が UUID でないときもここに来る。
const VALIDATION_FAILED = `絞り込みの指定が正しくありません。名前の検索語は ${MAX_QUERY_LENGTH} 文字まで、コーチは一覧から選んでください。`;

/** 検索語の文字数（Unicode のコードポイントで数える）が上限を超えていれば理由を返す。 */
function queryError(q: string): string | null {
  return [...q].length > MAX_QUERY_LENGTH ? QUERY_TOO_LONG : null;
}

/**
 * 0 件のときの文言。直近 30 日に予約のないメニューだけ・レッスンの種類で絞っているときは、その条件（種類は表示名）を文言に含める。
 */
function emptyMessage(filter: Filter): string {
  if (!filter.unbookedOnly && filter.lessonType === "") return "条件に合うメニューはありません";
  const subject = `${filter.unbookedOnly ? "直近 30 日に予約のない" : ""}${
    filter.lessonType === "" ? "" : `${LESSON_TYPE_LABELS[filter.lessonType]}の`
  }メニュー`;
  const others =
    filter.q !== "" || filter.sport !== "" || filter.publishedOnly || filter.coachId !== "";
  return others ? `${subject}のうち、条件に合うメニューはありません` : `${subject}はありません`;
}

function durationLabel(menu: LessonMenu): string {
  return menu.duration_minutes === null ? "—" : `${menu.duration_minutes} 分`;
}

function PublicationCell({ menu }: { menu: LessonMenu }) {
  const unpublish = menu.headquarters_unpublish;
  if (menu.is_published) return <>公開中</>;
  if (unpublish === undefined) return <>非公開</>;
  return (
    <>
      非公開（本部）
      <span className="block text-xs text-text-muted">
        {formatDateTime(unpublish.unpublished_at)}
        {unpublish.reason === "" ? "" : `・理由: ${unpublish.reason}`}
      </span>
    </>
  );
}

/**
 * 本部の「レッスンメニュー」。すべてのコーチのメニューを横断で出す（listLessonMenus）。
 * 絞り込み（名前・スポーツ・レッスンの種類・公開中だけ・コーチ・直近 30 日に予約のないメニューだけ）は
 * API の `q` / `sport` / `lesson_type` / `published` / `coach_id` / `unbooked` に渡し、
 * 並びは契約の正本どおり返った順のまま出す。
 * 名前の検索語は入力して「探す」（または Enter）で確定し、スポーツ・レッスンの種類・公開中だけ・コーチ・直近 30 日に予約のないメニューだけは選んだときに読み直す
 * （「直近 30 日に予約のないメニューだけ」を入れたら `unbooked=true` を付けて読み直し、外したら付けずに読み直す。URL には残さない）。
 * スポーツ・レッスンの種類・公開中だけ・コーチ・直近 30 日に予約のないメニューだけを変えたときも、検索欄に入っている語を一緒に確定して読み直す
 * （欄に見えている語と一覧に効いている条件を常に一致させる。語が上限を超えていれば理由を出し、条件は変えない）。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順。`initialCoachId`（URL の `?coach_id=`）があれば、そのコーチで絞って開く
 * （コーチの一覧にない ID でも絞り込みはそのまま API に渡し、契約どおり 0 件になる）。`initialQuery`（URL の `?q=`）があれば、その名前で絞って開く。
 * レッスンの種類の選択肢と表示名は `LESSON_TYPE_LABELS` の 1 か所から取る（URL には残さない）。
 * 0 件のときは、直近 30 日に予約のないメニューだけ・レッスンの種類で絞っていればその条件を含めた文言を出す。
 * 絞り込みを変えるたびに、コーチと名前の検索語を `onQueryChange` で知らせる（画面は URL のクエリに残す）。
 * 各行には直近 30 日の予約の数（`recent_booking_count`。数え方は契約の正本）を右寄せ・等幅の数字で出し、隣の「公開」の列との間をあける（並び替えには使わない）。
 * その数は、そのメニューで絞った「予約」の画面（`?lesson_menu_id=`）へのリンクにする。
 * 各行は確かめてから本部が非公開にする（unpublishLessonMenu）・本部が非公開にしたものを公開に戻す（republishLessonMenu）。
 * 操作の列のボタンの後ろに、そのメニューの「公開の記録」の画面への「記録」を置く。
 */
export function LessonMenuCatalog({
  api,
  initialCoachId,
  initialQuery,
  onQueryChange,
}: {
  api: Pick<
    HeadquartersApi,
    "listLessonMenus" | "listCoaches" | "unpublishLessonMenu" | "republishLessonMenu"
  >;
  initialCoachId?: string;
  initialQuery?: string;
  onQueryChange?: (query: LessonMenuCatalogQuery) => void;
}) {
  const initialQ = (initialQuery ?? "").trim();
  const [filter, setFilter] = useState<Filter>({
    q: initialQ,
    sport: "",
    lessonType: "",
    publishedOnly: false,
    coachId: initialCoachId ?? "",
    unbookedOnly: false,
  });
  // 名前の検索欄の入力中の値（確定する（探す・Enter・ほかの絞り込みを変える）までは一覧を読み直さない）。
  const [queryDraft, setQueryDraft] = useState(initialQ);
  const [queryDraftError, setQueryDraftError] = useState<string | null>(null);
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchMenus = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listLessonMenus({
        ...(next.q === "" ? {} : { q: next.q }),
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.lessonType === "" ? {} : { lesson_type: next.lessonType }),
        ...(next.publishedOnly ? { published: true } : {}),
        ...(next.coachId === "" ? {} : { coach_id: next.coachId }),
        ...(next.unbookedOnly ? { unbooked: true } : {}),
      });
      return response.ok
        ? { kind: "loaded", menus: response.data.lesson_menus }
        : {
            kind: "failed",
            message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
          };
    },
    [api],
  );

  // 開いたときの読み込み（URL のコーチと名前の検索語だけで絞る）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchMenus({
      q: initialQ,
      sport: "",
      lessonType: "",
      publishedOnly: false,
      coachId: initialCoachId ?? "",
      unbookedOnly: false,
    }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchMenus, initialCoachId, initialQ]);

  // コーチの選択肢。読めなくてもメニューの一覧は出し、コーチの絞り込みだけを使えなくする。
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

  const coachOptions = coaches.kind === "loaded" ? coaches.coaches : [];
  // URL で渡されたコーチがコーチの一覧にない（読めていない・ない ID）ときも、選んだ状態を select に出す。
  const unknownCoachSelected =
    filter.coachId !== "" && !coachOptions.some((coach) => coach.coach_id === filter.coachId);

  async function reload(next: Filter) {
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchMenus(next);
    if (request === latestRequest.current) setState(loaded);
  }

  async function changeFilter(next: Filter) {
    setFilter(next);
    setQueryDraftError(null);
    setResult(null);
    onQueryChange?.({ coachId: next.coachId, q: next.q });
    await reload(next);
  }

  // 検索欄の語を確定し、条件 change と合わせて読み直す。語が上限を超えていれば理由を出し、条件は変えない。
  async function commitFilter(change: Partial<Omit<Filter, "q">>) {
    const q = queryDraft.trim();
    const error = queryError(q);
    if (error !== null) {
      setQueryDraftError(error);
      return;
    }
    setQueryDraft(q);
    await changeFilter({ ...filter, ...change, q });
  }

  async function submitQuery(event: FormEvent) {
    event.preventDefault();
    await commitFilter({});
  }

  async function changePublication(
    menu: LessonMenu,
    response: Awaited<ReturnType<HeadquartersApi["unpublishLessonMenu"]>>,
    done: string,
  ) {
    if (response.ok) {
      setResult({ ok: true, message: `「${menu.name}」を${done}。` });
      // 応答は LessonMenu（説明つき・表示名なし）なので、行の公開中かと本部の非公開だけを応答で置き換える。
      const { is_published, headquarters_unpublish } = response.data;
      setState((current) =>
        current.kind === "loaded"
          ? {
              ...current,
              menus: current.menus.map((row) => {
                if (row.lesson_menu_id !== menu.lesson_menu_id) return row;
                const next: LessonMenu = { ...row, is_published };
                delete next.headquarters_unpublish;
                if (headquarters_unpublish !== undefined) {
                  next.headquarters_unpublish = headquarters_unpublish;
                }
                return next;
              }),
            }
          : current,
      );
      return;
    }
    // 409（すでに本部が非公開にした・本部が非公開にしたものでない）などは ErrorCode の文言を出し、一覧を今の状態に取り直す。
    setResult(response);
    if (response.code !== undefined) await reload(filter);
  }

  /** 一覧の列。操作の列は確かめる文と理由の欄を開くので、文の列（long_text）として折り返す。 */
  const columns: DataTableColumn<LessonMenu>[] = [
    { header: "スポーツ", cell: (menu) => SPORT_LABELS[menu.sport] },
    { header: "コーチ", cell: (menu) => menu.coach_display_name ?? "（名前を表示できません）" },
    { header: "名前", cell: (menu) => menu.name },
    { header: "種類", cell: (menu) => LESSON_TYPE_LABELS[menu.lesson_type] },
    { header: "時間", variant: "numeric", cell: durationLabel },
    {
      header: "直近 30 日の予約",
      variant: "numeric",
      cell: (menu) => (
        <Link
          href={lessonMenuBookingListPath(menu.lesson_menu_id)}
          aria-label={`「${menu.name}」の予約を見る`}
          className="text-primary underline"
        >
          {menu.recent_booking_count} 件
        </Link>
      ),
    },
    { header: "公開", cell: (menu) => <PublicationCell menu={menu} /> },
    {
      header: "操作",
      variant: "long_text",
      cell: (menu) => (
        <>
          <LessonMenuPublicationAction
            menu={menu}
            onUnpublish={async (reason) =>
              changePublication(
                menu,
                await api.unpublishLessonMenu(menu.lesson_menu_id, reason === "" ? {} : { reason }),
                "非公開にしました",
              )
            }
            onRepublish={async () =>
              changePublication(menu, await api.republishLessonMenu(menu.lesson_menu_id), "公開に戻しました")
            }
          />
          <Link
            href={lessonMenuPublicationHistoryPath(menu.lesson_menu_id)}
            aria-label={`「${menu.name}」の公開の記録を見る`}
            className="ml-2 text-primary underline"
          >
            記録
          </Link>
        </>
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <p className="text-sm">コーチが登録したレッスンメニューを、コーチをまたいで見られます。</p>
      <div className="flex flex-wrap items-end gap-4">
        <form className="flex items-end gap-2" onSubmit={(event) => void submitQuery(event)}>
          <label className="block">
            名前で探す
            <input
              type="search"
              className="mt-1 block rounded-md border px-2 py-1"
              value={queryDraft}
              onChange={(event) => setQueryDraft(event.target.value)}
            />
          </label>
          <Button type="submit" variant="secondary">
            探す
          </Button>
        </form>
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.sport}
            onChange={(event) =>
              void commitFilter({ sport: event.target.value as Sport | "" })
            }
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
          レッスンの種類
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.lessonType}
            onChange={(event) =>
              void commitFilter({ lessonType: event.target.value as LessonType | "" })
            }
          >
            <option value="">すべて</option>
            {LESSON_TYPES.map((lessonType) => (
              <option key={lessonType} value={lessonType}>
                {LESSON_TYPE_LABELS[lessonType]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={filter.publishedOnly}
            onChange={(event) =>
              void commitFilter({ publishedOnly: event.target.checked })
            }
          />
          公開中だけ
        </label>
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.coachId}
            disabled={coaches.kind !== "loaded" && !unknownCoachSelected}
            onChange={(event) => void commitFilter({ coachId: event.target.value })}
          >
            <option value="">すべて</option>
            {unknownCoachSelected && (
              <option value={filter.coachId}>
                {coaches.kind === "loading" ? "読み込み中…" : "（一覧にないコーチ）"}
              </option>
            )}
            {coachOptions.map((coach) => (
              <option key={coach.coach_id} value={coach.coach_id}>
                {coach.display_name}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={filter.unbookedOnly}
            onChange={(event) => void commitFilter({ unbookedOnly: event.target.checked })}
          />
          直近 30 日に予約のないメニューだけ
        </label>
      </div>
      {queryDraftError !== null && <ResultMessage result={{ ok: false, message: queryDraftError }} />}
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.menus.length === 0 && (
        <p className="text-sm">{emptyMessage(filter)}</p>
      )}
      {state.kind === "loaded" && state.menus.length > 0 && (
        <DataTable
          label="レッスンメニューの一覧"
          columns={columns}
          rows={state.menus}
          rowKey={(menu) => menu.lesson_menu_id}
        />
      )}
      {state.kind === "loaded" && state.menus.length > 0 && (
        // 数え方は契約の listLessonMenus（照会した時刻の 30 日前から、キャンセルされていない予約）に合わせる。
        <p className="text-xs text-text-muted">この 30 日間に入った予約（キャンセルされていないもの）の数です。</p>
      )}
    </div>
  );
}
