use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::{catalog::LessonMenuId, scheduling::BookingId};

use crate::Unavailable;

/// live-lesson が scheduling に、通話セッションに結び付いた予約の予約した時点のレッスンメニューを問い合わせるポート
/// （本部が見る通話セッションの記録の一覧をレッスンメニューで絞る。`listLiveLessonSessions` の `lesson_menu_id`）。
///
/// 予約とその写しの持ち主は scheduling で、live-lesson は読むだけ。合うかの判定は domain の
/// [`domain::live_lesson::SessionHistoryCriteria::matches_lesson_menu`]。実装は
/// [`crate::scheduling::SchedulingSessionBookingLessonMenuLookup`]。
pub trait SessionBookingLessonMenuLookup: Send + Sync {
    /// `booking_ids` の予約それぞれの予約した時点のレッスンメニュー。メニューのない予約・ない ID は結果に含めない。
    fn find_booking_lesson_menus(
        &self,
        booking_ids: &BTreeSet<BookingId>,
    ) -> impl Future<Output = Result<BTreeMap<BookingId, LessonMenuId>, Unavailable>> + Send;
}
