import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { StudentDirectory } from "@/components/headquarters/StudentDirectory";
import { StudentDirectoryHome, studentDirectoryPath } from "@/components/headquarters/StudentDirectoryHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは StudentDirectoryHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const SUZUKI = "22222222-2222-4222-8222-222222222222";
const TAKAHASHI = "22222222-2222-4222-8222-222222222223";

// コーチの一覧（listCoaches）の返った順。選択肢はこの順のまま出す。
const COACHES: Schemas["Coach"][] = [
  { coach_id: SUZUKI, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 12 },
  { coach_id: TAKAHASHI, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

const STUDENTS: Schemas["Student"][] = [
  {
    student_id: "11111111-1111-4111-8111-000000000001",
    display_name: "山田 花子",
    sport: "tennis",
    contract_type: "subscription",
    plan: "plus",
    last_booked_at: "2026-10-25T01:30:00Z",
    recent_booking_count: 3,
    registered_at: "2026-10-20T15:30:00Z",
  },
  {
    student_id: "11111111-1111-4111-8111-000000000002",
    display_name: "佐藤 一郎",
    sport: "golf",
    contract_type: "one_shot",
    plan: null,
    last_booked_at: null,
    recent_booking_count: 0,
    registered_at: "2026-10-10T00:00:00Z",
  },
  {
    student_id: "11111111-1111-4111-8111-000000000003",
    display_name: "鈴木 次郎",
    sport: "tennis",
    contract_type: null,
    plan: null,
    last_booked_at: "2026-10-01T09:00:00Z",
    recent_booking_count: 1,
    registered_at: "2026-09-30T00:00:00Z",
  },
];

const RECENT_BOOKING_COUNT_NOTE =
  "直近 30 日の予約は、今日までの 30 日にその生徒が作った予約（体験予約を含む）のうち、キャンセルされていないものの数です。";

function listOf(students: Schemas["Student"][]) {
  return vi.fn<HeadquartersApi["listStudents"]>(async () => ({ ok: true, data: { students } }));
}

function coachesOf(coaches: Schemas["Coach"][] = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

function cellsOf(row: HTMLElement) {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

function bodyRows() {
  return within(screen.getByRole("table", { name: "生徒の一覧" })).getAllByRole("row").slice(1);
}

describe("本部の生徒", () => {
  test("読み込み中を出し、読めたら各行に表示名・スポーツ・契約プラン・最後に予約した日・直近 30 日の予約・登録日を API の順で出す", async () => {
    const listStudents = listOf(STUDENTS);
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    await screen.findByRole("table", { name: "生徒の一覧" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(bodyRows().map(cellsOf)).toEqual([
      ["山田 花子", "テニス", "月額（プラス）", "2026/10/25", "3 件", "2026/10/21"],
      // ワンショットはプランがなく、予約がなければ「まだ予約がありません」
      ["佐藤 一郎", "ゴルフ", "ワンショット", "まだ予約がありません", "0 件", "2026/10/10"],
      // 受講権がなければ「未契約」。最後に予約した日は日本時間の日付
      ["鈴木 次郎", "テニス", "未契約", "2026/10/1", "1 件", "2026/9/30"],
    ]);
    expect(within(screen.getByRole("table")).getAllByRole("columnheader")[3]).toHaveTextContent(
      "最後に予約した日",
    );
    // 開いたときは条件を付けずに読む
    expect(listStudents).toHaveBeenCalledTimes(1);
    expect(listStudents).toHaveBeenCalledWith({});
    expect(screen.getByLabelText("スポーツ")).toHaveValue("");
    expect(screen.getByLabelText("この日以降に予約していない")).toHaveValue("");
    expect(screen.getByLabelText("コーチ")).toHaveValue("");
  });

  test("各行に直近 30 日の予約の数を「n 件」で出し（0 も空欄にせず「0 件」）、表の上に数え方を添える", async () => {
    render(<StudentDirectory api={{ listStudents: listOf(STUDENTS), listCoaches: coachesOf() }} />);

    const table = await screen.findByRole("table", { name: "生徒の一覧" });
    expect(within(table).getByRole("columnheader", { name: "直近 30 日の予約" })).toBeInTheDocument();
    expect(bodyRows().map((row) => cellsOf(row)[4])).toEqual(["3 件", "0 件", "1 件"]);
    expect(screen.getByText(RECENT_BOOKING_COUNT_NOTE)).toBeInTheDocument();
  });

  test("スポーツは「すべて」と 6 種類から選べる", async () => {
    render(<StudentDirectory api={{ listStudents: listOf(STUDENTS), listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    const options = within(screen.getByLabelText("スポーツ")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "すべて",
      "テニス",
      "ゴルフ",
      "野球",
      "サッカー",
      "バスケットボール",
      "水泳",
    ]);
  });

  test("スポーツを選ぶと API の sport に渡して読み直し、返った行だけを出す", async () => {
    const listStudents = listOf(STUDENTS);
    listStudents.mockResolvedValueOnce({
      ok: true,
      data: { students: STUDENTS },
    });
    listStudents.mockResolvedValueOnce({
      ok: true,
      data: { students: [STUDENTS[1]!] },
    });
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    await waitFor(() => expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["佐藤 一郎"]));
    expect(listStudents).toHaveBeenLastCalledWith({ sport: "golf" });

    await user.selectOptions(screen.getByLabelText("スポーツ"), "すべて");
    await waitFor(() => expect(bodyRows()).toHaveLength(3));
    expect(listStudents).toHaveBeenLastCalledWith({});
  });

  test("日付を入れると API の not_booked_since に渡し、スポーツと組み合わせられる。日付を消せば外す", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    const since = screen.getByLabelText("この日以降に予約していない");
    fireEvent.change(since, { target: { value: "2026-10-15" } });
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        not_booked_since: "2026-10-15",
      }),
    );

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
      }),
    );

    fireEvent.change(since, { target: { value: "" } });
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ sport: "tennis" }));
    expect(listStudents).toHaveBeenCalledTimes(4);
  });

  test("先に変えた条件の結果が遅れて返っても、最後の条件の結果を出す", async () => {
    let resolveSlow: (
      value: Awaited<ReturnType<HeadquartersApi["listStudents"]>>,
    ) => void = () => {};
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    listStudents.mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)));
    listStudents.mockResolvedValueOnce({
      ok: true,
      data: { students: [STUDENTS[1]!] },
    });
    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    await waitFor(() => expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["佐藤 一郎"]));

    resolveSlow({ ok: true, data: { students: [STUDENTS[0]!, STUDENTS[2]!] } });
    await waitFor(() => expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["佐藤 一郎"]));
  });

  test("コーチは「すべて」とコーチの一覧の返った順から選べる", async () => {
    render(<StudentDirectory api={{ listStudents: listOf(STUDENTS), listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "鈴木 コーチ",
      "高橋 コーチ",
    ]);
  });

  test("コーチを選ぶと API の coach_id に渡し、ほかの絞り込みと組み合わせられる。「すべて」に戻せば外す", async () => {
    const listStudents = listOf(STUDENTS);
    const onCoachChange = vi.fn();
    const user = userEvent.setup();
    render(
      <StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} onCoachChange={onCoachChange} />,
    );
    await screen.findByRole("table");
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [STUDENTS[0]!] } });
    await user.selectOptions(select, "高橋 コーチ");
    await waitFor(() => expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["山田 花子"]));
    expect(listStudents).toHaveBeenLastCalledWith({ coach_id: TAKAHASHI });
    expect(onCoachChange).toHaveBeenLastCalledWith(TAKAHASHI);

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    fireEvent.change(screen.getByLabelText("この日以降に予約していない"), { target: { value: "2026-10-15" } });
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
        coach_id: TAKAHASHI,
      }),
    );
    // コーチを変えていないので知らせない
    expect(onCoachChange).toHaveBeenCalledTimes(1);

    await user.selectOptions(select, "すべて");
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({ sport: "tennis", not_booked_since: "2026-10-15" }),
    );
    expect(onCoachChange).toHaveBeenLastCalledWith("");
    expect(onCoachChange).toHaveBeenCalledTimes(2);
  });

  test("渡されたコーチで絞って開き、そのコーチを選んだ状態にする", async () => {
    const listStudents = listOf([STUDENTS[0]!]);
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} initialCoachId={SUZUKI} />);

    await screen.findByRole("table");
    expect(listStudents).toHaveBeenCalledTimes(1);
    expect(listStudents).toHaveBeenCalledWith({ coach_id: SUZUKI });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toHaveValue(SUZUKI));
  });

  test("渡されたコーチが一覧になければ「（一覧にないコーチ）」を選んだ状態で出し、「すべて」に戻せる", async () => {
    const unknown = "99999999-9999-4999-8999-999999999999";
    const listStudents = listOf([]);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} initialCoachId={unknown} />);

    expect(await screen.findByText("条件に合う生徒はいません")).toBeInTheDocument();
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toHaveValue(unknown));
    expect(within(select).getByRole("option", { selected: true })).toHaveTextContent("（一覧にないコーチ）");

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: STUDENTS } });
    await user.selectOptions(select, "すべて");
    await waitFor(() => expect(bodyRows()).toHaveLength(3));
    expect(listStudents).toHaveBeenLastCalledWith({});
    expect(within(select).queryByRole("option", { name: "（一覧にないコーチ）" })).not.toBeInTheDocument();
  });

  test("422 なら汎用の文言に直す場所（コーチを選び直す）を添えて出し、一覧は出さない", async () => {
    const listStudents = vi.fn<HeadquartersApi["listStudents"]>(async () => ({
      ok: false,
      message: "入力の内容が正しくありません。",
      code: "validation_failed",
    }));
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} initialCoachId="not-a-uuid" />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力の内容が正しくありません。コーチは一覧から選び直してください。",
    );
    expect(listStudents).toHaveBeenCalledWith({ coach_id: "not-a-uuid" });
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("コーチの一覧が読めなくてもほかの条件で生徒を出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<StudentDirectory api={{ listStudents: listOf(STUDENTS), listCoaches }} />);

    expect(await screen.findByRole("table", { name: "生徒の一覧" })).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(screen.getByRole("combobox", { name: "コーチ" })).toBeDisabled();
    expect(screen.getByLabelText("スポーツ")).toBeEnabled();
  });

  test("コーチで絞り込んで 0 件なら「条件に合う生徒はいません」と出し、表は出さない", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.selectOptions(select, "鈴木 コーチ");
    expect(await screen.findByText("条件に合う生徒はいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("絞り込んで 0 件なら「条件に合う生徒はいません」と出し、表は出さない", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.selectOptions(screen.getByLabelText("スポーツ"), "水泳");
    expect(await screen.findByText("条件に合う生徒はいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("まだ生徒はいません")).not.toBeInTheDocument();
  });

  test("生徒が 0 人なら「まだ生徒はいません」と出し、表は出さない", async () => {
    render(<StudentDirectory api={{ listStudents: listOf([]), listCoaches: coachesOf() }} />);

    expect(await screen.findByText("まだ生徒はいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(RECENT_BOOKING_COUNT_NOTE)).not.toBeInTheDocument();
    expect(screen.queryByText("条件に合う生徒はいません")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listStudents = vi.fn<HeadquartersApi["listStudents"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "権限がないためアクセスできません。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("まだ生徒はいません")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument());
  });
});

const HAS_UNANSWERED_ONLY = "返事を待っているメッセージのある生徒だけ";

describe("本部の生徒の返事を待っているメッセージのある生徒だけ", () => {
  test("開いたときは外れていて、入れたら has_unanswered=true を付けて読み直し、外したら付けずに読み直す", async () => {
    const listStudents = listOf(STUDENTS);
    const onCoachChange = vi.fn();
    const user = userEvent.setup();
    render(
      <StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} onCoachChange={onCoachChange} />,
    );
    await screen.findByRole("table");
    const toggle = screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY });
    expect(toggle).not.toBeChecked();
    expect(listStudents).toHaveBeenCalledWith({});

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [STUDENTS[0]!] } });
    await user.click(toggle);
    await waitFor(() => expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["山田 花子"]));
    expect(listStudents).toHaveBeenLastCalledWith({ has_unanswered: true });
    expect(toggle).toBeChecked();

    await user.click(toggle);
    await waitFor(() => expect(bodyRows()).toHaveLength(3));
    expect(listStudents).toHaveBeenLastCalledWith({});
    expect(listStudents).toHaveBeenCalledTimes(3);
    // コーチは変えていないので知らせない
    expect(onCoachChange).not.toHaveBeenCalled();
  });

  test("スポーツ・この日以降に予約していない・コーチと組み合わせ、どれを変えても has_unanswered=true を付けたまま読み直す", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());

    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ has_unanswered: true }));

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ sport: "tennis", has_unanswered: true }));

    fireEvent.change(screen.getByLabelText("この日以降に予約していない"), { target: { value: "2026-10-15" } });
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
        has_unanswered: true,
      }),
    );

    await user.selectOptions(select, "鈴木 コーチ");
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
        coach_id: SUZUKI,
        has_unanswered: true,
      }),
    );

    // 切り替えを外しても、ほかの条件は残す
    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
        coach_id: SUZUKI,
      }),
    );
  });

  test("渡されたコーチで開いたときも、入れたらそのコーチと組み合わせて読み直す", async () => {
    const listStudents = listOf([STUDENTS[0]!]);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} initialCoachId={SUZUKI} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ coach_id: SUZUKI, has_unanswered: true }));
  });

  test("該当がなければ「返事を待っているメッセージのある生徒はいません」を出し、表は出さない", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    expect(await screen.findByText("返事を待っているメッセージのある生徒はいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(RECENT_BOOKING_COUNT_NOTE)).not.toBeInTheDocument();
    expect(screen.queryByText("まだ生徒はいません")).not.toBeInTheDocument();
    expect(screen.queryByText("条件に合う生徒はいません")).not.toBeInTheDocument();
  });

  test("ほかの条件と組み合わせて該当がなければ、返事を待っているメッセージのある生徒のうち条件に合う人がいないと出す", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ has_unanswered: true }));
    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.selectOptions(screen.getByLabelText("スポーツ"), "水泳");
    expect(
      await screen.findByText("返事を待っているメッセージのある生徒のうち、条件に合う生徒はいません"),
    ).toBeInTheDocument();
    expect(listStudents).toHaveBeenLastCalledWith({ sport: "swimming", has_unanswered: true });
    expect(screen.queryByRole("table")).not.toBeInTheDocument();

    // 外せば、ふつうの絞り込みの空の文言に戻る
    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    expect(await screen.findByText("条件に合う生徒はいません")).toBeInTheDocument();
  });
});

const HAS_PENDING_VIDEO_ONLY = "返却待ちの動画のある生徒だけ";
const HAS_UNHANDLED_LOW_RATED_ONLY = "未対応の低い評価をつけた生徒だけ";

describe.each([
  { label: HAS_PENDING_VIDEO_ONLY, query: { has_pending_video: true }, subject: "返却待ちの動画のある生徒" },
  {
    label: HAS_UNHANDLED_LOW_RATED_ONLY,
    query: { has_unhandled_low_rated: true },
    subject: "未対応の低い評価をつけた生徒",
  },
])("本部の生徒の$label", ({ label, query, subject }) => {
  test("開いたときは外れていて、入れたら契約のクエリを true で付けて読み直し、外したら付けずに読み直す", async () => {
    const listStudents = listOf(STUDENTS);
    const onCoachChange = vi.fn();
    const user = userEvent.setup();
    render(
      <StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} onCoachChange={onCoachChange} />,
    );
    await screen.findByRole("table");
    const toggle = screen.getByRole("checkbox", { name: label });
    expect(toggle).not.toBeChecked();
    expect(listStudents).toHaveBeenCalledWith({});

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [STUDENTS[0]!] } });
    await user.click(toggle);
    await waitFor(() => expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["山田 花子"]));
    expect(listStudents).toHaveBeenLastCalledWith(query);
    expect(toggle).toBeChecked();

    await user.click(toggle);
    await waitFor(() => expect(bodyRows()).toHaveLength(3));
    expect(listStudents).toHaveBeenLastCalledWith({});
    expect(listStudents).toHaveBeenCalledTimes(3);
    expect(onCoachChange).not.toHaveBeenCalled();
  });

  test("スポーツ・この日以降に予約していない・コーチと組み合わせ、どれを変えても付けたまま読み直す", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());

    await user.click(screen.getByRole("checkbox", { name: label }));
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith(query));

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ sport: "tennis", ...query }));

    fireEvent.change(screen.getByLabelText("この日以降に予約していない"), { target: { value: "2026-10-15" } });
    await user.selectOptions(select, "鈴木 コーチ");
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
        coach_id: SUZUKI,
        ...query,
      }),
    );

    // 切り替えを外しても、ほかの条件は残す
    await user.click(screen.getByRole("checkbox", { name: label }));
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        sport: "tennis",
        not_booked_since: "2026-10-15",
        coach_id: SUZUKI,
      }),
    );
  });

  test("該当がなければ条件を含めた空の文言を出し、表は出さない", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.click(screen.getByRole("checkbox", { name: label }));
    expect(await screen.findByText(`${subject}はいません`)).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(RECENT_BOOKING_COUNT_NOTE)).not.toBeInTheDocument();

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.selectOptions(screen.getByLabelText("スポーツ"), "水泳");
    expect(await screen.findByText(`${subject}のうち、条件に合う生徒はいません`)).toBeInTheDocument();
    expect(listStudents).toHaveBeenLastCalledWith({ sport: "swimming", ...query });
  });
});

describe("本部の生徒の切り替えの組み合わせ", () => {
  test("3 つの切り替えを同時に入れられ、すべてを付けて読み直し、1 つ外せばそれだけを外す", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    await user.click(screen.getByRole("checkbox", { name: HAS_PENDING_VIDEO_ONLY }));
    await user.click(screen.getByRole("checkbox", { name: HAS_UNHANDLED_LOW_RATED_ONLY }));
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({
        has_unanswered: true,
        has_pending_video: true,
        has_unhandled_low_rated: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: HAS_PENDING_VIDEO_ONLY }));
    await waitFor(() =>
      expect(listStudents).toHaveBeenLastCalledWith({ has_unanswered: true, has_unhandled_low_rated: true }),
    );
  });

  test("複数の切り替えで 0 件なら、入れた条件をつないだ空の文言を出す", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("checkbox", { name: HAS_PENDING_VIDEO_ONLY }));
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ has_pending_video: true }));
    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.click(screen.getByRole("checkbox", { name: HAS_UNHANDLED_LOW_RATED_ONLY }));
    expect(
      await screen.findByText("返却待ちの動画があり、未対応の低い評価をつけた生徒はいません"),
    ).toBeInTheDocument();

    listStudents.mockResolvedValueOnce({ ok: true, data: { students: [] } });
    await user.click(screen.getByRole("checkbox", { name: HAS_UNANSWERED_ONLY }));
    expect(
      await screen.findByText(
        "返事を待っているメッセージがあり、返却待ちの動画があり、未対応の低い評価をつけた生徒はいません",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});

describe("生徒の絞り込みと URL", () => {
  test("「すべて」のコーチはクエリに出さず、選んだコーチを契約のクエリの名前で残す", () => {
    expect(studentDirectoryPath("")).toBe("/headquarters/students");
    expect(studentDirectoryPath(SUZUKI)).toBe(`/headquarters/students?coach_id=${SUZUKI}`);
  });

  test("画面でコーチを変えると、履歴を積まずに URL のクエリを置き換える。ほかの絞り込みでは置き換えない", async () => {
    const user = userEvent.setup();
    const listStudents = listOf(STUDENTS);
    headquartersApiMock.mockReturnValue({ listStudents, listCoaches: coachesOf() });
    render(<StudentDirectoryHome token="mock.headquarters.x" initialCoachId={SUZUKI} />);
    await screen.findByRole("table", { name: "生徒の一覧" });
    expect(listStudents).toHaveBeenCalledWith({ coach_id: SUZUKI });
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toHaveValue(SUZUKI));

    await user.selectOptions(select, "すべて");
    expect(replace).toHaveBeenLastCalledWith("/headquarters/students", { scroll: false });
    await user.selectOptions(select, "高橋 コーチ");
    expect(replace).toHaveBeenLastCalledWith(`/headquarters/students?coach_id=${TAKAHASHI}`, { scroll: false });
    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    expect(replace).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(listStudents).toHaveBeenLastCalledWith({ sport: "golf", coach_id: TAKAHASHI }));
  });
});

describe("画面への配置", () => {
  test("本部の画面から生徒の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "生徒の一覧を見る" })).toHaveAttribute(
      "href",
      "/headquarters/students",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <StudentDirectoryHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "生徒" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
