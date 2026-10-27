import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { CoachReviewSummaryLookup, formatAverageRating } from "@/components/CoachReviewSummaryLookup";
import { RoleGate } from "@/components/RoleGate";
import { CoachHome } from "@/components/coach/CoachHome";
import { CoachReviewSummaryHome } from "@/components/headquarters/CoachReviewSummaryHome";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const COACH_ID = "22222222-2222-4222-8222-222222222222";

/** 件数と平均の一覧（dl）を [見出し, 値] の組にする。 */
function summaryOf(list: HTMLElement) {
  return Array.from(list.querySelectorAll("dt")).map((dt) => [
    dt.textContent,
    dt.nextElementSibling?.textContent,
  ]);
}

describe("コーチの評価の件数と平均", () => {
  test("入力したコーチ ID で getCoachReviewSummary を呼び、件数と平均を出す", async () => {
    const getCoachReviewSummary = vi.fn<CoachApi["getCoachReviewSummary"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, review_count: 12, average_rating: 4.5 },
    }));
    const user = userEvent.setup();
    render(<CoachReviewSummaryLookup api={{ getCoachReviewSummary }} description="説明" />);

    await user.type(screen.getByLabelText("コーチ ID"), ` ${COACH_ID} `);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(getCoachReviewSummary).toHaveBeenCalledWith(COACH_ID);
    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toEqual([
      ["コーチ ID", COACH_ID],
      ["評価の件数", "12 件"],
      ["点の平均（1〜5）", "4.50"],
    ]);
  });

  test("評価が 0 件（平均が null）なら、平均はまだないと出す", async () => {
    const getCoachReviewSummary = vi.fn<CoachApi["getCoachReviewSummary"]>(async (coachId) => ({
      ok: true,
      data: { coach_id: coachId, review_count: 0, average_rating: null },
    }));
    const user = userEvent.setup();
    render(<CoachReviewSummaryLookup api={{ getCoachReviewSummary }} description="説明" />);

    await user.type(screen.getByLabelText("コーチ ID"), COACH_ID);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    const summary = await screen.findByLabelText("評価の件数と平均");
    expect(summaryOf(summary)).toEqual([
      ["コーチ ID", COACH_ID],
      ["評価の件数", "0 件"],
      ["点の平均（1〜5）", "評価はまだありません"],
    ]);
  });

  test("コーチ ID が空なら API を呼ばずに知らせる", async () => {
    const getCoachReviewSummary = vi.fn<CoachApi["getCoachReviewSummary"]>();
    const user = userEvent.setup();
    render(<CoachReviewSummaryLookup api={{ getCoachReviewSummary }} description="説明" />);

    await user.type(screen.getByLabelText("コーチ ID"), "   ");
    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(getCoachReviewSummary).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("コーチ ID を入力してください。");
  });

  test("API のエラー（他のコーチの分の 403 など）の文言を出し、前の結果は消す", async () => {
    const getCoachReviewSummary = vi
      .fn<CoachApi["getCoachReviewSummary"]>()
      .mockResolvedValueOnce({
        ok: true,
        data: { coach_id: COACH_ID, review_count: 3, average_rating: 3.67 },
      })
      .mockResolvedValueOnce({ ok: false, message: "権限がないためアクセスできません。" });
    const user = userEvent.setup();
    render(<CoachReviewSummaryLookup api={{ getCoachReviewSummary }} description="説明" />);

    await user.type(screen.getByLabelText("コーチ ID"), COACH_ID);
    await user.click(screen.getByRole("button", { name: "評価を見る" }));
    await screen.findByLabelText("評価の件数と平均");

    await user.click(screen.getByRole("button", { name: "評価を見る" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByLabelText("評価の件数と平均")).not.toBeInTheDocument();
  });

  test("平均は API が丸めた値を小数第 2 位まで揃えて出す", () => {
    expect(formatAverageRating(4)).toBe("4.00");
    expect(formatAverageRating(3.67)).toBe("3.67");
    expect(formatAverageRating(null)).toBe("評価はまだありません");
  });
});

describe("画面への配置", () => {
  test("コーチの画面に「自分の評価」を出す", () => {
    render(<CoachHome token="mock.coach.x" />);

    const region = screen.getByRole("region", { name: "自分の評価" });
    expect(within(region).getByLabelText("コーチ ID")).toBeInTheDocument();
  });

  test("本部の画面からコーチの評価の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "評価の件数と平均を見る" })).toHaveAttribute(
      "href",
      "/headquarters/coach-review-summary",
    );
  });

  test("本部のコーチの評価の画面は、コーチ ID を入力して見る", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <CoachReviewSummaryHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "コーチの評価" });
    expect(within(region).getByLabelText("コーチ ID")).toBeInTheDocument();
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは本部のコーチの評価の画面に入れず、ログイン画面へ戻す", () => {
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
