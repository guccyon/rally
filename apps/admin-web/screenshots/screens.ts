import type { Page } from "@playwright/test";

import type { Role } from "@/session/session";

// キャプチャする画面の一覧。画面を足したり、たどり方（押す文言）を変えたりしたらここを直す。
// id はファイル名と、前後の比較の突き合わせに使うので変えない（変えると「消えた画面」と「新しい画面」になる）。

export type Screen = {
  id: string;
  title: string;
  /** ログインしておくロール（なければログインしない） */
  role?: Role;
  path: string;
  /** 開いたあとにする操作（タブを押す等） */
  open?: (page: Page) => Promise<void>;
};

const coachTab = (name: string) => async (page: Page) => {
  await page.getByRole("tab", { name: new RegExp(`^${name}`) }).click();
};

export const SCREENS: Screen[] = [
  { id: "login", title: "ログイン", path: "/" },
  // デザインシステムの部品のカタログ（本番のメニューには出さない）。部品が変わるとここに出る
  { id: "design-catalog", title: "デザインシステム: 部品のカタログ", path: "/design" },
  { id: "coach-tasks", title: "コーチ: 対応待ち", role: "coach", path: "/coach" },
  { id: "coach-slots", title: "コーチ: 予約枠", role: "coach", path: "/coach", open: coachTab("予約枠") },
  {
    id: "coach-lesson-menus",
    title: "コーチ: レッスンメニュー",
    role: "coach",
    path: "/coach",
    open: coachTab("レッスンメニュー"),
  },
  { id: "coach-videos", title: "コーチ: 動画の添削", role: "coach", path: "/coach", open: coachTab("動画の添削") },
  {
    id: "coach-chat",
    title: "コーチ: チャット",
    role: "coach",
    path: "/coach",
    open: async (page) => {
      await coachTab("チャット")(page);
      await page.getByRole("button", { name: /山田 花子/ }).first().click();
    },
  },
  { id: "coach-live", title: "コーチ: 通話への参加", role: "coach", path: "/coach", open: coachTab("通話への参加") },
  { id: "coach-reviews", title: "コーチ: 自分の評価", role: "coach", path: "/coach", open: coachTab("自分の評価") },
  { id: "coach-profile", title: "コーチ: 自己紹介", role: "coach", path: "/coach", open: coachTab("自己紹介") },
  {
    id: "coach-notifications",
    title: "コーチ: お知らせ",
    role: "coach",
    path: "/coach",
    open: async (page) => {
      await page.getByRole("button", { name: /^お知らせ/ }).click();
    },
  },
  { id: "hq-home", title: "本部: ホーム", role: "headquarters", path: "/headquarters" },
  { id: "hq-slot-supply", title: "本部: 予約枠の需給", role: "headquarters", path: "/headquarters/slot-supply" },
  {
    id: "hq-coach-reviews",
    title: "本部: コーチの評価",
    role: "headquarters",
    path: "/headquarters/coach-review-summary",
  },
  {
    id: "hq-coach-reviews-selected",
    title: "本部: コーチの評価（低い評価から移ったとき）",
    role: "headquarters",
    path: "/headquarters/coach-review-summary?coach_id=22222222-2222-4222-8222-222222222222",
  },
  { id: "hq-low-reviews", title: "本部: 低い評価", role: "headquarters", path: "/headquarters/low-reviews" },
  { id: "hq-coaches", title: "本部: コーチの受付", role: "headquarters", path: "/headquarters/coaches" },
  {
    id: "hq-coach-overview",
    title: "本部: コーチの様子",
    role: "headquarters",
    path: "/headquarters/coaches",
    open: async (page) => {
      await page.getByRole("link", { name: "鈴木 コーチ" }).click();
      await page.getByRole("heading", { name: "鈴木 コーチ" }).waitFor();
      await page.getByRole("table", { name: "受付の記録" }).waitFor();
    },
  },
  { id: "hq-lesson-menus", title: "本部: レッスンメニュー", role: "headquarters", path: "/headquarters/lesson-menus" },
  {
    id: "hq-lesson-menus-by-coach",
    title: "本部: コーチ 1 人のレッスンメニュー",
    role: "headquarters",
    path: "/headquarters/coach-overview?coach_id=22222222-2222-4222-8222-222222222222",
    open: async (page) => {
      await page.getByRole("link", { name: "このコーチのレッスンメニューを見る" }).click();
      await page.getByRole("table", { name: "レッスンメニューの一覧" }).waitFor();
    },
  },
  {
    id: "hq-lesson-menu-unpublish",
    title: "本部: レッスンメニューの非公開の確認",
    role: "headquarters",
    path: "/headquarters/lesson-menus",
    open: async (page) => {
      await page.getByRole("button", { name: "「スイングの見直し」を非公開にする" }).click();
    },
  },
  { id: "hq-students", title: "本部: 生徒", role: "headquarters", path: "/headquarters/students" },
  {
    id: "hq-student-activity",
    title: "本部: 生徒の利用の様子",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
  },
  { id: "hq-bookings", title: "本部: 予約", role: "headquarters", path: "/headquarters/bookings" },
  {
    id: "hq-overdue-live-sessions",
    title: "本部: 終わっていない通話",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions",
  },
  { id: "hq-announcements", title: "本部: お知らせ", role: "headquarters", path: "/headquarters/announcements" },
  {
    id: "hq-announcement-confirm",
    title: "本部: お知らせの送る前の確認",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      await page.getByLabel("対象").selectOption({ label: "テニスの生徒" });
      await page.getByLabel("題").fill("コートの予約方法が変わります");
      await page.getByLabel("本文").fill("11月から、コートの予約はアプリの「予約」から行えます。\n詳しくはお知らせの一覧をご覧ください。");
      await page.getByRole("button", { name: "内容を確かめる" }).click();
    },
  },
  {
    id: "hq-pending-video-reviews",
    title: "本部: 返却待ちの動画添削",
    role: "headquarters",
    path: "/headquarters/pending-video-reviews",
  },
];
