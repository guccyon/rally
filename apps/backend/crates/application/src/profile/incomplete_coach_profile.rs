use std::time::SystemTime;

use domain::{identity::Account, profile::MissingCoachProfileFields};

/// 本部が見る、プロフィールに足りない項目のあるコーチ（[`super::ListIncompleteCoachProfiles`]）の 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteCoachProfile {
    /// コーチのアカウント
    pub account: Account,
    /// 足りない（空の）項目。空にはならない
    pub missing_fields: MissingCoachProfileFields,
    /// プロフィールを最後に更新した時刻。保存先がプロフィールの更新の時刻を持たない（記録していない）間は `None`
    pub updated_at: Option<SystemTime>,
}
