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
  recent_lesson_count: 12,
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

function recentLessonsOf(row: HTMLElement) {
  return within(row).getAllByRole("cell")[2]!.textContent;
}

const RECENT_LESSON_COUNT_NOTE =
  "直近 30 日のレッスンは、今日までの 30 日に完了したビデオ通話（同期レッスン）と動画添削（非同期レッスン）の合計です。";

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

    expect(suspendCoach).toHaveBeenCalledWith(SUZUKI.coach_id, {});
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

  test("止める確認で理由（任意）を書いて確定すると、理由を付けて送る", async () => {
    const user = userEvent.setup();
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
    }));
    renderList({
      listCoaches: listOf(COACHES, [TAKAHASHI, { ...SUZUKI, acceptance_status: "suspended" }]),
      suspendCoach,
    });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    const group = within(row).getByRole("group", { name: "コーチの受付の確認" });
    await user.type(within(group).getByLabelText("理由（任意）"), "連絡がつかない");
    await user.click(within(group).getByRole("button", { name: "停止を確定" }));

    expect(suspendCoach).toHaveBeenCalledWith(SUZUKI.coach_id, { reason: "連絡がつかない" });
    expect(await screen.findByRole("status")).toHaveTextContent("鈴木 コーチの受付を止めました。");
  });

  test("「やめる」で送らずに戻り、書いた理由は残さない", async () => {
    const user = userEvent.setup();
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>();
    renderList({ suspendCoach });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.type(within(row).getByLabelText("理由（任意）"), "下書き");
    await user.click(within(row).getByRole("button", { name: "やめる" }));

    expect(suspendCoach).not.toHaveBeenCalled();
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    expect(within(row).getByLabelText("理由（任意）")).toHaveValue("");
  });

  test("止める理由は 200 文字（コードポイント）まで。超えたら確定を押せず、超えた文字数を出す", async () => {
    const user = userEvent.setup();
    renderList({});

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    const reason = within(row).getByLabelText("理由（任意）");
    expect(within(row).getByText("あと 200 文字")).toBeInTheDocument();

    // サロゲートペアの絵文字も 1 文字と数える
    await user.click(reason);
    await user.paste("🎾".repeat(200));
    expect(within(row).getByText("あと 0 文字")).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "停止を確定" })).toBeEnabled();

    await user.paste("あ");
    expect(within(row).getByText("200 文字を 1 文字超えています")).toBeInTheDocument();
    expect(reason).toHaveAttribute("aria-invalid", "true");
    expect(within(row).getByRole("button", { name: "停止を確定" })).toBeDisabled();
  });

  test("再開の確認には理由の入力を出さない", async () => {
    const user = userEvent.setup();
    renderList({});

    const row = await rowOf("高橋 コーチ");
    await user.click(within(row).getByRole("button", { name: "再開する" }));
    expect(within(row).getByRole("group", { name: "コーチの受付の確認" })).toBeInTheDocument();
    expect(within(row).queryByLabelText("理由（任意）")).not.toBeInTheDocument();
    expect(within(row).queryByText(/文字/)).not.toBeInTheDocument();
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

  test("各行に直近 30 日のレッスンの数を「n 回」で出し（0 も「0 回」）、表の上に数え方を添える", async () => {
    renderList({});

    const table = await screen.findByRole("table", { name: "コーチの一覧" });
    expect(within(table).getByRole("columnheader", { name: "直近 30 日のレッスン" })).toBeInTheDocument();
    expect(recentLessonsOf(await rowOf("鈴木 コーチ"))).toBe("12 回");
    expect(recentLessonsOf(await rowOf("高橋 コーチ"))).toBe("0 回");
    expect(screen.getByText(RECENT_LESSON_COUNT_NOTE)).toBeInTheDocument();
  });

  test("コーチが 0 人なら「コーチがまだいません」を出し、表と数え方の説明は出さない", async () => {
    renderList({ listCoaches: listOf([]) });

    expect(await screen.findByText("コーチがまだいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(RECENT_LESSON_COUNT_NOTE)).not.toBeInTheDocument();
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
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "accepting", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } });

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(listCoaches).toHaveBeenCalledWith({ acceptance_status: "accepting", sport: "golf" });
    expect(screen.getByRole("combobox", { name: "受付の状態" })).toHaveValue("accepting");
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("golf");
  });

  test("渡された条件で開いて 0 件なら「条件に合うコーチはいません」を出す", async () => {
    renderList({ listCoaches: listOf([]) }, { initialFilter: { acceptanceStatus: "", sport: "swimming", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } });

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
  });

  test("絞り込みを変えるたびに、変えた後の条件を知らせる", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    renderList({ listCoaches: listOf(COACHES, [TAKAHASHI], [TAKAHASHI]) }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "suspended", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "suspended", sport: "tennis", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
    expect(onFilterChange).toHaveBeenCalledTimes(2);
  });
});

const INACTIVE_ONLY = "直近 30 日にレッスンのないコーチだけ";

describe("本部のコーチの受付の直近 30 日にレッスンのないコーチだけ", () => {
  test("開いたときは外れていて、入れたら inactive=true を付けて読み直し、外したら付けずに読み直す", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES, [TAKAHASHI], COACHES);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });
    const toggle = screen.getByRole("checkbox", { name: INACTIVE_ONLY });
    expect(toggle).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledWith({});

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ inactive: true }));
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "", sport: "", q: "", inactive: true, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
    expect(toggle).toBeChecked();
    await waitFor(async () => expect(await rowOf("鈴木 コーチ")).toBeUndefined());
    expect(recentLessonsOf((await rowOf("高橋 コーチ"))!)).toBe("0 回");

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({}));
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
    await waitFor(async () => expect(await rowOf("鈴木 コーチ")).toBeDefined());
    expect(listCoaches).toHaveBeenCalledTimes(3);
  });

  test("受付の状態・スポーツ・検索欄の語と組み合わせ、どれを変えても inactive=true を付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [TAKAHASHI], [TAKAHASHI], [TAKAHASHI], [TAKAHASHI], [TAKAHASHI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended" }));

    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", inactive: true }),
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", sport: "tennis", inactive: true }),
    );

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "高橋{Enter}");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "suspended",
        sport: "tennis",
        q: "高橋",
        inactive: true,
      }),
    );

    // 検索欄に入れただけの語も、切り替えたときに一緒に確定する。
    await user.clear(screen.getByRole("textbox", { name: "表示名" }));
    await user.type(screen.getByRole("textbox", { name: "表示名" }), "  高 ");
    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", sport: "tennis", q: "高" }),
    );
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("高");
    expect(listCoaches).toHaveBeenCalledTimes(6);
  });

  test("検索欄の語が上限を超えたまま切り替えたら、理由を出して条件を変えない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "あ".repeat(51));
    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));

    expect(await screen.findByRole("alert")).toHaveTextContent("表示名は 50 文字までで探してください。");
    expect(screen.getByRole("checkbox", { name: INACTIVE_ONLY })).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("該当がなければ「直近 30 日にレッスンのないコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    renderList({ listCoaches: listOf(COACHES, []) });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));

    expect(await screen.findByText("直近 30 日にレッスンのないコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
  });

  test("ほかの条件と組み合わせて該当がなければ、直近 30 日にレッスンのないコーチのうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "swimming", q: "", inactive: true, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } },
    );

    expect(
      await screen.findByText("直近 30 日にレッスンのないコーチのうち、条件に合うコーチはいません"),
    ).toBeInTheDocument();
  });

  test("渡された条件（URL のクエリ）で入れた状態で開き、inactive=true を付けて読む", async () => {
    const listCoaches = listOf([TAKAHASHI]);
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: true, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } });

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(listCoaches).toHaveBeenCalledWith({ inactive: true });
    expect(screen.getByRole("checkbox", { name: INACTIVE_ONLY })).toBeChecked();
  });

  test("入れたまま止めたら、inactive=true を付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf([SUZUKI], [{ ...SUZUKI, acceptance_status: "suspended" }]);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
    }));
    renderList({ listCoaches, suspendCoach }, { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: true, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } });

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    expect(listCoaches).toHaveBeenLastCalledWith({ inactive: true });
  });
});

const UNHANDLED_LOW_RATED_ONLY = "未対応の低い評価のあるコーチだけ";

describe("本部のコーチの受付の未対応の低い評価のあるコーチだけ", () => {
  test("開いたときは外れていて、入れたら has_unhandled_low_rated=true を付けて読み直し、外したら付けずに読み直す", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES, [SUZUKI], COACHES);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });
    const toggle = screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY });
    expect(toggle).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledWith({});

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ has_unhandled_low_rated: true }));
    expect(onFilterChange).toHaveBeenLastCalledWith({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: true,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    expect(toggle).toBeChecked();
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeUndefined());
    expect(await rowOf("鈴木 コーチ")).toBeDefined();

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({}));
    expect(onFilterChange).toHaveBeenLastCalledWith({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeDefined());
    expect(listCoaches).toHaveBeenCalledTimes(3);
  });

  test("受付の状態・スポーツ・検索欄の語・直近 30 日にレッスンのないコーチだけと組み合わせ、どれを変えても付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ has_unhandled_low_rated: true }));

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "accepting", has_unhandled_low_rated: true }),
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        has_unhandled_low_rated: true,
      }),
    );

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "鈴木{Enter}");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        has_unhandled_low_rated: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        inactive: true,
        has_unhandled_low_rated: true,
      }),
    );

    // 検索欄に入れただけの語も、切り替えたときに一緒に確定する。
    await user.clear(screen.getByRole("textbox", { name: "表示名" }));
    await user.type(screen.getByRole("textbox", { name: "表示名" }), "  鈴 ");
    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴",
        inactive: true,
      }),
    );
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("鈴");
    expect(listCoaches).toHaveBeenCalledTimes(7);
  });

  test("検索欄の語が上限を超えたまま切り替えたら、理由を出して条件を変えない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "あ".repeat(51));
    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));

    expect(await screen.findByRole("alert")).toHaveTextContent("表示名は 50 文字までで探してください。");
    expect(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY })).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("該当がなければ「未対応の低い評価のあるコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    renderList({ listCoaches: listOf(COACHES, []) });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));

    expect(await screen.findByText("未対応の低い評価のあるコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
  });

  test("ほかの条件と組み合わせて該当がなければ、未対応の低い評価のあるコーチのうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "suspended", sport: "", q: "", inactive: false, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false } },
    );

    expect(
      await screen.findByText("未対応の低い評価のあるコーチのうち、条件に合うコーチはいません"),
    ).toBeInTheDocument();
  });

  test("直近 30 日にレッスンのないコーチだけと両方入れて該当がなければ、両方の条件を文言に含める", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: true, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false } },
    );
    expect(
      await screen.findByText("直近 30 日にレッスンがなく、未対応の低い評価のあるコーチはいません"),
    ).toBeInTheDocument();
  });

  test("両方入れてほかの条件とも組み合わせて該当がなければ、両方の条件のうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "鈴木", inactive: true, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false } },
    );
    expect(
      await screen.findByText("直近 30 日にレッスンがなく、未対応の低い評価のあるコーチのうち、条件に合うコーチはいません"),
    ).toBeInTheDocument();
  });

  test("渡された条件（URL のクエリ）で入れた状態で開き、has_unhandled_low_rated=true を付けて読む", async () => {
    const listCoaches = listOf([SUZUKI]);
    renderList(
      { listCoaches },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false } },
    );

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(listCoaches).toHaveBeenCalledWith({ has_unhandled_low_rated: true });
    expect(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: INACTIVE_ONLY })).not.toBeChecked();
  });

  test("入れたまま止めたら、has_unhandled_low_rated=true を付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf([SUZUKI], [{ ...SUZUKI, acceptance_status: "suspended" }]);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
    }));
    renderList(
      { listCoaches, suspendCoach },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false } },
    );

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    expect(listCoaches).toHaveBeenLastCalledWith({ has_unhandled_low_rated: true });
  });
});

const UNANSWERED_ONLY = "返事のないメッセージのあるコーチだけ";

describe("本部のコーチの受付の返事のないメッセージのあるコーチだけ", () => {
  test("開いたときは外れていて、入れたら has_unanswered=true を付けて読み直し、外したら付けずに読み直す", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES, [SUZUKI], COACHES);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });
    const toggle = screen.getByRole("checkbox", { name: UNANSWERED_ONLY });
    expect(toggle).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledWith({});

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ has_unanswered: true }));
    expect(onFilterChange).toHaveBeenLastCalledWith({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: true,
      hasOverdueVideo: false,
    });
    expect(toggle).toBeChecked();
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeUndefined());
    expect(await rowOf("鈴木 コーチ")).toBeDefined();

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({}));
    expect(onFilterChange).toHaveBeenLastCalledWith({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeDefined());
    expect(listCoaches).toHaveBeenCalledTimes(3);
  });

  test("受付の状態・スポーツ・検索欄の語・ほかの切り替えと組み合わせ、どれを変えても付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: UNANSWERED_ONLY }));
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ has_unanswered: true }));

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "accepting", has_unanswered: true }),
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        has_unanswered: true,
      }),
    );

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "鈴木{Enter}");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        has_unanswered: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        inactive: true,
        has_unanswered: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        inactive: true,
        has_unhandled_low_rated: true,
        has_unanswered: true,
      }),
    );

    // 検索欄に入れただけの語も、切り替えたときに一緒に確定する。
    await user.clear(screen.getByRole("textbox", { name: "表示名" }));
    await user.type(screen.getByRole("textbox", { name: "表示名" }), "  鈴 ");
    await user.click(screen.getByRole("checkbox", { name: UNANSWERED_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴",
        inactive: true,
        has_unhandled_low_rated: true,
      }),
    );
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("鈴");
    expect(listCoaches).toHaveBeenCalledTimes(8);
  });

  test("検索欄の語が上限を超えたまま切り替えたら、理由を出して条件を変えない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "あ".repeat(51));
    await user.click(screen.getByRole("checkbox", { name: UNANSWERED_ONLY }));

    expect(await screen.findByRole("alert")).toHaveTextContent("表示名は 50 文字までで探してください。");
    expect(screen.getByRole("checkbox", { name: UNANSWERED_ONLY })).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("該当がなければ「返事のないメッセージのあるコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    renderList({ listCoaches: listOf(COACHES, []) });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: UNANSWERED_ONLY }));

    expect(await screen.findByText("返事のないメッセージのあるコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
  });

  test("ほかの条件と組み合わせて該当がなければ、返事のないメッセージのあるコーチのうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "tennis", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: true, hasOverdueVideo: false } },
    );

    expect(
      await screen.findByText("返事のないメッセージのあるコーチのうち、条件に合うコーチはいません"),
    ).toBeInTheDocument();
  });

  test("ほかの切り替えと一緒に入れて該当がなければ、入れた切り替えの条件をすべて文言に含める", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: true, hasUnanswered: true, hasOverdueVideo: false } },
    );
    expect(
      await screen.findByText("未対応の低い評価があり、返事のないメッセージのあるコーチはいません"),
    ).toBeInTheDocument();
  });

  test("3 つの切り替えをすべて入れてほかの条件とも組み合わせて該当がなければ、すべての条件のうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "suspended", sport: "", q: "", inactive: true, hasUnhandledLowRated: true, hasUnanswered: true, hasOverdueVideo: false } },
    );
    expect(
      await screen.findByText(
        "直近 30 日にレッスンがなく、未対応の低い評価があり、返事のないメッセージのあるコーチのうち、条件に合うコーチはいません",
      ),
    ).toBeInTheDocument();
  });

  test("渡された条件（URL のクエリ）で入れた状態で開き、has_unanswered=true を付けて読む", async () => {
    const listCoaches = listOf([SUZUKI]);
    renderList(
      { listCoaches },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: true, hasOverdueVideo: false } },
    );

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(listCoaches).toHaveBeenCalledWith({ has_unanswered: true });
    expect(screen.getByRole("checkbox", { name: UNANSWERED_ONLY })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY })).not.toBeChecked();
  });

  test("入れたまま止めたら、has_unanswered=true を付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf([SUZUKI], [{ ...SUZUKI, acceptance_status: "suspended" }]);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
    }));
    renderList(
      { listCoaches, suspendCoach },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: true, hasOverdueVideo: false } },
    );

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    expect(listCoaches).toHaveBeenLastCalledWith({ has_unanswered: true });
  });
});

const OVERDUE_VIDEO_ONLY = "返却の遅れている動画のあるコーチだけ";

describe("本部のコーチの受付の返却の遅れている動画のあるコーチだけ", () => {
  test("開いたときは外れていて、入れたら has_overdue_video=true を付けて読み直し、外したら付けずに読み直す", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES, [SUZUKI], COACHES);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });
    const toggle = screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY });
    expect(toggle).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledWith({});

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ has_overdue_video: true }));
    expect(onFilterChange).toHaveBeenLastCalledWith({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: true,
    });
    expect(toggle).toBeChecked();
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeUndefined());
    expect(await rowOf("鈴木 コーチ")).toBeDefined();

    await user.click(toggle);
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({}));
    expect(onFilterChange).toHaveBeenLastCalledWith({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    await waitFor(async () => expect(await rowOf("高橋 コーチ")).toBeDefined());
    expect(listCoaches).toHaveBeenCalledTimes(3);
  });

  test("受付の状態・スポーツ・検索欄の語・ほかの切り替えと組み合わせ、どれを変えても付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI], [SUZUKI]);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY }));
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ has_overdue_video: true }));

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "accepting", has_overdue_video: true }),
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        has_overdue_video: true,
      }),
    );

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "鈴木{Enter}");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        has_overdue_video: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        inactive: true,
        has_overdue_video: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        inactive: true,
        has_unhandled_low_rated: true,
        has_overdue_video: true,
      }),
    );

    await user.click(screen.getByRole("checkbox", { name: UNANSWERED_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴木",
        inactive: true,
        has_unhandled_low_rated: true,
        has_unanswered: true,
        has_overdue_video: true,
      }),
    );

    // 検索欄に入れただけの語も、切り替えたときに一緒に確定する。
    await user.clear(screen.getByRole("textbox", { name: "表示名" }));
    await user.type(screen.getByRole("textbox", { name: "表示名" }), "  鈴 ");
    await user.click(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY }));
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({
        acceptance_status: "accepting",
        sport: "tennis",
        q: "鈴",
        inactive: true,
        has_unhandled_low_rated: true,
        has_unanswered: true,
      }),
    );
    expect(screen.getByRole("textbox", { name: "表示名" })).toHaveValue("鈴");
    expect(listCoaches).toHaveBeenCalledTimes(9);
  });

  test("検索欄の語が上限を超えたまま切り替えたら、理由を出して条件を変えない", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "あ".repeat(51));
    await user.click(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY }));

    expect(await screen.findByRole("alert")).toHaveTextContent("表示名は 50 文字までで探してください。");
    expect(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY })).not.toBeChecked();
    expect(listCoaches).toHaveBeenCalledTimes(1);
  });

  test("該当がなければ「返却の遅れている動画のあるコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    renderList({ listCoaches: listOf(COACHES, []) });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY }));

    expect(await screen.findByText("返却の遅れている動画のあるコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("コーチがまだいません")).not.toBeInTheDocument();
  });

  test("ほかの条件と組み合わせて該当がなければ、返却の遅れている動画のあるコーチのうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "tennis", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: true } },
    );

    expect(
      await screen.findByText("返却の遅れている動画のあるコーチのうち、条件に合うコーチはいません"),
    ).toBeInTheDocument();
  });

  test("ほかの切り替えと一緒に入れて該当がなければ、入れた切り替えの条件をすべて文言に含める", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: true, hasOverdueVideo: true } },
    );
    expect(
      await screen.findByText("返事のないメッセージがあり、返却の遅れている動画のあるコーチはいません"),
    ).toBeInTheDocument();
  });

  test("4 つの切り替えをすべて入れてほかの条件とも組み合わせて該当がなければ、すべての条件のうち条件に合う人がいないと出す", async () => {
    renderList(
      { listCoaches: listOf([]) },
      { initialFilter: { acceptanceStatus: "suspended", sport: "", q: "", inactive: true, hasUnhandledLowRated: true, hasUnanswered: true, hasOverdueVideo: true } },
    );
    expect(
      await screen.findByText(
        "直近 30 日にレッスンがなく、未対応の低い評価があり、返事のないメッセージがあり、返却の遅れている動画のあるコーチのうち、条件に合うコーチはいません",
      ),
    ).toBeInTheDocument();
  });

  test("渡された条件（URL のクエリ）で入れた状態で開き、has_overdue_video=true を付けて読む", async () => {
    const listCoaches = listOf([SUZUKI]);
    renderList(
      { listCoaches },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: true } },
    );

    await screen.findByRole("table", { name: "コーチの一覧" });
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(listCoaches).toHaveBeenCalledWith({ has_overdue_video: true });
    expect(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: UNANSWERED_ONLY })).not.toBeChecked();
  });

  test("入れたまま止めたら、has_overdue_video=true を付けたまま読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf([SUZUKI], [{ ...SUZUKI, acceptance_status: "suspended" }]);
    const suspendCoach = vi.fn<HeadquartersApi["suspendCoach"]>(async () => ({
      ok: true,
      data: { coach_id: SUZUKI.coach_id, acceptance_status: "suspended" },
    }));
    renderList(
      { listCoaches, suspendCoach },
      { initialFilter: { acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: true } },
    );

    const row = await rowOf("鈴木 コーチ");
    await user.click(within(row).getByRole("button", { name: "受付を止める" }));
    await user.click(within(row).getByRole("button", { name: "停止を確定" }));

    await waitFor(() => expect(listCoaches).toHaveBeenCalledTimes(2));
    expect(listCoaches).toHaveBeenLastCalledWith({ has_overdue_video: true });
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
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "", sport: "", q: "鈴木", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
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

  test("受付の状態・スポーツを変えたときは、検索欄に入っている語も一緒に確定して、欄の語と条件を一致させる", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES, [SUZUKI], [], [SUZUKI]);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });
    const input = screen.getByRole("textbox", { name: "表示名" });

    await user.type(input, " 鈴木\u3000");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "tennis", q: "鈴木" }));
    expect(onFilterChange).toHaveBeenLastCalledWith({ acceptanceStatus: "", sport: "tennis", q: "鈴木", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
    expect(input).toHaveValue("鈴木");

    await user.clear(input);
    await user.type(input, "高橋");
    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    await waitFor(() =>
      expect(listCoaches).toHaveBeenLastCalledWith({ acceptance_status: "suspended", sport: "tennis", q: "高橋" }),
    );
    expect(input).toHaveValue("高橋");

    // 欄を空にしてから変えたら、検索語の条件も外す。
    await user.clear(input);
    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "すべて");
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "tennis" }));
    expect(listCoaches).toHaveBeenCalledTimes(4);
  });

  test("検索欄の語が上限を超えたまま受付の状態・スポーツを変えたら、理由を出して条件を変えない", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listCoaches = listOf(COACHES);
    renderList({ listCoaches }, { onFilterChange });
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.type(screen.getByRole("textbox", { name: "表示名" }), "あ".repeat(51));
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");

    expect(await screen.findByRole("alert")).toHaveTextContent("表示名は 50 文字までで探してください。");
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("");
    expect(listCoaches).toHaveBeenCalledTimes(1);
    expect(onFilterChange).not.toHaveBeenCalled();
  });

  test("空にして確定したら q を付けずに読み直す", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf([SUZUKI], COACHES);
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "", sport: "", q: "鈴木", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } });
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
    renderList({ listCoaches }, { initialFilter: { acceptanceStatus: "", sport: "", q: "あ".repeat(51), inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false } });

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
    expect(coachAcceptancePath({ acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false })).toBe("/headquarters/coaches");
    expect(coachAcceptancePath({ acceptanceStatus: "suspended", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?acceptance_status=suspended",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "accepting", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?acceptance_status=accepting&sport=golf",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "suspended", sport: "tennis", q: "鈴木 コーチ", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?acceptance_status=suspended&sport=tennis&q=%E9%88%B4%E6%9C%A8+%E3%82%B3%E3%83%BC%E3%83%81",
    );
  });

  test("クエリから条件を読み、契約の値でなければ「すべて」にする", () => {
    expect(coachAcceptanceFilterFrom(new URLSearchParams("acceptance_status=suspended&sport=tennis"))).toEqual({
      acceptanceStatus: "suspended",
      sport: "tennis",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams(""))).toEqual({ acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("q=%E3%80%80%E9%88%B4%E6%9C%A8+&sport=golf"))).toEqual({
      acceptanceStatus: "",
      sport: "golf",
      q: "鈴木",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("acceptance_status=paused&sport=chess"))).toEqual({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
  });

  test("直近 30 日にレッスンのないコーチだけは inactive=true で残し、true のときだけ読む", () => {
    expect(coachAcceptancePath({ acceptanceStatus: "", sport: "", q: "", inactive: true, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?inactive=true",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "suspended", sport: "tennis", q: "高橋", inactive: true, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?acceptance_status=suspended&sport=tennis&q=%E9%AB%98%E6%A9%8B&inactive=true",
    );
    expect(coachAcceptanceFilterFrom(new URLSearchParams("inactive=true&sport=golf"))).toEqual({
      acceptanceStatus: "",
      sport: "golf",
      q: "",
      inactive: true,
      hasUnhandledLowRated: false,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("inactive=false")).inactive).toBe(false);
    expect(coachAcceptanceFilterFrom(new URLSearchParams("inactive=1")).inactive).toBe(false);
  });

  test("画面で直近 30 日にレッスンのないコーチだけを入れると、ほかの条件と合わせて URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [TAKAHASHI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(
      <CoachAcceptanceHome token="mock.headquarters.x" initialFilter={{ acceptanceStatus: "", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false }} />,
    );
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: INACTIVE_ONLY }));
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches?sport=golf&inactive=true", { scroll: false });
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "golf", inactive: true }));
  });

  test("未対応の低い評価のあるコーチだけは has_unhandled_low_rated=true で残し、true のときだけ読む", () => {
    expect(coachAcceptancePath({ acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?has_unhandled_low_rated=true",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "accepting", sport: "golf", q: "", inactive: true, hasUnhandledLowRated: true, hasUnanswered: false, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?acceptance_status=accepting&sport=golf&inactive=true&has_unhandled_low_rated=true",
    );
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_unhandled_low_rated=true&inactive=true"))).toEqual({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: true,
      hasUnhandledLowRated: true,
      hasUnanswered: false,
      hasOverdueVideo: false,
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_unhandled_low_rated=false")).hasUnhandledLowRated).toBe(false);
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_unhandled_low_rated=1")).hasUnhandledLowRated).toBe(false);
  });

  test("画面で未対応の低い評価のあるコーチだけを入れると、ほかの条件と合わせて URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(
      <CoachAcceptanceHome
        token="mock.headquarters.x"
        initialFilter={{ acceptanceStatus: "", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false }}
      />,
    );
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: UNHANDLED_LOW_RATED_ONLY }));
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches?sport=golf&has_unhandled_low_rated=true", {
      scroll: false,
    });
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "golf", has_unhandled_low_rated: true }));
  });

  test("返事のないメッセージのあるコーチだけは has_unanswered=true で残し、true のときだけ読む", () => {
    expect(coachAcceptancePath({ acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: true, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?has_unanswered=true",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "accepting", sport: "golf", q: "", inactive: true, hasUnhandledLowRated: true, hasUnanswered: true, hasOverdueVideo: false })).toBe(
      "/headquarters/coaches?acceptance_status=accepting&sport=golf&inactive=true&has_unhandled_low_rated=true&has_unanswered=true",
    );
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_unanswered=true&has_unhandled_low_rated=true"))).toEqual({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: true,
      hasUnanswered: true,
      hasOverdueVideo: false,
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_unanswered=false")).hasUnanswered).toBe(false);
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_unanswered=1")).hasUnanswered).toBe(false);
  });

  test("画面で返事のないメッセージのあるコーチだけを入れると、ほかの条件と合わせて URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(
      <CoachAcceptanceHome
        token="mock.headquarters.x"
        initialFilter={{ acceptanceStatus: "", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false }}
      />,
    );
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: UNANSWERED_ONLY }));
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches?sport=golf&has_unanswered=true", {
      scroll: false,
    });
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "golf", has_unanswered: true }));
  });

  test("返却の遅れている動画のあるコーチだけは has_overdue_video=true で残し、true のときだけ読む", () => {
    expect(coachAcceptancePath({ acceptanceStatus: "", sport: "", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: true })).toBe(
      "/headquarters/coaches?has_overdue_video=true",
    );
    expect(coachAcceptancePath({ acceptanceStatus: "accepting", sport: "golf", q: "", inactive: true, hasUnhandledLowRated: true, hasUnanswered: true, hasOverdueVideo: true })).toBe(
      "/headquarters/coaches?acceptance_status=accepting&sport=golf&inactive=true&has_unhandled_low_rated=true&has_unanswered=true&has_overdue_video=true",
    );
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_overdue_video=true&has_unanswered=true"))).toEqual({
      acceptanceStatus: "",
      sport: "",
      q: "",
      inactive: false,
      hasUnhandledLowRated: false,
      hasUnanswered: true,
      hasOverdueVideo: true,
    });
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_overdue_video=false")).hasOverdueVideo).toBe(false);
    expect(coachAcceptanceFilterFrom(new URLSearchParams("has_overdue_video=1")).hasOverdueVideo).toBe(false);
  });

  test("画面で返却の遅れている動画のあるコーチだけを入れると、ほかの条件と合わせて URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(
      <CoachAcceptanceHome
        token="mock.headquarters.x"
        initialFilter={{ acceptanceStatus: "", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false }}
      />,
    );
    await screen.findByRole("table", { name: "コーチの一覧" });

    await user.click(screen.getByRole("checkbox", { name: OVERDUE_VIDEO_ONLY }));
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches?sport=golf&has_overdue_video=true", {
      scroll: false,
    });
    await waitFor(() => expect(listCoaches).toHaveBeenLastCalledWith({ sport: "golf", has_overdue_video: true }));
  });

  test("クエリの確かめに使う選択肢は凍結してあり、import した側から書き換えられない", () => {
    expect(Object.isFrozen(ACCEPTANCE_STATUSES)).toBe(true);
    expect(Object.isFrozen(SPORTS)).toBe(true);
  });

  test("画面で絞り込みを変えると、履歴を積まずに URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listCoaches = listOf(COACHES, [SUZUKI]);
    headquartersApiMock.mockReturnValue({ listCoaches, suspendCoach: vi.fn(), resumeCoach: vi.fn() });
    render(<CoachAcceptanceHome token="mock.headquarters.x" initialFilter={{ acceptanceStatus: "", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false }} />);
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
      <CoachAcceptanceHome token="mock.headquarters.x" initialFilter={{ acceptanceStatus: "", sport: "golf", q: "", inactive: false, hasUnhandledLowRated: false, hasUnanswered: false, hasOverdueVideo: false }} />,
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
