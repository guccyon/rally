import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { LiveSessionRecordList } from "@/components/headquarters/LiveSessionRecordList";
import { LiveSessionRecordsHome } from "@/components/headquarters/LiveSessionRecordsHome";
import { recentPeriod } from "@/components/headquarters/bookingPeriod";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは LiveSessionRecordsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

// 今は 2026-10-26 12:00（日本時間）。Date だけを止め、待ち合わせのタイマーは本物のまま。
const NOW = new Date("2026-10-26T03:00:00Z");

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

const SUZUKI = "22222222-2222-4222-8222-222222222222";
const TAKAHASHI = "22222222-2222-4222-8222-222222222223";

const COACHES: Schemas["Coach"][] = [
  { coach_id: SUZUKI, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 3 },
  { coach_id: TAKAHASHI, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

// API が返す並び（開始の新しい順）。画面はこの順のまま出す。
const SESSIONS: Schemas["LiveLessonSessionRecord"][] = [
  {
    session_id: "55555555-5555-4555-8555-000000000001",
    booking_id: "66666666-6666-4666-8666-000000000001",
    coach_id: SUZUKI,
    coach_display_name: "鈴木 コーチ",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "山田 花子",
    started_at: "2026-10-26T02:30:00Z",
    ended_at: null,
    duration_minutes: null,
  },
  {
    session_id: "55555555-5555-4555-8555-000000000002",
    booking_id: "66666666-6666-4666-8666-000000000002",
    coach_id: "77777777-7777-4777-8777-777777777777",
    student_id: "11111111-1111-4111-8111-000000000002",
    started_at: "2026-10-24T01:00:00Z",
    ended_at: "2026-10-24T01:45:30Z",
    duration_minutes: 45,
  },
];

function listOf(sessions: Schemas["LiveLessonSessionRecord"][]) {
  return vi.fn<HeadquartersApi["listLiveLessonSessions"]>(async ({ from, to }) => ({
    ok: true,
    data: { from, to, sessions },
  }));
}

function coachesOf(coaches: Schemas["Coach"][] = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

const TABLE = "ライブレッスンの記録の一覧";

describe("既定の期間", () => {
  test("今日（日本時間）までの 7 日（今日を含む）", () => {
    expect(recentPeriod(NOW.getTime())).toEqual({ from: "2026-10-20", to: "2026-10-26" });
    // 日本時間で翌日 0 時を過ぎていれば、UTC ではまだ前の日でも今日は日本時間で決める。
    expect(recentPeriod(Date.parse("2026-10-26T15:30:00Z"))).toEqual({ from: "2026-10-21", to: "2026-10-27" });
  });
});

describe("本部のライブレッスンの記録", () => {
  test("読み込み中を出し、既定の期間・すべてのコーチで読み、コーチ・生徒・開始・終了を API の順で表にする", async () => {
    const listLiveLessonSessions = listOf(SESSIONS);
    render(<LiveSessionRecordList api={{ listLiveLessonSessions, listCoaches: coachesOf() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: TABLE });
    expect(listLiveLessonSessions).toHaveBeenCalledTimes(1);
    expect(listLiveLessonSessions).toHaveBeenCalledWith({ from: "2026-10-20", to: "2026-10-26" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent))).toEqual([
      ["鈴木 コーチ", "山田 花子", formatDateTime("2026-10-26T02:30:00Z"), "終了の記録なし"],
      [
        "（名前を表示できません）",
        "（名前を表示できません）",
        formatDateTime("2026-10-24T01:00:00Z"),
        `${formatDateTime("2026-10-24T01:45:30Z")}（45 分）`,
      ],
    ]);
    expect(screen.getByText("2026年10月20日（火） 〜 2026年10月26日（月）・2 件")).toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-20");
    expect(screen.getByLabelText("終了日")).toHaveValue("2026-10-26");
    expect(screen.getByRole("combobox", { name: "コーチ" })).toHaveValue("");
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("コーチの選択肢はコーチの一覧の順で、期間とコーチを選び直して「この条件で見る」で読み直し、応答の期間を出す", async () => {
    const user = userEvent.setup();
    const listLiveLessonSessions = listOf([]);
    render(<LiveSessionRecordList api={{ listLiveLessonSessions, listCoaches: coachesOf() }} />);
    await screen.findByText("この条件の通話の記録はありません");

    const select = screen.getByRole("combobox", { name: "コーチ" });
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "鈴木 コーチ",
      "高橋 コーチ",
    ]);
    await user.selectOptions(select, "高橋 コーチ");
    await user.clear(screen.getByLabelText("開始日"));
    await user.type(screen.getByLabelText("開始日"), "2026-10-01");
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-10-15");
    // 選んだだけでは読み直さない。
    expect(listLiveLessonSessions).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listLiveLessonSessions).toHaveBeenLastCalledWith({
      from: "2026-10-01",
      to: "2026-10-15",
      coach_id: TAKAHASHI,
    });
    expect(await screen.findByText("2026年10月1日（木） 〜 2026年10月15日（木）・0 件")).toBeInTheDocument();
  });

  test("0 件なら「この条件の通話の記録はありません」と出し、表は出さない", async () => {
    render(<LiveSessionRecordList api={{ listLiveLessonSessions: listOf([]), listCoaches: coachesOf() }} />);

    expect(await screen.findByText("この条件の通話の記録はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: TABLE })).not.toBeInTheDocument();
  });

  test("終了日が開始日より前・31 日を超える期間は送らずに理由を出す", async () => {
    const user = userEvent.setup();
    const listLiveLessonSessions = listOf(SESSIONS);
    render(<LiveSessionRecordList api={{ listLiveLessonSessions, listCoaches: coachesOf() }} />);
    await screen.findByRole("table", { name: TABLE });

    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-10-19");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("終了日は開始日と同じか、それより後の日付にしてください。");

    await user.clear(screen.getByLabelText("開始日"));
    await user.type(screen.getByLabelText("開始日"), "2026-09-18");
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-10-19");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");

    expect(listLiveLessonSessions).toHaveBeenCalledTimes(1);
    // 前に読めた一覧はそのまま残す。
    expect(screen.getByRole("table", { name: TABLE })).toBeInTheDocument();
  });

  test("サーバーの 422 は汎用の文言に、期間とコーチの直し方を添えて出す", async () => {
    const listLiveLessonSessions = vi.fn<HeadquartersApi["listLiveLessonSessions"]>(async () => ({
      ok: false,
      message: "入力の内容が正しくありません。",
      code: "validation_failed",
    }));
    render(<LiveSessionRecordList api={{ listLiveLessonSessions, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力の内容が正しくありません。期間は 31 日までで、終了日は開始日と同じか後の日付にしてください。コーチは一覧から選び直してください。",
    );
    expect(screen.queryByRole("table", { name: TABLE })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listLiveLessonSessions = vi.fn<HeadquartersApi["listLiveLessonSessions"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
      code: "forbidden",
    }));
    render(<LiveSessionRecordList api={{ listLiveLessonSessions, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(/^権限がないためアクセスできません。$/);
    expect(screen.queryByRole("table", { name: TABLE })).not.toBeInTheDocument();
    expect(screen.queryByText("この条件の通話の記録はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("コーチの一覧が読めなくても期間だけで記録を出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<LiveSessionRecordList api={{ listLiveLessonSessions: listOf(SESSIONS), listCoaches }} />);

    expect(await screen.findByRole("table", { name: TABLE })).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("コーチの一覧を読めませんでした。サーバーに接続できません。");
    expect(screen.getByRole("combobox", { name: "コーチ" })).toBeDisabled();
  });

  test("条件を続けて変えたときは最後の条件の結果だけを出す", async () => {
    const user = userEvent.setup();
    let resolveSlow: (() => void) | undefined;
    const listLiveLessonSessions = vi.fn<HeadquartersApi["listLiveLessonSessions"]>(async ({ from, to, coach_id }) => {
      if (coach_id === SUZUKI) await new Promise<void>((resolve) => (resolveSlow = resolve));
      return { ok: true, data: { from, to, sessions: coach_id === SUZUKI ? SESSIONS : [] } };
    });
    render(<LiveSessionRecordList api={{ listLiveLessonSessions, listCoaches: coachesOf() }} />);
    await screen.findByText("この条件の通話の記録はありません");

    const select = screen.getByRole("combobox", { name: "コーチ" });
    await user.selectOptions(select, "鈴木 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await user.selectOptions(select, "高橋 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await screen.findByText("この条件の通話の記録はありません");
    resolveSlow?.();

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.getByText("この条件の通話の記録はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: TABLE })).not.toBeInTheDocument();
  });

  test("セッションを終わらせる操作は出さない（ボタンは「この条件で見る」だけ）", async () => {
    render(<LiveSessionRecordList api={{ listLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() }} />);

    await screen.findByRole("table", { name: TABLE });
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual(["この条件で見る"]);
  });
});

describe("画面への配置", () => {
  test("本部の画面からライブレッスンの記録の画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "ライブレッスンの記録" });
    expect(within(region).getByRole("link", { name: "通話の開始と終了の記録を見る" })).toHaveAttribute(
      "href",
      "/headquarters/live-sessions",
    );
  });

  test("ライブレッスンの記録の画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({ listLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <LiveSessionRecordsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "ライブレッスンの記録" });
    expect(await within(region).findByRole("table", { name: TABLE })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listLiveLessonSessions: listOf(SESSIONS), listCoaches: coachesOf() });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <LiveSessionRecordsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "ライブレッスンの記録" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
