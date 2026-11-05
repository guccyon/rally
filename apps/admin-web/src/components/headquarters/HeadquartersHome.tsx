import Link from "next/link";

import { NotImplemented, Section } from "@/components/ui/Section";

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
      <Section title="自己紹介の足りないコーチ">
        <Link href="/headquarters/coaches/incomplete-profiles" className="text-primary underline">
          自己紹介が書きかけのコーチを見る
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
      <Section title="キャンセルされた予約">
        <Link href="/headquarters/cancelled-bookings" className="text-primary underline">
          キャンセルされた予約を見る
        </Link>
      </Section>
      <Section title="終わっていないビデオ通話">
        <Link href="/headquarters/overdue-live-sessions" className="text-primary underline">
          時間の過ぎたビデオ通話を見る
        </Link>
      </Section>
      <Section title="ビデオ通話の記録">
        <Link href="/headquarters/live-sessions" className="text-primary underline">
          ビデオ通話の開始と終了の記録を見る
        </Link>
      </Section>
      <Section title="お知らせ">
        <Link href="/headquarters/announcements" className="text-primary underline">
          生徒にお知らせを送る
        </Link>
      </Section>
      <Section title="返却待ちの動画添削">
        <Link href="/headquarters/pending-video-reviews" className="text-primary underline">
          返却の遅れている添削を見る
        </Link>
      </Section>
      <Section title="返した動画添削">
        <Link href="/headquarters/returned-video-reviews" className="text-primary underline">
          返した添削とかかった時間を見る
        </Link>
      </Section>
      <Section title="返事のないメッセージ">
        <Link href="/headquarters/unanswered-messages" className="text-primary underline">
          返事の遅れているやり取りを見る
        </Link>
      </Section>
      <Section title="完了記録の一覧">
        <NotImplemented>完了したレッスンの一覧を取る API が admin-web の契約にまだありません。</NotImplemented>
      </Section>
    </>
  );
}
