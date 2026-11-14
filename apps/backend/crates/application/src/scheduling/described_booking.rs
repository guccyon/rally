use domain::{
    profile::DisplayName,
    scheduling::{Booking, Slot},
};

/// 生徒が作った・確定した・キャンセルした予約に、予約したスロットとコーチの表示名を添えたもの（[`super::DescribeBooking`]
/// の結果）。student-app の `createBooking`・`confirmBooking`・`cancelBooking` が返す予約の終わりの時刻・スポーツ・
/// コーチの表示名をここから読む。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedBooking {
    pub booking: Booking,
    /// 予約したスロット（枠の終わり・スポーツ）
    pub slot: Slot,
    /// 予約のコーチの表示名（今の表示名）。コーチのアカウントが引けなければ `None`
    pub coach_display_name: Option<DisplayName>,
}
