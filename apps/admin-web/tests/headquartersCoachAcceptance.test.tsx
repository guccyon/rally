import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { CoachAcceptanceHome } from "@/components/headquarters/CoachAcceptanceHome";
import { CoachAcceptanceList } from "@/components/headquarters/CoachAcceptanceList";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const SUZUKI: Schemas["Coach"] = {
  coach_id: "22222222-2222-4222-8222-000000000001",
  display_name: "鈴木 コーチ",
  acceptance_status: "accepting",
};
const TAKAHASHI: Schemas["Coach"] = {
  coach_id: "22222222-2222-4222-8222-000000000002",
  display_name: "高橋 コーチ",
  acceptance_status: "suspended",
};
// API の並び（表示名のコードポイントの順）。
const COACHES = [TAKAHASHI, SUZUKI];

function listOf(...lists: Schemas["Coach"][][]) {
  const fn = vi.fn<HeadquartersApi["listCoaches"]>();
  for (const coaches of lists) fn.mockResolvedValueOnce({ ok: true, data: { coaches } });
  return fn;
}

function renderList(api: Partial<Pick<HeadquartersApi, "listCoaches" | "suspendCoach" | "resumeCoach">>) {
  render(
    <CoachAcceptanceList
      api={{
        listCoaches: api.listCoaches ?? listOf(COACHES),
        suspendCoach: api.suspendCoach ?? vi.fn(),
        resumeCoach: api.resumeCoach ?? vi.fn(),
      }}
    />,
  );
}

async function rowOf(name: string) {
  const table = await screen.findByRole("table", { name: "コーチの一覧" });
  return within(table)
    .getAllByRole("row")
    .find((row) => within(row).queryAllByRole("cell")[0]?.textContent === name)!;
}

function statusOf(row: HTMLElement) {
  return within(row).getAllByRole("cell")[1]!.textContent;
}

describe("本部のコーチの受付", () => {
  test("読み込み中を出し、読めたら API の順で各行に表示名と受付の状態と、状態に応じたボタンを出す", async () => {
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "コーチの一覧" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    const names = within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[0]!.textContent);
    expect(names).toEqual(["高橋 コーチ", "鈴木 コーチ"]);

    const suzuki = await rowOf("鈴木 コーチ");
    expect(statusOf(suzuki)).toBe("受付中");
    expect(within(suzuki).getByRole("button", { name: "受付を止める" })).toBeInTheDocument();
    expect(within(suzuki).queryByRole("button", { name: "再開する" })).not.toBeInTheDocument();

    const takahashi = await rowOf("高橋 コーチ");
    expect(statusOf(takahashi)).toBe("停止中");
    expect(within(takahashi).getByRole("button", { name: "再開する" })).toBeInTheDocument();
    expect(within(takahashi).queryByRole("button", { name: "受付を止める" })).not.toBeInTheDocument();
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("各行の表示名は、そのコーチの様子へのリンク", async () => {
    renderList({});

    const row = await rowOf("鈴木 コーチ");
    expect(within(row).getByRole("link", { name: "鈴木 コーチ" })).toHaveAttribute(
      "href",
      `/headquarters/coach-overview?coach_id=${SUZUKI.coach_id}`,
    );
  });

  test("押したら行の中で確かめ、「やめる」なら送らない（ブラウザの confirm は使わない）", async () => {
    const user = userEvent.setup();
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>();
    const confirmSpy = vi.spyOn(window, "confirm");
    renderList({ suspendCoach });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));

    const group = within(row).getByRole("group", { name: "コーチの受付の確認" });
    expect(group).toHaveTextContent("このコーチの受付を止めますか？");
    expect(suspendCoach).not.toHaveBeenCalled();
    expect(confirmSpy).not.toHaveBeenCalled();

    await user.click(within(group).getByRole("button", { name: "やめる" }));
    expect(within(row).queryByRole("group")).not.toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "受付を止める" })).toBeInTheDocument();
    expect(suspendCoach).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  test("止めるのを確定したらそのコーチを送り、応答の状態を行に入れて「再開する」にする。一覧は取り直さない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
    }));
    renderList({ listCoaches, suspendCoach });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    expect(suspendCoach).toHaveBeenCalledWith(SUZUKI.coach_id);
    expect(await screen.findByRole("status")).toHaveTextContent("鈴木 コーチの受付を止めました。");
    const updated = await rowOf("鈴木 コーチ");
    expect(statusOf(updated)).toBe("停止中");
    expect(within(updated).getByRole("button", { name: "再開する" })).toBeInTheDocument();
    expect(within(updated).queryByRole("group")).not.toBeInTheDocument();
    // 他の行はそのまま。
    expect(statusOf(await rowOf("高橋 コーチ"))).toBe("停止中");
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("再開を確定したら resumeCoach を送り、行を「受付中」にして「受付を止める」にする", async () => {
    const user = userEvent.setup();
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>();
    const resumeCoach = vi.fn<HeadquartersApi["resumeCoach"]>(async () => ({
      ok: true,
      data: { coach_id: TAKAHASHI.coach_id, acceptance_status: "accepting" },
    }));
    renderList({ suspendCoach, resumeCoach });

    const row = await rowOf("高橋 コーチ");
    await user.click(within(row).getByRole("button", { name: "再開する" }));
    expect(within(row).getByRole("group", { name: "コーチの受付の確認" })).toHaveTextContent(
      "このコーチの受付を再開しますか？",
    );
    await user.click(within(row).getByRole("button", { name: "再開を確定" }));

    expect(resumeCoach).toHaveBeenCalledWith(TAKAHASHI.coach_id);
    expect(suspendCoach).not.toHaveBeenCalled();
    expect(await screen.findByRole("status")).toHaveTextContent("高橋 コーチの受付を再開しました。");
    const updated = await rowOf("高橋 コーチ");
    expect(statusOf(updated)).toBe("受付中");
    expect(within(updated).getByRole("button", { name: "受付を止める" })).toBeInTheDocument();
  });

  test("409 はサーバーの ErrorCode の文言を出し、一覧を今の状態に取り直す", async () => {
    const user = userEvent.setup();
    // 読み込んだ後に、別の画面で鈴木 コーチが止められた。
    const listCoaches = listOf(COACHES, [TAKAHASHI, { ...SUZUKI, acceptance_status: "suspended" }]);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: false,
      code: "coach_already_suspended",
      message: "このコーチの受付はすでに停止しています。",
    }));
    renderList({ listCoaches, suspendCoach });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("このコーチの受付はすでに停止しています。");
    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    const updated = await rowOf("鈴木 コーチ");
    await waitFor(() => expect(statusOf(updated)).toBe("停止中"));
    expect(within(updated).getByRole("button", { name: "再開する" })).toBeInTheDocument();
  });

  test("接続できないときは文言を出し、行はそのまま（一覧は取り直さない）", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    renderList({ listCoaches, suspendCoach });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(statusOf(await rowOf("鈴木 コーチ"))).toBe("受付中");
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("コーチが 0 人なら「コーチがまだいません」を出し、表は出さない", async () => {
    renderList({ listCoaches: listOf([]) });

    expect(await screen.findByText("コーチがまだいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("一覧の API が失敗したときはその文言だけを出す", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      code: "forbidden",
      message: "権限がないためアクセスできません。",
    }));
    renderList({ listCoaches });

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("本部の画面からコーチの受付の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "受付を止める・再開する" })).toHaveAttribute(
      "href",
      "/headquarters/coaches",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <CoachAcceptanceHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "コーチの受付" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
