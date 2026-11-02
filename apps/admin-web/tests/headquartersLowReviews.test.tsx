import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
    handling: {
      handled_by: "55555555-5555-4555-8555-555555555555",
      handled_at: "2026-10-30T01:00:00Z",
    },
  },
];

const HANDLING: Schemas["LowRatedReviewHandling"] = {
  handled_by: "55555555-5555-4555-8555-555555555555",
  handled_at: "2026-10-31T03:04:05Z",
};

function markOk() {
  return vi.fn<HeadquartersApi["markLowRatedLessonReviewHandled"]>(async () => ({
    ok: true,
    data: HANDLING,
  }));
}

const noMark = () => vi.fn<HeadquartersApi["markLowRatedLessonReviewHandled"]>();

function listOf(reviews: Schemas["LowRatedLessonReview"][]) {
  return vi.fn<HeadquartersApi["listLowRatedLessonReviews"]>(async () => ({
    ok: true,
    data: { max_rating: 2, period_days: 30, reviews },
  }));
}

describe("本部の低い評価", () => {
  test("読み込み中を出し、読めたら 1 件ずつ点・コーチ・メニュー・一言・送った時刻を API の順で出す", async () => {
    const listLowRatedLessonReviews = listOf(REVIEWS);
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled: noMark() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const list = await screen.findByRole("list", { name: "低い評価の一覧" });
    expect(listLowRatedLessonReviews).toHaveBeenCalledTimes(1);
    expect(listLowRatedLessonReviews).toHaveBeenCalledWith({});
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(Array.from(items[0]!.querySelectorAll("p")).map((p) => p.textContent)).toEqual([
      "1 点（5 点満点）",
      "コーチ: 佐藤 健",
      "メニュー: フォア強化 30 分",
      "通話が途中で何度も切れました",
      formatDateTime("2026-10-30T12:14:05Z"),
      "対応: 未対応",
    ]);
    expect(items[1]).toHaveTextContent("2 点");
    expect(items[1]).toHaveTextContent("説明が早口でした");
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("しきい値と期間は応答の値を添える", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS), markLowRatedLessonReviewHandled: noMark() }} />);

    expect(await screen.findByText("直近 30 日に送られた、2 点以下の評価です。")).toBeInTheDocument();
  });

  test("メニューがなければメニューの行を出さない", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS), markLowRatedLessonReviewHandled: noMark() }} />);

    const items = within(await screen.findByRole("list", { name: "低い評価の一覧" })).getAllByRole(
      "listitem",
    );
    expect(items[1]).not.toHaveTextContent("メニュー:");
  });

  test("コーチの名前から、そのコーチを選んだ状態のコーチの評価の画面へ移れる", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS), markLowRatedLessonReviewHandled: noMark() }} />);

    expect(await screen.findByRole("link", { name: "佐藤 健" })).toHaveAttribute(
      "href",
      `/headquarters/coach-review-summary?coach_id=${COACH_ID}`,
    );
  });

  test("コーチの表示名が引けなくても、名前の代わりの文言からコーチの評価へ移れる", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS), markLowRatedLessonReviewHandled: noMark() }} />);

    expect(await screen.findByRole("link", { name: "（名前を表示できません）" })).toHaveAttribute(
      "href",
      `/headquarters/coach-review-summary?coach_id=${OTHER_COACH_ID}`,
    );
  });

  test("0 件なら「低い評価はまだありません」と出し、一覧は出さない", async () => {
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews: listOf([]), markLowRatedLessonReviewHandled: noMark() }} />);

    expect(await screen.findByText("低い評価はまだありません")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "低い評価の一覧" })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listLowRatedLessonReviews = vi.fn<HeadquartersApi["listLowRatedLessonReviews"]>(
      async () => ({ ok: false, message: "権限がないためアクセスできません。" }),
    );
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled: noMark() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("list", { name: "低い評価の一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("低い評価はまだありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("本部の低い評価の対応済み", () => {
  const firstLabel = `佐藤 健への${formatDateTime("2026-10-30T12:14:05Z")}の評価`;

  async function itemsOf() {
    return within(await screen.findByRole("list", { name: "低い評価の一覧" })).getAllByRole("listitem");
  }

  test("対応済みの行は日時を出して「対応済みにする」を置かず、未対応の行にだけ置く", async () => {
    render(
      <LowRatedReviewList
        api={{ listLowRatedLessonReviews: listOf(REVIEWS), markLowRatedLessonReviewHandled: noMark() }}
      />,
    );

    const items = await itemsOf();
    expect(items[0]).toHaveTextContent("対応: 未対応");
    expect(within(items[0]!).getByRole("button", { name: `${firstLabel}を対応済みにする` })).toBeInTheDocument();
    expect(items[1]).toHaveTextContent(`対応: 対応済み（${formatDateTime("2026-10-30T01:00:00Z")}）`);
    expect(within(items[1]!).queryByRole("button")).not.toBeInTheDocument();
  });

  test("押すと行の中で確かめ、やめれば送らずに元に戻る", async () => {
    const user = userEvent.setup();
    const markLowRatedLessonReviewHandled = markOk();
    render(
      <LowRatedReviewList api={{ listLowRatedLessonReviews: listOf(REVIEWS), markLowRatedLessonReviewHandled }} />,
    );

    await user.click(await screen.findByRole("button", { name: `${firstLabel}を対応済みにする` }));
    const group = screen.getByRole("group", { name: `${firstLabel}の対応済みの確認` });
    expect(group).toHaveTextContent("この評価を対応済みにしますか？");
    await user.click(within(group).getByRole("button", { name: "やめる" }));

    expect(markLowRatedLessonReviewHandled).not.toHaveBeenCalled();
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: `${firstLabel}を対応済みにする` })).toBeInTheDocument();
  });

  test("確定で送り、送っている間はボタンを止め、付いた印を行に入れてボタンを外す", async () => {
    const user = userEvent.setup();
    let resolve: (value: Awaited<ReturnType<HeadquartersApi["markLowRatedLessonReviewHandled"]>>) => void =
      () => {};
    const markLowRatedLessonReviewHandled = vi.fn<HeadquartersApi["markLowRatedLessonReviewHandled"]>(
      () => new Promise((r) => (resolve = r)),
    );
    const listLowRatedLessonReviews = listOf(REVIEWS);
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled }} />);

    await user.click(await screen.findByRole("button", { name: `${firstLabel}を対応済みにする` }));
    await user.click(screen.getByRole("button", { name: "対応済みを確定" }));

    expect(markLowRatedLessonReviewHandled).toHaveBeenCalledWith(REVIEWS[0]!.review_id);
    expect(screen.getByRole("button", { name: "対応済みにしています…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "やめる" })).toBeDisabled();

    resolve({ ok: true, data: HANDLING });
    expect(await screen.findByRole("status")).toHaveTextContent("評価を対応済みにしました。");
    const items = await itemsOf();
    expect(items[0]).toHaveTextContent(`対応: 対応済み（${formatDateTime("2026-10-31T03:04:05Z")}）`);
    expect(within(items[0]!).queryByRole("button")).not.toBeInTheDocument();
    expect(listLowRatedLessonReviews).toHaveBeenCalledTimes(1);
  });

  test("断られたら（409 など）文言を出して一覧を取り直す", async () => {
    const user = userEvent.setup();
    const markLowRatedLessonReviewHandled = vi.fn<HeadquartersApi["markLowRatedLessonReviewHandled"]>(
      async () => ({
        ok: false,
        code: "lesson_review_not_low_rated",
        message: "低い評価ではないため、対応済みにできません。",
      }),
    );
    const listLowRatedLessonReviews = listOf(REVIEWS);
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled }} />);

    await user.click(await screen.findByRole("button", { name: `${firstLabel}を対応済みにする` }));
    await user.click(screen.getByRole("button", { name: "対応済みを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("低い評価ではないため、対応済みにできません。");
    await itemsOf();
    expect(listLowRatedLessonReviews).toHaveBeenCalledTimes(2);
  });

  test("つながらなかったら文言を出し、一覧は取り直さずに行の「対応済みにする」を残す", async () => {
    const user = userEvent.setup();
    const markLowRatedLessonReviewHandled = vi.fn<HeadquartersApi["markLowRatedLessonReviewHandled"]>(
      async () => ({ ok: false, message: "サーバーに接続できません。" }),
    );
    const listLowRatedLessonReviews = listOf(REVIEWS);
    render(<LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled }} />);

    await user.click(await screen.findByRole("button", { name: `${firstLabel}を対応済みにする` }));
    await user.click(screen.getByRole("button", { name: "対応済みを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(listLowRatedLessonReviews).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: `${firstLabel}を対応済みにする` })).toBeInTheDocument();
  });

  test("「未対応だけ」を入れると handled=false で読み直し、外すと条件なしで読み直す", async () => {
    const user = userEvent.setup();
    const listLowRatedLessonReviews = vi.fn<HeadquartersApi["listLowRatedLessonReviews"]>(
      async (filter) => ({
        ok: true,
        data: {
          max_rating: 2,
          period_days: 30,
          reviews: filter?.handled === false ? [REVIEWS[0]!] : REVIEWS,
        },
      }),
    );
    render(
      <LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled: noMark() }} />,
    );
    expect(await itemsOf()).toHaveLength(2);

    await user.click(screen.getByRole("checkbox", { name: "未対応だけ" }));
    expect(listLowRatedLessonReviews).toHaveBeenLastCalledWith({ handled: false });
    await screen.findByText("直近 30 日に送られた、2 点以下の評価です。");
    expect(await itemsOf()).toHaveLength(1);

    await user.click(screen.getByRole("checkbox", { name: "未対応だけ" }));
    expect(listLowRatedLessonReviews).toHaveBeenLastCalledWith({});
    await screen.findByText("直近 30 日に送られた、2 点以下の評価です。");
    expect(await itemsOf()).toHaveLength(2);
  });

  test("「未対応だけ」で 0 件なら「未対応の低い評価はありません」と出す", async () => {
    const user = userEvent.setup();
    const listLowRatedLessonReviews = vi.fn<HeadquartersApi["listLowRatedLessonReviews"]>(
      async (filter) => ({
        ok: true,
        data: { max_rating: 2, period_days: 30, reviews: filter?.handled === false ? [] : REVIEWS },
      }),
    );
    render(
      <LowRatedReviewList api={{ listLowRatedLessonReviews, markLowRatedLessonReviewHandled: noMark() }} />,
    );
    await itemsOf();

    await user.click(screen.getByRole("checkbox", { name: "未対応だけ" }));

    expect(await screen.findByText("未対応の低い評価はありません")).toBeInTheDocument();
    expect(screen.queryByText("低い評価はまだありません")).not.toBeInTheDocument();
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
    headquartersApiMock.mockReturnValue({
      listLowRatedLessonReviews: listOf(REVIEWS),
      markLowRatedLessonReviewHandled: noMark(),
    });
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
    headquartersApiMock.mockReturnValue({
      listLowRatedLessonReviews: listOf(REVIEWS),
      markLowRatedLessonReviewHandled: noMark(),
    });
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
