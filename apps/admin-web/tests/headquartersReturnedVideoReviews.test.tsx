import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { PendingVideoReviewsHome } from "@/components/headquarters/PendingVideoReviewsHome";
import { ReturnedVideoReviewList } from "@/components/headquarters/ReturnedVideoReviewList";
import { ReturnedVideoReviewsHome } from "@/components/headquarters/ReturnedVideoReviewsHome";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは ReturnedVideoReviewsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const COACH_1 = "22222222-2222-4222-8222-000000000001";
const COACH_2 = "22222222-2222-4222-8222-000000000002";

// API が返す並び（返した時刻の新しい順）。画面はこの順と値のまま出す。
const REVIEWS: Schemas["ReturnedVideoReview"][] = [
  {
    video_id: "55555555-5555-4555-8555-000000000001",
    sport: "golf",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "佐藤 太郎",
    coach_id: COACH_1,
    coach_display_name: "高橋 コーチ",
    submitted_at: "2026-10-22T09:30:00Z",
    returned_at: "2026-10-25T20:10:00Z",
    lead_time_hours: 82,
  },
  {
    video_id: "55555555-5555-4555-8555-000000000002",
    sport: "tennis",
    student_id: "11111111-1111-4111-8111-000000000002",
    coach_id: COACH_2,
    submitted_at: "2026-10-20T01:00:00Z",
    returned_at: "2026-10-20T01:59:00Z",
    lead_time_hours: 0,
  },
];

const COACHES: Schemas["Coach"][] = [
  { coach_id: COACH_1, display_name: "高橋 コーチ", acceptance_status: "accepting", recent_lesson_count: 3 },
  { coach_id: COACH_2, display_name: "鈴木 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

type Response = Awaited<ReturnType<HeadquartersApi["listReturnedVideoReviews"]>>;

function listOf(reviews: Schemas["ReturnedVideoReview"][], from = "2026-10-20", to = "2026-10-26") {
  return vi.fn<HeadquartersApi["listReturnedVideoReviews"]>(async () => ({
    ok: true,
    data: { from, to, reviews },
  }));
}

function coachesOf(coaches = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

function cellsOf(row: HTMLElement): (string | null)[] {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

async function changePeriod(user: ReturnType<typeof userEvent.setup>, from: string, to: string) {
  await user.clear(screen.getByLabelText("開始日"));
  await user.type(screen.getByLabelText("開始日"), from);
  await user.clear(screen.getByLabelText("終了日"));
  await user.type(screen.getByLabelText("終了日"), to);
}

describe("本部の返した動画添削", () => {
  test("読み込み中を出し、開いたときに期間を付けずに読み、各行に生徒・コーチ・スポーツ・送った日時・返した日時・かかった時間を API の順と値のまま出す", async () => {
    const listReturnedVideoReviews = listOf(REVIEWS);
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "返した動画添削の一覧" });
    expect(listReturnedVideoReviews).toHaveBeenCalledTimes(1);
    expect(listReturnedVideoReviews).toHaveBeenCalledWith({});
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      [
        "佐藤 太郎",
        "高橋 コーチ",
        "ゴルフ",
        formatDateTime("2026-10-22T09:30:00Z"),
        formatDateTime("2026-10-25T20:10:00Z"),
        "82 時間",
      ],
      [
        "（名前を表示できません）",
        "（名前を表示できません）",
        "テニス",
        formatDateTime("2026-10-20T01:00:00Z"),
        formatDateTime("2026-10-20T01:59:00Z"),
        "0 時間",
      ],
    ]);
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("API の既定で実際に使った期間を件数とともに出し、期間の欄にも入れる", async () => {
    render(
      <ReturnedVideoReviewList
        api={{ listReturnedVideoReviews: listOf(REVIEWS), listCoaches: coachesOf() }}
      />,
    );

    expect(await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・2 件")).toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-20");
    expect(screen.getByLabelText("終了日")).toHaveValue("2026-10-26");
  });

  test("コーチの選択肢は「すべて」とコーチの一覧の返った順", async () => {
    render(
      <ReturnedVideoReviewList
        api={{ listReturnedVideoReviews: listOf(REVIEWS), listCoaches: coachesOf() }}
      />,
    );
    await screen.findByRole("table");

    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());
    const options = within(screen.getByLabelText("コーチ")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["すべて", "高橋 コーチ", "鈴木 コーチ"]);
  });

  test("期間とコーチを選んで「この条件で見る」で API の from / to / coach_id に渡して読み直す", async () => {
    const listReturnedVideoReviews = listOf(REVIEWS);
    const user = userEvent.setup();
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await changePeriod(user, "2026-10-01", "2026-10-31");
    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    expect(listReturnedVideoReviews).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listReturnedVideoReviews).toHaveBeenLastCalledWith({
      from: "2026-10-01",
      to: "2026-10-31",
      coach_id: COACH_2,
    });

    await user.selectOptions(screen.getByLabelText("コーチ"), "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listReturnedVideoReviews).toHaveBeenLastCalledWith({ from: "2026-10-01", to: "2026-10-31" });
  });

  test("終了日が開始日より前・31 日を超える期間は送らずに理由を出す", async () => {
    const listReturnedVideoReviews = listOf(REVIEWS);
    const user = userEvent.setup();
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-20", "2026-10-19");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "終了日は開始日と同じか、それより後の日付にしてください。",
    );

    await changePeriod(user, "2026-09-01", "2026-10-02");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");
    expect(listReturnedVideoReviews).toHaveBeenCalledTimes(1);
  });

  test("API の 422 は、期間とコーチのどこを直すかを添えた理由を出し、一覧は出さない", async () => {
    const listReturnedVideoReviews = vi.fn<HeadquartersApi["listReturnedVideoReviews"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力の内容が正しくありません。",
    }));
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "期間かコーチの指定が正しくありません。期間は 1〜31 日で、終了日は開始日と同じかそれより後の日付にしてください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("後から変えた条件の結果だけを出す（前の条件の遅れた結果で上書きしない）", async () => {
    let resolveSlow: (value: Response) => void = () => {};
    const listReturnedVideoReviews = vi
      .fn<HeadquartersApi["listReturnedVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", reviews: REVIEWS } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)))
      .mockResolvedValueOnce({
        ok: true,
        data: { from: "2026-10-20", to: "2026-10-26", reviews: [REVIEWS[0]!] },
      });
    const user = userEvent.setup();
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    resolveSlow({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", reviews: [REVIEWS[1]!] } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.getByRole("row", { name: /佐藤 太郎/ })).toBeInTheDocument();
  });

  test("0 件なら「この期間に返した動画添削はありません」と出し、表は出さない", async () => {
    render(
      <ReturnedVideoReviewList api={{ listReturnedVideoReviews: listOf([]), listCoaches: coachesOf() }} />,
    );

    expect(await screen.findByText("この期間に返した動画添削はありません")).toBeInTheDocument();
    expect(screen.getByText("2026年10月20日（火） 〜 2026年10月26日（月）・0 件")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("コーチで絞って 0 件なら「この期間にこのコーチが返した動画添削はありません」と出す", async () => {
    const listReturnedVideoReviews = vi
      .fn<HeadquartersApi["listReturnedVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", reviews: REVIEWS } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", reviews: [] } });
    const user = userEvent.setup();
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(
      await screen.findByText("この期間にこのコーチが返した動画添削はありません"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listReturnedVideoReviews = vi.fn<HeadquartersApi["listReturnedVideoReviews"]>(async () => ({
      ok: false,
      code: "forbidden",
      message: "権限がないためアクセスできません。",
    }));
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("この期間に返した動画添削はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("コーチの一覧が読めなくても記録は出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<ReturnedVideoReviewList api={{ listReturnedVideoReviews: listOf(REVIEWS), listCoaches }} />);

    expect(await screen.findByRole("table", { name: "返した動画添削の一覧" })).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(screen.getByLabelText("コーチ")).toBeDisabled();
  });
});

describe("画面への配置", () => {
  test("本部の画面から返した動画添削の画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "返した動画添削" });
    expect(
      within(region).getByRole("link", { name: "返した添削とかかった時間を見る" }),
    ).toHaveAttribute("href", "/headquarters/returned-video-reviews");
  });

  test("返却待ちの動画添削の画面から返した動画添削の画面へ移れる", () => {
    headquartersApiMock.mockReturnValue({
      listPendingVideoReviews: vi.fn(async () => ({
        ok: true,
        data: { as_of: "2026-10-26T03:00:00Z", videos: [] },
      })),
    });
    render(<PendingVideoReviewsHome token="mock.headquarters.x" />);

    expect(screen.getByRole("link", { name: "返した動画添削の記録を見る" })).toHaveAttribute(
      "href",
      "/headquarters/returned-video-reviews",
    );
  });

  test("返した動画添削の画面は開いたときに記録を読み、本部の画面と返却待ちの画面へ移れる", async () => {
    headquartersApiMock.mockReturnValue({
      listReturnedVideoReviews: listOf(REVIEWS),
      listCoaches: coachesOf(),
    });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <ReturnedVideoReviewsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "返した動画添削" });
    expect(
      await within(region).findByRole("table", { name: "返した動画添削の一覧" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(screen.getByRole("link", { name: "返却待ちの動画添削を見る" })).toHaveAttribute(
      "href",
      "/headquarters/pending-video-reviews",
    );
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({
      listReturnedVideoReviews: listOf(REVIEWS),
      listCoaches: coachesOf(),
    });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <ReturnedVideoReviewsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "返した動画添削" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
