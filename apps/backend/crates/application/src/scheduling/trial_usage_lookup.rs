use std::future::Future;

use domain::identity::AccountId;

use crate::Unavailable;

/// scheduling が「この生徒は予約の外で体験を使い終えたか」を問い合わせるポート（体験は 1 回だけ。ADR 0014）。
///
/// 体験を使ったかのうち、予約（キャンセルされていない予約があるか）は scheduling が自分で見る。予約の外の事実
/// （月額契約があるか・完了したレッスンがあるか・送った動画があるか）の判定は実装が持ち、scheduling は答えに従うだけ。
/// 実装は [`crate::entitlement::EntitlementTrialUsageLookup`]。予約の作成（[`super::CreateBooking`]）が体験予約の
/// ときに、本人の予約の一覧（[`super::ListMyBookings`]）が `trial_available` を出すときに呼ぶ。
pub trait TrialUsageLookup: Send + Sync {
    /// `student_id` の生徒が、予約の外で体験を使い終えているか（体験予約できない理由があるか）。
    fn has_used_trial_outside_bookings(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
