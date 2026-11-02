import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { PendingVideoReviewList } from "@/components/headquarters/PendingVideoReviewList";
import {
  PendingVideoReviewsHome,
  pendingVideoReviewFilterFrom,
  pendingVideoReviewsPath,
} from "@/components/headquarters/PendingVideoReviewsHome";
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

const COACH_1 = "22222222-2222-4222-8222-000000000001";
const COACH_2 = "22222222-2222-4222-8222-000000000002";

// コーチの一覧（listCoaches）が返す並び。選択肢はこの順のまま出す。
const COACHES: Schemas["Coach"][] = [
  { coach_id: COACH_2, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 3 },
  { coach_id: COACH_1, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

type Response = Awaited<ReturnType<HeadquartersApi["listPendingVideoReviews"]>>;

function coachesOf(coaches = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

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
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "返却待ちの動画添削の一覧" });
    expect(listPendingVideoReviews).toHaveBeenCalledTimes(1);
    expect(listPendingVideoReviews).toHaveBeenCalledWith({});
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      ["佐藤 太郎", "高橋 コーチ", "ゴルフ", formatDateTime("2026-10-22T09:30:00Z"), "89 時間48 時間超"],
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

  test("経過が 48 時間以上の行だけを薄い赤と「48 時間超」の札で目立たせ、一覧の上に件数を出す", async () => {
    const videos = [
      { ...VIDEOS[0]!, video_id: "55555555-5555-4555-8555-000000000011", elapsed_hours: 89 },
      // ちょうど 48 時間は含める
      { ...VIDEOS[0]!, video_id: "55555555-5555-4555-8555-000000000012", elapsed_hours: 48 },
      { ...VIDEOS[1]!, video_id: "55555555-5555-4555-8555-000000000013", elapsed_hours: 47 },
      { ...VIDEOS[1]!, video_id: "55555555-5555-4555-8555-000000000014", elapsed_hours: 6 },
    ];
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(videos), listCoaches: coachesOf() }} />);
    const table = await screen.findByRole("table", { name: "返却待ちの動画添削の一覧" });

    expect(screen.getByText("48 時間以上たったものが 2 件あります。")).toBeInTheDocument();
    const [h89, h48, h47, h6] = within(table).getAllByRole("row").slice(1) as [
      HTMLElement,
      HTMLElement,
      HTMLElement,
      HTMLElement,
    ];
    for (const row of [h89, h48]) {
      expect(row).toHaveClass("bg-danger-container");
      expect(within(row).getByText("48 時間超")).toHaveClass("bg-danger");
    }
    for (const row of [h47, h6]) {
      expect(row).not.toHaveClass("bg-danger-container");
      expect(within(row).queryByText("48 時間超")).not.toBeInTheDocument();
    }
  });

  test("48 時間以上たったものがなければ、件数の注意は出さない", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf([VIDEOS[1]!]), listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    expect(screen.queryByText(/時間以上たったものが/)).not.toBeInTheDocument();
    expect(screen.queryByText("48 時間超")).not.toBeInTheDocument();
  });

  test("絞り込んで読み直したら、件数の注意も読み直した一覧で数え直す", async () => {
    const listPendingVideoReviews = vi
      .fn<HeadquartersApi["listPendingVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: VIDEOS } })
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: [VIDEOS[1]!] } });
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);
    expect(await screen.findByText("48 時間以上たったものが 1 件あります。")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.queryByText(/時間以上たったものが/)).not.toBeInTheDocument();
  });

  test("経過時間がいつ時点のものかを応答の as_of で添える", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() }} />);

    expect(
      await screen.findByText(`${formatDateTime(AS_OF)} 時点の経過時間です（1 時間に満たない端数は切り捨て）。`),
    ).toBeInTheDocument();
  });

  test("経過時間の下限は「すべて」と 24・48・72・168 時間以上から選べる", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() }} />);
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
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);
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
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    resolveSlow({ ok: true, data: { as_of: AS_OF, videos: [VIDEOS[1]!] } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.getByRole("row", { name: /佐藤 太郎/ })).toBeInTheDocument();
  });

  test("条件なしで 0 件なら「返却を待っている動画添削はありません」と出し、表は出さない", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf([]), listCoaches: coachesOf() }} />);

    expect(await screen.findByText("返却を待っている動画添削はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(/時間以上たったものが/)).not.toBeInTheDocument();
  });

  test("絞り込んで 0 件なら「条件に合う返却待ちの動画添削はありません」と出す", async () => {
    const listPendingVideoReviews = vi
      .fn<HeadquartersApi["listPendingVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: VIDEOS } })
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: [] } });
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);
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
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("返却を待っている動画添削はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("コーチの絞り込み", () => {
  test("コーチの選択肢は「すべて」とコーチの一覧の返った順で、読めるまでは選べない", async () => {
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() }} />);

    expect(screen.getByLabelText("コーチ")).toBeDisabled();
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());
    const options = within(screen.getByLabelText("コーチ")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["すべて", "鈴木 コーチ", "高橋 コーチ"]);
  });

  test("選んだコーチを API の coach_id に渡して読み直し、スポーツ・経過時間と組み合わせられる", async () => {
    const listPendingVideoReviews = listOf(VIDEOS);
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    await waitFor(() => expect(listPendingVideoReviews).toHaveBeenLastCalledWith({ coach_id: COACH_1 }));
    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    await user.selectOptions(screen.getByLabelText("送ってからの経過"), "48 時間以上");
    await waitFor(() =>
      expect(listPendingVideoReviews).toHaveBeenLastCalledWith({
        sport: "golf",
        min_elapsed_hours: 48,
        coach_id: COACH_1,
      }),
    );
    await user.selectOptions(screen.getByLabelText("コーチ"), "すべて");
    await waitFor(() =>
      expect(listPendingVideoReviews).toHaveBeenLastCalledWith({ sport: "golf", min_elapsed_hours: 48 }),
    );
    expect(listPendingVideoReviews).toHaveBeenCalledTimes(5);
  });

  test("絞り込みを変えるたびに onFilterChange で条件を知らせる", async () => {
    const onFilterChange = vi.fn();
    const user = userEvent.setup();
    render(
      <PendingVideoReviewList
        api={{ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() }}
        onFilterChange={onFilterChange}
      />,
    );
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());
    expect(onFilterChange).not.toHaveBeenCalled();

    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    expect(onFilterChange).toHaveBeenLastCalledWith({ sport: "", minElapsedHours: null, coachId: COACH_2 });
    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    expect(onFilterChange).toHaveBeenLastCalledWith({ sport: "tennis", minElapsedHours: null, coachId: COACH_2 });
  });

  test("渡された条件（URL のクエリ）で開き、選択欄にもその条件を出す", async () => {
    const listPendingVideoReviews = listOf(VIDEOS);
    render(
      <PendingVideoReviewList
        api={{ listPendingVideoReviews, listCoaches: coachesOf() }}
        initialFilter={{ sport: "golf", minElapsedHours: 72, coachId: COACH_1 }}
      />,
    );
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    expect(listPendingVideoReviews).toHaveBeenCalledTimes(1);
    expect(listPendingVideoReviews).toHaveBeenCalledWith({ sport: "golf", min_elapsed_hours: 72, coach_id: COACH_1 });
    expect(screen.getByLabelText("コーチ")).toHaveValue(COACH_1);
    expect(screen.getByLabelText("スポーツ")).toHaveValue("golf");
    expect(screen.getByLabelText("送ってからの経過")).toHaveValue("72");
  });

  test("コーチだけで絞って 0 件なら「このコーチの返却待ちの動画添削はありません」と出す", async () => {
    const listPendingVideoReviews = vi
      .fn<HeadquartersApi["listPendingVideoReviews"]>()
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: VIDEOS } })
      .mockResolvedValueOnce({ ok: true, data: { as_of: AS_OF, videos: [] } });
    const user = userEvent.setup();
    render(<PendingVideoReviewList api={{ listPendingVideoReviews, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    expect(await screen.findByText("このコーチの返却待ちの動画添削はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("コーチとほかの条件で絞って 0 件なら「条件に合う返却待ちの動画添削はありません」と出す", async () => {
    render(
      <PendingVideoReviewList
        api={{ listPendingVideoReviews: listOf([]), listCoaches: coachesOf() }}
        initialFilter={{ sport: "tennis", minElapsedHours: null, coachId: COACH_2 }}
      />,
    );

    expect(await screen.findByText("条件に合う返却待ちの動画添削はありません")).toBeInTheDocument();
  });

  test("422（URL の coach_id が UUID でないなど）は、サーバーの文言に直す場所を添えて出す", async () => {
    const listPendingVideoReviews = vi.fn<HeadquartersApi["listPendingVideoReviews"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力内容を確認してください。",
    }));
    render(
      <PendingVideoReviewList
        api={{ listPendingVideoReviews, listCoaches: coachesOf() }}
        initialFilter={{ sport: "", minElapsedHours: null, coachId: "not-a-uuid" }}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力内容を確認してください。スポーツ・送ってからの経過・コーチは一覧から選び直してください。",
    );
    expect(listPendingVideoReviews).toHaveBeenCalledWith({ coach_id: "not-a-uuid" });
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("コーチの一覧が読めなくても一覧は出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<PendingVideoReviewList api={{ listPendingVideoReviews: listOf(VIDEOS), listCoaches }} />);

    expect(await screen.findByRole("table", { name: "返却待ちの動画添削の一覧" })).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(screen.getByLabelText("コーチ")).toBeDisabled();
    expect(screen.getByLabelText("スポーツ")).toBeEnabled();
  });
});

describe("絞り込みの URL", () => {
  test("スポーツ・経過時間・コーチを契約のクエリの名前で URL に残し、「すべて」の条件は出さない", () => {
    expect(pendingVideoReviewsPath({ sport: "", minElapsedHours: null, coachId: "" })).toBe(
      "/headquarters/pending-video-reviews",
    );
    expect(pendingVideoReviewsPath({ sport: "", minElapsedHours: null, coachId: COACH_1 })).toBe(
      `/headquarters/pending-video-reviews?coach_id=${COACH_1}`,
    );
    expect(pendingVideoReviewsPath({ sport: "tennis", minElapsedHours: 48, coachId: COACH_1 })).toBe(
      `/headquarters/pending-video-reviews?sport=tennis&min_elapsed_hours=48&coach_id=${COACH_1}`,
    );
  });

  test("URL のクエリから条件を読み、選択肢にないスポーツ・経過時間は「すべて」にする", () => {
    expect(
      pendingVideoReviewFilterFrom(new URLSearchParams(`sport=golf&min_elapsed_hours=168&coach_id=${COACH_2}`)),
    ).toEqual({ sport: "golf", minElapsedHours: 168, coachId: COACH_2 });
    expect(pendingVideoReviewFilterFrom(new URLSearchParams(""))).toEqual({
      sport: "",
      minElapsedHours: null,
      coachId: "",
    });
    expect(pendingVideoReviewFilterFrom(new URLSearchParams("sport=cricket&min_elapsed_hours=5"))).toEqual({
      sport: "",
      minElapsedHours: null,
      coachId: "",
    });
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
    headquartersApiMock.mockReturnValue({ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() });
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

  test("絞り込みを変えたら URL のクエリを置き換えて残す", async () => {
    headquartersApiMock.mockReturnValue({ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() });
    const user = userEvent.setup();
    render(<PendingVideoReviewsHome token="mock.headquarters.x" />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    expect(replace).toHaveBeenLastCalledWith(`/headquarters/pending-video-reviews?coach_id=${COACH_1}`, {
      scroll: false,
    });
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listPendingVideoReviews: listOf(VIDEOS), listCoaches: coachesOf() });
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
