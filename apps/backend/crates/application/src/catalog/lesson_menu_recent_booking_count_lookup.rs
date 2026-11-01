use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    time::SystemTime,
};

use domain::catalog::LessonMenuId;

use crate::Unavailable;

/// catalog が scheduling にレッスンメニューごとの直近の予約の数を問い合わせるポート（本部が見るメニューの横断の一覧に
/// 添える）。
///
/// 予約の持ち主は scheduling で、catalog は読むだけ。数え方（期間とキャンセルの扱い）は domain の
/// [`domain::scheduling::RecentLessonMenuBookings`]。実装は [`crate::scheduling::SchedulingLessonMenuRecentBookingCountLookup`]。
pub trait LessonMenuRecentBookingCountLookup: Send + Sync {
    /// `lesson_menu_ids` のメニューそれぞれの、`now` に照会したときの直近の予約の数。数える予約が 1 件もないメニューは
    /// 結果に含めない。
    fn count_recent_bookings(
        &self,
        lesson_menu_ids: &BTreeSet<LessonMenuId>,
        now: SystemTime,
    ) -> impl Future<Output = Result<BTreeMap<LessonMenuId, u32>, Unavailable>> + Send;
}
