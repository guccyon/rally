import Link from "next/link";

import { NotImplemented, Section } from "@/components/Section";

/** 本部の画面（読むだけ）。完了記録の一覧は API が admin-web の契約にまだないので「未実装」と出す。 */
export function HeadquartersHome() {
  return (
    <>
      <Section title="スポーツ別の予約枠の需給">
        <Link href="/headquarters/slot-supply" className="text-primary underline">
          需給を見る
        </Link>
      </Section>
      <Section title="コーチの評価">
        <Link href="/headquarters/coach-review-summary" className="text-primary underline">
          評価の件数と平均を見る
        </Link>
      </Section>
      <Section title="低い評価">
        <Link href="/headquarters/low-reviews" className="text-primary underline">
          低い評価の一覧を見る
        </Link>
      </Section>
      <Section title="コーチの受付">
        <Link href="/headquarters/coaches" className="text-primary underline">
          受付を止める・再開する
        </Link>
      </Section>
      <Section title="レッスンメニュー">
        <Link href="/headquarters/lesson-menus" className="text-primary underline">
          コーチのメニューを見る
        </Link>
      </Section>
      <Section title="生徒">
        <Link href="/headquarters/students" className="text-primary underline">
          生徒の一覧を見る
        </Link>
      </Section>
      <Section title="予約">
        <Link href="/headquarters/bookings" className="text-primary underline">
          予約の一覧を見る
        </Link>
      </Section>
      <Section title="終わっていない通話">
        <Link href="/headquarters/overdue-live-sessions" className="text-primary underline">
          時間の過ぎた通話を見る
        </Link>
      </Section>
      <Section title="お知らせ">
        <Link href="/headquarters/announcements" className="text-primary underline">
          生徒にお知らせを送る
        </Link>
      </Section>
      <Section title="完了記録の一覧">
        <NotImplemented>完了したレッスンの一覧を取る API が admin-web の契約にまだありません。</NotImplemented>
      </Section>
    </>
  );
}
