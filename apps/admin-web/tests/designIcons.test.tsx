// 同梱のアイコン（src/components/ui/icons/）。paths.ts が SVG と同じ path を持ち、部品が文字の記号でなく SVG を描くこと。
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { ICON_PATHS } from "@/components/ui/icons/paths";
import { ListRow } from "@/components/ui/ListRow";
import { Notice } from "@/components/ui/Notice";
import { STATUS_ICONS, type Status } from "@/components/ui/status";
import { StatusTag } from "@/components/ui/StatusTag";
import { TextField } from "@/components/ui/TextField";

// jsdom の環境では import.meta.url がファイルの URL にならないので、admin-web の直下（vitest の cwd）から組む。
const dir = join(process.cwd(), "src/components/ui/icons");

test("paths.ts は同梱の SVG と同じ名前と path を持ち、ライセンスを添えてある", () => {
  const svgs = readdirSync(dir).filter((f) => f.endsWith(".svg"));
  expect(Object.keys(ICON_PATHS).sort()).toEqual(svgs.map((f) => f.replace(/\.svg$/, "")).sort());
  for (const file of svgs) {
    const svg = readFileSync(join(dir, file), "utf8");
    expect(svg).toContain('viewBox="0 -960 960 960"');
    const d = svg.match(/<path d="([^"]+)"/)?.[1];
    expect(ICON_PATHS[file.replace(/\.svg$/, "") as keyof typeof ICON_PATHS], file).toBe(d);
  }
  expect(readFileSync(join(dir, "LICENSE"), "utf8")).toContain("Apache License");
});

test("状態のタグと知らせは、状態ごとの SVG のアイコンを出し、読み上げない", () => {
  const statuses: Status[] = ["info", "success", "warning", "danger", "neutral"];
  for (const status of statuses) {
    const { container, unmount } = render(
      <>
        <StatusTag label="確定待ち" status={status} />
        <Notice message="受付を停止しました" status={status} />
      </>,
    );
    const icons = container.querySelectorAll(`svg[data-icon="${STATUS_ICONS[status]}"]`);
    expect(icons).toHaveLength(2);
    icons.forEach((icon) => expect(icon).toHaveAttribute("aria-hidden", "true"));
    unmount();
  }
});

test("入力欄の誤りと一覧の行の詳細へのしるしも SVG で、文字の記号を使わない", () => {
  const { container } = render(
    <>
      <TextField label="表示名" error="表示名は 1〜20 文字で入れてください。" />
      <ListRow title="コーチから返事が届きました" onClick={() => {}} />
    </>,
  );
  expect(container.querySelector('svg[data-icon="error"]')).not.toBeNull();
  expect(container.querySelector('svg[data-icon="chevron_right"]')).not.toBeNull();
  expect(screen.getByRole("button", { name: "コーチから返事が届きました" })).toBeInTheDocument();
  expect(container.textContent).not.toMatch(/[⚠›ℹ✓◷]/);
});
