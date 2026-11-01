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
  recent_lesson_count: 0,
};
const TAKAHASHI: Schemas["Coach"] = {
  coach_id: "22222222-2222-4222-8222-000000000002",
  display_name: "高橋 コーチ",
  acceptance_status: "suspended",
  recent_lesson_count: 0,
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

describe("本部のコーチの受付の絞り込み", () => {
  test("開いたときは条件を付けずに読み、絞り込みの既定は「すべて」", async () => {
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledWith({});
    expect(screen.getByRole("combobox", { name: "受付の状態" })).toHaveValue("");
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("");
    const statusOptions = within(screen.getByRole("combobox", { name: "受付の状態" }))
      .getAllByRole("option")
      .map((option) => option.textContent);
    expect(statusOptions).toEqual(["すべて", "受付中", "停止中"]);
    const sportOptions = within(screen.getByRole("combobox", { name: "スポーツ" }))
      .getAllByRole("option")
      .map((option) => option.textContent);
    expect(sportOptions).toEqual(["すべて", "テニス", "ゴルフ", "野球", "サッカー", "バスケットボール", "水泳"]);
  });

  test("受付の状態とスポーツを変えるたびに、組み合わせた条件で読み直して返った行を出す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [TAKAHASHI], [TAKAHASHI], [SUZUKI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended" }));
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeDefined());

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", sport: "tennis" }),
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "すべて");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "tennis" }));
    const table = await screen.findByRole("table", { name: "コーチの一覧" });
    await waitFor(() =>
      expect(
        within(table)
          .getAllByRole("row")
          .slice(1)
          .map((row) => within(row).getAllByRole("cell")[0]!.textContent),
      ).toEqual(["鈴木 コーチ"]),
    );
    expect(listCoaches).toHaveBeenCalledTimes(4);
  });

  test("続けて変えたときは、最後の条件の結果だけを出す", async () => {
    const user = userEvent.setup();
    let resolveSlow: (value: Awaited<ReturnType<HeadquartersApi["listCoaches"]>>) => void = () => {};
    const listCoaches = vi
      .fn<HeadquartersApi["listCoaches"]>()
      .mockResolvedValueOnce({ ok: true, data: { coaches: COACHES } })
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveSlow = resolve;
          }),
      )
      .mockResolvedValueOnce({ ok: true, data: { coaches: [SUZUKI] } });
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    await waitFor(async () => expect(await rowOf("鈴木 コーチ")).toBeDefined());

    // 前の条件（停止中）の応答が遅れて返っても上書きしない。
    resolveSlow({ ok: true, data: { coaches: [TAKAHASHI] } });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(await rowOf("鈴木 コーチ")).toBeDefined();
    expect(await rowOf("高橋 コーチ")).toBeUndefined();
  });

  test("絞り込んで 0 件なら「条件に合うコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    renderList({ listCoaches: listOf(COACHES, []) });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "水泳");

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
  });

  test("絞り込んだ読み直しが失敗したときはその文言だけを出す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    listCoaches.mockResolvedValueOnce({
      ok: false,
      code: "validation_failed",
      message: "入力内容を確かめてください。",
    });
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");

    expect(await screen.findByRole("alert")).toHaveTextContent("入力内容を確かめてください。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("条件に合うコーチはいません")).not.toBeInTheDocument();
  });

  test("絞り込んだまま 409 で取り直すときは、今の条件で読む", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI], [{ ...SUZUKI, acceptance_status: "suspended" }]);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: false,
      code: "coach_already_suspended",
      message: "このコーチの受付はすでに停止しています。",
    }));
    renderList({ listCoaches, suspendCoach });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeUndefined());
    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(3));
    expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "accepting" });
    await waitFor(async () => expect(statusOf(await rowOf("鈴木 コーチ"))).toBe("停止中"));
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
