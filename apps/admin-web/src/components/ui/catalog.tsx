import type { ReactNode } from "react";

import { Badge } from "./Badge";
import { Button, type ButtonVariant } from "./Button";
import { Card } from "./Card";
import { ConfirmDialog } from "./ConfirmDialog";
import { EmptyState } from "./EmptyState";
import { ListRow } from "./ListRow";
import { LoadingBar, SectionLoading, Spinner } from "./Loading";
import { Notice } from "./Notice";
import { StatusTag } from "./StatusTag";
import { TextArea, TextField } from "./TextField";
import type { Status } from "./status";

/**
 * カタログの 1 枠。component・variant・state は specs/design/components.yaml の id とキー（対応はテストで確かめる）。
 * example は同じ部品・種類・状態の見本を見分ける名前（状態の色など。components.yaml には対応しない）。
 */
export type CatalogEntry = {
  component: string;
  variant?: string;
  state?: string;
  example?: string;
  render: () => ReactNode;
};

/** components.yaml にあるが admin-web では使わない部品と種類（理由は components.yaml の not_for・各部品の注記）。 */
export const NOT_IN_ADMIN_WEB: { component: string; variant?: string; reason: string }[] = [
  { component: "score_panel", reason: "components.yaml の not_for に admin-web とある" },
  { component: "button", variant: "on_dark", reason: "admin-web は濃い面（inverse_surface）を使わない" },
  { component: "badge", variant: "on_dark", reason: "admin-web は濃い面（inverse_surface）を使わない" },
  { component: "notice", variant: "toast", reason: "admin-web の Notice は inline だけ（toast はまだない）" },
];

const noop = () => {};

const BUTTON_LABELS: Record<ButtonVariant, string> = {
  primary: "予約する",
  secondary: "保存する",
  danger: "キャンセルする",
  text: "予約をすべて見る",
};

const STATUS_EXAMPLES: { status: Status; label: string }[] = [
  { status: "info", label: "お知らせ" },
  { status: "success", label: "確定済み" },
  { status: "warning", label: "確定待ち" },
  { status: "danger", label: "受付停止" },
  { status: "neutral", label: "キャンセル済み" },
];

const NOTICE_MESSAGES: Record<Exclude<Status, "neutral">, string> = {
  info: "11月から、コートの予約はアプリから行えます。",
  success: "お知らせを 12 人の生徒に送りました。",
  warning: "開始まで 1 時間を切った確定待ちの予約があります。",
  danger: "予約を読み込めませんでした。時間をおいてもう一度開いてください。",
};

/**
 * 確認のダイアログは画面全体に重ねる部品なので、transform を付けた枠を基準にしてその中だけに重ねる
 * （transform のある祖先は position: fixed の基準になる）。
 */
function DialogFrame({ children }: { children: ReactNode }) {
  return <div className="relative h-72 overflow-hidden rounded-md border [transform:translateZ(0)]">{children}</div>;
}

/** 部品 × 種類 × 状態の見本。components.yaml に部品や種類を足したら、ここにも足す。 */
export const CATALOG: CatalogEntry[] = [
  ...(["primary", "secondary", "danger", "text"] as const).flatMap((variant) => [
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
      state: "loading",
      render: () => (
        <Button variant={variant} loading>
          {BUTTON_LABELS[variant]}
        </Button>
      ),
    },
  ]),
  ...(["single_line", "multi_line"] as const).flatMap((variant) => {
    const Field = variant === "single_line" ? TextField : TextArea;
    const props =
      variant === "single_line"
        ? { label: "表示名", helper: "20 文字まで", defaultValue: "鈴木 コーチ" }
        : {
            label: "自己紹介",
            helper: "500 文字まで",
            counter: "21 / 500",
            defaultValue: "テニスを教えて 10 年になります。",
          };
    return [
      { component: "text_field", variant, state: "default", render: () => <Field {...props} /> },
      { component: "text_field", variant, state: "disabled", render: () => <Field {...props} disabled /> },
      {
        component: "text_field",
        variant,
        state: "error",
        render: () => <Field {...props} error="1〜20 文字で入れてください。" />,
      },
    ];
  }),
  {
    component: "card",
    render: () => (
      <Card>
        <p className="font-bold">10/28（水）12:00〜12:30</p>
        <p className="text-sm text-text-muted">テニス・コーチ: 鈴木</p>
        <StatusTag label="確定済み" status="success" />
      </Card>
    ),
  },
  {
    component: "card",
    example: "押せる",
    render: () => (
      <Card onClick={noop}>
        <p className="font-bold">スイングの見直し</p>
        <p className="text-sm text-text-muted">テニス・1 回 60 分</p>
      </Card>
    ),
  },
  {
    component: "list_row",
    render: () => (
      <ListRow title="10/28（水）12:00 の予約が確定しました。" subtitle="2026/10/26 10:00" onClick={noop} />
    ),
  },
  {
    component: "list_row",
    state: "unread",
    render: () => <ListRow title="コーチから返事が届きました" subtitle="2026/10/27 18:04" unread onClick={noop} />,
  },
  { component: "badge", variant: "count", render: () => <Badge count={3} /> },
  { component: "badge", variant: "dot", render: () => <Badge /> },
  ...STATUS_EXAMPLES.map(({ status, label }) => ({
    component: "status_tag",
    example: status,
    render: () => <StatusTag label={label} status={status} />,
  })),
  ...(["info", "success", "warning", "danger"] as const).map((state) => ({
    component: "notice",
    variant: "inline",
    state,
    render: () => <Notice message={NOTICE_MESSAGES[state]} status={state} />,
  })),
  {
    component: "empty_state",
    render: () => (
      <EmptyState
        message="まだ低い評価はありません。評価が届くと、ここに新しい順に出ます。"
        action={<Button variant="secondary">評価の一覧を見る</Button>}
      />
    ),
  },
  { component: "loading", variant: "bar", render: () => <LoadingBar /> },
  { component: "loading", variant: "section_text", render: () => <SectionLoading message="予約を読み込んでいます…" /> },
  { component: "loading", variant: "spinner", render: () => <Spinner /> },
  ...([false, true] as const).map((danger) => ({
    component: "confirm_dialog",
    example: danger ? "危険な操作" : "送る前の確認",
    render: () => (
      <DialogFrame>
        <ConfirmDialog
          open
          title={danger ? "受付を止めますか？" : "お知らせを送りますか？"}
          body={
            danger
              ? "鈴木 コーチの新しい予約の受付を止めます。止めているあいだ、生徒はこのコーチの枠を予約できません。"
              : "テニスの生徒 12 人に「コートの予約方法が変わります」を送ります。送るとすぐに届き、取り消せません。"
          }
          confirmLabel={danger ? "受付を止める" : "送る"}
          danger={danger}
          onConfirm={noop}
          onCancel={noop}
        />
      </DialogFrame>
    ),
  })),
];

/** カタログの枠の名前（見出しと突き合わせに使う）。例: `button / primary / disabled`、`status_tag / warning` */
export function catalogEntryName(entry: Pick<CatalogEntry, "component" | "variant" | "state" | "example">): string {
  return [entry.component, entry.variant, entry.state, entry.example].filter(Boolean).join(" / ");
}
