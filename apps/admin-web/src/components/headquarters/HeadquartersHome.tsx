import Link from "next/link";

import { NotImplemented, Section } from "@/components/Section";

/** 本部の画面（読むだけ）。予約・完了記録の一覧は API が admin-web の契約にまだないので「未実装」と出す。 */
export function HeadquartersHome() {
  return (
    <>
      <Section title="スポーツ別の予約枠の需給">
        <Link href="/headquarters/slot-supply" className="text-blue-600 underline">
          需給を見る
        </Link>
      </Section>
      <Section title="コーチの評価">
        <Link href="/headquarters/coach-review-summary" className="text-blue-600 underline">
          評価の件数と平均を見る
        </Link>
      </Section>
      <Section title="レッスンメニュー">
        <Link href="/headquarters/lesson-menus" className="text-blue-600 underline">
          コーチのメニューを見る
        </Link>
      </Section>
      <Section title="予約の一覧">
        <NotImplemented>予約の一覧を取る API が admin-web の契約にまだありません。</NotImplemented>
      </Section>
      <Section title="完了記録の一覧">
        <NotImplemented>完了したレッスンの一覧を取る API が admin-web の契約にまだありません。</NotImplemented>
      </Section>
    </>
  );
}
