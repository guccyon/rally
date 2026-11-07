use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::scheduling::BookingId;

use super::CoachLeaveNotice;
use crate::Unavailable;

/// notification が scheduling に、本部がキャンセルした予約のお知らせに添える中身（[`CoachLeaveNotice`]）を問い合わせる
/// ポート（自分のお知らせの一覧で本文を決める）。実装は [`crate::scheduling::SchedulingCoachLeaveNoticeLookup`]。
pub trait CoachLeaveNoticeLookup: Send + Sync {
    /// `booking_ids` の予約のうち、コーチのお休みで本部がキャンセルしたものの中身。そうでない予約は結果に含めない。
    fn find_coach_leave_notices(
        &self,
        booking_ids: &BTreeSet<BookingId>,
    ) -> impl Future<Output = Result<BTreeMap<BookingId, CoachLeaveNotice>, Unavailable>> + Send;
}
