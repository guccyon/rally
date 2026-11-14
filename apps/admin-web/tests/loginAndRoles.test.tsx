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

  test("本部の完了記録の一覧は開発者向けの語を使わず、まだ見られないと伝える", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("region", { name: "予約" })).not.toHaveTextContent("未実装");
    const records = screen.getByRole("region", { name: "完了記録の一覧" });
    expect(records).toHaveTextContent("まだこの画面では見られません");
    expect(records).not.toHaveTextContent(/未実装|API|契約/);
  });

  test("本部の画面から予約枠の需給の画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "需給を見る" })).toHaveAttribute(
      "href",
      "/headquarters/slot-supply",
    );
  });
});
