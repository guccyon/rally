import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { OverdueLiveSessionList } from "@/components/headquarters/OverdueLiveSessionList";
import { OverdueLiveSessionsHome, overdueLiveSessionsPath } from "@/components/headquarters/OverdueLiveSessionsHome";
import { formatDateTime, formatElapsed } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは OverdueLiveSessionsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const SATO = "22222222-2222-4222-8222-222222222222";
const TAKAHASHI = "77777777-7777-4777-8777-777777777777";

// コーチの一覧（listCoaches）の返った順。選択肢はこの順のまま出す。
const COACHES: Schemas["Coach"][] = [
  { coach_id: SATO, display_name: "佐藤 健", acceptance_status: "accepting", recent_lesson_count: 12 },
  { coach_id: TAKAHASHI, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

function coachesOf(coaches: Schemas["Coach"][] = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

// API が返す並び（開始の古い順）。画面はこの順のまま出す。
const SESSIONS: Schemas["OverdueLiveLessonSession"][] = [
  {
    session_id: "55555555-5555-4555-8555-000000000001",
    booking_id: "66666666-6666-4666-8666-000000000001",
    coach_id: SATO,
    coach_display_name: "佐藤 健",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "山田 花子",
    started_at: "2026-10-30T01:00:00Z",
    elapsed_sec: 26 * 3600 + 5 * 60 + 59,
  },
  {
    session_id: "55555555-5555-4555-8555-000000000002",
    booking_id: "66666666-6666-4666-8666-000000000002",
    coach_id: TAKAHASHI,
    student_id: "11111111-1111-4111-8111-000000000002",
    started_at: "2026-10-31T00:30:00Z",
    elapsed_sec: 3 * 3600,
  },
];

function listOf(sessions: Schemas["OverdueLiveLessonSession"][]) {
  return vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(async (filter = {}) => ({
    ok: true,
    data: { older_than_hours: filter.older_than_hours ?? 3, sessions },
  }));
}

describe("経過時間の表示", () => {
  test("秒を「N 時間 M 分」にし、分未満は切り捨てる", () => {
    expect(formatElapsed(0)).toBe("0 時間 0 分");
    expect(formatElapsed(3 * 3600 + 59)).toBe("3 時間 0 分");
    expect(formatElapsed(26 * 3600 + 5 * 60 + 59)).toBe("26 時間 5 分");
  });
});

describe("本部の終わっていない通話", () => {
  test("読み込み中を出し、読めたら既定の 3 時間で、コーチ・生徒・開始・経過を API の順で表にする", async () => {
    const listOverdueLiveLessonSessions = listOf(SESSIONS);
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    expect(listOverdueLiveLessonSessions).toHaveBeenCalledTimes(1);
    expect(listOverdueLiveLessonSessions).toHaveBeenCalledWith({ older_than_hours: 3 });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent))).toEqual([
      ["佐藤 健", "山田 花子", formatDateTime("2026-10-30T01:00:00Z"), "26 時間 5 分"],
      ["（名前を表示できません）", "（名前を表示できません）", formatDateTime("2026-10-31T00:30:00Z"), "3 時間 0 分"],
    ]);
    expect(screen.getByRole("combobox", { name: "開始からの経過" })).toHaveValue("3");
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("使った下限は応答の値を添える", async () => {
    const listOverdueLiveLessonSessions = vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(
      async () => ({ ok: true, data: { older_than_hours: 5, sessions: SESSIONS } }),
    );
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }} />);

    expect(
      await screen.findByText("開始から 5 時間以上たって、まだ終わっていない通話です。経過は読み込んだ時点のものです。"),
    ).toBeInTheDocument();
  });

  test("経過時間の下限を変えると、その値で読み直す", async () => {
    const user = userEvent.setup();
    const listOverdueLiveLessonSessions = listOf(SESSIONS);
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }} />);
    await screen.findByRole("table", { name: "終わっていない通話の一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "開始からの経過" }), "1 日（24 時間）以上");

    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 24 });
    expect(await screen.findByText(/開始から 24 時間以上たって/)).toBeInTheDocument();
  });

  test("選べる下限は契約の範囲（1〜168 時間）の中だけ", async () => {
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf([]), listCoaches: coachesOf() }} />);

    const values = within(screen.getByRole("combobox", { name: "開始からの経過" }))
      .getAllByRole("option")
      .map((option) => Number((option as HTMLOptionElement).value));
    expect(Math.min(...values)).toBeGreaterThanOrEqual(1);
    expect(Math.max(...values)).toBeLessThanOrEqual(168);
    expect(values).toContain(3);
  });

  test("下限を続けて変えたときは、最後の条件の結果だけを出す", async () => {
    const user = userEvent.setup();
    let resolveSlow: (() => void) | undefined;
    const listOverdueLiveLessonSessions = vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(
      async (filter = {}) => {
        const hours = filter.older_than_hours ?? 3;
        if (hours === 1) await new Promise<void>((resolve) => (resolveSlow = resolve));
        return { ok: true, data: { older_than_hours: hours, sessions: hours === 1 ? SESSIONS : [] } };
      },
    );
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }} />);
    await screen.findByText("終わっていない通話はありません");

    const select = screen.getByRole("combobox", { name: "開始からの経過" });
    await user.selectOptions(select, "1 時間以上");
    await user.selectOptions(select, "6 時間以上");
    await screen.findByText(/開始から 6 時間以上たって/);
    resolveSlow?.();

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.getByText(/開始から 6 時間以上たって/)).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
  });

  test("0 件なら「終わっていない通話はありません」と出し、表は出さない", async () => {
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf([]), listCoaches: coachesOf() }} />);

    expect(await screen.findByText("終わっていない通話はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、表は出さない", async () => {
    const listOverdueLiveLessonSessions = vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(
      async () => ({ ok: false, message: "権限がないためアクセスできません。" }),
    );
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("終わっていない通話はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("セッションを終わらせる操作は出さない", async () => {
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() }} />);

    await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("コーチで絞る", () => {
  test("コーチの選択肢は「すべて」とコーチの一覧の返った順で、既定は「すべて」", async () => {
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() }} />);

    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());
    expect(select).toHaveValue("");
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "佐藤 健",
      "高橋 コーチ",
    ]);
  });

  test("コーチを選ぶと API の coach_id に渡し、経過時間と組み合わせられる。「すべて」に戻せば外す", async () => {
    const user = userEvent.setup();
    const listOverdueLiveLessonSessions = listOf(SESSIONS);
    const onCoachChange = vi.fn();
    render(
      <OverdueLiveSessionList
        api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }}
        onCoachChange={onCoachChange}
      />,
    );
    await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());

    await user.selectOptions(select, "高橋 コーチ");
    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 3, coach_id: TAKAHASHI });
    expect(onCoachChange).toHaveBeenLastCalledWith(TAKAHASHI);

    await user.selectOptions(screen.getByRole("combobox", { name: "開始からの経過" }), "1 日（24 時間）以上");
    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 24, coach_id: TAKAHASHI });
    // コーチを変えていないので知らせない
    expect(onCoachChange).toHaveBeenCalledTimes(1);

    await user.selectOptions(select, "すべて");
    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 24 });
    expect(onCoachChange).toHaveBeenLastCalledWith("");
    expect(onCoachChange).toHaveBeenCalledTimes(2);
  });

  test("渡されたコーチで絞って開き、そのコーチを選んだ状態にする", async () => {
    const listOverdueLiveLessonSessions = listOf([SESSIONS[0]!]);
    render(
      <OverdueLiveSessionList
        api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }}
        initialCoachId={SATO}
      />,
    );

    await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    expect(listOverdueLiveLessonSessions).toHaveBeenCalledTimes(1);
    expect(listOverdueLiveLessonSessions).toHaveBeenCalledWith({ older_than_hours: 3, coach_id: SATO });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toHaveValue(SATO));
  });

  test("渡されたコーチが一覧になければ「（一覧にないコーチ）」を選んだ状態で出し、「すべて」に戻せる", async () => {
    const unknown = "99999999-9999-4999-8999-999999999999";
    const user = userEvent.setup();
    const listOverdueLiveLessonSessions = vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(
      async (filter = {}) => ({
        ok: true,
        data: { older_than_hours: filter.older_than_hours ?? 3, sessions: filter.coach_id ? [] : SESSIONS },
      }),
    );
    render(
      <OverdueLiveSessionList
        api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }}
        initialCoachId={unknown}
      />,
    );

    expect(await screen.findByText("このコーチの終わっていない通話はありません")).toBeInTheDocument();
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toHaveValue(unknown));
    expect(within(select).getByRole("option", { selected: true })).toHaveTextContent("（一覧にないコーチ）");

    await user.selectOptions(select, "すべて");
    expect(await screen.findByRole("table", { name: "終わっていない通話の一覧" })).toBeInTheDocument();
    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 3 });
    expect(within(select).queryByRole("option", { name: "（一覧にないコーチ）" })).not.toBeInTheDocument();
  });

  test("コーチで絞って 0 件なら「このコーチの終わっていない通話はありません」と出し、表は出さない", async () => {
    const user = userEvent.setup();
    const listOverdueLiveLessonSessions = listOf(SESSIONS);
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }} />);
    await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());

    listOverdueLiveLessonSessions.mockResolvedValueOnce({ ok: true, data: { older_than_hours: 3, sessions: [] } });
    await user.selectOptions(select, "高橋 コーチ");
    expect(await screen.findByText("このコーチの終わっていない通話はありません")).toBeInTheDocument();
    expect(screen.queryByText("終わっていない通話はありません")).not.toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
  });

  test("422 なら汎用の文言に直す場所（コーチを選び直す）を添えて出し、表は出さない", async () => {
    const listOverdueLiveLessonSessions = vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(async () => ({
      ok: false,
      message: "入力の内容が正しくありません。",
      code: "validation_failed",
    }));
    render(
      <OverdueLiveSessionList
        api={{ listOverdueLiveLessonSessions, listCoaches: coachesOf() }}
        initialCoachId="not-a-uuid"
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力の内容が正しくありません。コーチは一覧から選び直してください。",
    );
    expect(listOverdueLiveLessonSessions).toHaveBeenCalledWith({ older_than_hours: 3, coach_id: "not-a-uuid" });
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
  });

  test("コーチの一覧が読めなくても経過時間で通話を出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf(SESSIONS), listCoaches }} />);

    expect(await screen.findByRole("table", { name: "終わっていない通話の一覧" })).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(screen.getByRole("combobox", { name: "コーチ" })).toBeDisabled();
    expect(screen.getByRole("combobox", { name: "開始からの経過" })).toBeEnabled();
  });
});

describe("終わっていない通話の絞り込みと URL", () => {
  test("「すべて」のコーチはクエリに出さず、選んだコーチを契約のクエリの名前で残す", () => {
    expect(overdueLiveSessionsPath("")).toBe("/headquarters/overdue-live-sessions");
    expect(overdueLiveSessionsPath(SATO)).toBe(`/headquarters/overdue-live-sessions?coach_id=${SATO}`);
  });

  test("画面でコーチを変えると、履歴を積まずに URL のクエリを置き換える。経過時間では置き換えない", async () => {
    const user = userEvent.setup();
    const listOverdueLiveLessonSessions = listOf(SESSIONS);
    headquartersApiMock.mockReturnValue({ listOverdueLiveLessonSessions, listCoaches: coachesOf() });
    render(<OverdueLiveSessionsHome token="mock.headquarters.x" initialCoachId={SATO} />);
    await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    expect(listOverdueLiveLessonSessions).toHaveBeenCalledWith({ older_than_hours: 3, coach_id: SATO });
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toHaveValue(SATO));

    await user.selectOptions(select, "すべて");
    expect(replace).toHaveBeenLastCalledWith("/headquarters/overdue-live-sessions", { scroll: false });
    await user.selectOptions(select, "高橋 コーチ");
    expect(replace).toHaveBeenLastCalledWith(`/headquarters/overdue-live-sessions?coach_id=${TAKAHASHI}`, {
      scroll: false,
    });
    await user.selectOptions(screen.getByRole("combobox", { name: "開始からの経過" }), "6 時間以上");
    expect(replace).toHaveBeenCalledTimes(2);
    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 6, coach_id: TAKAHASHI });
  });
});

describe("画面への配置", () => {
  test("本部の画面から終わっていない通話の画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "終わっていない通話" });
    expect(within(region).getByRole("link", { name: "時間の過ぎた通話を見る" })).toHaveAttribute(
      "href",
      "/headquarters/overdue-live-sessions",
    );
  });

  test("終わっていない通話の画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({ listOverdueLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <OverdueLiveSessionsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "終わっていない通話" });
    expect(await within(region).findByRole("table", { name: "終わっていない通話の一覧" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listOverdueLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <OverdueLiveSessionsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "終わっていない通話" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
