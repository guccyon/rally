import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import NotFound from "@/app/not-found";

describe("見つからないページ", () => {
  test("日本語の案内と、ログインの画面へ戻るリンクが出る", () => {
    render(<NotFound />);
    expect(screen.getByRole("heading", { name: "ページが見つかりません" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "ログインの画面へ戻る" })).toHaveAttribute("href", "/");
    expect(screen.queryByText(/This page could not be found/)).toBeNull();
  });
});
