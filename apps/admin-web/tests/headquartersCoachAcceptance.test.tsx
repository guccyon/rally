import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import {
  CoachAcceptanceHome,
  coachAcceptanceFilterFrom,
  coachAcceptancePath,
} from "@/components/headquarters/CoachAcceptanceHome";
import {
  ACCEPTANCE_STATUSES,
  type CoachAcceptanceFilter,
  CoachAcceptanceList,
  SPORTS,
  coachNameQuery,
} from "@/components/headquarters/CoachAcceptanceList";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは CoachAcceptanceHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
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

function renderList(
  api: Partial<Pick<HeadquartersApi, "listCoaches" | "suspendCoach" | "resumeCoach">>,
  props: { initialFilter?: CoachAcceptanceFilter; onFilterChange?: (filter: CoachAcceptanceFilter) => void } = {},
) {
  render(
    <CoachAcceptanceList
      api={{
        listCoaches: api.listCoaches ?? listOf(COACHES),
        suspendCoach: api.suspendCoach ?? vi.fn(),
        resumeCoach: api.resumeCoach ?? vi.fn(),
      }}
      {...props}
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

  test("止めるのを確定したらそのコーチを送り、応答の状態を行に入れて「再開する」にし、一覧を読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [TAKAHASHI, { ...SUZUKI, acceptance_status: "suspended" }]);
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
    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    expect(listCoaches).toHaveBeenLastCalledWith({});
    // 読み直しのあいだも読み込み中に戻さず、結果の文言も残す。
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("鈴木 コーチの受付を止めました。");
  });

  test("再開を確定したら resumeCoach を送り、行を「受付中」にして「受付を止める」にする", async () => {
    const user = userEvent.setup();
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>();
    const resumeCoach = vi.fn<HeadquartersApi["resumeCoach"]>(async () => ({
      ok: true,
      data: { coach_id: TAKAHASHI.coach_id, acceptance_status: "accepting" },
    }));
    const listCoaches = listOf(COACHES, [{ ...TAKAHASHI, acceptance_status: "accepting" }, SUZUKI]);
    renderList({ listCoaches, suspendCoach, resumeCoach });

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
    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    expect(listCoaches).toHaveBeenLastCalledWith({});
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

  test("絞り込んだまま止めたら今の条件で読み直し、条件に合わなくなった行は一覧から外れる", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI], []);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
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
    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("鈴木 コーチの受付を止めました。");
    expect(screen.getByRole("combobox", { name: "受付の状態" })).toHaveValue("accepting");
  });

  test("渡された条件（URL のクエリ）で開き、絞り込みにその値を出す", async () => {
    const listCoaches = listOf([SUZUKI]);
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "accepting", sport: "golf", q: "" } });

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(listCoaches).toHaveBeenCalledWith({ acceptance_status: "accepting", sport: "golf" });
    expect(screen.getByRole("combobox", { name: "受付の状態" })).toHaveValue("accepting");
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("golf");
  });

  test("渡された条件で開いて 0 件なら「条件に合うコーチはいません」を出す", async () => {
    renderList({ listCoaches: listOf([]) }, { initialFilter: { acceptanceStatus: "", sport: "swimming", q: "" } });

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
  });

  test("絞り込みを変えるたびに、変えた後の条件を知らせる", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    renderList({ listCoaches: listOf(COACHES, [TAKAHASHI], [TAKAHASHI]) }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "suspended", sport: "", q: "" });
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "suspended", sport: "tennis", q: "" });
    expect(onFilterChange).toHaveBeenCalledTimes(2);
  });
});

describe("本部のコーチの受付の表示名の検索", () => {
  test("検索欄は空で開き、入力しただけでは読み直さない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("");
    await user.type(screen.getByRole("textbox", { name: "表示名" }), "鈴木");
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("「探す」で前後の空白を除いた語を q に渡して読み直し、返った行を出す", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "\u3000鈴木 ");
    await user.click(screen.getByRole("button", { name: "探す" }));

    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ q: "鈴木" }));
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "", sport: "", q: "鈴木" });
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("鈴木");
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeUndefined());
    expect(await rowOf("鈴木 コーチ")).toBeDefined();
  });

  test("Enter でも確定し、受付の状態・スポーツと組み合わせる", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [TAKAHASHI], [TAKAHASHI], [TAKAHASHI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    await user.type(screen.getByRole("textbox", { name: "表示名" }), "高橋{Enter}");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", q: "高橋" }));

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", sport: "tennis", q: "高橋" }),
    );
    expect(listCoaches).toHaveBeenCalledTimes(4);
  });

  test("受付の状態・スポーツを変えたときは、確定していない入力を使わない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "鈴木");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");

    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "tennis" }));
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("鈴木");
  });

  test("空にして確定したら q を付けずに読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf([SUZUKI], COACHES);
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "", sport: "", q: "鈴木" } });
    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledWith({ q: "鈴木" });
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("鈴木");

    await user.clear(screen.getByRole("textbox", { name: "表示名" }));
    await user.click(screen.getByRole("button", { name: "探す" }));

    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({}));
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeDefined());
  });

  test("一致しなければ「条件に合うコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    renderList({ listCoaches: listOf(COACHES, []) });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "山田{Enter}");

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
  });

  test("前後の空白を除いて 50 文字を超える語は送らずに理由を出し、直して確定したら理由を消す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });
    const input = screen.getByRole("textbox", { name: "表示名" });

    await user.type(input, "あ".repeat(51));
    await user.click(screen.getByRole("button", { name: "探す" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("表示名は 50 文字までで探してください。");
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(await rowOf("鈴木 コーチ")).toBeDefined();

    await user.clear(input);
    await user.type(input, "鈴木{Enter}");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ q: "鈴木" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  test("サーバーの 422 は ErrorCode の文言に検索語の上限を添えて出す", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力の内容が正しくありません。",
    }));
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "", sport: "", q: "あ".repeat(51) } });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力の内容が正しくありません。表示名は 50 文字までで探してください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("検索語の確かめは前後の空白を除き、コードポイントで数える", () => {
    expect(coachNameQuery(" \u3000鈴木\u3000 ")).toEqual({ ok: true, q: "鈴木" });
    expect(coachNameQuery("   ")).toEqual({ ok: true, q: "" });
    // サロゲートペアの文字も 1 文字と数える。
    expect(coachNameQuery("𠮷".repeat(50))).toEqual({ ok: true, q: "𠮷".repeat(50) });
    expect(coachNameQuery("あ".repeat(51)).ok).toBe(false);
  });
});

describe("コーチの受付の絞り込みと URL", () => {
  test("「すべて」の条件はクエリに出さず、選んだ条件だけを契約のクエリの名前で残す", () => {
    expect(coachAcceptancePath({ acceptanceStatus: "", sport: "", q: "" })).toBe("/headquarters/coaches");
    expect(coachAcceptancePath({ acceptanceStatus: "suspended", sport: "", q: "" })).toBe(
      "/headquarters/coaches?acceptance_status=suspended",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "accepting", sport: "golf", q: "" })).toBe(
      "/headquarters/coaches?acceptance_status=accepting&sport=golf",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "suspended", sport: "tennis", q: "鈴木 コーチ" })).toBe(
      "/headquarters/coaches?acceptance_status=suspended&sport=tennis&q=%E9%88%B4%E6%9C%A8+%E3%82%B3%E3%83%BC%E3%83%81",
    );
  });

  test("クエリから条件を読み、契約の値でなければ「すべて」にする", () => {
    expect(coachAcceptanceFilterFrom(new URLSearchParams("acceptance_status=suspended&sport=tennis"))).toEqual({
      acceptanceStatus: "suspended",
      sport: "tennis",
      q: "",
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams(""))).toEqual({ acceptanceStatus: "", sport: "", q: "" });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("q=%E3%80%80%E9%88%B4%E6%9C%A8+&sport=golf"))).toEqual({
      acceptanceStatus: "",
      sport: "golf",
      q: "鈴木",
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("acceptance_status=paused&sport=chess"))).toEqual({
      acceptanceStatus: "",
      sport: "",
      q: "",
    });
  });

  test("クエリの確かめに使う選択肢は凍結してあり、import した側から書き換えられない", () => {
    expect(Object.isFrozen(ACCEPTANCE_STATUSES)).toBe(true);
    expect(Object.isFrozen(SPORTS)).toBe(true);
  });

  test("画面で絞り込みを変えると、履歴を積まずに URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(<CoachAcceptanceHome token="mock.headquarters.x" initialFilter={{ acceptanceStatus: "", sport: "golf", q: "" }} />);
    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledWith({ sport: "golf" });
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("golf");

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches?acceptance_status=accepting&sport=golf", {
      scroll: false,
    });
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "accepting", sport: "golf" }));
  });

  test("画面で検索語を確定すると、ほかの条件と合わせて URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(
      <CoachAcceptanceHome token="mock.headquarters.x" initialFilter={{ acceptanceStatus: "", sport: "golf", q: "" }} />,
    );
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "鈴木{Enter}");
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches?sport=golf&q=%E9%88%B4%E6%9C%A8", {
      scroll: false,
    });
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "golf", q: "鈴木" }));
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
