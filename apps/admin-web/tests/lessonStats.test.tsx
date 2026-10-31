import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { LessonStats } from "@/components/coach/LessonStats";
import { formatMonth } from "@/components/labels";

import { EMPTY_LESSON_STATS, monthlyStats } from "./lessonStatsFixtures";
import { noNotifications } from "./notificationFixtures";

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const STATS = {
  ...EMPTY_LESSON_STATS,
  current_month: monthlyStats("2026-10", {
    live_lesson_count: 5,
    async_lesson_count: 2,
    review_count: 3,
    average_rating: 4.67,
  }),
  previous_month: monthlyStats("2026-09", {
    live_lesson_count: 4,
    async_lesson_count: 0,
    review_count: 2,
    average_rating: 4,
  }),
};

type StatsApi = Pick<CoachApi, "getMyLessonStats">;

function mockApi(result: Awaited<ReturnType<CoachApi["getMyLessonStats"]>>): StatsApi {
  return { getMyLessonStats: vi.fn<CoachApi["getMyLessonStats"]>(async () => result) };
}

/** 表の行を [見出し, 今月, 先月] の組にする。 */
function rowsOf(table: HTMLElement) {
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => Array.from(row.children).map((cell) => cell.textContent));
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("コーチの今月と先月の実績", () => {
  test("今月と先月の通話・動画添削の回数と、評価の件数と平均を月ごとに並べる", async () => {
    const api = mockApi({ ok: true, data: STATS });
    render(<LessonStats api={api} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "今月と先月の実績" });
    expect(within(table).getByRole("columnheader", { name: "今月（2026年10月）" })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "先月（2026年9月）" })).toBeInTheDocument();
    expect(rowsOf(table)).toEqual([
      ["通話（同期レッスン）", "5 回", "4 回"],
      ["動画添削（非同期レッスン）", "2 回", "0 回"],
      ["評価の件数", "3 件", "2 件"],
      ["点の平均（1〜5）", "4.67", "4.00"],
    ]);
    expect(screen.queryByText(/完了したレッスンも受けた評価もまだありません/)).not.toBeInTheDocument();
    expect(api.getMyLessonStats).toHaveBeenCalledTimes(1);
  });

  test("評価のない月の平均は「評価はまだありません」を出す", async () => {
    const stats = {
      ...STATS,
      previous_month: monthlyStats("2026-09", { live_lesson_count: 1 }),
    };
    render(<LessonStats api={mockApi({ ok: true, data: stats })} />);

    const table = await screen.findByRole("table", { name: "今月と先月の実績" });
    expect(rowsOf(table)).toContainEqual(["点の平均（1〜5）", "4.67", "評価はまだありません"]);
    expect(screen.queryByText(/完了したレッスンも受けた評価もまだありません/)).not.toBeInTheDocument();
  });

  test("今月も先月も 0 件なら、0 を並べてまだない旨を出す", async () => {
    render(<LessonStats api={mockApi({ ok: true, data: EMPTY_LESSON_STATS })} />);

    const table = await screen.findByRole("table", { name: "今月と先月の実績" });
    expect(rowsOf(table)).toEqual([
      ["通話（同期レッスン）", "0 回", "0 回"],
      ["動画添削（非同期レッスン）", "0 回", "0 回"],
      ["評価の件数", "0 件", "0 件"],
      ["点の平均（1〜5）", "評価はまだありません", "評価はまだありません"],
    ]);
    expect(
      screen.getByText("今月と先月は、完了したレッスンも受けた評価もまだありません。"),
    ).toBeInTheDocument();
  });

  test("読み込めなかったときは、その文言だけを出す", async () => {
    render(
      <LessonStats
        api={mockApi({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" })}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("暦月の表示は「YYYY年M月」で、形の違う値はそのまま出す", () => {
    expect(formatMonth("2026-01")).toBe("2026年1月");
    expect(formatMonth("2026-12")).toBe("2026年12月");
    expect(formatMonth("2026/10")).toBe("2026/10");
  });

  test("コーチのホーム（開いたときの「対応待ち」）の上に出し、ほかのメニューでは出さない", async () => {
    const api = {
      listMyNotifications: noNotifications(),
      getMyLessonStats: vi.fn<CoachApi["getMyLessonStats"]>(async () => ({ ok: true, data: STATS })),
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: true,
        data: { videos: [] },
      })),
      listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
        ok: true,
        data: { bookings: [] },
      })),
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [] },
      })),
      getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({
        ok: false,
        message: "ログインし直してください。",
      })),
    };
    coachApiMock.mockReturnValue(api);
    const user = userEvent.setup();
    render(<CoachHome token="mock.coach.x" />);

    const region = screen.getByRole("region", { name: "対応待ち" });
    const table = await within(region).findByRole("table", { name: "今月と先月の実績" });
    expect(rowsOf(table)[0]).toEqual(["通話（同期レッスン）", "5 回", "4 回"]);
    // 実績は対応待ちの一覧より上に出す。
    const tasksHeading = await within(region).findByRole("heading", { name: /添削待ちの動画/ });
    expect(table.compareDocumentPosition(tasksHeading) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    await user.click(screen.getByRole("tab", { name: "自己紹介" }));
    expect(screen.queryByRole("table", { name: "今月と先月の実績" })).not.toBeInTheDocument();
  });
});
