import { SectionLoading } from "./Loading";
import { Notice } from "./Notice";
import { STATUS_DOT_CLASSES, type Status } from "./status";

// 経緯の並び（specs/design/components.yaml の timeline）。1 件に起きた出来事を古い順に、点と線でつないで縦に並べる。
// 11 件以上か期間で絞る記録は data_table にする。

export type TimelineEvent = {
  /** 並びの中で重ならない値（API の id や時刻と状態） */
  key: string;
  /** 「10/27（火）9:12」 */
  time: string;
  /** 状態なら glossary の呼び名（「確定済み」）、ほかは「〜した」（「取り下げた」） */
  label: string;
  /**
   * 状態の出来事だけ。booking_state・slot_state の対応の色（BOOKING_STATE_STATUS・SLOT_STATE_STATUS）を渡す。
   * 渡すと label を status_tag の字で、渡さないと body 700 で出し、点は border-strong にする。
   */
  status?: Status;
  /** 「本部: 鈴木」「理由: コーチのお休み」 */
  meta?: string;
};

/** 出来事を 1 つの文で読む（「10/27（火）9:12、確定済み、本部: 鈴木」）。 */
export function timelineEventSentence(event: Pick<TimelineEvent, "time" | "label" | "meta">): string {
  return [event.time, event.label, event.meta].filter(Boolean).join("、");
}

/**
 * 出来事は渡した順（API の順）のまま並べ、動きは付けない。rail（点と線）は読み上げず、出来事を 1 つの文で読む。
 * 読んでいるあいだは loading（section_text）、読めなかったときは error（notice の inline）、0 件は empty の文を出す。
 */
export function Timeline({
  label,
  events,
  empty,
  loading,
  error,
}: {
  /** ol の aria-label（「状態が変わった記録」） */
  label: string;
  events: TimelineEvent[];
  /** 0 件のときの文（「状態が変わった記録はありません」） */
  empty: string;
  loading?: string;
  error?: string;
}) {
  if (loading) return <SectionLoading message={loading} />;
  if (error) return <Notice message={error} status="danger" />;
  if (events.length === 0) return <p className="text-base text-text">{empty}</p>;
  return (
    <ol aria-label={label} className="m-0 list-none p-0">
      {events.map((event, index) => {
        const first = index === 0;
        const last = index === events.length - 1;
        const muted = event.status === "neutral" || event.status === undefined;
        return (
          <li key={event.key} className="flex gap-2">
            {/* rail。幅 16px に 8px の点、前後の出来事へ border の 2px の線 */}
            <span aria-hidden="true" className="flex w-4 shrink-0 flex-col items-center">
              <span className={`h-2 w-0.5 ${first ? "" : "bg-border"}`} />
              <span
                data-testid="timeline-dot"
                className={`size-2 shrink-0 rounded-pill ${STATUS_DOT_CLASSES[event.status ?? "neutral"]}`}
              />
              {!last && <span className="w-0.5 flex-1 bg-border" />}
            </span>
            <span className="sr-only">{timelineEventSentence(event)}</span>
            <div aria-hidden="true" className={`flex min-w-0 flex-col gap-1 ${last ? "" : "pb-4"}`}>
              <p className="text-sm text-text-muted tabular-nums">{event.time}</p>
              {event.status ? (
                <p className={`text-sm font-bold ${muted ? "text-text-muted" : "text-text"}`}>{event.label}</p>
              ) : (
                <p className="text-base font-bold text-text">{event.label}</p>
              )}
              {event.meta && <p className="text-sm text-text-muted">{event.meta}</p>}
            </div>
          </li>
        );
      })}
    </ol>
  );
}
