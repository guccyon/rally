import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { CoachProfileForm } from "@/components/coach/CoachProfileForm";

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const COACH_ID = "22222222-2222-4222-8222-222222222222";

const ACCOUNT = {
  account_id: COACH_ID,
  role: "coach" as const,
  display_name: "山田コーチ",
  bio: "テニス歴 20 年です。",
};

type ProfileApi = Pick<CoachApi, "getMyAccount" | "updateMyCoachProfile">;

function mockApi(overrides: Partial<ProfileApi> = {}) {
  return {
    getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({ ok: true, data: ACCOUNT })),
    updateMyCoachProfile: vi.fn<CoachApi["updateMyCoachProfile"]>(async (body) => ({
      ok: true,
      data: { ...ACCOUNT, ...body },
    })),
    ...overrides,
  };
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("自己紹介", () => {
  test("開いたときに本人の今の自己紹介文と残りの文字数を出す", async () => {
    const api = mockApi();
    render(<CoachProfileForm api={api} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const textarea = await screen.findByLabelText("自己紹介文");
    expect(textarea).toHaveValue("テニス歴 20 年です。");
    expect(screen.getByText("あと 388 文字")).toBeInTheDocument();
    expect(api.updateMyCoachProfile).not.toHaveBeenCalled();
  });

  test("まだ書いていない（bio が空・ない）ときは空の欄と 400 文字を出す", async () => {
    const withoutBio = { account_id: COACH_ID, role: "coach" as const, display_name: "山田コーチ" };
    const api = mockApi({
      getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({ ok: true, data: withoutBio })),
    });
    render(<CoachProfileForm api={api} />);

    expect(await screen.findByLabelText("自己紹介文")).toHaveValue("");
    expect(screen.getByText("あと 400 文字")).toBeInTheDocument();
  });

  test("編集した文を updateMyCoachProfile に渡して保存し、その旨を出す", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<CoachProfileForm api={api} />);

    const textarea = await screen.findByLabelText("自己紹介文");
    await user.clear(textarea);
    await user.type(textarea, " 初心者歓迎です ");
    expect(screen.getByText("あと 391 文字")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保存する" }));

    // 前後の空白も削らずに送る（契約: 書いたとおりに保存する）。
    expect(api.updateMyCoachProfile).toHaveBeenCalledWith({ bio: " 初心者歓迎です " });
    expect(await screen.findByRole("status")).toHaveTextContent("自己紹介を保存しました。");
    expect(textarea).toHaveValue(" 初心者歓迎です ");
  });

  test("空にして保存すると空文字列を送る（消せる）", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<CoachProfileForm api={api} />);

    await user.clear(await screen.findByLabelText("自己紹介文"));
    await user.click(screen.getByRole("button", { name: "保存する" }));

    expect(api.updateMyCoachProfile).toHaveBeenCalledWith({ bio: "" });
    expect(await screen.findByRole("status")).toHaveTextContent("自己紹介を保存しました。");
  });

  test("400 文字ちょうどなら保存できる（絵文字などもコードポイントで 1 文字と数える）", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<CoachProfileForm api={api} />);

    const text = "🎾".repeat(400);
    fireEvent.change(await screen.findByLabelText("自己紹介文"), { target: { value: text } });
    expect(screen.getByText("あと 0 文字")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保存する" }));

    expect(api.updateMyCoachProfile).toHaveBeenCalledWith({ bio: text });
  });

  test("400 文字を超えたら超えた数を出し、保存しても API を呼ばずに知らせる", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<CoachProfileForm api={api} />);

    fireEvent.change(await screen.findByLabelText("自己紹介文"), {
      target: { value: "あ".repeat(403) },
    });
    expect(screen.getByText("400 文字を 3 文字超えています")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "保存する" }));

    expect(api.updateMyCoachProfile).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "自己紹介は 400 文字までです。3 文字減らしてください。",
    );
  });

  test("保存の API のエラーの文言を出し、書いた文は欄に残す", async () => {
    const api = mockApi({
      updateMyCoachProfile: vi.fn<CoachApi["updateMyCoachProfile"]>(async () => ({
        ok: false,
        message: "入力の内容が正しくありません。",
      })),
    });
    const user = userEvent.setup();
    render(<CoachProfileForm api={api} />);

    const textarea = await screen.findByLabelText("自己紹介文");
    await user.type(textarea, "追記");
    await user.click(screen.getByRole("button", { name: "保存する" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("入力の内容が正しくありません。");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(textarea).toHaveValue("テニス歴 20 年です。追記");
  });

  test("本人のアカウントが取れなければ（401 など）その文言だけを出し、欄は出さない", async () => {
    const api = mockApi({
      getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({
        ok: false,
        message: "トークンがないか無効です。ログインし直してください。",
      })),
    });
    render(<CoachProfileForm api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "トークンがないか無効です。ログインし直してください。",
    );
    expect(screen.queryByLabelText("自己紹介文")).not.toBeInTheDocument();
  });

  test("本人のアカウントがコーチでなければ、その旨だけを出す", async () => {
    const api = mockApi({
      getMyAccount: vi.fn<CoachApi["getMyAccount"]>(async () => ({
        ok: true,
        data: { account_id: COACH_ID, role: "headquarters", display_name: "本部" },
      })),
    });
    render(<CoachProfileForm api={api} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチのアカウントでログインしたときだけ、自己紹介を書けます。",
    );
    expect(screen.queryByLabelText("自己紹介文")).not.toBeInTheDocument();
  });

  test("コーチの画面に「自己紹介」の欄があり、今の自己紹介文を出す", async () => {
    coachApiMock.mockReturnValue({
      ...mockApi(),
      // 同じ画面の「予約枠」と「動画の添削」と「通話への参加」が開いたときに呼ぶ。
      listMyCoachSlots: vi.fn<CoachApi["listMyCoachSlots"]>(async () => ({
        ok: true,
        data: { slots: [] },
      })),
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
      getCoachReviewSummary: vi.fn<CoachApi["getCoachReviewSummary"]>(async (coachId) => ({
        ok: true,
        data: { coach_id: coachId, review_count: 0, average_rating: null },
      })),
      listCoachReviews: vi.fn<CoachApi["listCoachReviews"]>(async (coachId) => ({
        ok: true,
        data: { coach_id: coachId, reviews: [] },
      })),
    });
    render(<CoachHome token="mock.coach.x" />);
    const region = screen.getByRole("region", { name: "自己紹介" });

    expect(await within(region).findByLabelText("自己紹介文")).toHaveValue("テニス歴 20 年です。");
  });
});
