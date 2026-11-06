import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { BookingStatusBadge } from "@/components/headquarters/BookingStatusBadge";

describe("予約の状態のタグ", () => {
  test.each([
    ["pending", "確定待ち", "bg-warning", "text-text"],
    ["confirmed", "確定済み", "bg-success", "text-text"],
    ["cancelled", "キャンセル済み", "bg-border-strong", "text-text-muted"],
  ] as const)("%s は「%s」を booking_state の色の点（%s）と字で出す", (status, label, dot, text) => {
    render(<BookingStatusBadge status={status} />);
    expect(screen.getByText(label)).toHaveClass(text);
    expect(screen.getByTestId("status-dot")).toHaveClass(dot);
  });
});
