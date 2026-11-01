import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { OverdueLiveSessionList } from "@/components/headquarters/OverdueLiveSessionList";
import { OverdueLiveSessionsHome } from "@/components/headquarters/OverdueLiveSessionsHome";
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

// API が返す並び（開始の古い順）。画面はこの順のまま出す。
const SESSIONS: Schemas["OverdueLiveLessonSession"][] = [
  {
    session_id: "55555555-5555-4555-8555-000000000001",
    booking_id: "66666666-6666-4666-8666-000000000001",
    coach_id: "22222222-2222-4222-8222-222222222222",
    coach_display_name: "佐藤 健",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "山田 花子",
    started_at: "2026-10-30T01:00:00Z",
    elapsed_sec: 26 * 3600 + 5 * 60 + 59,
  },
  {
    session_id: "55555555-5555-4555-8555-000000000002",
    booking_id: "66666666-6666-4666-8666-000000000002",
    coach_id: "77777777-7777-4777-8777-777777777777",
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
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions }} />);

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
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions }} />);

    expect(
      await screen.findByText("開始から 5 時間以上たって、まだ終わっていない通話です。経過は読み込んだ時点のものです。"),
    ).toBeInTheDocument();
  });

  test("経過時間の下限を変えると、その値で読み直す", async () => {
    const user = userEvent.setup();
    const listOverdueLiveLessonSessions = listOf(SESSIONS);
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions }} />);
    await screen.findByRole("table", { name: "終わっていない通話の一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "開始からの経過" }), "1 日（24 時間）以上");

    expect(listOverdueLiveLessonSessions).toHaveBeenLastCalledWith({ older_than_hours: 24 });
    expect(await screen.findByText(/開始から 24 時間以上たって/)).toBeInTheDocument();
  });

  test("選べる下限は契約の範囲（1〜168 時間）の中だけ", async () => {
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf([]) }} />);

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
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions }} />);
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
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf([]) }} />);

    expect(await screen.findByText("終わっていない通話はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、表は出さない", async () => {
    const listOverdueLiveLessonSessions = vi.fn<HeadquartersApi["listOverdueLiveLessonSessions"]>(
      async () => ({ ok: false, message: "権限がないためアクセスできません。" }),
    );
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table", { name: "終わっていない通話の一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("終わっていない通話はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("セッションを終わらせる操作は出さない", async () => {
    render(<OverdueLiveSessionList api={{ listOverdueLiveLessonSessions: listOf(SESSIONS) }} />);

    await screen.findByRole("table", { name: "終わっていない通話の一覧" });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
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
    headquartersApiMock.mockReturnValue({ listOverdueLiveLessonSessions: listOf(SESSIONS) });
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
    headquartersApiMock.mockReturnValue({ listOverdueLiveLessonSessions: listOf(SESSIONS) });
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
