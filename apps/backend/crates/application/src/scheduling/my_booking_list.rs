use std::collections::BTreeMap;

use domain::{
    identity::AccountId,
    scheduling::{Booking, Slot},
};

use super::CoachIntroduction;

/// 本人の予約の一覧（[`super::ListMyBookings`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyBookingList {
    /// 予約（状態を問わない）と、予約したスロット。スロットの開始時刻の新しい順で、同じ開始時刻は予約の作成の
    /// 新しい順、それも同じなら予約 ID の順
    pub items: Vec<(Booking, Slot)>,
    /// `items` のコーチの紹介（コーチの ID ごと）。紹介が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, CoachIntroduction>,
}
