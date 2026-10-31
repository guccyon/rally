import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { HeadquartersApi } from "@/api/headquarters";
import { formatAverageRating } from "@/components/CoachReviewSummaryView";
import { RoleGate } from "@/components/RoleGate";
import { CoachReviewPicker } from "@/components/headquarters/CoachReviewPicker";
import { CoachReviewSummaryHome } from "@/components/headquarters/CoachReviewSummaryHome";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは CoachReviewSummaryHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const OTHER_COACH_ID = "77777777-7777-4777-8777-777777777777";

// API が返す並び（表示名の順）。
const COACHES = [
  { coach_id: OTHER_COACH_ID, display_name: "佐藤コーチ" },
  { coach_id: COACH_ID, display_name: "山田コーチ" },
];

const REVIEW = {
  review_id: "33333333-3333-4333-8333-333333333333",
  lesson_id: "44444444-4444-4444-8444-444444444444",
  rating: 5,
  comment: "フォームがよく分かりました",
  submitted_at: "2026-10-20T03:00:00Z",
};

type PickerApi = Pick<HeadquartersApi, "listCoaches" | "getCoachReviewSummary" | "listCoachReviews">;

function mockApi(overrides: Partial<PickerApi> = {}) {
  return {
    listCoaches: vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: true,
      data: { coaches: COACHES },
    })),
    getCoachReviewSummary: vi.fn<HeadquartersApi["getCoachReviewSummary"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, review_count: 12, average_rating: 4.5 },
    })),
    listCoachReviews: vi.fn<HeadquartersApi["listCoachReviews"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, reviews: [REVIEW] },
    })),
    ...overrides,
  };
}

/** 件数と平均の一覧（dl）を [見出し, 値] の組にする。 */
function summaryOf(list: HTMLElement) {
  return Array.from(list.querySelectorAll("dt")).map((dt) => [
    dt.textContent,
    dt.nextElementSibling?.textContent,
  ]);
}

describe("本部のコーチの評価", () => {
  test("開いたときにコーチの一覧を取り、表示名を API の順に選択肢にする（ID は入力しない）", async () => {
    const api = mockApi();
    render(<CoachReviewPicker api={api} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const select = await screen.findByLabelText("コーチ");
    expect(api.listCoaches).toHaveBeenCalledTimes(1);
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "選んでください",
      "佐藤コーチ",
      "山田コーチ",
    ]);
    expect(screen.queryByLabelText("コーチ ID")).not.toBeInTheDocument();
    expect(api.getCoachReviewSummary).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("評価の件数と平均")).not.toBeInTheDocument();
  });

  test("一覧から選んだコーチの ID で件数と平均と評価の一覧を出す", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<CoachReviewPicker api={api} />);

    await user.selectOptions(await screen.findByLabelText("コーチ"), "山田コーチ");

    expect(api.getCoachReviewSummary).toHaveBeenCalledWith(COACH_ID);
    expect(api.listCoachReviews).toHaveBeenCalledWith(COACH_ID);
    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toEqual([
      ["コーチ ID", COACH_ID],
      ["評価の件数", "12 件"],
      ["点の平均（1〜5）", "4.50"],
    ]);
    const items = within(screen.getByRole("list", { name: "評価の一覧" })).getAllByRole("listitem");
    expect(items).toHaveLength(1);
    expect(items[0]).toHaveTextContent("5 点");
    expect(items[0]).toHaveTextContent("フォームがよく分かりました");
  });

  test("選んだコーチの評価が 0 件なら、平均も一覧も「まだない」と出す", async () => {
    const api = mockApi({
      getCoachReviewSummary: vi.fn<HeadquartersApi["getCoachReviewSummary"]>(async (coachId) => ({
        ok: true,
        data: { coach_id: coachId, review_count: 0, average_rating: null },
      })),
      listCoachReviews: vi.fn<HeadquartersApi["listCoachReviews"]>(async (coachId) => ({
        ok: true,
        data: { coach_id: coachId, reviews: [] },
      })),
    });
    const user = userEvent.setup();
    render(<CoachReviewPicker api={api} />);

    await user.selectOptions(await screen.findByLabelText("コーチ"), "佐藤コーチ");

    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toEqual([
      ["コーチ ID", OTHER_COACH_ID],
      ["評価の件数", "0 件"],
      ["点の平均（1〜5）", "評価はまだありません"],
    ]);
    expect(screen.getByText("まだ評価はありません")).toBeInTheDocument();
  });

  test("コーチが 1 人もいなければ、その旨を出して選択肢は出さない", async () => {
    const api = mockApi({
      listCoaches: vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
        ok: true,
        data: { coaches: [] },
      })),
    });
    render(<CoachReviewPicker api={api} />);

    expect(await screen.findByText("コーチがまだいません")).toBeInTheDocument();
    expect(screen.queryByLabelText("コーチ")).not.toBeInTheDocument();
  });

  test("コーチの一覧の API のエラー（403）の文言を出し、選択肢は出さない", async () => {
    const api = mockApi({
      listCoaches: vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
        ok: false,
        message: "権限がないためアクセスできません。",
      })),
    });
    render(<CoachReviewPicker api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByLabelText("コーチ")).not.toBeInTheDocument();
  });

  test("評価の API のエラーの文言を出し、前に選んだコーチの結果は消す", async () => {
    const api = mockApi({
      listCoachReviews: vi
        .fn<HeadquartersApi["listCoachReviews"]>()
        .mockResolvedValueOnce({ ok: true, data: { coach_id: COACH_ID, reviews: [REVIEW] } })
        .mockResolvedValueOnce({ ok: false, message: "サーバーに接続できません。" }),
    });
    const user = userEvent.setup();
    render(<CoachReviewPicker api={api} />);
    const select = await screen.findByLabelText("コーチ");

    await user.selectOptions(select, "山田コーチ");
    await screen.findByLabelText("評価の件数と平均");
    await user.selectOptions(select, "佐藤コーチ");

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(screen.queryByLabelText("評価の件数と平均")).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "評価の一覧" })).not.toBeInTheDocument();
  });

  test("選び直したら、前に選んだコーチの遅れて返った結果では上書きしない", async () => {
    let resolveFirst: (value: Awaited<ReturnType<HeadquartersApi["getCoachReviewSummary"]>>) => void =
      () => {};
    const api = mockApi({
      getCoachReviewSummary: vi
        .fn<HeadquartersApi["getCoachReviewSummary"]>()
        .mockImplementationOnce(() => new Promise((resolve) => (resolveFirst = resolve)))
        .mockImplementation(async (coachId) => ({
          ok: true,
          data: { coach_id: coachId, review_count: 1, average_rating: 5 },
        })),
    });
    const user = userEvent.setup();
    render(<CoachReviewPicker api={api} />);
    const select = await screen.findByLabelText("コーチ");

    await user.selectOptions(select, "山田コーチ");
    await user.selectOptions(select, "佐藤コーチ");
    const summary = await screen.findByLabelText("評価の件数と平均");
    resolveFirst({ ok: true, data: { coach_id: COACH_ID, review_count: 9, average_rating: 2 } });

    await waitFor(() => expect(api.getCoachReviewSummary).toHaveBeenCalledTimes(2));
    expect(summaryOf(summary)).toContainEqual(["コーチ ID", OTHER_COACH_ID]);
    expect(summaryOf(screen.getByLabelText("評価の件数と平均"))).toContainEqual(["評価の件数", "1 件"]);
  });

  test("initialCoachId がコーチの一覧にあれば、そのコーチを選んだ状態で評価を読む", async () => {
    const api = mockApi();
    render(<CoachReviewPicker api={api} initialCoachId={COACH_ID} />);

    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toContainEqual(["コーチ ID", COACH_ID]);
    expect(screen.getByLabelText("コーチ")).toHaveValue(COACH_ID);
    expect(api.getCoachReviewSummary).toHaveBeenCalledTimes(1);
    expect(api.listCoachReviews).toHaveBeenCalledWith(COACH_ID);
  });

  test("initialCoachId がコーチの一覧になければ「選んでください」のままにし、評価は読まない", async () => {
    const api = mockApi();
    render(<CoachReviewPicker api={api} initialCoachId="99999999-9999-4999-8999-999999999999" />);

    expect(await screen.findByLabelText("コーチ")).toHaveValue("");
    expect(api.getCoachReviewSummary).not.toHaveBeenCalled();
    expect(api.listCoachReviews).not.toHaveBeenCalled();
  });

  test("平均は API が丸めた値を小数第 2 位まで揃えて出す", () => {
    expect(formatAverageRating(4)).toBe("4.00");
    expect(formatAverageRating(3.67)).toBe("3.67");
    expect(formatAverageRating(null)).toBe("評価はまだありません");
  });
});

describe("画面への配置", () => {
  test("本部の画面からコーチの評価の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "評価の件数と平均を見る" })).toHaveAttribute(
      "href",
      "/headquarters/coach-review-summary",
    );
  });

  test("本部のコーチの評価の画面は、コーチを一覧から選んで見る", async () => {
    headquartersApiMock.mockReturnValue(mockApi());
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <CoachReviewSummaryHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "コーチの評価" });
    expect(await within(region).findByLabelText("コーチ")).toBeInTheDocument();
    expect(within(region).queryByLabelText("コーチ ID")).not.toBeInTheDocument();
    expect(within(region).queryByText(/未実装/)).not.toBeInTheDocument();
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは本部のコーチの評価の画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue(mockApi());
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <CoachReviewSummaryHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "コーチの評価" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
