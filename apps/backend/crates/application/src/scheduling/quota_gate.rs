use std::{future::Future, time::SystemTime};

use domain::identity::AccountId;

use crate::Unavailable;

/// scheduling が entitlement に「ビデオ通話をもう 1 回予約してよいか」を問い合わせるポート。
///
/// 判定の規則（月額は暦月に契約プランの `live_lessons_per_month` 回まで。不変条件 1）は entitlement が持ち、scheduling は答えに従うだけで回数も
/// 上限も知らない。実装は [`crate::entitlement::EntitlementQuotaGate`]。予約の確定（[`super::ConfirmBooking`]）
/// が確定の前に呼ぶ。枠の消費そのものは entitlement が境界間イベント `BookingConfirmed` を購読して行う。
pub trait QuotaGate: Send + Sync {
    /// `student_id` の生徒が `at` の時点でビデオ通話の予約をもう 1 回確定してよいか。
    fn may_book_live_lesson(
        &self,
        student_id: AccountId,
        at: SystemTime,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;

    /// `student_id` の生徒がビデオ通話の暦月の枠を持つか（月額の受講権があるか）。持たない生徒（受講権がない・
    /// ワンショット）の予約は、キャンセルしても回数を数えない（docs/adr/0005 の 6）。本人の予約の一覧
    /// （[`super::ListMyBookings`]）がキャンセルの見込みを出すときと、予約のキャンセル（[`super::CancelBooking`]）が
    /// キャンセル規定の判定に渡すときに呼ぶ（見込みと実際の判定を同じにする。#1605）。
    fn has_live_lesson_quota(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
