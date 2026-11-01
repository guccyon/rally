import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { LowRatedReviewList } from "@/components/headquarters/LowRatedReviewList";
import { LowRatedReviewsHome } from "@/components/headquarters/LowRatedReviewsHome";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは LowRatedReviewsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const OTHER_COACH_ID = "77777777-7777-4777-8777-777777777777";

// API が返す並び（送った時刻の新しい順）。画面はこの順のまま出す。
const REVIEWS: Schemas["LowRatedLessonReview"][] = [
  {
    review_id: "33333333-3333-4333-8333-000000000001",
    lesson_id: "44444444-4444-4444-8444-000000000001",
    coach_id: COACH_ID,
    coach_display_name: "佐藤 健",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "山田 花子",
    rating: 1,
    comment: "通話が途中で何度も切れました",
    submitted_at: "2026-10-30T12:14:05Z",
    lesson_menu: { lesson_menu_id: "aaaa0001-0000-4000-8000-000000000000", name: "フォア強化 30 分" },
  },
  {
    review_id: "33333333-3333-4333-8333-000000000002",
    lesson_id: "44444444-4444-4444-8444-000000000002",
    coach_id: OTHER_COACH_ID,
    student_id: "11111111-1111-4111-8111-000000000002",
    rating: 2,
    comment: "説明が早口でした",
    submitted_at: "2026-10-29T09:02:41Z",
    lesson_menu: null,
  },
];

function listOf(reviews: Schemas["LowRatedLessonReview"][]) {
  return vi.fn<HeadquartersApi["listLowRatedLessonReviews"]>(async () => ({
    ok: true,
    data: { max_rating: 2, period_days: 30, reviews },
  }));
}

describe("本部の低い評価", () => {
  test("読み込み中を出し、読めたら 1 件ずつ点・コーチ・メニュー・一言・送った時刻を API の順で出す", async () => {
    const listLowRatedLessonReviews = listOf(REVIEWS);
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const list = await screen.findByRole("list", { name: "低い評価の一覧" });
    expect(listLowRatedLessonReviews).toHaveBeenCalledTimes(1);
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(Array.from(items[0]!.querySelectorAll("p")).map((p) => p.textContent)).toEqual([
      "1 点（5 点満点）",
      "コーチ: 佐藤 健",
      "メニュー: フォア強化 30 分",
      "通話が途中で何度も切れました",
      formatDateTime("2026-10-30T12:14:05Z"),
    ]);
    expect(items[1]).toHaveTextContent("2 点");
    expect(items[1]).toHaveTextContent("説明が早口でした");
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("しきい値と期間は応答の値を添える", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS) }} />);

    expect(await screen.findByText("直近 30 日に送られた、2 点以下の評価です。")).toBeInTheDocument();
  });

  test("メニューがなければメニューの行を出さない", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS) }} />);

    const items = within(await screen.findByRole("list", { name: "低い評価の一覧" })).getAllByRole(
      "listitem",
    );
    expect(items[1]).not.toHaveTextContent("メニュー:");
  });

  test("コーチの名前から、そのコーチを選んだ状態のコーチの評価の画面へ移れる", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS) }} />);

    expect(await screen.findByRole("link", { name: "佐藤 健" })).toHaveAttribute(
      "href",
      `/headquarters/coach-review-summary?coach_id=${COACH_ID}`,
    );
  });

  test("コーチの表示名が引けなくても、名前の代わりの文言からコーチの評価へ移れる", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS) }} />);

    expect(await screen.findByRole("link", { name: "（名前を表示できません）" })).toHaveAttribute(
      "href",
      `/headquarters/coach-review-summary?coach_id=${OTHER_COACH_ID}`,
    );
  });

  test("0 件なら「低い評価はまだありません」と出し、一覧は出さない", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf([]) }} />);

    expect(await screen.findByText("低い評価はまだありません")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "低い評価の一覧" })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listLowRatedLessonReviews = vi.fn<HeadquartersApi["listLowRatedLessonReviews"]>(
      async () => ({ ok: false, message: "権限がないためアクセスできません。" }),
    );
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("list", { name: "低い評価の一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("低い評価はまだありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("本部の画面の「コーチの評価」のすぐ下から低い評価の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "低い評価の一覧を見る" })).toHaveAttribute(
      "href",
      "/headquarters/low-reviews",
    );
    const titles = screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent);
    expect(titles[titles.indexOf("コーチの評価") + 1]).toBe("低い評価");
  });

  test("低い評価の画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({ listLowRatedLessonReviews: listOf(REVIEWS) });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <LowRatedReviewsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "低い評価" });
    expect(await within(region).findByRole("list", { name: "低い評価の一覧" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listLowRatedLessonReviews: listOf(REVIEWS) });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <LowRatedReviewsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "低い評価" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
