use std::{future::Future, time::SystemTime};

use domain::identity::AccountId;

use crate::Unavailable;

/// scheduling が entitlement に「ビデオ通話をもう 1 回予約してよいか」を問い合わせるポート。
///
/// 判定の規則（月額は暦月 2 回まで。不変条件 1）は entitlement が持ち、scheduling は答えに従うだけで回数も
/// 上限も知らない。実装は [`crate::entitlement::EntitlementQuotaGate`]。予約の確定（[`super::ConfirmBooking`]）
/// が確定の前に呼ぶ。枠の消費そのものは entitlement が境界間イベント `BookingConfirmed` を購読して行う。
pub trait QuotaGate: Send + Sync {
    /// `student_id` の生徒が `at` の時点でビデオ通話の予約をもう 1 回確定してよいか。
    fn may_book_live_lesson(
        &self,
        student_id: AccountId,
        at: SystemTime,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
