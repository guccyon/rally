import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { BookingStatusBadge } from "@/components/headquarters/BookingStatusBadge";

describe("予約の状態の札", () => {
  test.each([
    ["pending", "確定待ち", ["bg-warning-container", "text-warning-on-container"]],
    ["confirmed", "確定済み", ["bg-success-container", "text-success-on-container"]],
    ["cancelled", "キャンセル済み", ["bg-surface", "text-text-muted"]],
  ] as const)("%s は「%s」を色の札で出す", (status, label, classes) => {
    render(<BookingStatusBadge status={status} />);
    expect(screen.getByText(label)).toHaveClass(...classes);
  });
});
