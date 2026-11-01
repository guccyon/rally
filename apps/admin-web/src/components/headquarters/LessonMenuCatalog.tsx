"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { LESSON_TYPE_LABELS, SPORT_LABELS, formatDateTime } from "@/components/labels";

import { LessonMenuPublicationAction } from "./LessonMenuPublicationAction";

type Sport = Schemas["Sport"];
type LessonMenu = Schemas["CoachLessonMenu"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; menus: LessonMenu[] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

type Filter = { sport: Sport | ""; publishedOnly: boolean; coachId: string };

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];

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
      <span className="block text-xs text-gray-500">
        {formatDateTime(unpublish.unpublished_at)}
        {unpublish.reason === "" ? "" : `・理由: ${unpublish.reason}`}
      </span>
    </>
  );
}

/**
 * 本部の「レッスンメニュー」。すべてのコーチのメニューを横断で出す（listLessonMenus）。
 * 絞り込み（スポーツ・公開中だけ・コーチ）は API の `sport` / `published` / `coach_id` に渡し、並びは契約の正本どおり返った順のまま出す。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順。`initialCoachId`（URL の `?coach_id=`）があれば、そのコーチで絞って開く
 * （コーチの一覧にない ID でも絞り込みはそのまま API に渡し、契約どおり 0 件になる）。
 * 各行は確かめてから本部が非公開にする（unpublishLessonMenu）・本部が非公開にしたものを公開に戻す（republishLessonMenu）。
 */
export function LessonMenuCatalog({
  api,
  initialCoachId,
}: {
  api: Pick<
    HeadquartersApi,
    "listLessonMenus" | "listCoaches" | "unpublishLessonMenu" | "republishLessonMenu"
  >;
  initialCoachId?: string;
}) {
  const [filter, setFilter] = useState<Filter>({
    sport: "",
    publishedOnly: false,
    coachId: initialCoachId ?? "",
  });
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchMenus = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listLessonMenus({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.publishedOnly ? { published: true } : {}),
        ...(next.coachId === "" ? {} : { coach_id: next.coachId }),
      });
      return response.ok
        ? { kind: "loaded", menus: response.data.lesson_menus }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（URL のコーチだけで絞る）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchMenus({ sport: "", publishedOnly: false, coachId: initialCoachId ?? "" }).then(
      (next) => {
        if (active && request === latestRequest.current) setState(next);
      },
    );
    return () => {
      active = false;
    };
  }, [fetchMenus, initialCoachId]);

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
    setResult(null);
    await reload(next);
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

  return (
    <div className="space-y-4">
      <p className="text-sm">コーチが登録したレッスンメニューを、コーチをまたいで見られます。</p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={filter.sport}
            onChange={(event) =>
              void changeFilter({ ...filter, sport: event.target.value as Sport | "" })
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
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={filter.publishedOnly}
            onChange={(event) =>
              void changeFilter({ ...filter, publishedOnly: event.target.checked })
            }
          />
          公開中だけ
        </label>
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={filter.coachId}
            disabled={coaches.kind !== "loaded" && !unknownCoachSelected}
            onChange={(event) => void changeFilter({ ...filter, coachId: event.target.value })}
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
      </div>
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.menus.length === 0 && (
        <p className="text-sm">条件に合うメニューはありません</p>
      )}
      {state.kind === "loaded" && state.menus.length > 0 && (
        <table aria-label="レッスンメニューの一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">スポーツ</th>
              <th className="py-1">コーチ</th>
              <th className="py-1">名前</th>
              <th className="py-1">種類</th>
              <th className="py-1">時間</th>
              <th className="py-1">公開</th>
              <th className="py-1">操作</th>
            </tr>
          </thead>
          <tbody>
            {state.menus.map((menu) => (
              <tr key={menu.lesson_menu_id} className="border-t">
                <td className="py-1">{SPORT_LABELS[menu.sport]}</td>
                <td className="py-1">{menu.coach_display_name ?? "（名前を表示できません）"}</td>
                <td className="py-1">{menu.name}</td>
                <td className="py-1">{LESSON_TYPE_LABELS[menu.lesson_type]}</td>
                <td className="py-1">{durationLabel(menu)}</td>
                <td className="py-1">
                  <PublicationCell menu={menu} />
                </td>
                <td className="py-1">
                  <LessonMenuPublicationAction
                    menu={menu}
                    onUnpublish={async (reason) =>
                      changePublication(
                        menu,
                        await api.unpublishLessonMenu(
                          menu.lesson_menu_id,
                          reason === "" ? {} : { reason },
                        ),
                        "非公開にしました",
                      )
                    }
                    onRepublish={async () =>
                      changePublication(
                        menu,
                        await api.republishLessonMenu(menu.lesson_menu_id),
                        "公開に戻しました",
                      )
                    }
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
