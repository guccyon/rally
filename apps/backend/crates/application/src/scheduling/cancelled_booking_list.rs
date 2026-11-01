use std::collections::BTreeMap;

use domain::{identity::AccountId, profile::DisplayName, scheduling::Booking};
use time::Date;

/// 本部が見るキャンセルされた予約の一覧（[`super::ListCancelledBookings`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelledBookingList {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Date,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Date,
    /// 期間にキャンセルした予約（キャンセル済みだけ）。キャンセルした時刻の新しい順で、同じ時刻は予約 ID の順
    pub bookings: Vec<Booking>,
    /// `bookings` のコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
    /// `bookings` の生徒の表示名（生徒の ID ごと）。表示名が引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
