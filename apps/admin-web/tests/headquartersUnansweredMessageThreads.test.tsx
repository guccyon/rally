import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import {
  OLDER_THAN_HOURS_OPTIONS,
  UnansweredMessageThreadList,
} from "@/components/headquarters/UnansweredMessageThreadList";
import {
  UnansweredMessageThreadsHome,
  studentUnansweredMessageThreadsPath,
  unansweredMessageThreadFilterFrom,
  unansweredMessageThreadsPath,
} from "@/components/headquarters/UnansweredMessageThreadsHome";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは UnansweredMessageThreadsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const COACH_1 = "22222222-2222-4222-8222-000000000001";
const COACH_2 = "22222222-2222-4222-8222-000000000002";
const STUDENT_1 = "11111111-1111-4111-8111-000000000001";

// API が返す並び（待ち始めた時刻の古い順）。画面はこの順のまま出す。
const THREADS: Schemas["UnansweredMessageThread"][] = [
  {
    thread_id: "cccccccc-cccc-4ccc-8ccc-000000000001",
    coach_id: COACH_1,
    coach_display_name: "高橋 コーチ",
    student_id: STUDENT_1,
    student_display_name: "佐藤 太郎",
    waiting_since: "2026-10-22T09:30:00Z",
    // 89 時間 59 分 59 秒（時間に切り捨てる）
    elapsed_sec: 89 * 3600 + 3599,
  },
  {
    thread_id: "cccccccc-cccc-4ccc-8ccc-000000000002",
    coach_id: COACH_2,
    student_id: "11111111-1111-4111-8111-000000000002",
    waiting_since: "2026-10-25T03:00:00Z",
    elapsed_sec: 24 * 3600,
  },
];

const COACHES: Schemas["Coach"][] = [
  { coach_id: COACH_2, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 3 },
  { coach_id: COACH_1, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

type Response = Awaited<ReturnType<HeadquartersApi["listUnansweredMessageThreads"]>>;

function coachesOf(coaches = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

function listOf(threads: Schemas["UnansweredMessageThread"][], olderThanHours = 24) {
  return vi.fn<HeadquartersApi["listUnansweredMessageThreads"]>(async () => ({
    ok: true,
    data: { older_than_hours: olderThanHours, threads },
  }));
}

function cellsOf(row: HTMLElement): (string | null)[] {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

describe("本部の返事のないメッセージ", () => {
  test("読み込み中を出し、開いたときに条件なしで読み、各行に生徒・コーチ・待ち始めた日時・経過を API の順で出す", async () => {
    const listUnansweredMessageThreads = listOf(THREADS);
    render(<UnansweredMessageThreadList api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "返事のないメッセージの一覧" });
    expect(listUnansweredMessageThreads).toHaveBeenCalledTimes(1);
    expect(listUnansweredMessageThreads).toHaveBeenCalledWith({});
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      ["佐藤 太郎", "高橋 コーチ", formatDateTime("2026-10-22T09:30:00Z"), "89 時間"],
      ["（名前を表示できません）", "（名前を表示できません）", formatDateTime("2026-10-25T03:00:00Z"), "24 時間"],
    ]);
    expect(screen.getByText("読み込んだ時点の経過時間です（1 時間に満たない端数は切り捨て）。")).toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("返事を待っている時間は 24（既定）・48・72・168 時間以上から選べ、開いたときは既定を選んでいる", async () => {
    render(
      <UnansweredMessageThreadList api={{ listUnansweredMessageThreads: listOf(THREADS), listCoaches: coachesOf() }} />,
    );
    await screen.findByRole("table");

    const select = screen.getByLabelText("返事を待っている時間");
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "24 時間以上",
      "48 時間以上",
      "72 時間以上",
      "168 時間以上",
    ]);
    expect(select).toHaveValue("24");
  });

  test("時間を API の older_than_hours に渡して読み直し、既定の 24 時間に戻したら渡さない", async () => {
    const listUnansweredMessageThreads = listOf(THREADS);
    const user = userEvent.setup();
    render(<UnansweredMessageThreadList api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "72 時間以上");
    await waitFor(() => expect(listUnansweredMessageThreads).toHaveBeenLastCalledWith({ older_than_hours: 72 }));
    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "24 時間以上");
    await waitFor(() => expect(listUnansweredMessageThreads).toHaveBeenLastCalledWith({}));
    expect(listUnansweredMessageThreads).toHaveBeenCalledTimes(3);
  });

  test("後から変えた条件の結果だけを出す（前の条件の遅れた結果で上書きしない）", async () => {
    let resolveSlow: (value: Response) => void = () => {};
    const listUnansweredMessageThreads = vi
      .fn<HeadquartersApi["listUnansweredMessageThreads"]>()
      .mockResolvedValueOnce({ ok: true, data: { older_than_hours: 24, threads: THREADS } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)))
      .mockResolvedValueOnce({ ok: true, data: { older_than_hours: 72, threads: [THREADS[0]!] } });
    const user = userEvent.setup();
    render(<UnansweredMessageThreadList api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "48 時間以上");
    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "72 時間以上");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    resolveSlow({ ok: true, data: { older_than_hours: 48, threads: [THREADS[1]!] } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.getByRole("row", { name: /佐藤 太郎/ })).toBeInTheDocument();
  });

  test("条件なしで 0 件なら、使った時間（応答の older_than_hours）を添えた空の表示を出し、表は出さない", async () => {
    render(
      <UnansweredMessageThreadList api={{ listUnansweredMessageThreads: listOf([]), listCoaches: coachesOf() }} />,
    );

    expect(await screen.findByText("返事を待って 24 時間以上たったメッセージはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("時間で絞って 0 件なら、その時間を添えた空の表示を出す", async () => {
    const listUnansweredMessageThreads = vi
      .fn<HeadquartersApi["listUnansweredMessageThreads"]>()
      .mockResolvedValueOnce({ ok: true, data: { older_than_hours: 24, threads: THREADS } })
      .mockResolvedValueOnce({ ok: true, data: { older_than_hours: 168, threads: [] } });
    const user = userEvent.setup();
    render(<UnansweredMessageThreadList api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "168 時間以上");
    expect(await screen.findByText("返事を待って 168 時間以上たったメッセージはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listUnansweredMessageThreads = vi.fn<HeadquartersApi["listUnansweredMessageThreads"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<UnansweredMessageThreadList api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(/メッセージはありません/)).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("コーチの絞り込み", () => {
  test("コーチの選択肢は「すべて」とコーチの一覧の返った順で、読めるまでは選べない", async () => {
    render(
      <UnansweredMessageThreadList api={{ listUnansweredMessageThreads: listOf(THREADS), listCoaches: coachesOf() }} />,
    );

    expect(screen.getByLabelText("コーチ")).toBeDisabled();
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());
    const options = within(screen.getByLabelText("コーチ")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["すべて", "鈴木 コーチ", "高橋 コーチ"]);
  });

  test("選んだコーチを API の coach_id に渡して読み直し、時間と組み合わせられる", async () => {
    const listUnansweredMessageThreads = listOf(THREADS);
    const onFilterChange = vi.fn();
    const user = userEvent.setup();
    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }}
        onFilterChange={onFilterChange}
      />,
    );
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());
    expect(onFilterChange).not.toHaveBeenCalled();

    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    await waitFor(() => expect(listUnansweredMessageThreads).toHaveBeenLastCalledWith({ coach_id: COACH_1 }));
    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "48 時間以上");
    await waitFor(() =>
      expect(listUnansweredMessageThreads).toHaveBeenLastCalledWith({ older_than_hours: 48, coach_id: COACH_1 }),
    );
    expect(onFilterChange).toHaveBeenLastCalledWith({ olderThanHours: 48, coachId: COACH_1, studentId: "" });
    await user.selectOptions(screen.getByLabelText("コーチ"), "すべて");
    await waitFor(() => expect(listUnansweredMessageThreads).toHaveBeenLastCalledWith({ older_than_hours: 48 }));
    expect(listUnansweredMessageThreads).toHaveBeenCalledTimes(4);
  });

  test("渡された条件（URL のクエリ）で開き、選択欄にもその条件を出す", async () => {
    const listUnansweredMessageThreads = listOf(THREADS, 72);
    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: 72, coachId: COACH_1, studentId: "" }}
      />,
    );
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    expect(listUnansweredMessageThreads).toHaveBeenCalledTimes(1);
    expect(listUnansweredMessageThreads).toHaveBeenCalledWith({ older_than_hours: 72, coach_id: COACH_1 });
    expect(screen.getByLabelText("コーチ")).toHaveValue(COACH_1);
    expect(screen.getByLabelText("返事を待っている時間")).toHaveValue("72");
  });

  test("コーチで絞って 0 件なら、そのコーチであることと時間を添えた空の表示を出す", async () => {
    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads: listOf([], 48), listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: 48, coachId: COACH_2, studentId: "" }}
      />,
    );

    expect(
      await screen.findByText("このコーチのやり取りに、返事を待って 48 時間以上たったメッセージはありません"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("422（URL の coach_id が UUID でないなど）は、サーバーの文言に直す場所を添えて出す", async () => {
    const listUnansweredMessageThreads = vi.fn<HeadquartersApi["listUnansweredMessageThreads"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力内容を確認してください。",
    }));
    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: null, coachId: "not-a-uuid", studentId: "" }}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力内容を確認してください。返事を待っている時間・コーチは一覧から選び直してください。",
    );
    expect(listUnansweredMessageThreads).toHaveBeenCalledWith({ coach_id: "not-a-uuid" });
  });

  test("コーチの一覧が読めなくても一覧は出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<UnansweredMessageThreadList api={{ listUnansweredMessageThreads: listOf(THREADS), listCoaches }} />);

    expect(await screen.findByRole("table", { name: "返事のないメッセージの一覧" })).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(screen.getByLabelText("コーチ")).toBeDisabled();
    expect(screen.getByLabelText("返事を待っている時間")).toBeEnabled();
  });
});

describe("生徒の絞り込み", () => {
  test("生徒で絞って開くと API の student_id に渡し、時間・コーチを変えて読み直しても絞り込みは残る", async () => {
    const listUnansweredMessageThreads = listOf([THREADS[0]!]);
    const user = userEvent.setup();
    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: null, coachId: "", studentId: STUDENT_1 }}
      />,
    );
    await screen.findByRole("table");
    expect(listUnansweredMessageThreads).toHaveBeenCalledWith({ student_id: STUDENT_1 });
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "168 時間以上");
    await waitFor(() =>
      expect(listUnansweredMessageThreads).toHaveBeenLastCalledWith({
        older_than_hours: 168,
        coach_id: COACH_1,
        student_id: STUDENT_1,
      }),
    );
  });

  test("生徒だけ・生徒とコーチの両方で絞って 0 件なら、絞った相手と時間を添えた空の表示を出す", async () => {
    const { unmount } = render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads: listOf([]), listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: null, coachId: "", studentId: STUDENT_1 }}
      />,
    );
    expect(
      await screen.findByText("この生徒のやり取りに、返事を待って 24 時間以上たったメッセージはありません"),
    ).toBeInTheDocument();
    unmount();

    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads: listOf([], 72), listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: 72, coachId: COACH_1, studentId: STUDENT_1 }}
      />,
    );
    expect(
      await screen.findByText("このコーチとこの生徒のやり取りに、返事を待って 72 時間以上たったメッセージはありません"),
    ).toBeInTheDocument();
  });

  test("生徒で絞っているときの 422 は、生徒の選び直しも添えた理由を出す", async () => {
    const listUnansweredMessageThreads = vi.fn<HeadquartersApi["listUnansweredMessageThreads"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力内容を確認してください。",
    }));
    render(
      <UnansweredMessageThreadList
        api={{ listUnansweredMessageThreads, listCoaches: coachesOf() }}
        initialFilter={{ olderThanHours: null, coachId: "", studentId: "not-a-uuid" }}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力内容を確認してください。返事を待っている時間・コーチは一覧から選び直してください。生徒は生徒の一覧から選び直してください。",
    );
  });
});

describe("絞り込みの URL", () => {
  test("時間・コーチ・生徒を契約のクエリの名前で URL に残し、既定の時間と「すべて」の条件は出さない", () => {
    expect(unansweredMessageThreadsPath({ olderThanHours: null, coachId: "", studentId: "" })).toBe(
      "/headquarters/unanswered-messages",
    );
    expect(unansweredMessageThreadsPath({ olderThanHours: 48, coachId: COACH_1, studentId: STUDENT_1 })).toBe(
      `/headquarters/unanswered-messages?older_than_hours=48&coach_id=${COACH_1}&student_id=${STUDENT_1}`,
    );
    expect(studentUnansweredMessageThreadsPath(STUDENT_1)).toBe(
      `/headquarters/unanswered-messages?student_id=${STUDENT_1}`,
    );
  });

  test("時間の選択肢は凍結してあり、import した側から書き換えられない", () => {
    expect(Object.isFrozen(OLDER_THAN_HOURS_OPTIONS)).toBe(true);
  });

  test("URL のクエリから条件を読み、選択肢にない時間と既定の 24 時間はクエリなしと同じにする", () => {
    expect(
      unansweredMessageThreadFilterFrom(
        new URLSearchParams(`older_than_hours=168&coach_id=${COACH_2}&student_id=${STUDENT_1}`),
      ),
    ).toEqual({ olderThanHours: 168, coachId: COACH_2, studentId: STUDENT_1 });
    expect(unansweredMessageThreadFilterFrom(new URLSearchParams("older_than_hours=24"))).toEqual({
      olderThanHours: null,
      coachId: "",
      studentId: "",
    });
    expect(unansweredMessageThreadFilterFrom(new URLSearchParams("older_than_hours=5"))).toEqual({
      olderThanHours: null,
      coachId: "",
      studentId: "",
    });
  });
});

describe("画面への配置", () => {
  test("本部の画面から返事のないメッセージの画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "返事のないメッセージ" });
    expect(within(region).getByRole("link", { name: "返事の遅れているやり取りを見る" })).toHaveAttribute(
      "href",
      "/headquarters/unanswered-messages",
    );
  });

  test("返事のないメッセージの画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({ listUnansweredMessageThreads: listOf(THREADS), listCoaches: coachesOf() });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <UnansweredMessageThreadsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "返事のないメッセージ" });
    expect(await within(region).findByRole("table", { name: "返事のないメッセージの一覧" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(screen.queryByText(/^生徒で絞っています:/)).not.toBeInTheDocument();
    expect(replace).not.toHaveBeenCalled();
  });

  test("絞り込みを変えたら URL のクエリを置き換えて残す", async () => {
    headquartersApiMock.mockReturnValue({ listUnansweredMessageThreads: listOf(THREADS), listCoaches: coachesOf() });
    const user = userEvent.setup();
    render(<UnansweredMessageThreadsHome token="mock.headquarters.x" />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    expect(replace).toHaveBeenLastCalledWith(`/headquarters/unanswered-messages?coach_id=${COACH_1}`, {
      scroll: false,
    });
  });

  test("生徒で絞った画面は一覧の上に生徒の名前と、生徒の絞り込みだけを外すリンクを出す", async () => {
    const getStudentActivity = vi.fn<HeadquartersApi["getStudentActivity"]>(async () => ({
      ok: true,
      data: {
        student_id: STUDENT_1,
        display_name: "佐藤 太郎",
        upcoming_bookings: [],
        recent_lessons: [],
        cancelled_booking_count: 0,
        submitted_video_count: 0,
        pending_video_review_count: 0,
      },
    }));
    headquartersApiMock.mockReturnValue({
      listUnansweredMessageThreads: listOf([THREADS[0]!]),
      listCoaches: coachesOf(),
      getStudentActivity,
    });
    const user = userEvent.setup();
    render(
      <UnansweredMessageThreadsHome
        token="mock.headquarters.x"
        initialFilter={{ olderThanHours: null, coachId: "", studentId: STUDENT_1 }}
      />,
    );

    const region = screen.getByRole("region", { name: "返事のないメッセージ" });
    expect(await within(region).findByText(/^生徒で絞っています:/)).toHaveTextContent("生徒で絞っています: 佐藤 太郎");
    expect(screen.getByRole("link", { name: "すべての生徒の返事のないメッセージを見る" })).toHaveAttribute(
      "href",
      "/headquarters/unanswered-messages",
    );
    await within(region).findByRole("table");

    await user.selectOptions(screen.getByLabelText("返事を待っている時間"), "48 時間以上");
    expect(replace).toHaveBeenLastCalledWith(
      `/headquarters/unanswered-messages?older_than_hours=48&student_id=${STUDENT_1}`,
      { scroll: false },
    );
    expect(screen.getByRole("link", { name: "すべての生徒の返事のないメッセージを見る" })).toHaveAttribute(
      "href",
      "/headquarters/unanswered-messages?older_than_hours=48",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listUnansweredMessageThreads: listOf(THREADS), listCoaches: coachesOf() });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <UnansweredMessageThreadsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "返事のないメッセージ" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
