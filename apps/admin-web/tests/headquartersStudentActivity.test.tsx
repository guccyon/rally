import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { StudentActivity } from "@/components/headquarters/StudentActivity";
import { StudentActivityHome, studentActivityPath } from "@/components/headquarters/StudentActivityHome";
import { StudentDirectory } from "@/components/headquarters/StudentDirectory";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const STUDENT_ID = "11111111-1111-4111-8111-000000000001";
const COACH_ID = "22222222-2222-4222-8222-222222222222";

function booking(n: number, values: Partial<Schemas["Booking"]>): Schemas["Booking"] {
  return {
    booking_id: `44444444-4444-4444-8444-00000000000${n}`,
    slot_id: `33333333-3333-4333-8333-00000000000${n}`,
    student_id: STUDENT_ID,
    coach_id: COACH_ID,
    is_trial: false,
    status: "confirmed",
    starts_at: "2026-10-27T01:00:00Z",
    created_at: "2026-10-20T00:00:00Z",
    confirmed_at: "2026-10-20T01:00:00Z",
    cancelled_at: null,
    lesson_menu: null,
    ...values,
  };
}

const ACTIVITY: Schemas["StudentActivity"] = {
  student_id: STUDENT_ID,
  display_name: "山田 花子",
  upcoming_bookings: [
    {
      booking: booking(1, {
        status: "pending",
        is_trial: true,
        confirmed_at: null,
        lesson_menu: { lesson_menu_id: "aaaaaaaa-aaaa-4aaa-8aaa-000000000001", name: "フォアハンドの基礎", duration_minutes: 60 },
      }),
      coach_display_name: "鈴木 コーチ",
    },
    // コーチのアカウントが引けないときは名前が載らない
    { booking: booking(2, { starts_at: "2026-10-28T09:00:00Z" }) },
  ],
  recent_lessons: [
    {
      lesson_id: "55555555-5555-4555-8555-000000000001",
      coach_id: COACH_ID,
      coach_display_name: "鈴木 コーチ",
      lesson_type: "live_lesson",
      completed_at: "2026-10-25T02:00:00Z",
    },
    {
      lesson_id: "55555555-5555-4555-8555-000000000002",
      coach_id: COACH_ID,
      lesson_type: "async_lesson",
      completed_at: "2026-10-20T15:30:00Z",
    },
  ],
  cancelled_booking_count: 3,
  submitted_video_count: 4,
  pending_video_review_count: 1,
  unanswered_message_thread_count: 0,
  low_rated_review_count: 0,
  unhandled_low_rated_review_count: 0,
};

function activityOf(activity: Schemas["StudentActivity"]) {
  return vi.fn<HeadquartersApi["getStudentActivity"]>(async () => ({ ok: true, data: activity }));
}

function rowsOf(name: string) {
  return within(screen.getByRole("table", { name }))
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent));
}

/** 利用の様子の項目（dt）の値（dd）。 */
function valueOf(term: string) {
  const dt = screen.getByText(term, { selector: "dt" });
  const dd = dt.nextElementSibling;
  if (!(dd instanceof HTMLElement) || dd.tagName !== "DD") throw new Error(`${term} の値がありません`);
  return dd;
}

describe("本部の生徒の利用の様子", () => {
  test("読み込み中を出し、読めたら表示名・キャンセルの件数・これからの予約・直近の受けたレッスンを API の順で出す", async () => {
    const getStudentActivity = activityOf(ACTIVITY);
    render(<StudentActivity api={{ getStudentActivity }} studentId={STUDENT_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    await screen.findByRole("table", { name: "これからの予約" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(getStudentActivity).toHaveBeenCalledWith(STUDENT_ID);
    expect(getStudentActivity).toHaveBeenCalledTimes(1);

    expect(screen.getByText("山田 花子")).toBeInTheDocument();
    expect(valueOf("キャンセルした予約")).toHaveTextContent(/^3 件（これまでのすべて）$/);
    expect(rowsOf("これからの予約")).toEqual([
      ["2026/10/27 10:00:00", "確定待ち", "鈴木 コーチ", "フォアハンドの基礎（60 分）", "体験予約"],
      ["2026/10/28 18:00:00", "確定済み", "（名前を表示できません）", "—", "—"],
    ]);
    expect(rowsOf("直近の受けたレッスン")).toEqual([
      ["2026/10/25 11:00:00", "同期（ビデオ通話）", "鈴木 コーチ"],
      ["2026/10/21 0:30:00", "非同期（動画の添削）", "（名前を表示できません）"],
    ]);
  });

  test("これからの予約の状態は、本部の予約と同じ状態のタグ（色の点と字）で出す", async () => {
    render(<StudentActivity api={{ getStudentActivity: activityOf(ACTIVITY) }} studentId={STUDENT_ID} />);

    const table = await screen.findByRole("table", { name: "これからの予約" });
    expect(within(within(table).getByText("確定待ち")).getByTestId("status-dot")).toHaveClass("bg-warning");
    expect(within(within(table).getByText("確定済み")).getByTestId("status-dot")).toHaveClass("bg-success");
  });

  test("予約もレッスンも 0 件ならそれぞれその旨を出し、表は出さない（キャンセルの件数は 0 件）", async () => {
    render(
      <StudentActivity
        api={{
          getStudentActivity: activityOf({
            ...ACTIVITY,
            upcoming_bookings: [],
            recent_lessons: [],
            cancelled_booking_count: 0,
            submitted_video_count: 0,
            pending_video_review_count: 0,
            unanswered_message_thread_count: 0,
            low_rated_review_count: 0,
            unhandled_low_rated_review_count: 0,
          }),
        }}
        studentId={STUDENT_ID}
      />,
    );

    expect(await screen.findByText("これからの予約はありません")).toBeInTheDocument();
    expect(screen.getByText("まだ受けたレッスンはありません")).toBeInTheDocument();
    expect(valueOf("キャンセルした予約")).toHaveTextContent(/^0 件（これまでのすべて）$/);
    expect(valueOf("送った動画")).toHaveTextContent(/^0 件（これまでのすべて）$/);
    expect(valueOf("返却待ち")).toHaveTextContent(/^0 件（いま返却を待っている動画）$/);
    expect(screen.queryByRole("link", { name: "返却待ちの動画添削を見る" })).not.toBeInTheDocument();
    expect(valueOf("返事を待っているメッセージ")).toHaveTextContent(/^0 件（返事を待って 24 時間以上たったやり取り）$/);
    expect(screen.queryByRole("link", { name: "返事のないメッセージを見る" })).not.toBeInTheDocument();
    expect(valueOf("つけた低い評価")).toHaveTextContent(/^0 件（直近 30 日の★2 以下。うち未対応 0 件）$/);
    expect(screen.queryByRole("link", { name: "低い評価を見る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("送った動画と返却待ちの数を、キャンセルの件数と同じ形で返った値のまま出し、返却待ちがあればその生徒で絞った一覧への入口を添える", async () => {
    render(<StudentActivity api={{ getStudentActivity: activityOf(ACTIVITY) }} studentId={STUDENT_ID} />);

    await screen.findByRole("table", { name: "これからの予約" });
    expect(valueOf("送った動画")).toHaveTextContent(/^4 件（これまでのすべて）$/);
    const pending = valueOf("返却待ち");
    expect(pending).toHaveTextContent(/^1 件（いま返却を待っている動画）返却待ちの動画添削を見る$/);
    // 入口はこの生徒で絞った返却待ちの一覧
    expect(within(pending).getByRole("link", { name: "返却待ちの動画添削を見る" })).toHaveAttribute(
      "href",
      `/headquarters/pending-video-reviews?student_id=${STUDENT_ID}`,
    );
    expect(within(pending).getByRole("link")).toHaveClass("text-primary", "underline");
  });

  test("返事を待っているメッセージの数を返却待ちと同じ形で返った値のまま出し、1 件以上ならその生徒で絞った一覧への入口を添える", async () => {
    render(
      <StudentActivity
        api={{
          getStudentActivity: activityOf({ ...ACTIVITY, pending_video_review_count: 0, unanswered_message_thread_count: 2 }),
        }}
        studentId={STUDENT_ID}
      />,
    );

    await screen.findByRole("table", { name: "これからの予約" });
    const messages = valueOf("返事を待っているメッセージ");
    expect(messages).toHaveTextContent(/^2 件（返事を待って 24 時間以上たったやり取り）返事のないメッセージを見る$/);
    // 入口はこの生徒で絞った返事のないメッセージの一覧（返却待ちの数にはよらない）
    expect(within(messages).getByRole("link", { name: "返事のないメッセージを見る" })).toHaveAttribute(
      "href",
      `/headquarters/unanswered-messages?student_id=${STUDENT_ID}`,
    );
    expect(within(messages).getByRole("link")).toHaveClass("text-primary", "underline");
  });

  test("返事を待っているメッセージが 0 件なら 0 件と出し、一覧への入口は出さない", async () => {
    render(
      <StudentActivity
        api={{ getStudentActivity: activityOf({ ...ACTIVITY, unanswered_message_thread_count: 0 }) }}
        studentId={STUDENT_ID}
      />,
    );

    await screen.findByRole("table", { name: "これからの予約" });
    expect(valueOf("返事を待っているメッセージ")).toHaveTextContent(/^0 件（返事を待って 24 時間以上たったやり取り）$/);
    expect(screen.queryByRole("link", { name: "返事のないメッセージを見る" })).not.toBeInTheDocument();
    // 返却待ちの入口はそのまま
    expect(screen.getByRole("link", { name: "返却待ちの動画添削を見る" })).toBeInTheDocument();
  });

  test("つけた低い評価の数と未対応の数を返事を待っているメッセージと同じ形で返った値のまま出し、1 件以上ならその生徒で絞った一覧への入口を添える", async () => {
    render(
      <StudentActivity
        api={{
          getStudentActivity: activityOf({
            ...ACTIVITY,
            pending_video_review_count: 0,
            unanswered_message_thread_count: 0,
            low_rated_review_count: 3,
            unhandled_low_rated_review_count: 1,
          }),
        }}
        studentId={STUDENT_ID}
      />,
    );

    await screen.findByRole("table", { name: "これからの予約" });
    const reviews = valueOf("つけた低い評価");
    expect(reviews).toHaveTextContent(/^3 件（直近 30 日の★2 以下。うち未対応 1 件）低い評価を見る$/);
    // 入口はこの生徒で絞った低い評価の一覧（返却待ち・返事を待っているメッセージの数にはよらない）
    expect(within(reviews).getByRole("link", { name: "低い評価を見る" })).toHaveAttribute(
      "href",
      `/headquarters/low-reviews?student_id=${STUDENT_ID}`,
    );
    expect(within(reviews).getByRole("link")).toHaveClass("text-primary", "underline");
  });

  test("つけた低い評価がすべて対応済み（未対応 0 件）でも、1 件以上なら入口を添える", async () => {
    render(
      <StudentActivity
        api={{
          getStudentActivity: activityOf({ ...ACTIVITY, low_rated_review_count: 2, unhandled_low_rated_review_count: 0 }),
        }}
        studentId={STUDENT_ID}
      />,
    );

    await screen.findByRole("table", { name: "これからの予約" });
    const reviews = valueOf("つけた低い評価");
    expect(reviews).toHaveTextContent(/^2 件（直近 30 日の★2 以下。うち未対応 0 件）低い評価を見る$/);
    expect(within(reviews).getByRole("link", { name: "低い評価を見る" })).toHaveAttribute(
      "href",
      `/headquarters/low-reviews?student_id=${STUDENT_ID}`,
    );
  });

  test("つけた低い評価が 0 件なら 0 件と出し、一覧への入口は出さない", async () => {
    render(
      <StudentActivity
        api={{
          getStudentActivity: activityOf({ ...ACTIVITY, low_rated_review_count: 0, unhandled_low_rated_review_count: 0 }),
        }}
        studentId={STUDENT_ID}
      />,
    );

    await screen.findByRole("table", { name: "これからの予約" });
    expect(valueOf("つけた低い評価")).toHaveTextContent(/^0 件（直近 30 日の★2 以下。うち未対応 0 件）$/);
    expect(screen.queryByRole("link", { name: "低い評価を見る" })).not.toBeInTheDocument();
    // 返却待ちの入口はそのまま
    expect(screen.getByRole("link", { name: "返却待ちの動画添削を見る" })).toBeInTheDocument();
  });

  test("返却待ちが送った動画と同じ数（すべて返却待ち）でも、そのまま出して入口を添える", async () => {
    render(
      <StudentActivity
        api={{ getStudentActivity: activityOf({ ...ACTIVITY, submitted_video_count: 2, pending_video_review_count: 2 }) }}
        studentId={STUDENT_ID}
      />,
    );

    await screen.findByRole("table", { name: "これからの予約" });
    expect(valueOf("送った動画")).toHaveTextContent(/^2 件（これまでのすべて）$/);
    expect(valueOf("返却待ち")).toHaveTextContent(/^2 件（いま返却を待っている動画）/);
    expect(screen.getByRole("link", { name: "返却待ちの動画添削を見る" })).toBeInTheDocument();
  });

  test("API が 404 なら「見つからない」を出す", async () => {
    const getStudentActivity = vi.fn<HeadquartersApi["getStudentActivity"]>(async () => ({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    }));
    render(<StudentActivity api={{ getStudentActivity }} studentId="not-a-uuid" />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "この生徒は見つかりません。生徒の一覧から選び直してください。",
    );
    expect(screen.queryByText("表示名")).not.toBeInTheDocument();
  });

  test("生徒の ID がなければ API を呼ばずに「見つからない」を出す", () => {
    const getStudentActivity = activityOf(ACTIVITY);
    render(<StudentActivity api={{ getStudentActivity }} studentId={undefined} />);

    expect(screen.getByRole("alert")).toHaveTextContent("この生徒は見つかりません。");
    expect(getStudentActivity).not.toHaveBeenCalled();
  });

  test("422 `validation_failed` が返れば、その理由の文言を出し、件数も入口も出さない", async () => {
    const getStudentActivity = vi.fn<HeadquartersApi["getStudentActivity"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "student_id が正しくありません。",
    }));
    render(<StudentActivity api={{ getStudentActivity }} studentId={STUDENT_ID} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("student_id が正しくありません。");
    expect(screen.queryByText("送った動画")).not.toBeInTheDocument();
    expect(screen.queryByText("返却待ち")).not.toBeInTheDocument();
    expect(screen.queryByText("返事を待っているメッセージ")).not.toBeInTheDocument();
    expect(screen.queryByText("つけた低い評価")).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "返却待ちの動画添削を見る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "返事のないメッセージを見る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "低い評価を見る" })).not.toBeInTheDocument();
  });

  test("404 以外の失敗（403 など）はその文言を出し、中身は出さない", async () => {
    const getStudentActivity = vi.fn<HeadquartersApi["getStudentActivity"]>(async () => ({
      ok: false,
      code: "forbidden",
      message: "権限がないためアクセスできません。",
    }));
    render(<StudentActivity api={{ getStudentActivity }} studentId={STUDENT_ID} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByText("この生徒は見つかりません。", { exact: false })).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument());
  });
});

describe("画面への配置", () => {
  test("生徒の一覧の表示名から、その生徒の利用の様子へ移れる", async () => {
    const listStudents = vi.fn<HeadquartersApi["listStudents"]>(async () => ({
      ok: true,
      data: {
        students: [
          {
            student_id: STUDENT_ID,
            display_name: "山田 花子",
            sport: "tennis",
            contract_type: null,
            plan: null,
            last_booked_at: null,
            recent_booking_count: 0,
            registered_at: "2026-10-20T00:00:00Z",
          },
        ],
      },
    }));
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches: [] } }));
    render(<StudentDirectory api={{ listStudents, listCoaches }} />);

    expect(await screen.findByRole("link", { name: "山田 花子" })).toHaveAttribute(
      "href",
      `/headquarters/students/activity?student_id=${STUDENT_ID}`,
    );
    expect(studentActivityPath(STUDENT_ID)).toBe(`/headquarters/students/activity?student_id=${STUDENT_ID}`);
  });

  test("利用の様子から生徒の一覧へ戻れる", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentActivityHome token={session.token} studentId={undefined} />}
      </RoleGate>,
    );

    expect(screen.getByRole("link", { name: "生徒の一覧へ戻る" })).toHaveAttribute("href", "/headquarters/students");
    expect(screen.queryByRole("link", { name: "この生徒の予約を見る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "この生徒のビデオ通話の記録を見る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "この生徒の終わっていないビデオ通話を見る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "この生徒の返した動画添削の記録を見る" })).not.toBeInTheDocument();
  });

  test("利用の様子から、その生徒で絞った予約の一覧を開ける", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentActivityHome token={session.token} studentId={STUDENT_ID} />}
      </RoleGate>,
    );

    expect(screen.getByRole("link", { name: "この生徒の予約を見る" })).toHaveAttribute(
      "href",
      `/headquarters/bookings?student_id=${STUDENT_ID}`,
    );
  });

  test("利用の様子から、その生徒で絞ったビデオ通話の記録を開ける", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentActivityHome token={session.token} studentId={STUDENT_ID} />}
      </RoleGate>,
    );

    expect(screen.getByRole("link", { name: "この生徒のビデオ通話の記録を見る" })).toHaveAttribute(
      "href",
      `/headquarters/live-sessions?student_id=${STUDENT_ID}`,
    );
  });

  test("利用の様子から、その生徒で絞った終わっていないビデオ通話を開ける", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentActivityHome token={session.token} studentId={STUDENT_ID} />}
      </RoleGate>,
    );

    expect(screen.getByRole("link", { name: "この生徒の終わっていないビデオ通話を見る" })).toHaveAttribute(
      "href",
      `/headquarters/overdue-live-sessions?student_id=${STUDENT_ID}`,
    );
  });

  test("利用の様子から、その生徒で絞った返した動画添削の記録を開ける", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentActivityHome token={session.token} studentId={STUDENT_ID} />}
      </RoleGate>,
    );

    expect(screen.getByRole("link", { name: "この生徒の返した動画添削の記録を見る" })).toHaveAttribute(
      "href",
      `/headquarters/returned-video-reviews?student_id=${STUDENT_ID}`,
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentActivityHome token={session.token} studentId={STUDENT_ID} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "生徒の利用の様子" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
