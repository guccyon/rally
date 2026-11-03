use time::Date;

/// コーチの受付を止めた・再開した記録の照会（[`super::ListCoachAcceptanceChanges`]）の条件。どれも省ける（すべて省けば
/// 今までどおりすべて）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListCoachAcceptanceChangesQuery {
    /// 変えた期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 変えた期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
}
