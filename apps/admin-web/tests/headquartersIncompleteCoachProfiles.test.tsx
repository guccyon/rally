import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { IncompleteCoachProfileList } from "@/components/headquarters/IncompleteCoachProfileList";
import {
  IncompleteCoachProfilesHome,
  incompleteCoachProfileFilterFrom,
  incompleteCoachProfilesPath,
} from "@/components/headquarters/IncompleteCoachProfilesHome";
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

// 読み込むたびに渡した一覧を順に返す（開いたとき・絞り込みを変えたとき）。尽きたら最初の一覧を返す。
function listOf(...responses: Schemas["IncompleteCoachProfile"][][]) {
  const list = vi.fn<HeadquartersApi["listIncompleteCoachProfiles"]>(async () => ({
    ok: true,
    data: { coaches: responses[0]! },
  }));
  for (const coaches of responses) list.mockResolvedValueOnce({ ok: true, data: { coaches } });
  return list;
}

function rowNames() {
  const table = screen.getByRole("table", { name: "自己紹介の足りないコーチの一覧" });
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell")[0]!.textContent);
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

describe("本部の自己紹介の足りないコーチの絞り込み", () => {
  test("開いたときは条件を付けずに読み、スポーツの絞り込みの既定は「すべて」", async () => {
    const listIncompleteCoachProfiles = listOf(COACHES);
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);

    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(listIncompleteCoachProfiles).toHaveBeenCalledWith({});
    const select = screen.getByRole("combobox", { name: "スポーツ" });
    expect(select).toHaveValue("");
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "テニス",
      "ゴルフ",
      "野球",
      "サッカー",
      "バスケットボール",
      "水泳",
    ]);
  });

  test("スポーツを変えるたびにその条件で読み直して返った行を出し、「すべて」で条件を外す", async () => {
    const user = userEvent.setup();
    const listIncompleteCoachProfiles = listOf(COACHES, [COACHES[1]!], COACHES);
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "ゴルフ");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({ sport: "golf" });
    await waitFor(() => expect(rowNames()).toEqual(["高橋 美咲"]));

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "すべて");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({});
    await waitFor(() => expect(rowNames()).toEqual(["佐藤 健", "高橋 美咲"]));
    expect(listIncompleteCoachProfiles).toHaveBeenCalledTimes(3);
  });

  test("続けて変えたときは、最後の条件の結果だけを出す", async () => {
    const user = userEvent.setup();
    let resolveSlow: (value: Awaited<ReturnType<HeadquartersApi["listIncompleteCoachProfiles"]>>) => void = () => {};
    const listIncompleteCoachProfiles = vi
      .fn<HeadquartersApi["listIncompleteCoachProfiles"]>()
      .mockResolvedValueOnce({ ok: true, data: { coaches: COACHES } })
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveSlow = resolve;
          }),
      )
      .mockResolvedValueOnce({ ok: true, data: { coaches: [COACHES[0]!] } });
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "ゴルフ");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await waitFor(() => expect(rowNames()).toEqual(["佐藤 健"]));

    // 前の条件（ゴルフ）の応答が遅れて返っても上書きしない。
    resolveSlow({ ok: true, data: { coaches: [COACHES[1]!] } });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(rowNames()).toEqual(["佐藤 健"]);
  });

  test("絞り込んで 0 件なら「条件に合うコーチはいません」を出し、表は出さない", async () => {
    const user = userEvent.setup();
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf(COACHES, []) }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "水泳");

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "自己紹介の足りないコーチの一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("自己紹介の足りないコーチはいません")).not.toBeInTheDocument();
  });

  test("絞り込んだ読み直しが失敗したときはその文言だけを出す", async () => {
    const user = userEvent.setup();
    const listIncompleteCoachProfiles = listOf(COACHES);
    listIncompleteCoachProfiles.mockResolvedValueOnce({
      ok: false,
      code: "validation_failed",
      message: "入力内容を確かめてください。",
    });
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");

    expect(await screen.findByRole("alert")).toHaveTextContent("入力内容を確かめてください。");
    expect(screen.queryByRole("table", { name: "自己紹介の足りないコーチの一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("条件に合うコーチはいません")).not.toBeInTheDocument();
  });

  test("渡された条件（URL のクエリ）で開き、絞り込みにその値を出す", async () => {
    const listIncompleteCoachProfiles = listOf([COACHES[1]!]);
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} initialFilter={{ sport: "golf", acceptanceStatus: "" }} />);

    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(listIncompleteCoachProfiles).toHaveBeenCalledTimes(1);
    expect(listIncompleteCoachProfiles).toHaveBeenCalledWith({ sport: "golf" });
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("golf");
  });

  test("渡された条件で開いて 0 件なら「条件に合うコーチはいません」を出す", async () => {
    render(
      <IncompleteCoachProfileList
        api={{ listIncompleteCoachProfiles: listOf([]) }}
        initialFilter={{ sport: "swimming", acceptanceStatus: "" }}
      />,
    );

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
  });

  test("絞り込みを変えるたびに、変えた後の条件を知らせる", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    render(
      <IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf(COACHES) }} onFilterChange={onFilterChange} />,
    );
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    expect(onFilterChange).toHaveBeenLastCalledWith({ sport: "tennis", acceptanceStatus: "" });
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "すべて");
    expect(onFilterChange).toHaveBeenLastCalledWith({ sport: "", acceptanceStatus: "" });
    expect(onFilterChange).toHaveBeenCalledTimes(2);
  });

  test("受付の状態の絞り込みの既定は「すべて」で、選択肢はコーチの受付の一覧と同じ言葉", async () => {
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf(COACHES) }} />);

    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    const select = screen.getByRole("combobox", { name: "受付の状態" });
    expect(select).toHaveValue("");
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "受付中",
      "停止中",
    ]);
  });

  test("受付の状態を変えるたびにその条件で読み直して返った行を出し、「すべて」で条件を外す", async () => {
    const user = userEvent.setup();
    const onFilterChange = vi.fn();
    const listIncompleteCoachProfiles = listOf(COACHES, [COACHES[0]!], [COACHES[1]!], COACHES);
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} onFilterChange={onFilterChange} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({ acceptance_status: "accepting" });
    expect(onFilterChange).toHaveBeenLastCalledWith({ sport: "", acceptanceStatus: "accepting" });
    await waitFor(() => expect(rowNames()).toEqual(["佐藤 健"]));

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({ acceptance_status: "suspended" });
    await waitFor(() => expect(rowNames()).toEqual(["高橋 美咲"]));

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "すべて");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({});
    await waitFor(() => expect(rowNames()).toEqual(["佐藤 健", "高橋 美咲"]));
    expect(listIncompleteCoachProfiles).toHaveBeenCalledTimes(4);
  });

  test("スポーツと受付の状態は組み合わせられ、片方を変えてももう片方の条件を残して読み直す", async () => {
    const user = userEvent.setup();
    const listIncompleteCoachProfiles = listOf(COACHES, [COACHES[1]!], [COACHES[1]!], []);
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "ゴルフ");
    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({ sport: "golf", acceptance_status: "accepting" });
    await waitFor(() => expect(rowNames()).toEqual(["高橋 美咲"]));

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "水泳");
    expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({ sport: "swimming", acceptance_status: "accepting" });
    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "受付の状態" })).toHaveValue("accepting");
  });

  test("受付の状態だけで絞って 0 件でも「条件に合うコーチはいません」を出す", async () => {
    const user = userEvent.setup();
    render(<IncompleteCoachProfileList api={{ listIncompleteCoachProfiles: listOf(COACHES, []) }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });

    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "停止中");

    expect(await screen.findByText("条件に合うコーチはいません")).toBeInTheDocument();
    expect(screen.queryByText("自己紹介の足りないコーチはいません")).not.toBeInTheDocument();
  });

  test("渡された受付の状態（URL のクエリ）で開き、スポーツと合わせて読む", async () => {
    const listIncompleteCoachProfiles = listOf([COACHES[1]!]);
    render(
      <IncompleteCoachProfileList
        api={{ listIncompleteCoachProfiles }}
        initialFilter={{ sport: "golf", acceptanceStatus: "accepting" }}
      />,
    );

    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(listIncompleteCoachProfiles).toHaveBeenCalledTimes(1);
    expect(listIncompleteCoachProfiles).toHaveBeenCalledWith({ sport: "golf", acceptance_status: "accepting" });
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("golf");
    expect(screen.getByRole("combobox", { name: "受付の状態" })).toHaveValue("accepting");
  });
});

describe("自己紹介の足りないコーチの絞り込みと URL", () => {
  test("「すべて」の条件はクエリに出さず、選んだスポーツを契約のクエリの名前で残す", () => {
    expect(incompleteCoachProfilesPath({ sport: "", acceptanceStatus: "" })).toBe("/headquarters/coaches/incomplete-profiles");
    expect(incompleteCoachProfilesPath({ sport: "golf", acceptanceStatus: "" })).toBe("/headquarters/coaches/incomplete-profiles?sport=golf");
  });

  test("受付の状態も契約のクエリの名前で残し、スポーツと並べて出す", () => {
    expect(incompleteCoachProfilesPath({ sport: "", acceptanceStatus: "accepting" })).toBe(
      "/headquarters/coaches/incomplete-profiles?acceptance_status=accepting",
    );
    expect(incompleteCoachProfilesPath({ sport: "golf", acceptanceStatus: "suspended" })).toBe(
      "/headquarters/coaches/incomplete-profiles?sport=golf&acceptance_status=suspended",
    );
  });

  test("クエリから受付の状態を読み、契約の値でなければ「すべて」にする", () => {
    expect(incompleteCoachProfileFilterFrom(new URLSearchParams("sport=tennis&acceptance_status=accepting"))).toEqual({
      sport: "tennis",
      acceptanceStatus: "accepting",
    });
    expect(incompleteCoachProfileFilterFrom(new URLSearchParams("acceptance_status=closed"))).toEqual({
      sport: "",
      acceptanceStatus: "",
    });
  });

  test("クエリから条件を読み、契約の値でなければ「すべて」にする", () => {
    expect(incompleteCoachProfileFilterFrom(new URLSearchParams("sport=tennis"))).toEqual({
      sport: "tennis",
      acceptanceStatus: "",
    });
    expect(incompleteCoachProfileFilterFrom(new URLSearchParams(""))).toEqual({ sport: "", acceptanceStatus: "" });
    expect(incompleteCoachProfileFilterFrom(new URLSearchParams("sport=chess"))).toEqual({ sport: "", acceptanceStatus: "" });
  });

  test("画面で絞り込みを変えると、履歴を積まずに URL のクエリを置き換える", async () => {
    const user = userEvent.setup();
    const listIncompleteCoachProfiles = listOf([COACHES[1]!], COACHES);
    headquartersApiMock.mockReturnValue({ listIncompleteCoachProfiles });
    render(<IncompleteCoachProfilesHome token="mock.headquarters.x" initialFilter={{ sport: "golf", acceptanceStatus: "" }} />);
    await screen.findByRole("table", { name: "自己紹介の足りないコーチの一覧" });
    expect(listIncompleteCoachProfiles).toHaveBeenCalledWith({ sport: "golf" });
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("golf");

    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "すべて");
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches/incomplete-profiles", { scroll: false });
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "野球");
    expect(replace).toHaveBeenLastCalledWith("/headquarters/coaches/incomplete-profiles?sport=baseball", {
      scroll: false,
    });
    await waitFor(() => expect(listIncompleteCoachProfiles).toHaveBeenLastCalledWith({ sport: "baseball" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "受付の状態" }), "受付中");
    expect(replace).toHaveBeenLastCalledWith(
      "/headquarters/coaches/incomplete-profiles?sport=baseball&acceptance_status=accepting",
      { scroll: false },
    );
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
