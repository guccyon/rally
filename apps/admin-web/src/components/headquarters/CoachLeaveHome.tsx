"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import { createApiClient } from "@/api/client";
import { headquartersApi, type HeadquartersApi } from "@/api/headquarters";
import { BOOKING_STATUS_LABELS, COACH_ACCEPTANCE_STATUS_LABELS, SLOT_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";
import { Button } from "@/components/ui/Button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { DateField } from "@/components/ui/DateField";
import { EmptyState } from "@/components/ui/EmptyState";
import { ListRow } from "@/components/ui/ListRow";
import { LoadingBar, SectionLoading } from "@/components/ui/Loading";
import { Notice } from "@/components/ui/Notice";
import { Section } from "@/components/ui/Section";
import { StatusTag } from "@/components/ui/StatusTag";
import { TextArea } from "@/components/ui/TextField";
import { SLOT_STATE_STATUS, type Status } from "@/components/ui/status";

import { bookingDetailPath } from "./BookingDetailHome";
import { cancelledBookingsPath } from "./CancelledBookingsHome";
import { coachOverviewPath } from "./CoachOverviewHome";
import {
  NOTE_MAX_CHARS,
  countChars,
  leavePeriodErrors,
  periodLabel,
  slotTimeLabel,
  type LeavePeriod,
  type LeavePeriodErrors,
} from "./coachLeave";

/** コーチを指すクエリの名前（`?coach_id=`）。 */
export const COACH_LEAVE_QUERY = "coach_id";

/** そのコーチのお休みの画面の URL。 */
export function coachLeavePath(coachId: string): string {
  return `/headquarters/coaches/leave?${new URLSearchParams({ [COACH_LEAVE_QUERY]: coachId })}`;
}

/** 読み込み・送信がこの時間より速く終われば、spinner と「読み込んでいます…」を出さない（microinteractions）。 */
const FEEDBACK_DELAY_MS = 300;

const BOOKING_STATUS_COLORS: Record<Schemas["BookingStatus"], Status> = {
  pending: "warning",
  confirmed: "success",
  cancelled: "neutral",
};

type Api = Pick<HeadquartersApi, "getCoachOverview" | "getCoachLeavePreview" | "startCoachLeave">;

type CoachState =
  | { kind: "loading" }
  | { kind: "loaded"; overview: Schemas["CoachOverview"] }
  | { kind: "not_found" }
  | { kind: "failed"; message: string };

type PreviewState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "loaded"; preview: Schemas["CoachLeavePreview"]; period: LeavePeriod }
  | { kind: "failed"; message: string };

type Outcome =
  | { ok: true; message: string; result: Schemas["CoachLeaveResult"] }
  | { ok: false; message: string };

/** 本部の画面: コーチのお休み（コーチの様子の「お休み」から開く）。 */
export function CoachLeaveHome({ token, coachId }: { token: string; coachId: string | undefined }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return <CoachLeave api={api} coachId={coachId} />;
}

/**
 * コーチのお休みの期間を決め、期間の予約と枠を確かめてから（getCoachLeavePreview）、1 回の確認で期間の予約をまとめて本部の
 * キャンセルにし、生徒に知らせ、枠を取り下げる（startCoachLeave）。開いたときにコーチの様子（getCoachOverview）を一度だけ
 * 読む。期間を変えたら一覧は残して「お休みにする」を押せなくし、送るのはいつも見ている一覧の期間と予約の ID。
 * 生徒へのお知らせの文面の正本はサーバー（見本を出すだけ）。`now` は今日の日付を決める時計（テスト用）。
 */
export function CoachLeave({
  api,
  coachId,
  now = Date.now,
}: {
  api: Api;
  coachId: string | undefined;
  now?: () => number;
}) {
  const router = useRouter();
  const [coach, setCoach] = useState<CoachState>(coachId === undefined ? { kind: "not_found" } : { kind: "loading" });
  const [period, setPeriod] = useState<LeavePeriod>({ from: "", to: "" });
  const [touched, setTouched] = useState<{ from: boolean; to: boolean }>({ from: false, to: false });
  const [preview, setPreview] = useState<PreviewState>({ kind: "idle" });
  const [slow, setSlow] = useState(false);
  const [note, setNote] = useState("");
  const [noteTouched, setNoteTouched] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [sending, setSending] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const requestSeq = useRef(0);
  // 部品は ref を受けないので、包みの要素から欄とボタンを探してフォーカスを移す
  const fromRef = useRef<HTMLDivElement>(null);
  const toRef = useRef<HTMLDivElement>(null);
  const noteRef = useRef<HTMLDivElement>(null);
  const startRef = useRef<HTMLDivElement>(null);
  const focusIn = (wrapper: HTMLDivElement | null) =>
    wrapper?.querySelector<HTMLElement>("input, textarea, button")?.focus();
  const outcomeRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (coachId === undefined) return;
    let active = true;
    void api.getCoachOverview(coachId).then((response) => {
      if (!active) return;
      if (response.ok) setCoach({ kind: "loaded", overview: response.data });
      else if (response.code === "not_found") setCoach({ kind: "not_found" });
      else setCoach({ kind: "failed", message: response.message });
    });
    return () => {
      active = false;
    };
  }, [api, coachId]);

  useEffect(() => {
    if (outcome) outcomeRef.current?.focus();
  }, [outcome]);

  const errors: LeavePeriodErrors = leavePeriodErrors(period, now());
  const shownErrors: LeavePeriodErrors = {
    from: touched.from ? errors.from : undefined,
    to: touched.to ? errors.to : undefined,
  };
  const noteLength = countChars(note);
  const noteError =
    noteTouched && noteLength > NOTE_MAX_CHARS ? `一言は ${NOTE_MAX_CHARS} 文字までにしてください。` : undefined;
  const done = outcome?.ok === true;
  const loaded = preview.kind === "loaded" ? preview : null;
  const stale = loaded !== null && (loaded.period.from !== period.from || loaded.period.to !== period.to);
  const ready = loaded !== null && !stale && !done;

  if (coach.kind === "not_found" || coachId === undefined) {
    return (
      <>
        <BackLinks coachId={coachId} />
        <Title />
        <p className="text-sm">このコーチは見つかりません。コーチの一覧から選び直してください。</p>
      </>
    );
  }

  const coachName = coach.kind === "loaded" ? coach.overview.display_name : (loaded?.preview.coach.display_name ?? "");

  /** 期間の予約と枠を読む。前の結果が遅れて返っても捨て、最後に押した期間の結果だけを出す。 */
  async function check() {
    setTouched({ from: true, to: true });
    if (errors.from || errors.to) {
      focusIn((errors.from ? fromRef : toRef).current);
      return;
    }
    const requested = { ...period };
    const seq = ++requestSeq.current;
    setPreview({ kind: "loading" });
    setOutcome(null);
    setSlow(false);
    const timer = setTimeout(() => {
      if (seq === requestSeq.current) setSlow(true);
    }, FEEDBACK_DELAY_MS);
    const response = await api.getCoachLeavePreview(coachId as string, requested);
    clearTimeout(timer);
    if (seq !== requestSeq.current) return;
    setSlow(false);
    if (response.ok) setPreview({ kind: "loaded", preview: response.data, period: requested });
    else
      setPreview({
        kind: "failed",
        message: `期間の予約を読み込めませんでした。「期間の予約と枠を確かめる」でもう一度読み込めます。（${response.message}）`,
      });
  }

  function openConfirm() {
    setNoteTouched(true);
    if (noteLength > NOTE_MAX_CHARS) {
      focusIn(noteRef.current);
      return;
    }
    setConfirming(true);
  }

  function cancelConfirm() {
    if (sending) return;
    setConfirming(false);
    focusIn(startRef.current);
  }

  async function send() {
    if (!loaded || sending) return;
    setSending(true);
    const response = await api.startCoachLeave(coachId as string, {
      from: loaded.period.from,
      to: loaded.period.to,
      note,
      expected_booking_ids: loaded.preview.bookings.map((booking) => booking.booking_id),
    });
    setSending(false);
    setConfirming(false);
    if (response.ok) {
      const result = response.data;
      setOutcome({
        ok: true,
        result,
        message: `${coachName} コーチを ${periodLabel(loaded.period)}のお休みにしました。予約 ${result.cancelled_booking_count} 件をキャンセルして生徒 ${result.notified_student_count} 人にお知らせを送り、枠 ${result.withdrawn_slot_count} 件を取り下げました。`,
      });
    } else if (response.code === "leave_preview_outdated" || response.code === "coach_leave_overlaps") {
      setOutcome({ ok: false, message: response.message });
    } else {
      setOutcome({ ok: false, message: `${response.message} 予約も枠も変えていません。` });
    }
  }

  const cancelledIds = new Set(outcome?.ok ? outcome.result.cancelled_booking_ids : []);

  return (
    <>
      {((preview.kind === "loading" && slow) || sending) && <LoadingBar />}
      <BackLinks coachId={coachId} />
      <Title />
      {outcome && (
        <div ref={outcomeRef} tabIndex={-1} className="mb-6 outline-none">
          <Notice message={outcome.message} status={outcome.ok ? "success" : "danger"} />
        </div>
      )}
      {done && (
        <div className="mb-6 flex flex-wrap gap-2">
          <Button
            variant="secondary"
            onClick={() => router.push(cancelledBookingsPath({ coachId, lessonMenuId: "", sport: "" }))}
          >
            キャンセルされた予約を見る
          </Button>
          <Button variant="text" onClick={() => router.push(coachOverviewPath(coachId))}>
            コーチの様子へ戻る
          </Button>
        </div>
      )}
      <Section title="お休みにするコーチ">
        {coach.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
        {coach.kind === "failed" && <Notice message={coach.message} status="danger" />}
        {coach.kind === "loaded" && (
          <dl aria-label="お休みにするコーチ" className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt>コーチ</dt>
            <dd className="font-bold">{coach.overview.display_name}</dd>
            <dt>スポーツ</dt>
            <dd>
              {loaded
                ? loaded.preview.coach.sports.map((sport) => SPORT_LABELS[sport]).join("・") || "—"
                : "期間を確かめると出ます"}
            </dd>
            <dt>受付</dt>
            <dd>
              <StatusTag
                label={COACH_ACCEPTANCE_STATUS_LABELS[coach.overview.acceptance_status]}
                status={coach.overview.acceptance_status === "accepting" ? "success" : "neutral"}
              />
            </dd>
          </dl>
        )}
      </Section>
      {!done && (
        <Section title="お休みの期間">
          <div className="flex flex-col gap-2 min-[600px]:flex-row">
            <div ref={fromRef} className="min-w-0 flex-1">
              <DateField
                label="始まりの日"
                value={period.from}
                error={shownErrors.from}
                onChange={(event) => setPeriod({ ...period, from: event.target.value })}
                onBlur={() => setTouched({ ...touched, from: true })}
              />
            </div>
            <div ref={toRef} className="min-w-0 flex-1">
              <DateField
                label="終わりの日"
                value={period.to}
                error={shownErrors.to}
                onChange={(event) => setPeriod({ ...period, to: event.target.value })}
                onBlur={() => setTouched({ ...touched, to: true })}
              />
            </div>
          </div>
          <p className="mt-2 text-sm text-text-muted">明日から 31 日までの期間を選べます。始まりの日と終わりの日を含みます。</p>
          <div className="mt-4">
            <Button
              variant="secondary"
              loading={preview.kind === "loading" && slow}
              disabled={preview.kind === "loading"}
              onClick={() => void check()}
            >
              期間の予約と枠を確かめる
            </Button>
          </div>
          {stale && (
            <p role="status" className="mt-2 text-sm">
              期間を変えました。「期間の予約と枠を確かめる」で読み直してください。
            </p>
          )}
        </Section>
      )}
      {preview.kind === "loading" && slow && <SectionLoading message="期間の予約と枠を読み込んでいます…" />}
      {preview.kind === "failed" && <Notice message={preview.message} status="danger" />}
      {loaded && loaded.preview.bookings.length === 0 && loaded.preview.open_slots.length === 0 && (
        <EmptyState
          message={`${periodLabel(loaded.period)} に、${loaded.preview.coach.display_name} コーチの予約と公開中の枠はありません。キャンセルや取り下げは要りません。しばらく新しい予約を受けたくないときは、コーチの一覧で受付を止めてください。`}
        />
      )}
      {loaded && (loaded.preview.bookings.length > 0 || loaded.preview.open_slots.length > 0) && (
        <>
          <Section title={`期間の予約（${loaded.preview.bookings.length} 件）`}>
            <p className="mb-2 text-sm text-text-muted">
              {periodLabel(loaded.period)}に始まる、確定待ちと確定済みの予約です。開始の早い順。行から予約の詳細を開けます。
            </p>
            {loaded.preview.bookings.length === 0 ? (
              <p className="text-sm">期間の予約はありません。</p>
            ) : (
              <ul aria-label="期間の予約の一覧">
                {loaded.preview.bookings.map((booking) => {
                  const status: Schemas["BookingStatus"] = cancelledIds.has(booking.booking_id) ? "cancelled" : booking.status;
                  const detail = booking.is_trial
                    ? "体験予約"
                    : booking.lesson_menu_name
                      ? `メニュー: ${booking.lesson_menu_name}`
                      : undefined;
                  return (
                    <li key={booking.booking_id}>
                      <ListRow
                        title={slotTimeLabel(booking.starts_at, booking.ends_at)}
                        subtitle={[`生徒: ${booking.student_display_name ?? "（名前を表示できません）"}`, detail]
                          .filter(Boolean)
                          .join("・")}
                        trailing={<StatusTag label={BOOKING_STATUS_LABELS[status]} status={BOOKING_STATUS_COLORS[status]} />}
                        onClick={() => router.push(bookingDetailPath(booking.booking_id))}
                      />
                    </li>
                  );
                })}
              </ul>
            )}
          </Section>
          {loaded.preview.open_slots.length > 0 && (
            <Section title={`期間の予約のない枠（${loaded.preview.open_slots.length} 件）`}>
              <p className="mb-2 text-sm text-text-muted">
                公開中で、まだ予約の入っていない枠です。予約のある枠も、予約をキャンセルしたあとに取り下げます。
              </p>
              <ul aria-label="期間の予約のない枠の一覧">
                {loaded.preview.open_slots.map((slot) => {
                  const state = done ? "closed" : "open";
                  return (
                    <li key={slot.slot_id}>
                      <ListRow
                        title={slotTimeLabel(slot.starts_at, slot.ends_at)}
                        subtitle={`定員 ${slot.capacity} 人・${slot.lesson_menu_name ? `メニュー: ${slot.lesson_menu_name}` : "メニューなし"}`}
                        trailing={<StatusTag label={SLOT_STATUS_LABELS[state]} status={SLOT_STATE_STATUS[state]} />}
                      />
                    </li>
                  );
                })}
              </ul>
            </Section>
          )}
          {!done && (
            <>
              <Section title="生徒へのお知らせ">
                {loaded.preview.notification_preview && (
                  <>
                    <p className="mb-2 text-sm text-text-muted">
                      予約した生徒 {loaded.preview.student_count} 人に、予約ごとに次のお知らせが届きます（プッシュ通知は、お知らせの受け取りをオンにした生徒だけ）。
                    </p>
                    <div className="mb-4 space-y-1 rounded-md border border-border bg-background p-4 text-sm">
                      <p className="font-bold">{loaded.preview.notification_preview.title}</p>
                      <p>{loaded.preview.notification_preview.body}</p>
                      <p className="text-text-muted">（日時は予約ごとに入ります。下の一言を書くと、この後ろに付きます）</p>
                    </div>
                  </>
                )}
                <div ref={noteRef}>
                <TextArea
                  label="生徒への一言（任意）"
                  helper={`${NOTE_MAX_CHARS} 文字まで`}
                  counter={`${noteLength} / ${NOTE_MAX_CHARS}`}
                  error={noteError}
                  value={note}
                  onChange={(event) => setNote(event.target.value)}
                  onBlur={() => setNoteTouched(true)}
                />
                </div>
              </Section>
              <div className="mb-6">
                <Notice
                  status="warning"
                  message="キャンセルした予約は元に戻せません。生徒の今月のビデオ通話の枠は使いません（キャンセル規定の回数にも数えません）。期間の外の予約と枠はそのままです。"
                />
              </div>
            </>
          )}
        </>
      )}
      {!done && (
        <div ref={startRef} className="space-y-2">
          <Button disabled={!ready} onClick={openConfirm}>
            お休みにする
          </Button>
          {!ready && <p className="text-sm text-text-muted">期間を確かめると押せます。</p>}
        </div>
      )}
      {sending && <SectionLoading message="お休みにしています…" />}
      {loaded && (
        <ConfirmDialog
          open={confirming}
          title={`${coachName} コーチをお休みにしますか？`}
          body={`${periodLabel(loaded.period)}の予約 ${loaded.preview.bookings.length} 件をキャンセルして生徒 ${loaded.preview.student_count} 人にお知らせを送り、枠 ${loaded.preview.slot_count} 件を取り下げます。キャンセルした予約は元に戻せません。生徒の今月のビデオ通話の枠は使いません。`}
          confirmLabel="キャンセルしてお休みにする"
          danger
          busy={sending}
          onConfirm={() => void send()}
          onCancel={cancelConfirm}
        />
      )}
    </>
  );
}

function Title() {
  return (
    <>
      <h2 className="mb-2 text-2xl font-bold">コーチのお休み</h2>
      <p className="mb-6 text-text-muted">
        お休みの期間を入れると、その期間の予約と枠が出ます。「お休みにする」で、予約をまとめてキャンセルして生徒にお知らせを送り、枠を取り下げます。
      </p>
    </>
  );
}

function BackLinks({ coachId }: { coachId: string | undefined }) {
  return (
    <p className="mb-4 space-x-4 text-sm">
      <Link href={coachId === undefined ? "/headquarters/coaches" : coachOverviewPath(coachId)} className="text-primary underline">
        {coachId === undefined ? "コーチの一覧へ戻る" : "コーチの様子へ戻る"}
      </Link>
      <Link href="/headquarters" className="text-primary underline">
        本部の画面へ戻る
      </Link>
    </p>
  );
}
