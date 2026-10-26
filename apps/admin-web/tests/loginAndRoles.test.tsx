import { render, screen } from "@testing-library/react";
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

describe("ログイン", () => {
  test("トークンとロールを保持し、ロールの画面へ移る", async () => {
    const user = userEvent.setup();
    render(<LoginForm />);

    await user.click(screen.getByLabelText("本部"));
    await user.type(screen.getByLabelText("アクセストークン"), " mock.headquarters.x ");
    await user.click(screen.getByRole("button", { name: "ログイン" }));

    expect(readSession()).toEqual({ role: "headquarters", token: "mock.headquarters.x" });
    expect(push).toHaveBeenCalledWith("/headquarters");
  });

  test("トークンが空ならログインしない", async () => {
    const user = userEvent.setup();
    render(<LoginForm />);

    await user.click(screen.getByRole("button", { name: "ログイン" }));

    expect(readSession()).toBeNull();
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

  test("未ログインならログイン画面へ戻す", () => {
    render(<RoleGate role="headquarters">{() => <HeadquartersHome />}</RoleGate>);

    expect(replace).toHaveBeenCalledWith("/");
  });

  test("本部の一覧は API がないので未実装と出す", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("region", { name: "予約の一覧" })).toHaveTextContent("未実装");
    expect(screen.getByRole("region", { name: "完了記録の一覧" })).toHaveTextContent("未実装");
  });
});
