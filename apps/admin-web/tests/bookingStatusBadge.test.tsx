import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { BookingStatusBadge } from "@/components/headquarters/BookingStatusBadge";

describe("予約の状態の札", () => {
  test.each([
    ["pending", "確定待ち", ["bg-amber-100", "text-amber-800"]],
    ["confirmed", "確定済み", ["bg-green-100", "text-green-800"]],
    ["cancelled", "キャンセル済み", ["bg-gray-100", "text-gray-500"]],
  ] as const)("%s は「%s」を色の札で出す", (status, label, classes) => {
    render(<BookingStatusBadge status={status} />);
    expect(screen.getByText(label)).toHaveClass(...classes);
  });
});
