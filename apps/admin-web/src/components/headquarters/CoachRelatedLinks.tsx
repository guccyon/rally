import Link from "next/link";

import { bookingListPath } from "./BookingListHome";
import { coachReviewSummaryPath } from "./CoachReviewSummaryHome";
import { lessonMenuCatalogPath } from "./LessonMenuCatalogHome";
import { LOW_RATED_REVIEWS_PATH } from "./LowRatedReviewsHome";

/**
 * コーチの様子の画面の上に置く、そのコーチで開く本部の画面へのリンクの並び。
 * 評価・レッスンメニュー・予約はそのコーチを選んだ（絞った）状態で開く。低い評価の一覧はコーチで絞れない（契約に
 * `coach_id` がない）ので、一覧をそのまま開く。リンクはコーチの ID だけで決まるので、様子の読み込みを待たない。
 */
export function CoachRelatedLinks({ coachId }: { coachId: string }) {
  const links = [
    { label: "評価", href: coachReviewSummaryPath(coachId) },
    { label: "低い評価", href: LOW_RATED_REVIEWS_PATH },
    { label: "レッスンメニュー", href: lessonMenuCatalogPath(coachId) },
    { label: "予約", href: bookingListPath(coachId) },
  ];
  return (
    <nav aria-label="このコーチで開く画面" className="mb-4 text-sm">
      <ul className="flex flex-wrap gap-2">
        {links.map(({ label, href }) => (
          <li key={label}>
            <Link href={href} className="inline-block rounded border px-3 py-1 text-blue-600 hover:bg-blue-50">
              {label}
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}
