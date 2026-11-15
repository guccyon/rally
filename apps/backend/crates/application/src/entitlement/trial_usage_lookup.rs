use std::future::Future;

use domain::{entitlement::TrialUsage, identity::AccountId};

use crate::Unavailable;

/// 生徒の体験の使用状況（domain の [`TrialUsage`]）の材料をそろえる照会のポート。体験予約の作成
/// （[`crate::scheduling::CreateBooking`]）・自分の予約の一覧の `trial_available`
/// （[`crate::scheduling::ListMyBookings`]）・動画の送付の門（[`super::EntitlementVideoReviewGate`]）が同じ照会を使い、
/// 判定は domain の規則に任せる。実装は [`super::EntitlementTrialUsageLookup`]。
///
/// 予約の有無は載せない（[`TrialUsage::with_active_booking`] は false のまま）。呼ぶ側が持っている予約から
/// domain の [`domain::scheduling::Booking::is_trial_available`] で足す（体験予約の作成は、予約を保存する
/// トランザクションの中で読んだ予約で決める）。照会だけで何も変えない。
pub trait TrialUsageLookup: Send + Sync {
    /// 生徒 `student_id` の体験の使用状況（月額の受講権・完了したレッスン・送った動画の有無）。
    fn find_trial_usage(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<TrialUsage, Unavailable>> + Send;
}
