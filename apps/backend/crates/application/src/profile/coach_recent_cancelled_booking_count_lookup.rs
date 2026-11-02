use std::future::Future;

use domain::{identity::AccountId, scheduling::CoachRecentCancelledBookingCount};

use crate::Unavailable;

/// profile が scheduling にコーチの直近にキャンセルされた予約の数を問い合わせるポート（本部が見るコーチ 1 人の様子に添える）。
///
/// 予約の持ち主は scheduling で、数え方と期間は domain の [`CoachRecentCancelledBookingCount`]、profile は読むだけ。実装は
/// [`crate::scheduling::SchedulingCoachRecentCancelledBookingCountLookup`]。
pub trait CoachRecentCancelledBookingCountLookup: Send + Sync {
    /// `coach_id` のコーチの、照会した時刻の直近の期間にキャンセルされた予約の数。予約がなければ 0 件。
    fn find_recent_cancelled_booking_count(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<CoachRecentCancelledBookingCount, Unavailable>> + Send;
}
