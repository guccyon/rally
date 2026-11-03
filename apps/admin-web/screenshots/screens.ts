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

// 予約枠の需給の期間を入れて「この条件で見る」で読み直し、読み直した結果の文言が出るまで待つ。
const slotSupplyPeriod = (from: string, to: string, shown: string) => async (page: Page) => {
  await page.getByRole("table").waitFor();
  await page.getByLabel("開始日").fill(from);
  await page.getByLabel("終了日").fill(to);
  await page.getByRole("button", { name: "この条件で見る" }).click();
  await page.getByText(shown).waitFor();
};

export const SCREENS: Screen[] = [
  { id: "login", title: "ログイン", path: "/" },
  // デザインシステムの部品のカタログ（本番のメニューには出さない）。部品が変わるとここに出る
  { id: "design-catalog", title: "デザインシステム: 部品のカタログ", path: "/design" },
  { id: "coach-tasks", title: "コーチ: 対応待ち", role: "coach", path: "/coach" },
  { id: "coach-slots", title: "コーチ: 予約枠", role: "coach", path: "/coach", open: coachTab("予約枠") },
  {
    id: "coach-slots-period-empty",
    title: "コーチ: 予約枠（期間を指定して 0 件）",
    role: "coach",
    path: "/coach",
    open: async (page) => {
      await coachTab("予約枠")(page);
      await page.getByLabel("開始日").fill("2026-12-21");
      await page.getByLabel("終了日").fill("2026-12-27");
      await page.getByRole("button", { name: "この期間で見る" }).click();
      await page.getByText("に始まる枠はありません。").waitFor();
    },
  },
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
    id: "hq-slot-supply-period",
    title: "本部: 予約枠の需給（期間を指定）",
    role: "headquarters",
    path: "/headquarters/slot-supply",
    open: slotSupplyPeriod("2026-11-02", "2026-11-08", "2026年11月2日"),
  },
  {
    id: "hq-slot-supply-coach",
    title: "本部: 予約枠の需給（期間とコーチを指定）",
    role: "headquarters",
    path: "/headquarters/slot-supply",
    open: async (page) => {
      await page.getByRole("table").waitFor();
      await page.getByLabel("開始日").fill("2026-11-02");
      await page.getByLabel("終了日").fill("2026-11-08");
      // 選択肢（コーチの一覧）を読むまでは欄が使えないので、selectOption が使えるようになるまで待つ。
      await page.getByRole("combobox", { name: "コーチ" }).selectOption({ label: "鈴木 コーチ" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByRole("heading", { name: "鈴木 コーチ の予約枠の需給" }).waitFor();
    },
  },
  {
    id: "hq-slot-supply-sport",
    title: "本部: 予約枠の需給（期間とスポーツを指定）",
    role: "headquarters",
    path: "/headquarters/slot-supply",
    open: async (page) => {
      await page.getByRole("table").waitFor();
      await page.getByLabel("開始日").fill("2026-11-02");
      await page.getByLabel("終了日").fill("2026-11-08");
      await page.getByRole("combobox", { name: "スポーツ" }).selectOption({ label: "ゴルフ" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByRole("heading", { name: "ゴルフを提供するコーチの予約枠の需給" }).waitFor();
    },
  },
  {
    id: "hq-slot-supply-lesson-menu",
    title: "本部: 予約枠の需給（期間とレッスンメニューを指定）",
    role: "headquarters",
    path: "/headquarters/slot-supply",
    open: async (page) => {
      await page.getByRole("table").waitFor();
      await page.getByLabel("開始日").fill("2026-11-02");
      await page.getByLabel("終了日").fill("2026-11-08");
      // 選択肢（レッスンメニューの一覧）を読むまでは欄が使えないので、selectOption が使えるようになるまで待つ。
      await page.getByRole("combobox", { name: "レッスンメニュー" }).selectOption({ label: "フォアハンドの基礎（鈴木 コーチ）" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText(/^レッスンメニューで絞っています: フォアハンドの基礎$/).waitFor();
      await page.getByRole("table").waitFor();
    },
  },
  {
    id: "hq-slot-supply-empty",
    title: "本部: 予約枠の需給（期間を指定して 0 件）",
    role: "headquarters",
    path: "/headquarters/slot-supply",
    open: slotSupplyPeriod("2026-12-21", "2026-12-27", "予約枠はなく"),
  },
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
  {
    // 点数で絞った状態（URL のクエリ max_rating に残した点で開く）。
    id: "hq-coach-reviews-max-rating",
    title: "本部: コーチの評価（点数で絞る）",
    role: "headquarters",
    path: "/headquarters/coach-review-summary?coach_id=22222222-2222-4222-8222-222222222222&max_rating=4",
    open: async (page) => {
      await page.getByText("受けた評価（4 点以下・新しい順）").waitFor();
    },
  },
  {
    id: "hq-coach-reviews-max-rating-empty",
    title: "本部: コーチの評価（点数で絞って 0 件）",
    role: "headquarters",
    path: "/headquarters/coach-review-summary?coach_id=22222222-2222-4222-8222-222222222222&max_rating=3",
    open: async (page) => {
      await page.getByText("3 点以下の評価はありません").waitFor();
    },
  },
  {
    // 期間で絞った状態（URL のクエリ from・to に残した期間で開く）。件数と平均は絞らない。
    id: "hq-coach-reviews-period",
    title: "本部: コーチの評価（期間で絞る）",
    role: "headquarters",
    path: "/headquarters/coach-review-summary?coach_id=22222222-2222-4222-8222-222222222222&from=2026-10-26&to=2026-11-01",
    open: async (page) => {
      await page.getByText("受けた評価（2026年10月26日（月） 〜 2026年11月1日（日）・新しい順）").waitFor();
    },
  },
  { id: "hq-low-reviews", title: "本部: 低い評価", role: "headquarters", path: "/headquarters/low-reviews" },
  {
    id: "hq-low-reviews-handling-confirm",
    title: "本部: 低い評価（対応済みにする前の確認）",
    role: "headquarters",
    path: "/headquarters/low-reviews",
    open: async (page) => {
      await page.getByRole("button", { name: /を対応済みにする$/ }).first().click();
      await page.getByRole("button", { name: "対応済みを確定" }).waitFor();
    },
  },
  { id: "hq-coaches", title: "本部: コーチの受付", role: "headquarters", path: "/headquarters/coaches" },
  {
    // 表示名で探した状態（URL のクエリ q に残した検索語で開く）。
    id: "hq-coaches-search",
    title: "本部: コーチの受付（表示名で探す）",
    role: "headquarters",
    path: "/headquarters/coaches?q=%E9%88%B4%E6%9C%A8",
    open: async (page) => {
      await page.getByRole("table", { name: "コーチの一覧" }).waitFor();
    },
  },
  {
    // 直近 30 日にレッスンのないコーチだけに絞った状態（URL のクエリ inactive=true で開く。該当がなければ空の表示）。
    id: "hq-coaches-inactive",
    title: "本部: コーチの受付（直近 30 日にレッスンのないコーチだけ）",
    role: "headquarters",
    path: "/headquarters/coaches?inactive=true",
    open: async (page) => {
      await page
        .getByRole("table", { name: "コーチの一覧" })
        .or(page.getByText("直近 30 日にレッスンのないコーチはいません"))
        .waitFor();
    },
  },
  {
    id: "hq-coach-suspend",
    title: "本部: コーチの受付の停止の確認",
    role: "headquarters",
    path: "/headquarters/coaches",
    open: async (page) => {
      await page.getByRole("button", { name: "受付を止める" }).click();
      await page.getByLabel("理由（任意）").waitFor();
    },
  },
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
    id: "hq-lesson-menus-search",
    title: "本部: レッスンメニューを名前で探す",
    role: "headquarters",
    path: "/headquarters/lesson-menus?q=%E3%82%B9%E3%82%A4%E3%83%B3%E3%82%B0",
    open: async (page) => {
      await page.getByRole("row", { name: /スイングの見直し/ }).waitFor();
    },
  },
  {
    // 「直近 30 日に予約のないメニューだけ」を入れた状態（URL には残さないので、開いてから切り替える）。
    id: "hq-lesson-menus-unbooked",
    title: "本部: レッスンメニュー（直近 30 日に予約のないメニューだけ）",
    role: "headquarters",
    path: "/headquarters/lesson-menus",
    open: async (page) => {
      await page.getByRole("table", { name: "レッスンメニューの一覧" }).waitFor();
      await page.getByLabel("直近 30 日に予約のないメニューだけ").check();
      await page.getByRole("row", { name: /パターの距離感/ }).waitFor();
      await page.getByRole("row", { name: /フォアハンドの基礎/ }).waitFor({ state: "detached" });
    },
  },
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
  {
    id: "hq-lesson-menu-history",
    title: "本部: レッスンメニューの公開の記録",
    role: "headquarters",
    path: "/headquarters/lesson-menus",
    open: async (page) => {
      await page.getByRole("link", { name: "「パターの距離感」の公開の記録を見る" }).click();
      await page.getByRole("heading", { name: "パターの距離感" }).waitFor();
      await page.getByRole("table", { name: "公開の記録" }).waitFor();
    },
  },
  {
    id: "hq-lesson-menu-history-no-name",
    title: "本部: レッスンメニューの公開の記録（名前を引けないとき）",
    role: "headquarters",
    // レッスンメニューの一覧（fixtures の /lesson-menus）にない ID。記録の応答は ID を問わず同じものが返る。
    path: "/headquarters/lesson-menus/history?lesson_menu_id=aaaa0099-0000-4000-8000-000000000000",
    open: async (page) => {
      await page.getByRole("heading", { name: "（名前を表示できません）" }).waitFor();
      await page.getByRole("table", { name: "公開の記録" }).waitFor();
    },
  },
  { id: "hq-students", title: "本部: 生徒", role: "headquarters", path: "/headquarters/students" },
  {
    id: "hq-students-by-coach",
    title: "本部: 生徒（コーチで絞る）",
    role: "headquarters",
    path: "/headquarters/students?coach_id=22222222-2222-4222-8222-222222222222",
    open: async (page) => {
      await page.getByRole("combobox", { name: "コーチ" }).and(page.locator(":enabled")).waitFor();
      await page.getByRole("table", { name: "生徒の一覧" }).waitFor();
    },
  },
  {
    id: "hq-students-by-coach-empty",
    title: "本部: 生徒（コーチで絞って 0 件）",
    role: "headquarters",
    path: "/headquarters/students",
    open: async (page) => {
      // 選択肢（コーチの一覧）を読むまでは欄が使えないので、selectOption が使えるようになるまで待つ。
      await page.getByRole("combobox", { name: "コーチ" }).selectOption({ label: "高橋 コーチ" });
      await page.getByText("条件に合う生徒はいません").waitFor();
    },
  },
  {
    id: "hq-students-invalid-coach",
    title: "本部: 生徒（コーチの指定が正しくない 422）",
    role: "headquarters",
    path: "/headquarters/students?coach_id=not-a-uuid",
    open: async (page) => {
      await page.getByRole("alert").filter({ hasText: "コーチは一覧から選び直してください。" }).waitFor();
    },
  },
  {
    id: "hq-student-activity",
    title: "本部: 生徒の利用の様子",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
  },
  {
    id: "hq-student-announcements-unread",
    title: "本部: 生徒の受け取ったお知らせ（未読だけ）",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("checkbox", { name: "未読だけ" }).check();
      await page.getByText(/^未読のものだけを、送った新しい順に/).waitFor();
      await page.getByRole("table", { name: "受け取ったお知らせ" }).waitFor();
    },
  },
  { id: "hq-bookings", title: "本部: 予約", role: "headquarters", path: "/headquarters/bookings" },
  {
    id: "hq-bookings-by-coach",
    title: "本部: コーチ 1 人の予約",
    role: "headquarters",
    path: "/headquarters/coach-overview?coach_id=22222222-2222-4222-8222-222222222222",
    open: async (page) => {
      await page
        .getByRole("navigation", { name: "このコーチで開く画面" })
        .getByRole("link", { name: "予約", exact: true })
        .click();
      await page.getByText(/^このコーチの枠への予約を/).waitFor();
      await page.getByText(/・\d+ 件$/).waitFor();
    },
  },
  {
    id: "hq-bookings-by-student",
    title: "本部: 生徒 1 人の予約",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("link", { name: "この生徒の予約を見る" }).click();
      await page.getByText(/^この生徒の予約を/).waitFor();
      await page.getByText(/^生徒で絞っています: 山田 花子$/).waitFor();
      await page.getByText(/・\d+ 件$/).waitFor();
    },
  },
  {
    id: "hq-bookings-by-lesson-menu",
    title: "本部: レッスンメニュー 1 件の予約",
    role: "headquarters",
    path: "/headquarters/lesson-menus",
    open: async (page) => {
      await page.getByRole("link", { name: "「フォアハンドの基礎」の予約を見る" }).click();
      await page.getByText(/^このレッスンメニューの予約を/).waitFor();
      await page.getByText(/^レッスンメニューで絞っています: フォアハンドの基礎$/).waitFor();
      await page.getByText(/・\d+ 件$/).waitFor();
    },
  },
  {
    id: "hq-bookings-by-sport",
    title: "本部: スポーツで絞った予約",
    role: "headquarters",
    path: "/headquarters/bookings",
    open: async (page) => {
      await page.getByText(/・\d+ 件$/).waitFor();
      await page.getByRole("combobox", { name: "スポーツ" }).selectOption({ label: "テニス" });
      await page.getByText(/・テニス 3 件$/).waitFor();
    },
  },
  {
    id: "hq-cancelled-bookings",
    title: "本部: キャンセルされた予約",
    role: "headquarters",
    path: "/headquarters/cancelled-bookings",
  },
  {
    id: "hq-cancelled-bookings-coach",
    title: "本部: キャンセルされた予約（コーチで絞る）",
    role: "headquarters",
    path: "/headquarters/cancelled-bookings",
    open: async (page) => {
      // ラベルの文字だけで探すと、レッスンメニューの選択肢（「…（鈴木 コーチ）」）にも当たるので、選択欄の名前で探す。
      await page.getByRole("combobox", { name: "コーチ" }).selectOption({ label: "高橋 コーチ" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText("・1 件").waitFor();
    },
  },
  {
    id: "hq-cancelled-bookings-lesson-menu",
    title: "本部: キャンセルされた予約（レッスンメニューで絞る）",
    role: "headquarters",
    path: "/headquarters/cancelled-bookings",
    open: async (page) => {
      await page.getByRole("combobox", { name: "レッスンメニュー" }).selectOption({ label: "フォアハンドの基礎（鈴木 コーチ）" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText(/^レッスンメニューで絞っています: フォアハンドの基礎$/).waitFor();
      await page.getByText("・1 件").waitFor();
    },
  },
  {
    id: "hq-cancelled-bookings-by-sport",
    title: "本部: キャンセルされた予約（スポーツで絞る）",
    role: "headquarters",
    path: "/headquarters/cancelled-bookings",
    open: async (page) => {
      await page.getByText(/・\d+ 件$/).waitFor();
      await page.getByRole("combobox", { name: "スポーツ" }).selectOption({ label: "テニス" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText(/・テニス 2 件$/).waitFor();
    },
  },
  {
    id: "hq-booking-detail",
    title: "本部: 予約の詳細",
    role: "headquarters",
    path: "/headquarters/bookings/detail?booking_id=44440010-0000-4000-8000-000000000000",
  },
  {
    id: "hq-incomplete-coach-profiles",
    title: "本部: 自己紹介の足りないコーチ",
    role: "headquarters",
    path: "/headquarters/coaches/incomplete-profiles",
  },
  {
    id: "hq-incomplete-coach-profiles-empty",
    title: "本部: 自己紹介の足りないコーチ（絞り込んで 0 件）",
    role: "headquarters",
    path: "/headquarters/coaches/incomplete-profiles",
    open: async (page) => {
      await page.getByLabel("スポーツ").selectOption({ label: "水泳" });
      await page.getByText("条件に合うコーチはいません").waitFor();
    },
  },
  {
    id: "hq-incomplete-coach-profiles-accepting",
    title: "本部: 自己紹介の足りないコーチ（受付中で絞る）",
    role: "headquarters",
    path: "/headquarters/coaches/incomplete-profiles",
    open: async (page) => {
      await page.getByLabel("受付の状態").selectOption({ label: "受付中" });
      await page.getByRole("link", { name: "伊藤 コーチ" }).waitFor();
      await page.getByRole("link", { name: "高橋 コーチ" }).waitFor({ state: "detached" });
    },
  },
  {
    // 表示名で探した状態（URL のクエリ q に残した検索語で開く）。
    id: "hq-incomplete-coach-profiles-search",
    title: "本部: 自己紹介の足りないコーチ（表示名で探す）",
    role: "headquarters",
    path: "/headquarters/coaches/incomplete-profiles?q=%E9%AB%98%E6%A9%8B",
    open: async (page) => {
      await page.getByRole("link", { name: "高橋 コーチ" }).waitFor();
      await page.getByRole("link", { name: "伊藤 コーチ" }).waitFor({ state: "detached" });
    },
  },
  {
    id: "hq-incomplete-coach-profiles-search-empty",
    title: "本部: 自己紹介の足りないコーチ（表示名で探して 0 件）",
    role: "headquarters",
    path: "/headquarters/coaches/incomplete-profiles",
    open: async (page) => {
      await page.getByRole("link", { name: "伊藤 コーチ" }).waitFor();
      await page.getByRole("textbox", { name: "表示名" }).fill("山本");
      await page.getByRole("button", { name: "探す" }).click();
      await page.getByText("表示名に「山本」を含む、自己紹介の足りないコーチはいません").waitFor();
    },
  },
  {
    id: "hq-overdue-live-sessions",
    title: "本部: 終わっていない通話",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions",
  },
  {
    id: "hq-overdue-live-sessions-by-coach",
    title: "本部: 終わっていない通話（コーチで絞る）",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions?coach_id=22222222-2222-4222-8222-222222222222",
    open: async (page) => {
      await page.getByRole("combobox", { name: "コーチ" }).and(page.locator(":enabled")).waitFor();
      await page.getByRole("table", { name: "終わっていない通話の一覧" }).waitFor();
    },
  },
  {
    id: "hq-overdue-live-sessions-by-sport",
    title: "本部: 終わっていない通話（スポーツで絞る）",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions?sport=tennis",
    open: async (page) => {
      await page.getByRole("combobox", { name: "スポーツ" }).and(page.locator(":enabled")).waitFor();
      await page.getByRole("table", { name: "終わっていない通話の一覧" }).waitFor();
    },
  },
  {
    id: "hq-overdue-live-sessions-by-coach-empty",
    title: "本部: 終わっていない通話（コーチで絞って 0 件）",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions",
    open: async (page) => {
      // 選択肢（コーチの一覧）を読むまでは欄が使えないので、selectOption が使えるようになるまで待つ。
      await page.getByRole("combobox", { name: "コーチ" }).selectOption({ label: "高橋 コーチ" });
      await page.getByText("このコーチの終わっていない通話はありません").waitFor();
    },
  },
  {
    id: "hq-overdue-live-sessions-invalid-coach",
    title: "本部: 終わっていない通話（コーチの指定が正しくない 422）",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions?coach_id=not-a-uuid",
    open: async (page) => {
      await page.getByRole("alert").filter({ hasText: "コーチは一覧から選び直してください。" }).waitFor();
    },
  },
  {
    id: "hq-overdue-live-sessions-by-student",
    title: "本部: 生徒 1 人の終わっていない通話",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("link", { name: "この生徒の終わっていない通話を見る" }).click();
      await page.getByText(/^生徒で絞っています: 山田 花子$/).waitFor();
      await page.getByRole("table", { name: "終わっていない通話の一覧" }).waitFor();
    },
  },
  {
    id: "hq-overdue-live-sessions-invalid-student",
    title: "本部: 終わっていない通話（生徒の指定が正しくない 422）",
    role: "headquarters",
    path: "/headquarters/overdue-live-sessions?student_id=not-a-uuid",
    open: async (page) => {
      await page.getByRole("alert").filter({ hasText: "生徒は生徒の一覧から選び直してください。" }).waitFor();
    },
  },
  {
    id: "hq-live-sessions",
    title: "本部: ライブレッスンの記録",
    role: "headquarters",
    path: "/headquarters/live-sessions",
  },
  {
    id: "hq-live-sessions-by-student",
    title: "本部: 生徒 1 人のライブレッスンの記録",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("link", { name: "この生徒のライブレッスンの記録を見る" }).click();
      await page.getByText(/^生徒で絞っています: 山田 花子$/).waitFor();
      await page.getByRole("table", { name: "ライブレッスンの記録の一覧" }).waitFor();
    },
  },
  {
    id: "hq-live-sessions-by-sport",
    title: "本部: スポーツで絞ったライブレッスンの記録",
    role: "headquarters",
    path: "/headquarters/live-sessions",
    open: async (page) => {
      await page.getByLabel("スポーツ").selectOption({ label: "テニス" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText(/・2 件$/).waitFor();
    },
  },
  {
    id: "hq-live-sessions-empty",
    title: "本部: ライブレッスンの記録（0 件）",
    role: "headquarters",
    path: "/headquarters/live-sessions",
    open: async (page) => {
      await page.getByLabel("コーチ").selectOption({ label: "高橋 コーチ" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText("この条件の通話の記録はありません").waitFor();
    },
  },
  { id: "hq-announcements", title: "本部: お知らせ", role: "headquarters", path: "/headquarters/announcements" },
  {
    id: "hq-announcements-period",
    title: "本部: お知らせ（送ったお知らせを期間で絞る）",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      // 「開始日」「終了日」「この期間で見る」は下書きの欄にもあるので、送ったお知らせの欄の中で探す
      const sent = page.getByRole("region", { name: "送ったお知らせ", exact: true });
      await sent.getByLabel("開始日").fill("2026-09-01");
      await sent.getByLabel("終了日").fill("2026-09-30");
      await sent.getByRole("button", { name: "この期間で見る" }).click();
      await page.getByText("この期間に送ったお知らせはありません").waitFor();
    },
  },
  {
    id: "hq-announcement-confirm",
    title: "本部: お知らせの送る前の確認",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      await page.getByLabel("対象").selectOption({ label: "テニスの生徒" });
      await page.getByLabel("題", { exact: true }).fill("コートの予約方法が変わります");
      await page.getByLabel("本文").fill("11月から、コートの予約はアプリの「予約」から行えます。\n詳しくはお知らせの一覧をご覧ください。");
      await page.getByRole("button", { name: "内容を確かめる" }).click();
    },
  },
  {
    id: "hq-announcement-withdraw",
    title: "本部: お知らせの取り下げの確認",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      await page.getByRole("button", { name: "「水泳の体験予約キャンペーン」を取り下げる" }).click();
    },
  },
  {
    id: "hq-announcement-drafts",
    title: "本部: お知らせの下書き",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      await page.getByLabel("題", { exact: true }).fill("年末年始の営業について");
      await page.getByLabel("本文").fill("12月29日から1月3日は、チャットの返信が遅れます。");
      await page.getByRole("button", { name: "下書きに保存" }).click();
      await page.getByRole("list", { name: "下書きの一覧" }).waitFor();
    },
  },
  {
    id: "hq-announcement-drafts-search",
    title: "本部: お知らせの下書きを題で探す（見つからないとき）",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      // 「題で探す」と「探す」は送ったお知らせの欄にもあるので、下書きの欄の中で探す
      const drafts = page.getByRole("region", { name: "下書き", exact: true });
      await drafts.getByLabel("題で探す").fill("夏休み");
      await drafts.getByRole("button", { name: "探す", exact: true }).click();
      await page.getByText("題に「夏休み」を含む下書きはありません").waitFor();
    },
  },
  {
    id: "hq-announcement-drafts-filter",
    title: "本部: お知らせの下書きを期間とスポーツで絞る（見つからないとき）",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      const drafts = page.getByRole("region", { name: "下書き", exact: true });
      await drafts.getByLabel("開始日").fill("2026-09-01");
      await drafts.getByLabel("終了日").fill("2026-09-30");
      await drafts.getByRole("button", { name: "この期間で見る" }).click();
      await drafts.getByLabel("スポーツ").selectOption({ label: "ゴルフ" });
      await page
        .getByText("2026年9月1日（火） 〜 2026年9月30日（水）に最後に直した、ゴルフの生徒に向けた下書きはありません")
        .waitFor();
    },
  },
  {
    id: "hq-announcements-search",
    title: "本部: 送ったお知らせを題で探す（見つからないとき）",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      const sent = page.getByRole("region", { name: "送ったお知らせ", exact: true });
      await sent.getByLabel("題で探す").fill("夏休み");
      await sent.getByRole("button", { name: "探す", exact: true }).click();
      await page.getByText("この期間に送った、題に「夏休み」を含むお知らせはありません").waitFor();
    },
  },
  {
    id: "hq-announcements-sport",
    title: "本部: 送ったお知らせをスポーツで絞る（見つからないとき）",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      const sent = page.getByRole("region", { name: "送ったお知らせ", exact: true });
      await sent.getByLabel("スポーツ").selectOption({ label: "ゴルフ" });
      await page.getByText("この期間にゴルフの生徒に向けて送ったお知らせはありません").waitFor();
    },
  },
  {
    id: "hq-announcements-withdrawn",
    title: "本部: 送ったお知らせを取り下げたかで絞る",
    role: "headquarters",
    path: "/headquarters/announcements",
    open: async (page) => {
      // 各行の「「…」を取り下げる」も「取り下げ」を含むので、欄の名前は exact で探す
      const sent = page.getByRole("region", { name: "送ったお知らせ", exact: true });
      await sent.getByLabel("取り下げ", { exact: true }).selectOption({ label: "取り下げたもの" });
      await page.getByText(/・取り下げたもの・1 件/).waitFor();
    },
  },
  {
    id: "hq-pending-video-reviews",
    title: "本部: 返却待ちの動画添削",
    role: "headquarters",
    path: "/headquarters/pending-video-reviews",
  },
  {
    id: "hq-pending-video-reviews-coach",
    title: "本部: 返却待ちの動画添削（コーチで絞ったとき）",
    role: "headquarters",
    path: "/headquarters/pending-video-reviews?coach_id=22222222-2222-4222-8222-222222222223",
  },
  {
    id: "hq-pending-video-reviews-by-student",
    title: "本部: 生徒 1 人の返却待ちの動画添削",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("link", { name: "返却待ちの動画添削を見る" }).click();
      await page.getByText(/^生徒で絞っています: 山田 花子$/).waitFor();
      await page.getByRole("table", { name: "返却待ちの動画添削の一覧" }).waitFor();
    },
  },
  {
    id: "hq-unanswered-messages",
    title: "本部: 返事のないメッセージ",
    role: "headquarters",
    path: "/headquarters",
    open: async (page) => {
      await page.getByRole("link", { name: "返事の遅れているやり取りを見る" }).click();
      await page.getByRole("table", { name: "返事のないメッセージの一覧" }).waitFor();
    },
  },
  {
    id: "hq-unanswered-messages-coach-empty",
    title: "本部: 返事のないメッセージ（コーチで絞って 0 件のとき）",
    role: "headquarters",
    path: "/headquarters/unanswered-messages?coach_id=22222222-2222-4222-8222-222222222223",
    open: async (page) => {
      await page.getByText("このコーチのやり取りに、返事を待って 24 時間以上たったメッセージはありません").waitFor();
    },
  },
  {
    // コーチで絞った応答は fixtures にないので、条件なしの応答（どれも鈴木 コーチのやり取り）が返る。
    id: "hq-unanswered-messages-by-coach",
    title: "本部: コーチ 1 人の返事のないメッセージ",
    role: "headquarters",
    path: "/headquarters/coach-overview?coach_id=22222222-2222-4222-8222-222222222222",
    open: async (page) => {
      await page.getByRole("link", { name: "このコーチの返事のないメッセージを見る" }).click();
      await page.getByRole("combobox", { name: "コーチ" }).and(page.locator(":enabled")).waitFor();
      await page.getByRole("table", { name: "返事のないメッセージの一覧" }).waitFor();
    },
  },
  {
    id: "hq-unanswered-messages-by-student",
    title: "本部: 生徒 1 人の返事のないメッセージ",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("link", { name: "返事のないメッセージを見る" }).click();
      await page.getByText(/^生徒で絞っています: 山田 花子$/).waitFor();
      await page.getByRole("table", { name: "返事のないメッセージの一覧" }).waitFor();
    },
  },
  {
    id: "hq-returned-video-reviews",
    title: "本部: 返した動画添削",
    role: "headquarters",
    path: "/headquarters/returned-video-reviews",
  },
  {
    id: "hq-returned-video-reviews-by-student",
    title: "本部: 生徒 1 人の返した動画添削",
    role: "headquarters",
    path: "/headquarters/students/activity?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      await page.getByRole("link", { name: "この生徒の返した動画添削の記録を見る" }).click();
      await page.getByText(/^生徒で絞っています: 山田 花子$/).waitFor();
      await page.getByRole("table", { name: "返した動画添削の一覧" }).waitFor();
    },
  },
  {
    id: "hq-returned-video-reviews-by-student-empty",
    title: "本部: 生徒 1 人の返した動画添削（コーチでも絞って 0 件）",
    role: "headquarters",
    path: "/headquarters/returned-video-reviews?student_id=11111111-1111-4111-8111-111111111111",
    open: async (page) => {
      // 選択肢（コーチの一覧）を読むまでは欄が使えないので、selectOption が使えるようになるまで待つ。
      await page.getByRole("combobox", { name: "コーチ" }).selectOption({ label: "高橋 コーチ" });
      await page.getByRole("button", { name: "この条件で見る" }).click();
      await page.getByText("この期間に条件に合う返した動画添削はありません").waitFor();
    },
  },
  {
    id: "hq-returned-video-reviews-invalid-student",
    title: "本部: 返した動画添削（生徒の指定が正しくない 422）",
    role: "headquarters",
    path: "/headquarters/returned-video-reviews?student_id=not-a-uuid",
    open: async (page) => {
      await page.getByRole("alert").filter({ hasText: "生徒は生徒の一覧から選び直してください。" }).waitFor();
    },
  },
];
