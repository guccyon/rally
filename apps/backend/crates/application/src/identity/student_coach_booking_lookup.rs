use std::{collections::BTreeSet, future::Future};

use domain::identity::AccountId;

use crate::Unavailable;

/// identity が scheduling に、あるコーチを予約したことがある生徒を問い合わせるポート（本部が見る生徒の一覧をコーチで絞る）。
///
/// 予約の持ち主は scheduling で、identity は読むだけ。実装は
/// [`crate::scheduling::SchedulingStudentCoachBookingLookup`]。
pub trait StudentCoachBookingLookup: Send + Sync {
    /// `coach_id` のコーチの枠を一度でも予約した生徒の ID（予約の状態は問わない。数え方は domain の
    /// [`domain::scheduling::CoachBookedStudents`]）。ないコーチ・コーチでない ID なら空。
    fn find_students_who_booked_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<BTreeSet<AccountId>, Unavailable>> + Send;
}
