import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { IncompleteCoachProfileList } from "@/components/headquarters/IncompleteCoachProfileList";
import { IncompleteCoachProfilesHome } from "@/components/headquarters/IncompleteCoachProfilesHome";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは IncompleteCoachProfilesHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

// API が返す並び（登録の古い順）。画面はこの順のまま出す。
const COACHES: Schemas["IncompleteCoachProfile"][] = [
  {
    coach_id: "22222222-2222-4222-8222-222222222222",
    display_name: "佐藤 健",
    missing_fields: ["bio"],
    updated_at: "2026-10-30T01:00:00Z",
    registered_at: "2026-09-01T00:00:00Z",
  },
  {
    coach_id: "77777777-7777-4777-8777-777777777777",
    display_name: "高橋 美咲",
    missing_fields: ["bio"],
    updated_at: null,
    registered_at: "2026-10-05T03:30:00Z",
  },
];

function listOf(coaches: Schemas["IncompleteCoachProfile"][]) {
  return vi.fn<HeadquartersApi["listIncompleteCoachProfiles"]>(async () => ({ ok: true, data: { coaches } }));
}

describe("本部の自己紹介の足りないコーチ", () => {
  test("読み込み中を出し、読めたらコーチ・空の項目・最後に更新した日時を API の順で表にする", async () => {
    const listIncompleteCoachProfiles = listOf(COACHES);
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(listIncompleteCoachProfiles).toHaveBeenCalledTimes(1);
    expect(within(table).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "コーチ",
      "空の項目",
      "最後に更新した日時",
    ]);
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent))).toEqual([
      ["佐藤 健", "自己紹介文", formatDateTime("2026-10-30T01:00:00Z")],
      ["高橋 美咲", "自己紹介文", `記録なし（登録: ${formatDateTime("2026-10-05T03:30:00Z")}）`],
    ]);
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("コーチの名前からそのコーチの様子へ移れる", async () => {
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf(COACHES) }} />);

    const table = await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(within(table).getByRole("link", { name: "佐藤 健" })).toHaveAttribute(
      "href",
      "/headquarters/coach-overview?coach_id=22222222-2222-4222-8222-222222222222",
    );
    expect(within(table).getByRole("link", { name: "高橋 美咲" })).toHaveAttribute(
      "href",
      "/headquarters/coach-overview?coach_id=77777777-7777-4777-8777-777777777777",
    );
  });

  test("0 件なら「自己紹介の足りないコーチはいません」と出し、表は出さない", async () => {
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf([]) }} />);

    expect(await screen.findByText("自己紹介の足りないコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "自己紹介の足りないコーチの一覧" })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、表は出さない", async () => {
    const listIncompleteCoachProfiles = vi.fn<HeadquartersApi["listIncompleteCoachProfiles"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table", { name: "自己紹介の足りないコーチの一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("自己紹介の足りないコーチはいません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("コーチへ連絡する操作は出さない", async () => {
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf(COACHES) }} />);

    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("本部の画面から自己紹介の足りないコーチの画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "自己紹介の足りないコーチ" });
    expect(within(region).getByRole("link", { name: "自己紹介が書きかけのコーチを見る" })).toHaveAttribute(
      "href",
      "/headquarters/coaches/incomplete-profiles",
    );
  });

  test("自己紹介の足りないコーチの画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({ listIncompleteCoachProfiles: listOf(COACHES) });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <IncompleteCoachProfilesHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "自己紹介の足りないコーチ" });
    expect(await within(region).findByRole("table", { name: "自己紹介の足りないコーチの一覧" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(replace).not.toHaveBeenCalled();
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listIncompleteCoachProfiles: listOf(COACHES) });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <IncompleteCoachProfilesHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "自己紹介の足りないコーチ" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
