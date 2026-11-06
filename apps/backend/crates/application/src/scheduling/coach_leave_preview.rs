use std::collections::BTreeMap;

use domain::{
    catalog::{LessonMenu, LessonMenuId},
    identity::{Account, AccountId},
    profile::DisplayName,
    scheduling::{CoachAcceptanceStatus, CoachLeaveTargets},
};
use time::Date;

/// 本部が見る、コーチのお休みの期間の予約と枠（[`super::PreviewCoachLeave`] が返す。admin-web の `getCoachLeavePreview`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLeavePreview {
    /// お休みにするコーチのアカウント（表示名・提供スポーツ）
    pub coach: Account,
    /// コーチの今の受付（停止中のコーチもお休みにできる）
    pub acceptance_status: CoachAcceptanceStatus,
    /// お休みの始まりの日（日本時間）
    pub from: Date,
    /// お休みの終わりの日（日本時間。含む）
    pub to: Date,
    /// 期間の予約と、予約のない公開中の枠と、件数
    pub targets: CoachLeaveTargets,
    /// 予約した生徒の表示名（引けない生徒は含めない）
    pub students: BTreeMap<AccountId, DisplayName>,
    /// 予約のない枠に結びついたレッスンメニュー（引けないメニューは含めない。予約は予約した時点の写しを持つ）
    pub lesson_menus: BTreeMap<LessonMenuId, LessonMenu>,
}
