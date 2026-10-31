use std::collections::BTreeMap;

use domain::{identity::AccountId, profile::DisplayName, scheduling::Booking};
use time::Date;

/// 本部が見る予約の一覧（[`super::ListBookings`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookingList {
    /// 期間の始まりの日付（日本時間。含む）。省いたときは今日
    pub from: Date,
    /// 期間の終わりの日付（日本時間。含む）。省いたときは `from` から 7 日目
    pub to: Date,
    /// 期間に開始する予約（状態を問わない）。開始時刻の古い順で、同じ開始時刻は予約の作成の新しい順、それも同じなら
    /// 予約 ID の順
    pub bookings: Vec<Booking>,
    /// `bookings` のコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
    /// `bookings` の生徒の表示名（生徒の ID ごと）。表示名が引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
