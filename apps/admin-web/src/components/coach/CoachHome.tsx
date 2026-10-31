"use client";

import { useEffect, useMemo, useState } from "react";

import { createApiClient, type Schemas } from "@/api/client";
import { coachApi } from "@/api/coach";
import { Section } from "@/components/Section";

import { CoachChat } from "./CoachChat";
import { CoachNotifications, countUnreadNotifications } from "./CoachNotifications";
import { CoachProfileForm } from "./CoachProfileForm";
import { CoachTasks } from "./CoachTasks";
import { LessonMenus } from "./LessonMenus";
import { LessonStats } from "./LessonStats";
import { LiveSessionPanel } from "./LiveSessionPanel";
import { MyCoachReviews } from "./MyCoachReviews";
import { countPendingTasks, loadPendingTasks, type PendingTasks } from "./pendingTasks";
import { PublishSlotForm } from "./PublishSlotForm";
import { ReturnVideoReviewForm } from "./ReturnVideoReviewForm";

type Tab = "tasks" | "slots" | "menus" | "videos" | "chat" | "live" | "reviews" | "profile";

/** タブのほかに開ける画面（見出しの「お知らせ」から開く。メニューのタブには並べない）。 */
type View = Tab | "notifications";

const TABS: { id: Tab; title: string }[] = [
  { id: "tasks", title: "対応待ち" },
  { id: "slots", title: "予約枠" },
  { id: "menus", title: "レッスンメニュー" },
  { id: "videos", title: "動画の添削" },
  { id: "chat", title: "チャット" },
  { id: "live", title: "通話への参加" },
  { id: "reviews", title: "自分の評価" },
  { id: "profile", title: "自己紹介" },
];

/**
 * お知らせの種類 → 行を押したときに移るタブ（お知らせは行き先の ID を持たないので、その種類の一覧の画面）。
 * 新しい予約は「通話への参加」（自分の予約の一覧）、添削の依頼は「動画の添削」、メッセージは「チャット」。
 */
const NOTIFICATION_TABS: Record<Schemas["NotificationKind"], Tab> = {
  booking_confirmed: "live",
  video_review_requested: "videos",
  message_received: "chat",
};

const NOTIFICATION_DESTINATIONS: Record<string, string> = Object.fromEntries(
  Object.entries(NOTIFICATION_TABS).map(([kind, tab]) => [kind, TABS.find((item) => item.id === tab)!.title]),
);

/** 対応待ちから移るときに、移った先の画面で開いておくもの（添削する動画・チャットのスレッド）。 */
type Focus = { videoId?: string; threadId?: string };

/**
 * コーチの画面: メニュー（タブ）で、対応待ち・枠の公開・レッスンメニューの登録と編集・添削の返却・
 * 生徒とのチャット（添削の依頼をスレッドの中から返せる）・通話への参加（ADR 0002 Q3）・自分の評価・自己紹介の編集を切り替える。開いたときは対応待ち。
 * 対応待ちの上には、今月と先月のレッスンの実績（LessonStats）を出す。
 * メニューの「対応待ち」には未対応の件数のバッジを出し（0 件なら出さない）、タブを切り替えるたびに数え直す。
 * メニューの上の「お知らせ」には未読のお知らせの数のバッジを出し（0 件なら出さない）、押すとお知らせの一覧を開いて
 * すべて既読にする。未読の数も開いたときとタブを切り替えるたびに数え直す。
 */
export function CoachHome({ token }: { token: string }) {
  const api = useMemo(() => coachApi(createApiClient(token)), [token]);
  const [tab, setTab] = useState<View>("tasks");
  const [focus, setFocus] = useState<Focus>({});
  const [tasks, setTasks] = useState<PendingTasks | null>(null);
  const [unreadNotifications, setUnreadNotifications] = useState(0);

  // 開いたときとタブを切り替えたときに対応待ちを取り直す（ほかの画面で添削を返したり既読にしたりした分を数え直す）。
  // 取り直している間は、前の結果を出したままにする。
  useEffect(() => {
    let active = true;
    void loadPendingTasks(api, Date.now()).then((next) => {
      if (active) setTasks(next);
    });
    return () => {
      active = false;
    };
  }, [api, tab]);

  // 見出しの未読の数。お知らせの一覧を開いている間は、一覧が既読にしたあとの数（onLoaded）を使う。
  // 読めなければ前の数のままにし、失敗の文言は出さない。
  useEffect(() => {
    if (tab === "notifications") return;
    let active = true;
    void api.listMyNotifications().then((next) => {
      if (active && next.ok) setUnreadNotifications(countUnreadNotifications(next.data.notifications));
    });
    return () => {
      active = false;
    };
  }, [api, tab]);

  function open(next: View, nextFocus: Focus = {}) {
    setTab(next);
    setFocus(nextFocus);
  }

  const pendingCount = tasks ? countPendingTasks(tasks) : 0;
  const current = TABS.find((item) => item.id === tab);

  return (
    <>
      <div className="mb-2 flex justify-end">
        <button
          type="button"
          aria-pressed={tab === "notifications"}
          aria-label={unreadNotifications > 0 ? `お知らせ（未読 ${unreadNotifications} 件）` : undefined}
          onClick={() => open("notifications")}
          className={`rounded border px-3 py-1 text-sm ${tab === "notifications" ? "bg-gray-100 font-semibold" : ""}`}
        >
          お知らせ
          {unreadNotifications > 0 && (
            <span aria-hidden className="ml-1 rounded-full bg-red-600 px-2 text-xs text-white">
              {unreadNotifications}
            </span>
          )}
        </button>
      </div>
      <div role="tablist" aria-label="コーチのメニュー" className="mb-6 flex flex-wrap gap-1 border-b">
        {TABS.map((item) => {
          const selected = item.id === tab;
          const badge = item.id === "tasks" && pendingCount > 0 ? pendingCount : null;
          return (
            <button
              key={item.id}
              type="button"
              role="tab"
              id={`coach-tab-${item.id}`}
              aria-selected={selected}
              aria-controls="coach-tab-panel"
              aria-label={badge ? `${item.title}（未対応 ${badge} 件）` : undefined}
              onClick={() => open(item.id)}
              className={`rounded-t px-3 py-2 text-sm ${selected ? "border border-b-0 bg-white font-semibold" : "text-gray-600"}`}
            >
              {item.title}
              {badge && (
                <span aria-hidden className="ml-1 rounded-full bg-red-600 px-2 text-xs text-white">
                  {badge}
                </span>
              )}
            </button>
          );
        })}
      </div>
      {tab === "notifications" && (
        <Section title="お知らせ">
          <CoachNotifications
            api={api}
            destinations={NOTIFICATION_DESTINATIONS}
            onLoaded={(notifications) => setUnreadNotifications(countUnreadNotifications(notifications))}
            onOpen={(kind) => open(NOTIFICATION_TABS[kind])}
          />
        </Section>
      )}
      {current && (
        <div role="tabpanel" id="coach-tab-panel" aria-labelledby={`coach-tab-${tab}`}>
          <Section title={current.title}>
            {tab === "tasks" && (
              <div className="space-y-6">
                <LessonStats api={api} />
                <CoachTasks
                  tasks={tasks}
                  onOpenVideo={(videoId) => open("videos", { videoId })}
                  onOpenBookings={() => open("live")}
                  onOpenThread={(threadId) => open("chat", { threadId })}
                />
              </div>
            )}
            {tab === "slots" && <PublishSlotForm api={api} />}
            {tab === "menus" && <LessonMenus api={api} />}
            {tab === "videos" && <ReturnVideoReviewForm api={api} initialVideoId={focus.videoId} />}
            {tab === "chat" && <CoachChat api={api} initialThreadId={focus.threadId} />}
            {tab === "live" && <LiveSessionPanel api={api} />}
            {tab === "reviews" && <MyCoachReviews api={api} />}
            {tab === "profile" && <CoachProfileForm api={api} />}
          </Section>
        </div>
      )}
    </>
  );
}
