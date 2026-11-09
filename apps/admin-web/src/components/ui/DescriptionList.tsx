import type { ReactNode } from "react";

import { Icon } from "./Icon";
import { SectionLoading } from "./Loading";
import { Notice } from "./Notice";

// ラベルと値の並び（specs/design/components.yaml の description_list）。画面に dl を直に書かず、この部品で描く。

/** 種類（description_list.variants）。inline も画面の幅 600px 未満では上下に積む。 */
export type DescriptionListVariant = "inline" | "stacked";

export type DescriptionItem = {
  /** glossary の名詞で短く（「今の状態」）。「：」を付けない */
  term: string;
  /** 値。null・undefined・空の文字は「—」にして項目を消さない。文字の改行は残す */
  value?: ReactNode;
  /** 別の API を待つ項目だけ。value の代わりに「読み込み中…」 */
  loading?: boolean;
  /** この項目だけ読めなかったとき。value の代わりに error_text（何が起きたかとどうすればよいか） */
  error?: string;
};

/** 値がないときの表記（data_table の wording と同じ）。 */
export const NO_VALUE = "—";

const ITEM_CLASSES: Record<DescriptionListVariant, string> = {
  inline: "grid grid-cols-1 gap-y-1 min-[600px]:grid-cols-[10em_1fr] min-[600px]:items-baseline min-[600px]:gap-x-4",
  stacked: "grid grid-cols-1 gap-y-1",
};

function Value({ item }: { item: DescriptionItem }) {
  if (item.loading) {
    return (
      <dd role="status" className="m-0 text-sm text-text-muted">
        読み込み中…
      </dd>
    );
  }
  if (item.error) {
    return (
      <dd role="alert" className="m-0 flex items-start gap-1 text-sm text-danger">
        <Icon name="error" size={20} className="shrink-0" />
        {item.error}
      </dd>
    );
  }
  const empty = item.value === null || item.value === undefined || item.value === "";
  return (
    <dd className="m-0 whitespace-pre-line text-base text-text tabular-nums [word-break:auto-phrase]">
      {empty ? NO_VALUE : item.value}
    </dd>
  );
}

/**
 * 1 件の項目をラベル（term 10em・small 700 text-muted）と値（body）の組で縦に並べる。面と枠で囲わず、項目のあいだに border の 1px。
 * 全体を読んでいるあいだは loading（section_text）、全体を読めなかったときは error（notice の inline）を渡す。
 * 画面に 2 つ以上置くときは label（aria-label）を付ける。
 */
export function DescriptionList({
  items,
  variant = "inline",
  label,
  loading,
  error,
}: {
  items: DescriptionItem[];
  variant?: DescriptionListVariant;
  label?: string;
  /** 全体を読んでいるあいだの文（「予約を読み込んでいます…」） */
  loading?: string;
  /** 全体を読めなかったときの文 */
  error?: string;
}) {
  if (loading) return <SectionLoading message={loading} />;
  if (error) return <Notice message={error} status="danger" />;
  return (
    <dl aria-label={label} className="m-0">
      {items.map((item) => (
        <div key={item.term} className={`${ITEM_CLASSES[variant]} border-t py-2 first:border-t-0`}>
          <dt className="text-sm font-bold text-text-muted [word-break:auto-phrase]">{item.term}</dt>
          <Value item={item} />
        </div>
      ))}
    </dl>
  );
}
