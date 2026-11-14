import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { LoginForm } from "@/components/LoginForm";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { readSession, saveSession } from "@/session/session";

const push = vi.fn();
const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, replace }) }));

beforeEach(() => {
  push.mockReset();
  replace.mockReset();
});

type Account = { account_id: string; role: "student" | "coach" | "headquarters"; display_name: string };

const accountOf = (role: Account["role"]) => async () => ({
  ok: true as const,
  data: { account_id: "00000000-0000-0000-0000-000000000022", role, display_name: "名前" },
});

async function login(role: "コーチ" | "本部", token: string) {
  const user = userEvent.setup();
  await user.click(screen.getByLabelText(role));
  await user.type(screen.getByLabelText("アクセストークン"), token);
  await user.click(screen.getByRole("button", { name: "ログイン" }));
}

describe("ログイン", () => {
  test("トークンの役割が選んだロールと合えば、トークンとロールを保持し、ロールの画面へ移る", async () => {
    const getAccount = vi.fn(accountOf("headquarters"));
    render(<LoginForm getAccount={getAccount} />);

    await login("本部", " mock.headquarters.x ");

    expect(getAccount).toHaveBeenCalledWith("mock.headquarters.x");
    expect(readSession()).toEqual({ role: "headquarters", token: "mock.headquarters.x" });
    expect(push).toHaveBeenCalledWith("/headquarters");
  });

  test("コーチのトークンでも、ロールにコーチを選べば入れる", async () => {
    render(<LoginForm getAccount={accountOf("coach")} />);

    await login("コーチ", "mock.coach.22");

    expect(readSession()).toEqual({ role: "coach", token: "mock.coach.22" });
    expect(push).toHaveBeenCalledWith("/coach");
  });

  test("本部を選んでコーチのトークンを入れると、コーチのものだと出して入れない", async () => {
    render(<LoginForm getAccount={accountOf("coach")} />);

    await login("本部", "mock.coach.22");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "このトークンはコーチのものです（本部のものではありません）。ロール「コーチ」を選んでログインしてください。",
    );
    expect(readSession()).toBeNull();
    expect(push).not.toHaveBeenCalled();
  });

  test("コーチを選んで本部のトークンを入れると、本部のものだと出して入れない", async () => {
    render(<LoginForm getAccount={accountOf("headquarters")} />);

    await login("コーチ", "mock.headquarters.1");

    expect(await screen.findByRole("alert")).toHaveTextContent("このトークンは本部のものです");
    expect(readSession()).toBeNull();
    expect(push).not.toHaveBeenCalled();
  });

  test("生徒のトークンでは、管理画面に入れないと出して入れない", async () => {
    render(<LoginForm getAccount={accountOf("student")} />);

    await login("コーチ", "mock.student.5");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "このトークンは生徒のものです。生徒のトークンでは管理画面に入れません。",
    );
    expect(readSession()).toBeNull();
    expect(push).not.toHaveBeenCalled();
  });

  test("無効なトークン（401）なら、無効だと出して入れない", async () => {
    const getAccount = async () => ({
      ok: false as const,
      message: "トークンがないか無効です。ログインし直してください。",
      code: "unauthorized" as const,
    });
    render(<LoginForm getAccount={getAccount} />);

    await login("本部", "bad");

    expect(await screen.findByRole("alert")).toHaveTextContent("トークンが無効です。");
    expect(readSession()).toBeNull();
    expect(push).not.toHaveBeenCalled();
  });

  test("通信に失敗したら、失敗した旨を出して保持しない", async () => {
    const getAccount = async () => ({ ok: false as const, message: "サーバーに接続できません。" });
    render(<LoginForm getAccount={getAccount} />);

    await login("本部", "mock.headquarters.x");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "ログインできませんでした。サーバーに接続できません。",
    );
    expect(readSession()).toBeNull();
    expect(push).not.toHaveBeenCalled();
  });

  test("確かめているあいだは「ログイン」を押せない", async () => {
    let resolve: (value: Awaited<ReturnType<ReturnType<typeof accountOf>>>) => void = () => {};
    const getAccount = vi.fn(
      () => new Promise<Awaited<ReturnType<ReturnType<typeof accountOf>>>>((r) => (resolve = r)),
    );
    render(<LoginForm getAccount={getAccount} />);

    await login("本部", "mock.headquarters.x");

    const button = screen.getByRole("button", { name: "ログイン" });
    expect(button).toBeDisabled();
    await userEvent.setup().click(button);
    expect(getAccount).toHaveBeenCalledTimes(1);

    await act(async () => resolve(await accountOf("headquarters")()));
    expect(push).toHaveBeenCalledWith("/headquarters");
  });

  test("トークンが空ならログインしない", async () => {
    const getAccount = vi.fn(accountOf("coach"));
    const user = userEvent.setup();
    render(<LoginForm getAccount={getAccount} />);

    await user.click(screen.getByRole("button", { name: "ログイン" }));

    expect(readSession()).toBeNull();
    expect(getAccount).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("トークンを入力してください。");
  });
});

describe("ロールでの出し分け", () => {
  test("ロールが合えば中身を出す", () => {
    saveSession({ role: "coach", token: "t" });
    render(<RoleGate role="coach">{() => <p>コーチの中身</p>}</RoleGate>);

    expect(screen.getByText("コーチの中身")).toBeInTheDocument();
    expect(replace).not.toHaveBeenCalled();
  });

  test("ロールが違えば中身を出さずログイン画面へ戻す", () => {
    saveSession({ role: "headquarters", token: "t" });
    render(<RoleGate role="coach">{() => <p>コーチの中身</p>}</RoleGate>);

    expect(screen.queryByText("コーチの中身")).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });

  test("枠の幅は既定で reading（768px）、本部の一覧の画面は wide（1280px）にする", () => {
    saveSession({ role: "headquarters", token: "t" });
    const { unmount } = render(<RoleGate role="headquarters">{() => <p>読む画面</p>}</RoleGate>);
    expect(screen.getByText("読む画面").parentElement).toHaveClass("mx-auto", "max-w-[768px]", "px-4", "min-[600px]:px-6");
    unmount();

    render(
      <RoleGate role="headquarters" width="wide">
        {() => <p>一覧の画面</p>}
      </RoleGate>,
    );
    const frame = screen.getByText("一覧の画面").parentElement;
    expect(frame).toHaveClass("mx-auto", "max-w-[1280px]", "[&_p]:max-w-[40em]");
    expect(frame).not.toHaveClass("max-w-[768px]");
  });

  test("未ログインならログイン画面へ戻す", () => {
    render(<RoleGate role="headquarters">{() => <HeadquartersHome />}</RoleGate>);

    expect(replace).toHaveBeenCalledWith("/");
  });

  test("本部の完了記録の一覧は API がないので未実装と出す", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("region", { name: "予約" })).not.toHaveTextContent("未実装");
    expect(screen.getByRole("region", { name: "完了記録の一覧" })).toHaveTextContent("未実装");
  });

  test("本部の画面から予約枠の需給の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "需給を見る" })).toHaveAttribute(
      "href",
      "/headquarters/slot-supply",
    );
  });
});
