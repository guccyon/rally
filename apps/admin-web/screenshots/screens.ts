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
  { id: "hq-coaches", title: "本部: コーチの受付", role: "headquarters", path: "/headquarters/coaches" },
  { id: "hq-lesson-menus", title: "本部: レッスンメニュー", role: "headquarters", path: "/headquarters/lesson-menus" },
  { id: "hq-students", title: "本部: 生徒", role: "headquarters", path: "/headquarters/students" },
  { id: "hq-bookings", title: "本部: 予約", role: "headquarters", path: "/headquarters/bookings" },
];
