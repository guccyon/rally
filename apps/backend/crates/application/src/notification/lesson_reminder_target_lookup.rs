use std::{future::Future, time::SystemTime};

use super::LessonReminderTarget;
use crate::Unavailable;

/// notification が scheduling に、レッスンのリマインドの対象（[`LessonReminderTarget`]）を問い合わせるポート。実装は
/// [`crate::scheduling::SchedulingLessonReminderTargetLookup`]。
pub trait LessonReminderTargetLookup: Send + Sync {
    /// 開始が `from`（含む）から `until`（含まない）までの枠への予約のうち、問い合わせた時点で確定しているもの（確定待ちと、
    /// キャンセル・コーチのお休みで消えた予約は含めない）。並びは決めない。読むだけで何も変えない。
    fn confirmed_bookings_starting_between(
        &self,
        from: SystemTime,
        until: SystemTime,
    ) -> impl Future<Output = Result<Vec<LessonReminderTarget>, Unavailable>> + Send;
}
