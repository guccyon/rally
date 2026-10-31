use std::{future::Future, time::SystemTime};

use domain::identity::AccountId;

use super::StudentBookingActivity;
use crate::Unavailable;

/// identity が scheduling に生徒 1 人の予約の様子（これからの予約とキャンセルの件数）を問い合わせるポート（本部が見る
/// 生徒 1 人の利用の様子に添える）。
///
/// 予約の持ち主は scheduling で、identity は読むだけ。実装は
/// [`crate::scheduling::SchedulingStudentBookingActivityLookup`]。
pub trait StudentBookingActivityLookup: Send + Sync {
    /// 生徒 `student_id` の、`now` から見たこれからの予約とキャンセルした予約の件数（[`StudentBookingActivity`]）。
    /// 予約がなければ空。
    fn find_booking_activity(
        &self,
        student_id: AccountId,
        now: SystemTime,
    ) -> impl Future<Output = Result<StudentBookingActivity, Unavailable>> + Send;
}
