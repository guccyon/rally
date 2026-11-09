import type { ReactNode } from "react";

import { Badge } from "./Badge";
import { Button, type ButtonVariant } from "./Button";
import { CameraPreview, type CameraPreviewState } from "./CameraPreview";
import { Card } from "./Card";
import { ConfirmDialog } from "./ConfirmDialog";
import { DataTable, type DataTableColumn } from "./DataTable";
import { DateField, DateRangeField } from "./DateField";
import { dateRangeError, type DateRange } from "./dateRange";
import { DescriptionList, type DescriptionItem } from "./DescriptionList";
import { EmptyState } from "./EmptyState";
import { JoinBar } from "./JoinBar";
import { LevelMeter } from "./LevelMeter";
import { ListRow } from "./ListRow";
import { LoadingBar, SectionLoading, Spinner } from "./Loading";
import { MediaToggle, type MediaToggleKind } from "./MediaToggle";
import { Notice } from "./Notice";
import { Select, selectNotSelected } from "./Select";
import { StatusTag } from "./StatusTag";
import { TextArea, TextField } from "./TextField";
import { Timeline, type TimelineEvent } from "./Timeline";
import { SLOT_STATE_STATUS, type Status } from "./status";

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
export const NOT_IN_ADMIN_WEB: {
  component: string;
  variant?: string;
  reason: string;
}[] = [
  {
    component: "score_panel",
    reason: "components.yaml の not_for に admin-web とある",
  },
  {
    component: "button",
    variant: "on_dark",
    reason: "admin-web は濃い面（inverse_surface）を使わない",
  },
  {
    component: "badge",
    variant: "on_dark",
    reason: "admin-web は濃い面（inverse_surface）を使わない",
  },
  {
    component: "notice",
    variant: "toast",
    reason: "admin-web の Notice は inline と success だけ（toast はまだない）",
  },
  {
    component: "card",
    variant: "booking",
    reason: "admin-web は予約を data_table で見比べる（patterns の list）。日付のコマのライムは student-app の次のレッスンだけ",
  },
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

/** スロットの状態（glossary の slot.states）の見本。色は tokens.yaml の color.slot_state。 */
const SLOT_STATE_EXAMPLES: {
  state: keyof typeof SLOT_STATE_STATUS;
  label: string;
}[] = [
  { state: "open", label: "公開中" },
  { state: "booked", label: "予約済み" },
  { state: "closed", label: "終了" },
];

/** 期間の見本（日付の欄と誤りの型）。上限は契約の 31 日。 */
const MAX_DAYS = 31;
const RANGE_EXAMPLES: { example: string; value: DateRange }[] = [
  {
    example: "終わりが始まりより前",
    value: { from: "2026-11-03", to: "2026-10-28" },
  },
  { example: "31 日を超える", value: { from: "2026-10-01", to: "2026-11-30" } },
  { example: "空の欄", value: { from: "2026-10-28", to: "" } },
];

/** 選択欄の見本の選択肢。先頭の行は絞り込みなら「すべて」、必須の欄なら「選んでください」（値は空）。 */
function CoachOptions() {
  return (
    <>
      <option value="">すべて</option>
      <option value="c1">山田 花子</option>
      <option value="c2">佐藤 健</option>
    </>
  );
}

function SportOptions({ first }: { first: string }) {
  return (
    <>
      <option value="">{first}</option>
      <option value="tennis">テニス</option>
      <option value="baseball">野球</option>
    </>
  );
}

const NOTICE_MESSAGES: Record<Exclude<Status, "neutral">, string> = {
  info: "11月から、コートの予約はアプリから行えます。",
  success: "お知らせを 12 人の生徒に送りました。",
  warning: "開始まで 1 時間を切った確定待ちの予約があります。",
  danger: "予約を読み込めませんでした。時間をおいてもう一度開いてください。",
};

type BookingRow = {
  id: string;
  start: string;
  status: { label: string; status: Status };
  coach: string;
  student: string;
  menu: string;
  kind: string;
  urgent?: boolean;
};

const CONFIRMED_BOOKING: BookingRow = {
  id: "b1",
  start: "12:00",
  status: { label: "確定済み", status: "success" },
  coach: "山田 花子",
  student: "鈴木 一郎",
  menu: "フォアハンドの基礎（30 分）",
  kind: "—",
};

const PENDING_BOOKING: BookingRow = {
  id: "b2",
  start: "18:00",
  status: { label: "確定待ち", status: "warning" },
  coach: "佐藤 健",
  student: "田中 美咲",
  menu: "バッティングの基本（30 分）",
  kind: "体験予約",
};

const CANCELLED_BOOKING: BookingRow = {
  id: "b3",
  start: "19:30",
  status: { label: "キャンセル済み", status: "neutral" },
  coach: "山田 花子",
  student: "高橋 蓮",
  menu: "サーブの安定（30 分）",
  kind: "—",
};

const BOOKING_ROWS = [CONFIRMED_BOOKING, PENDING_BOOKING, CANCELLED_BOOKING];

const BOOKING_COLUMNS: DataTableColumn<BookingRow>[] = [
  {
    header: "開始",
    cell: (row) => (
      <a href="#" className="text-primary underline">
        {row.start}
      </a>
    ),
  },
  {
    header: "状態",
    cell: (row) => (
      <span className="inline-flex gap-2">
        <StatusTag label={row.status.label} status={row.status.status} />
        {row.urgent && <StatusTag label="開始まで 24 時間以内" status="danger" urgent />}
      </span>
    ),
  },
  { header: "コーチ", cell: (row) => row.coach },
  { header: "生徒", cell: (row) => row.student },
  { header: "レッスンメニュー", cell: (row) => row.menu },
  { header: "種類", cell: (row) => row.kind },
];

type SupplyRow = {
  coach: string;
  published: number;
  booked: number;
  comment: string;
};

const SUPPLY_ROWS: SupplyRow[] = [
  {
    coach: "山田 花子",
    published: 12,
    booked: 9,
    comment: "フォームを動画で見せてもらえて、直すところがよく分かりました。",
  },
  { coach: "佐藤 健", published: 8, booked: 2, comment: "—" },
];

const SUPPLY_COACH_COLUMN: DataTableColumn<SupplyRow> = {
  header: "コーチ",
  cell: (row) => row.coach,
};

const SUPPLY_COLUMNS: DataTableColumn<SupplyRow>[] = [
  SUPPLY_COACH_COLUMN,
  {
    header: "公開した枠",
    variant: "numeric",
    cell: (row) => `${row.published} 件`,
  },
  {
    header: "予約された枠",
    variant: "numeric",
    cell: (row) => `${row.booked} 件`,
  },
  {
    header: "埋まった割合",
    variant: "numeric",
    cell: (row) => `${Math.round((row.booked / row.published) * 100)}%`,
  },
];

/** 表の見本の枠。Section と同じく面で囲わない（0.5.0）。narrow は幅 360px。 */
function TableFrame({ children, narrow = false }: { children: ReactNode; narrow?: boolean }) {
  return <div className={narrow ? "w-[360px]" : undefined}>{children}</div>;
}

/**
 * 確認のダイアログは画面全体に重ねる部品なので、transform を付けた枠を基準にしてその中だけに重ねる
 * （transform のある祖先は position: fixed の基準になる）。
 */
function DialogFrame({ children }: { children: ReactNode }) {
  return <div className="relative h-72 overflow-hidden rounded-md border [transform:translateZ(0)]">{children}</div>;
}

/** ラベルと値の並びの見本（予約の詳細）。値がない項目も消さず「—」。 */
const BOOKING_DETAIL_ITEMS: DescriptionItem[] = [
  { term: "レッスンメニュー", value: "フォアハンドの基礎（30 分）" },
  { term: "枠の時刻", value: "10/28（水）12:00〜12:30" },
  { term: "種類", value: null },
  { term: "今の状態", value: <StatusTag label="確定済み" status="success" /> },
];

/** 狭い幅と複数行の文の見本（stacked）。 */
const ANNOUNCEMENT_ITEMS: DescriptionItem[] = [
  { term: "コーチ", value: "山田 花子" },
  {
    term: "点の平均（1〜5）",
    value: (
      <>
        4.8{" "}
        <a href="#coach-reviews" className="ml-4 text-sm text-primary underline">
          コーチの評価を見る
        </a>
      </>
    ),
  },
  { term: "本文", value: "11/3（火）は設備の点検のため、ビデオ通話を使えません。\n代わりの日は追ってお知らせします。" },
];

/** 経緯の見本（予約の状態が変わった記録。booking_state の対応の色）。 */
const BOOKING_HISTORY: TimelineEvent[] = [
  { key: "pending", time: "10/20（火）9:12", label: "確定待ち", status: "warning" },
  { key: "confirmed", time: "10/21（水）8:30", label: "確定済み", status: "success", meta: "コーチ: 山田" },
  {
    key: "cancelled",
    time: "10/27（火）18:04",
    label: "キャンセル済み",
    status: "neutral",
    meta: "本部: 鈴木・理由: コーチのお休み",
  },
];

/** 対応のない出来事（お知らせの取り下げ）。点は border-strong。 */
const ANNOUNCEMENT_HISTORY: TimelineEvent[] = [
  { key: "sent", time: "11/2（月）10:00", label: "送った", meta: "本部: 鈴木" },
  { key: "withdrawn", time: "11/2（月）10:25", label: "取り下げた", meta: "本部: 鈴木" },
];

/** 入・切の切り替えの見本。状態（components.yaml の media_toggle.states）ごとに、入と切の 2 つを並べる。 */
const MEDIA_TOGGLE_STATES = ["default", "disabled", "loading"] as const;

function MediaTogglePair({ kind, state }: { kind: MediaToggleKind; state: (typeof MEDIA_TOGGLE_STATES)[number] }) {
  return (
    <div className="flex max-w-xl gap-2">
      {[true, false].map((on) => (
        <MediaToggle
          key={String(on)}
          kind={kind}
          on={on}
          onToggle={noop}
          disabled={state === "disabled"}
          loading={state === "loading"}
          className="flex-1"
        />
      ))}
    </div>
  );
}

/** カメラの映りの枠の見本。映っている（default）は、カタログでは映像を開かないので地の色だけ。 */
const CAMERA_PREVIEW_EXAMPLES: { state: string; preview: CameraPreviewState; example?: string }[] = [
  { state: "default", preview: "live", example: "映像なし" },
  { state: "loading", preview: "waiting" },
  { state: "disabled", preview: "off" },
  { state: "error", preview: "denied", example: "使えない" },
  { state: "error", preview: "none", example: "ない" },
];

/** 入室の帯の見本。帯は画面の下に固定するので、カタログでは枠の中に置く。 */
function JoinBarFrame({ children }: { children: ReactNode }) {
  return <div className="overflow-hidden rounded-md border border-border pt-4">{children}</div>;
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
        ? {
            label: "表示名",
            helper: "20 文字まで",
            defaultValue: "鈴木 コーチ",
          }
        : {
            label: "自己紹介",
            helper: "500 文字まで",
            counter: "21 / 500",
            defaultValue: "テニスを教えて 10 年になります。",
          };
    return [
      {
        component: "text_field",
        variant,
        state: "default",
        render: () => <Field {...props} />,
      },
      {
        component: "text_field",
        variant,
        state: "disabled",
        render: () => <Field {...props} disabled />,
      },
      {
        component: "text_field",
        variant,
        state: "error",
        render: () => <Field {...props} error="1〜20 文字で入れてください。" />,
      },
    ];
  }),
  {
    component: "date_field",
    variant: "date",
    state: "default",
    render: () => <DateField label="送る日" defaultValue="2026-10-28" />,
  },
  {
    component: "date_field",
    variant: "date_range",
    state: "default",
    render: () => (
      <DateRangeField value={{ from: "2026-10-28", to: "2026-11-03" }} onChange={noop} helper="31 日まで選べます" />
    ),
  },
  ...RANGE_EXAMPLES.map(({ example, value }) => ({
    component: "date_field",
    variant: "date_range",
    state: "error",
    example,
    render: () => (
      <DateRangeField
        value={value}
        onChange={noop}
        helper="31 日まで選べます"
        error={dateRangeError(value, MAX_DAYS)}
      />
    ),
  })),
  {
    component: "date_field",
    variant: "date_range",
    state: "loading",
    render: () => <DateRangeField value={{ from: "", to: "" }} onChange={noop} loading />,
  },
  {
    component: "select",
    state: "default",
    render: () => (
      <Select label="コーチ" defaultValue="">
        <CoachOptions />
      </Select>
    ),
  },
  {
    component: "select",
    state: "disabled",
    render: () => (
      <Select label="対象" helper="お知らせを送っているあいだは変えられません。" defaultValue="tennis" disabled>
        <option value="">生徒全員（ゴルフの生徒も含む）</option>
        <option value="tennis">テニスの生徒</option>
      </Select>
    ),
  },
  {
    component: "select",
    state: "loading",
    render: () => (
      <Select label="コーチ" defaultValue="" loading>
        <option value="">すべて</option>
      </Select>
    ),
  },
  {
    component: "select",
    state: "failed",
    render: () => (
      <Select label="レッスンメニュー" defaultValue="" loadFailed>
        <option value="">すべて</option>
      </Select>
    ),
  },
  {
    component: "select",
    state: "error",
    render: () => (
      <Select label="スポーツ" defaultValue="" error={selectNotSelected("スポーツ")} fullWidth>
        <SportOptions first="選んでください" />
      </Select>
    ),
  },
  {
    component: "select",
    state: "default",
    example: "絞り込みの行",
    render: () => (
      <div className="flex flex-wrap items-end gap-4">
        <DateField label="開始日" defaultValue="2026-10-28" />
        <Select label="コーチ" defaultValue="c1">
          <CoachOptions />
        </Select>
        <Select label="スポーツ" defaultValue="">
          <SportOptions first="すべて" />
        </Select>
        <Select label="キャンセルした人" defaultValue="student">
          <option value="">すべて</option>
          <option value="student">生徒</option>
          <option value="coach">コーチ</option>
          <option value="headquarters">本部</option>
        </Select>
        <Button>この条件で見る</Button>
      </div>
    ),
  },
  {
    component: "card",
    variant: "plain",
    render: () => (
      <Card>
        <p className="text-xl font-black">10/28（水）12:00〜12:30</p>
        <p className="text-sm text-text-muted">テニス・コーチ: 鈴木</p>
        <StatusTag label="確定済み" status="success" />
      </Card>
    ),
  },
  {
    component: "card",
    variant: "plain",
    example: "押せる",
    render: () => (
      <Card onClick={noop}>
        <p className="text-xl font-black">スイングの見直し</p>
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
    variant: "default",
    example: status,
    render: () => <StatusTag label={label} status={status} />,
  })),
  ...SLOT_STATE_EXAMPLES.map(({ state, label }) => ({
    component: "status_tag",
    variant: "default",
    example: `slot_state ${state}`,
    render: () => <StatusTag label={label} status={SLOT_STATE_STATUS[state]} />,
  })),
  {
    component: "status_tag",
    variant: "urgent",
    render: () => <StatusTag label="開始まで 24 時間以内" status="danger" urgent />,
  },
  ...(["info", "warning", "danger"] as const).map((state) => ({
    component: "notice",
    variant: "inline",
    state,
    render: () => <Notice message={NOTICE_MESSAGES[state]} status={state} />,
  })),
  // 成功は inline ではなく濃い面と完了の印（notice.variants.success）
  {
    component: "notice",
    variant: "success",
    render: () => <Notice message={NOTICE_MESSAGES.success} status="success" />,
  },
  {
    component: "empty_state",
    variant: "screen",
    render: () => (
      <EmptyState
        variant="screen"
        message="まだお知らせを送っていません。最初のお知らせを書いて生徒に届けましょう。"
        action={{ label: "お知らせを書く", onClick: noop }}
      />
    ),
  },
  {
    component: "empty_state",
    variant: "section",
    render: () => (
      <EmptyState
        variant="section"
        message="まだ低い評価はありません。評価が届くと、ここに新しい順に出ます。"
        action={{ label: "評価の一覧を見る", onClick: noop }}
      />
    ),
  },
  { component: "loading", variant: "bar", render: () => <LoadingBar /> },
  {
    component: "loading",
    variant: "section_text",
    render: () => <SectionLoading message="予約を読み込んでいます…" />,
  },
  { component: "loading", variant: "spinner", render: () => <Spinner /> },
  {
    component: "data_table",
    variant: "text",
    render: () => (
      <TableFrame>
        <DataTable label="10/28（水）の予約" columns={BOOKING_COLUMNS} rows={BOOKING_ROWS} rowKey={(row) => row.id} />
      </TableFrame>
    ),
  },
  {
    component: "data_table",
    variant: "numeric",
    render: () => (
      <TableFrame>
        <DataTable
          label="コーチごとの予約枠の需給"
          columns={SUPPLY_COLUMNS}
          rows={SUPPLY_ROWS}
          rowKey={(row) => row.coach}
        />
      </TableFrame>
    ),
  },
  {
    component: "data_table",
    variant: "long_text",
    render: () => (
      <TableFrame>
        <DataTable
          label="コーチごとの最新の一言"
          columns={[
            SUPPLY_COACH_COLUMN,
            {
              header: "最新の一言",
              variant: "long_text",
              cell: (row) => row.comment,
            },
          ]}
          rows={SUPPLY_ROWS}
          rowKey={(row) => row.coach}
        />
      </TableFrame>
    ),
  },
  {
    component: "data_table",
    variant: "text",
    state: "narrow",
    render: () => (
      <TableFrame narrow>
        <p className="mb-2 max-w-[40em] text-sm">10/28（水）〜 11/3（火）・3 件</p>
        <DataTable label="10/28（水）の予約" columns={BOOKING_COLUMNS} rows={BOOKING_ROWS} rowKey={(row) => row.id} />
      </TableFrame>
    ),
  },
  {
    component: "data_table",
    variant: "text",
    example: "急ぎの行",
    render: () => (
      <TableFrame>
        <DataTable
          label="10/28（水）の予約"
          columns={BOOKING_COLUMNS}
          rows={[{ ...PENDING_BOOKING, id: "b0", start: "9:00", urgent: true }, CONFIRMED_BOOKING]}
          rowKey={(row) => row.id}
          highlightRow={(row) => row.urgent === true}
        />
      </TableFrame>
    ),
  },
  {
    component: "description_list",
    variant: "inline",
    state: "default",
    render: () => <DescriptionList label="予約の詳細" items={BOOKING_DETAIL_ITEMS} />,
  },
  {
    component: "description_list",
    variant: "stacked",
    state: "default",
    render: () => (
      <TableFrame narrow>
        <DescriptionList variant="stacked" label="お知らせの中身" items={ANNOUNCEMENT_ITEMS} />
      </TableFrame>
    ),
  },
  {
    component: "description_list",
    variant: "inline",
    state: "loading",
    example: "項目ごと",
    render: () => (
      <DescriptionList
        label="生徒の様子"
        items={[
          { term: "表示名", value: "鈴木 一郎" },
          { term: "スポーツ", loading: true },
          { term: "返却待ち", error: "件数を読めませんでした。画面を更新すると、もう一度読み込めます。" },
        ]}
      />
    ),
  },
  {
    component: "description_list",
    variant: "inline",
    state: "loading",
    render: () => <DescriptionList items={[]} loading="予約を読み込んでいます…" />,
  },
  {
    component: "description_list",
    variant: "inline",
    state: "error",
    render: () => (
      <DescriptionList items={[]} error="予約を読み込めませんでした。画面を更新すると、もう一度読み込めます。" />
    ),
  },
  {
    component: "timeline",
    state: "default",
    render: () => <Timeline label="状態が変わった記録" events={BOOKING_HISTORY} empty="状態が変わった記録はありません" />,
  },
  {
    component: "timeline",
    state: "default",
    example: "対応のない出来事",
    render: () => <Timeline label="お知らせの経緯" events={ANNOUNCEMENT_HISTORY} empty="経緯はありません" />,
  },
  {
    component: "timeline",
    state: "loading",
    render: () => (
      <Timeline
        label="状態が変わった記録"
        events={[]}
        empty="状態が変わった記録はありません"
        loading="状態が変わった記録を読み込んでいます…"
      />
    ),
  },
  {
    component: "timeline",
    state: "error",
    render: () => (
      <Timeline
        label="状態が変わった記録"
        events={[]}
        empty="状態が変わった記録はありません"
        error="状態が変わった記録を読めませんでした。画面を更新すると、もう一度読み込めます。"
      />
    ),
  },
  {
    component: "timeline",
    state: "empty",
    render: () => <Timeline label="状態が変わった記録" events={[]} empty="状態が変わった記録はありません" />,
  },
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
  ...(["mic", "camera", "speaker_test"] as const).flatMap((kind) =>
    MEDIA_TOGGLE_STATES.map((state) => ({
      component: "media_toggle",
      variant: kind,
      state,
      render: () => <MediaTogglePair kind={kind} state={state} />,
    })),
  ),
  {
    component: "media_toggle",
    variant: "mic",
    state: "error",
    render: () => (
      <div className="flex max-w-xl flex-col gap-2">
        <Notice message="マイクを切り替えられませんでした。もう一度押してください。" status="danger" />
        <MediaTogglePair kind="mic" state="default" />
      </div>
    ),
  },
  ...CAMERA_PREVIEW_EXAMPLES.map(({ state, preview, example }) => ({
    component: "camera_preview",
    state,
    example,
    render: () => (
      <CameraPreview
        state={preview}
        caption={preview === "live" ? "映像はこの端末の中だけで映し、送りも残しもしません" : undefined}
      />
    ),
  })),
  {
    component: "level_meter",
    state: "default",
    render: () => <LevelMeter level={6} label="音が届いています" />,
  },
  {
    component: "level_meter",
    state: "loading",
    render: () => <LevelMeter level={0} label="マイクの許可を待っています" stopped />,
  },
  {
    component: "level_meter",
    state: "disabled",
    render: () => <LevelMeter level={0} label="マイクを切っています" stopped />,
  },
  {
    component: "level_meter",
    state: "error",
    render: () => <LevelMeter level={0} label="マイクが見つかりません。なくても、開始時刻から入室できます。" stopped />,
  },
  {
    component: "join_bar",
    state: "disabled",
    render: () => (
      <JoinBarFrame>
        <JoinBar
          startTime="12:00"
          remainingSeconds={168}
          meta="田中 美咲・フォアハンドの基礎（30 分）"
          onJoin={noop}
          secondaryAction={{ label: "予約の一覧に戻る", onClick: noop }}
        />
      </JoinBarFrame>
    ),
  },
  {
    component: "join_bar",
    state: "default",
    render: () => (
      <JoinBarFrame>
        <JoinBar
          startTime="12:00"
          remainingSeconds={0}
          meta="田中 美咲・フォアハンドの基礎（30 分）"
          onJoin={noop}
          secondaryAction={{ label: "予約の一覧に戻る", onClick: noop }}
        />
      </JoinBarFrame>
    ),
  },
  {
    component: "join_bar",
    state: "loading",
    render: () => (
      <JoinBarFrame>
        <JoinBar startTime="12:00" remainingSeconds={0} loading onJoin={noop} />
      </JoinBarFrame>
    ),
  },
  {
    component: "join_bar",
    state: "error",
    render: () => (
      <JoinBarFrame>
        <JoinBar
          startTime="12:00"
          remainingSeconds={0}
          error="この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。"
          onJoin={noop}
        />
      </JoinBarFrame>
    ),
  },
];

/** カタログの枠の名前（見出しと突き合わせに使う）。例: `button / primary / disabled`、`status_tag / warning` */
export function catalogEntryName(entry: Pick<CatalogEntry, "component" | "variant" | "state" | "example">): string {
  return [entry.component, entry.variant, entry.state, entry.example].filter(Boolean).join(" / ");
}
