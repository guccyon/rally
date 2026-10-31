import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { StudentDirectory } from "@/components/headquarters/StudentDirectory";
import { StudentDirectoryHome } from "@/components/headquarters/StudentDirectoryHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const STUDENTS: Schemas["Student"][] = [
  {
    student_id: "11111111-1111-4111-8111-000000000001",
    display_name: "山田 花子",
    sport: "tennis",
    contract_type: "subscription",
    plan: "plus",
    last_booked_at: "2026-10-25T01:30:00Z",
    registered_at: "2026-10-20T15:30:00Z",
  },
  {
    student_id: "11111111-1111-4111-8111-000000000002",
    display_name: "佐藤 一郎",
    sport: "golf",
    contract_type: "one_shot",
    plan: null,
    last_booked_at: null,
    registered_at: "2026-10-10T00:00:00Z",
  },
  {
    student_id: "11111111-1111-4111-8111-000000000003",
    display_name: "鈴木 次郎",
    sport: "tennis",
    contract_type: null,
    plan: null,
    last_booked_at: "2026-10-01T09:00:00Z",
    registered_at: "2026-09-30T00:00:00Z",
  },
];

function listOf(students: Schemas["Student"][]) {
  return vi.fn<HeadquartersApi["listStudents"]>(async () => ({ ok: true, data: { students } }));
}

function cellsOf(row: HTMLElement) {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

function bodyRows() {
  return within(screen.getByRole("table", { name: "生徒の一覧" })).getAllByRole("row").slice(1);
}

describe("本部の生徒", () => {
  test("読み込み中を出し、読めたら各行に表示名・スポーツ・契約プラン・最後の予約・登録日を API の順で出す", async () => {
    const listStudents = listOf(STUDENTS);
    render(<StudentDirectory api={{ listStudents }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    await screen.findByRole("table", { name: "生徒の一覧" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(bodyRows().map(cellsOf)).toEqual([
      ["山田 花子", "テニス", "月額（プラス）", "2026/10/25 10:30:00", "2026/10/21"],
      // ワンショットはプランがなく、予約がなければ「—」
      ["佐藤 一郎", "ゴルフ", "ワンショット", "—", "2026/10/10"],
      // 受講権がなければ「未契約」
      ["鈴木 次郎", "テニス", "未契約", "2026/10/1 18:00:00", "2026/9/30"],
    ]);
    expect(listStudents).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText("スポーツ")).toHaveValue("");
  });

  test("スポーツは「すべて」と 6 種類から選べる", async () => {
    render(<StudentDirectory api={{ listStudents: listOf(STUDENTS) }} />);
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

  test("スポーツの絞り込みは画面の中でし、API を読み直さない", async () => {
    const listStudents = listOf(STUDENTS);
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["山田 花子", "鈴木 次郎"]);

    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    expect(bodyRows().map((row) => cellsOf(row)[0])).toEqual(["佐藤 一郎"]);

    await user.selectOptions(screen.getByLabelText("スポーツ"), "すべて");
    expect(bodyRows()).toHaveLength(3);
    expect(listStudents).toHaveBeenCalledTimes(1);
  });

  test("絞り込んで 0 件なら「このスポーツの生徒はいません」と出し、表は出さない", async () => {
    const user = userEvent.setup();
    render(<StudentDirectory api={{ listStudents: listOf(STUDENTS) }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "水泳");
    expect(screen.getByText("このスポーツの生徒はいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("生徒が 0 人なら「まだ生徒はいません」と出し、表は出さない", async () => {
    render(<StudentDirectory api={{ listStudents: listOf([]) }} />);

    expect(await screen.findByText("まだ生徒はいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("このスポーツの生徒はいません")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listStudents = vi.fn<HeadquartersApi["listStudents"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<StudentDirectory api={{ listStudents }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("まだ生徒はいません")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument());
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
