import type { ReactNode } from "react";

import { Button } from "./Button";
import { Card } from "./Card";
import { ConfirmStep } from "./ConfirmStep";
import { EmptyState } from "./EmptyState";
import { List } from "./List";
import { ListRow } from "./ListRow";
import { Loading } from "./Loading";
import { ResultMessage } from "./ResultMessage";
import { Section } from "./Section";
import { Select } from "./Select";
import { TextArea } from "./TextArea";
import { TextInput } from "./TextInput";

/** カタログの 1 枠。component・variant・state は specs/design/components.yaml の id（対応はテストで確かめる）。 */
export type CatalogEntry = {
  component: string;
  variant?: string;
  state: string;
  render: () => ReactNode;
};

const noop = () => {};

const SPORT_OPTIONS = (
  <>
    <option value="tennis">テニス</option>
    <option value="soccer">サッカー</option>
  </>
);

const CONFIRM_ITEMS = [
  { term: "対象", detail: "テニスの生徒" },
  { term: "題", detail: "コートの予約方法が変わります" },
];

const BUTTON_LABELS = { primary: "内容を確かめる", secondary: "直す", danger: "非公開にする" } as const;

const INPUTS = {
  single_line: (props: { disabled?: boolean; error?: string }) => (
    <TextInput label="題" defaultValue="コートの予約方法が変わります" {...props} />
  ),
  multi_line: (props: { disabled?: boolean; error?: string }) => (
    <TextArea label="本文" rows={3} defaultValue={"11月から、コートの予約はアプリから行えます。"} {...props} />
  ),
  select: (props: { disabled?: boolean; error?: string }) => (
    <Select label="スポーツ" defaultValue="tennis" {...props}>
      {SPORT_OPTIONS}
    </Select>
  ),
};

/** 部品 × 種類 × 状態の全部。部品や状態を足したら、ここにも足す。 */
export const CATALOG: CatalogEntry[] = [
  ...(["primary", "secondary", "danger"] as const).flatMap((variant) => [
    {
      component: "button",
      variant,
      state: "default",
      render: () => <Button variant={variant}>{BUTTON_LABELS[variant]}</Button>,
    },
    {
      component: "button",
      variant,
      state: "disabled",
      render: () => (
        <Button variant={variant} disabled>
          {BUTTON_LABELS[variant]}
        </Button>
      ),
    },
    {
      component: "button",
      variant,
      state: "busy",
      render: () => (
        <Button variant={variant} busy busyLabel="送っています…">
          {BUTTON_LABELS[variant]}
        </Button>
      ),
    },
  ]),
  ...(["single_line", "multi_line", "select"] as const).flatMap((variant) => {
    const Input = INPUTS[variant];
    return [
      { component: "input", variant, state: "default", render: () => <Input /> },
      { component: "input", variant, state: "disabled", render: () => <Input disabled /> },
      { component: "input", variant, state: "invalid", render: () => <Input error="入力を確かめてください。" /> },
    ];
  }),
  {
    component: "card",
    state: "default",
    render: () => (
      <Card label="今月の実績">
        <p className="font-semibold">今月の実績</p>
        <p className="text-sm">完了したレッスン 12 件</p>
      </Card>
    ),
  },
  ...(["plain", "action"] as const).flatMap((variant) =>
    (["default", "disabled"] as const).map((state) => ({
      component: "list_row",
      variant,
      state,
      render: () => (
        <List label={`一覧の行（${variant}・${state}）`}>
          <ListRow
            onOpen={variant === "action" ? noop : undefined}
            disabled={state === "disabled"}
            trailing={variant === "action" ? "「予約」へ" : undefined}
          >
            <span className="block">10/28（水）12:00 の予約が確定しました。</span>
            <span className="block text-gray-600">2026/10/26 10:00</span>
          </ListRow>
        </List>
      ),
    })),
  ),
  {
    component: "section",
    state: "default",
    render: () => (
      <Section title="お知らせを送る">
        <p className="text-sm">欄の中身</p>
      </Section>
    ),
  },
  {
    component: "result_message",
    variant: "success",
    state: "default",
    render: () => <ResultMessage result={{ ok: true, message: "お知らせを 12 人の生徒に送りました。" }} />,
  },
  {
    component: "result_message",
    variant: "failure",
    state: "default",
    render: () => (
      <ResultMessage result={{ ok: false, message: "送れませんでした。時間をおいてもう一度お試しください。" }} />
    ),
  },
  { component: "empty_state", state: "default", render: () => <EmptyState>低い評価はまだありません</EmptyState> },
  { component: "loading", state: "default", render: () => <Loading /> },
  ...(["primary", "danger"] as const).flatMap((variant) =>
    (["default", "busy"] as const).map((state) => ({
      component: "confirm_step",
      variant,
      state,
      render: () => (
        <ConfirmStep
          label={`確認の段（${variant}・${state}）`}
          message="次の内容でテニスの生徒に送りますか？送るとすぐに生徒に届きます。"
          items={CONFIRM_ITEMS}
          confirmLabel={variant === "danger" ? "非公開にする" : "送るのを確定"}
          busyLabel="送っています…"
          variant={variant}
          busy={state === "busy"}
          onConfirm={noop}
          onCancel={noop}
        />
      ),
    })),
  ),
];

/** カタログの枠の名前（見出しと突き合わせに使う）。例: `button / primary / disabled` */
export function catalogEntryName(entry: Pick<CatalogEntry, "component" | "variant" | "state">): string {
  return [entry.component, entry.variant, entry.state].filter(Boolean).join(" / ");
}
