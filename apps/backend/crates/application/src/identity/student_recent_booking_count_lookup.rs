use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    time::SystemTime,
};

use domain::identity::AccountId;

use crate::Unavailable;

/// identity が scheduling に生徒ごとの直近の予約の数を問い合わせるポート（本部が見る生徒の一覧に添える）。
///
/// 予約の持ち主は scheduling で、identity は読むだけ。数え方（期間とキャンセルの扱い）は domain の
/// [`domain::scheduling::RecentStudentBookings`]。実装は
/// [`crate::scheduling::SchedulingStudentRecentBookingCountLookup`]。
pub trait StudentRecentBookingCountLookup: Send + Sync {
    /// `student_ids` の生徒それぞれの、`now` に照会したときの直近の予約の数。数える予約が 1 件もない生徒は結果に
    /// 含めない。
    fn count_recent_bookings(
        &self,
        student_ids: &BTreeSet<AccountId>,
        now: SystemTime,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, u32>, Unavailable>> + Send;
}
