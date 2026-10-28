use std::collections::BTreeMap;

use domain::{
    identity::AccountId,
    profile::DisplayName,
    scheduling::{Booking, Slot},
};

/// コーチの自分の予約の一覧（[`super::ListCoachBookings`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachBookingList {
    /// コーチのスロットへの予約（状態を問わない）と、予約されたスロット。スロットの開始時刻の古い順で、同じ開始時刻は
    /// 予約の作成の新しい順、それも同じなら予約 ID の順
    pub items: Vec<(Booking, Slot)>,
    /// `items` の生徒の表示名（生徒の ID ごと）。表示名が引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
