"use client";

import { useEffect, useMemo, useState } from "react";

import { createApiClient } from "@/api/client";
import { coachApi } from "@/api/coach";
import { Section } from "@/components/Section";

import { CoachChat } from "./CoachChat";
import { CoachProfileForm } from "./CoachProfileForm";
import { CoachTasks } from "./CoachTasks";
import { LessonMenus } from "./LessonMenus";
import { LiveSessionPanel } from "./LiveSessionPanel";
import { MyCoachReviews } from "./MyCoachReviews";
import { countPendingTasks, loadPendingTasks, type PendingTasks } from "./pendingTasks";
import { PublishSlotForm } from "./PublishSlotForm";
import { ReturnVideoReviewForm } from "./ReturnVideoReviewForm";

type Tab = "tasks" | "slots" | "menus" | "videos" | "chat" | "live" | "reviews" | "profile";

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

/** 対応待ちから移るときに、移った先の画面で開いておくもの（添削する動画・チャットのスレッド）。 */
type Focus = { videoId?: string; threadId?: string };

/**
 * コーチの画面: メニュー（タブ）で、対応待ち・枠の公開・レッスンメニューの登録と編集・添削の返却・
 * 生徒とのチャット（添削の依頼をスレッドの中から返せる）・通話への参加（ADR 0002 Q3）・自分の評価・自己紹介の編集を切り替える。開いたときは対応待ち。
 * メニューの「対応待ち」には未対応の件数のバッジを出し（0 件なら出さない）、タブを切り替えるたびに数え直す。
 */
export function CoachHome({ token }: { token: string }) {
  const api = useMemo(() => coachApi(createApiClient(token)), [token]);
  const [tab, setTab] = useState<Tab>("tasks");
  const [focus, setFocus] = useState<Focus>({});
  const [tasks, setTasks] = useState<PendingTasks | null>(null);

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

  function open(next: Tab, nextFocus: Focus = {}) {
    setTab(next);
    setFocus(nextFocus);
  }

  const pendingCount = tasks ? countPendingTasks(tasks) : 0;
  const current = TABS.find((item) => item.id === tab)!;

  return (
    <>
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
      <div role="tabpanel" id="coach-tab-panel" aria-labelledby={`coach-tab-${tab}`}>
        <Section title={current.title}>
          {tab === "tasks" && (
            <CoachTasks
              tasks={tasks}
              onOpenVideo={(videoId) => open("videos", { videoId })}
              onOpenBookings={() => open("live")}
              onOpenThread={(threadId) => open("chat", { threadId })}
            />
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
    </>
  );
}
