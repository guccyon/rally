import { render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { CoachOverview } from "@/components/headquarters/CoachOverview";
import { CoachOverviewHome, coachOverviewPath } from "@/components/headquarters/CoachOverviewHome";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace: vi.fn() }) }));

const COACH_ID = "22222222-2222-4222-8222-000000000001";

const OVERVIEW: Schemas["CoachOverview"] = {
  coach_id: COACH_ID,
  display_name: "鈴木 コーチ",
  acceptance_status: "suspended",
  review_count: 12,
  average_rating: 4.5,
  low_rated_review_count: 2,
  unhandled_low_rated_review_count: 1,
  current_month: {
    month: "2026-11",
    live_lesson_count: 3,
    async_lesson_count: 2,
    review_count: 4,
    average_rating: 4.25,
  },
  previous_month: {
    month: "2026-10",
    live_lesson_count: 7,
    async_lesson_count: 1,
    review_count: 0,
    average_rating: null,
  },
  published_lesson_menu_count: 5,
};

function apiOf(result: Result<Schemas["CoachOverview"]>) {
  return { getCoachOverview: vi.fn<HeadquartersApi["getCoachOverview"]>(async () => result) };
}

function valueOf(label: string) {
  const term = within(screen.getByLabelText("コーチの様子")).getByText(label, { selector: "dt" });
  return term.nextElementSibling as HTMLElement;
}

describe("本部のコーチの様子", () => {
  test("読み込み中を出し、読めたら受付の状態・評価の件数と平均・公開中のメニューの数・今月と先月の実績を出す", async () => {
    const api = apiOf({ ok: true, data: OVERVIEW });
    render(<CoachOverview api={api} coachId={COACH_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "鈴木 コーチ" })).toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(api.getCoachOverview).toHaveBeenCalledTimes(1);
    expect(api.getCoachOverview).toHaveBeenCalledWith(COACH_ID);

    expect(valueOf("受付")).toHaveTextContent("停止中");
    expect(valueOf("評価の件数")).toHaveTextContent("12 件");
    expect(valueOf("点の平均（1〜5）")).toHaveTextContent("4.50");
    expect(valueOf("公開中のレッスンメニュー")).toHaveTextContent("5 件");

    const table = screen.getByRole("table", { name: "今月と先月の実績" });
    expect(within(table).getByRole("columnheader", { name: "今月（2026年11月）" })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "先月（2026年10月）" })).toBeInTheDocument();
    const live = within(table).getByRole("row", { name: /^通話（同期レッスン）/ });
    expect(within(live).getAllByRole("cell").map((cell) => cell.textContent)).toEqual(["3 回", "7 回"]);
    const asyncRow = within(table).getByRole("row", { name: /^動画添削（非同期レッスン）/ });
    expect(within(asyncRow).getAllByRole("cell").map((cell) => cell.textContent)).toEqual(["2 回", "1 回"]);
  });

  test("「コーチの評価を見る」は、そのコーチを選んだ状態のコーチの評価へのリンク", async () => {
    render(<CoachOverview api={apiOf({ ok: true, data: OVERVIEW })} coachId={COACH_ID} />);

    const link = await screen.findByRole("link", { name: "コーチの評価を見る" });
    expect(link).toHaveAttribute("href", `/headquarters/coach-review-summary?coach_id=${COACH_ID}`);
  });

  test("「このコーチのレッスンメニューを見る」は、そのコーチで絞ったレッスンメニューへのリンク", async () => {
    render(<CoachOverview api={apiOf({ ok: true, data: OVERVIEW })} coachId={COACH_ID} />);

    const link = await screen.findByRole("link", { name: "このコーチのレッスンメニューを見る" });
    expect(link).toHaveAttribute("href", `/headquarters/lesson-menus?coach_id=${COACH_ID}`);
  });

  test("評価が 0 件なら平均は「評価はまだありません」、受付中なら「受付中」", async () => {
    const data = { ...OVERVIEW, acceptance_status: "accepting", review_count: 0, average_rating: null } as const;
    render(<CoachOverview api={apiOf({ ok: true, data })} coachId={COACH_ID} />);

    await screen.findByRole("heading", { name: "鈴木 コーチ" });
    expect(valueOf("受付")).toHaveTextContent("受付中");
    expect(valueOf("評価の件数")).toHaveTextContent("0 件");
    expect(valueOf("点の平均（1〜5）")).toHaveTextContent("評価はまだありません");
  });

  test("受付を止める・再開する操作は出さない", async () => {
    render(<CoachOverview api={apiOf({ ok: true, data: OVERVIEW })} coachId={COACH_ID} />);

    await screen.findByRole("heading", { name: "鈴木 コーチ" });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  test("404 not_found なら「見つかりません」を出す", async () => {
    const api = apiOf({ ok: false, code: "not_found", message: "対象が見つかりません（ID を確かめてください）。" });
    render(<CoachOverview api={api} coachId={COACH_ID} />);

    expect(await screen.findByText("このコーチは見つかりません。コーチの一覧から選び直してください。")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  test("コーチの ID がなければ読まずに「見つかりません」を出す", () => {
    const api = apiOf({ ok: true, data: OVERVIEW });
    render(<CoachOverview api={api} coachId={undefined} />);

    expect(screen.getByText("このコーチは見つかりません。コーチの一覧から選び直してください。")).toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(api.getCoachOverview).not.toHaveBeenCalled();
  });

  test("ほかの失敗はその文言だけを出す", async () => {
    render(
      <CoachOverview
        api={apiOf({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" })}
        coachId={COACH_ID}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("heading", { name: "鈴木 コーチ" })).not.toBeInTheDocument();
  });

  test("接続できないときもその文言を出す", async () => {
    render(<CoachOverview api={apiOf({ ok: false, message: "サーバーに接続できません。" })} coachId={COACH_ID} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
  });

  test("画面にはコーチの一覧と本部の画面へ戻るリンクがある", () => {
    render(<CoachOverviewHome token="mock.headquarters.abc" coachId={undefined} />);

    expect(screen.getByRole("link", { name: "コーチの一覧へ戻る" })).toHaveAttribute("href", "/headquarters/coaches");
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
  });

  test("coachOverviewPath はコーチの ID をクエリに入れる", () => {
    expect(coachOverviewPath(COACH_ID)).toBe(`/headquarters/coach-overview?coach_id=${COACH_ID}`);
  });
});
