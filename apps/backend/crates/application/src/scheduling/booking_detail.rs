use domain::{
    profile::DisplayName,
    scheduling::{Booking, Slot},
};

/// 本部が見る予約 1 件の詳細（[`super::GetBooking`] の結果）。状態の移り変わりは予約から読む
/// （[`Booking::status_changes`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookingDetail {
    /// 予約（予約した時点のレッスンメニューの写しを含む）
    pub booking: Booking,
    /// 予約したスロット（枠の開始と終わり）
    pub slot: Slot,
    /// 予約のコーチの表示名。コーチのアカウントが引けなければ `None`
    pub coach_display_name: Option<DisplayName>,
    /// 予約した生徒の表示名。生徒のアカウントが引けなければ `None`
    pub student_display_name: Option<DisplayName>,
}
