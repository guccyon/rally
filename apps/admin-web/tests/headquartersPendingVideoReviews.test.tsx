import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { PendingVideoReviewList } from "@/components/headquarters/PendingVideoReviewList";
import { PendingVideoReviewsHome } from "@/components/headquarters/PendingVideoReviewsHome";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは PendingVideoReviewsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const AS_OF = "2026-10-26T03:00:00Z";

// API が返す並び（送った時刻の古い順）。画面はこの順のまま出す。
const VIDEOS: Schemas["PendingVideoReview"][] = [
  {
    video_id: "55555555-5555-4555-8555-000000000001",
    sport: "golf",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "佐藤 太郎",
    coach_id: "22222222-2222-4222-8222-000000000001",
    coach_display_name: "高橋 コーチ",
    submitted_at: "2026-10-22T09:30:00Z",
    elapsed_hours: 89,
  },
  {
    video_id: "55555555-5555-4555-8555-000000000002",
    sport: "tennis",
    student_id: "11111111-1111-4111-8111-000000000002",
    coach_id: "22222222-2222-4222-8222-000000000002",
    submitted_at: "2026-10-25T20:10:00Z",
    elapsed_hours: 6,
  },
];

type Response = Awaited<ReturnType<HeadquartersApi["listPendingVideoReviews"]>>;

function listOf(videos: Schemas["PendingVideoReview"][]) {
  return vi.fn<HeadquartersApi["listPendingVideoReviews"]>(async () => ({
    ok: true,
    data: { as_of: AS_OF, videos },
  }));
}

function cellsOf(row: HTMLElement): (string | null)[] {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

describe("本部の返却待ちの動画添削", () => {
  test("読み込み中を出し、開いたときに条件なしで読み、各行に生徒・コーチ・スポーツ・送った日時・経過を API の順で出す", async () => {
    const listPendingVideoReviews = listOf(VIDEOS);
    render(<PendingVideoReviewList api={{ listPendingVideoReviews }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "返却待ちの動画添削の一覧" });
    expect(listPendingVideoReviews).toHaveBeenCalledTimes(1);
    expect(listPendingVideoReviews).toHaveBeenCalledWith({});
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      ["佐藤 太郎", "高橋 コーチ", "ゴルフ", formatDateTime("2026-10-22T09:30:00Z"), "89 時間"],
      [
        "（名前を表示できません）",
        "（名前を表示できません）",
        "テニス",
        formatDateTime("2026-10-25T20:10:00Z"),
        "6 時間",
      ],
    ]);
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("経過時間がいつ時点のものかを応答の as_of で添える", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(VIDEOS) }} />);

    expect(
      await screen.findByText(`${formatDateTime(AS_OF)} 時点の経過時間です（1 時間に満たない端数は切り捨て）。`),
    ).toBeInTheDocument();
  });

  test("経過時間の下限は「すべて」と 24・48・72・168 時間以上から選べる", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(VIDEOS) }} />);
    await screen.findByRole("table");

    const options = within(screen.getByLabelText("送ってからの経過")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "すべて",
      "24 時間以上",
      "48 時間以上",
      "72 時間以上",
      "168 時間以上",
    ]);
    expect(within(screen.getByLabelText("スポーツ")).getAllByRole("option")).toHaveLength(7);
  });

  test("経過時間の下限とスポーツの絞り込みを API の min_elapsed_hours / sport に渡して読み直す", async () => {
    const listPendingVideoReviews = listOf(VIDEOS);
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("送ってからの経過"), "48 時間以上");
    await waitFor(() =>
      expect(listPendingVideoReviews).toHaveBeenLastCalledWith({ min_elapsed_hours: 48 }),
    );
    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    await waitFor(() =>
      expect(listPendingVideoReviews).toHaveBeenLastCalledWith({
        sport: "golf",
        min_elapsed_hours: 48,
      }),
    );
    await user.selectOptions(screen.getByLabelText("送ってからの経過"), "すべて");
    await waitFor(() => expect(listPendingVideoReviews).toHaveBeenLastCalledWith({ sport: "golf" }));
    expect(listPendingVideoReviews).toHaveBeenCalledTimes(4);
  });

  test("後から変えた条件の結果だけを出す（前の条件の遅れた結果で上書きしない）", async () => {
    let resolveSlow: (value: Response) => void = () => {};
    const listPendingVideoReviews = vi
      .fn<HeadquartersApi["listPendingVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: VIDEOS } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)))
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: [VIDEOS[0]!] } });
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    resolveSlow({ ok: true, data: { as_of: AS_OF, videos: [VIDEOS[1]!] } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.getByRole("row", { name: /佐藤 太郎/ })).toBeInTheDocument();
  });

  test("条件なしで 0 件なら「返却を待っている動画添削はありません」と出し、表は出さない", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf([]) }} />);

    expect(await screen.findByText("返却を待っている動画添削はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("絞り込んで 0 件なら「条件に合う返却待ちの動画添削はありません」と出す", async () => {
    const listPendingVideoReviews = vi
      .fn<HeadquartersApi["listPendingVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: VIDEOS } })
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: [] } });
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("送ってからの経過"), "168 時間以上");
    expect(await screen.findByText("条件に合う返却待ちの動画添削はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listPendingVideoReviews = vi.fn<HeadquartersApi["listPendingVideoReviews"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<PendingVideoReviewList api={{ listPendingVideoReviews }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("返却を待っている動画添削はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("本部の画面から返却待ちの動画添削の画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "返却待ちの動画添削" });
    expect(within(region).getByRole("link", { name: "返却の遅れている添削を見る" })).toHaveAttribute(
      "href",
      "/headquarters/pending-video-reviews",
    );
  });

  test("返却待ちの動画添削の画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({ listPendingVideoReviews: listOf(VIDEOS) });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <PendingVideoReviewsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "返却待ちの動画添削" });
    expect(
      await within(region).findByRole("table", { name: "返却待ちの動画添削の一覧" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listPendingVideoReviews: listOf(VIDEOS) });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <PendingVideoReviewsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "返却待ちの動画添削" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
