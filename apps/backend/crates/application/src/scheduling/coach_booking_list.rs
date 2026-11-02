use std::collections::BTreeMap;

use domain::{
    identity::AccountId,
    profile::DisplayName,
    scheduling::{Booking, Slot},
};
use time::Date;

/// コーチの自分の予約の一覧（[`super::ListCoachBookings`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachBookingList {
    /// 期間の始まりの日付（日本時間。含む）。日付をどちらも省いて期間で絞らなかったときは `None`
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）。日付をどちらも省いて期間で絞らなかったときは `None`
    pub to: Option<Date>,
    /// コーチのスロットへの予約（状態を問わない。期間があればスロットの開始がその期間に入るものだけ）と、予約された
    /// スロット。スロットの開始時刻の古い順で、同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順
    pub items: Vec<(Booking, Slot)>,
    /// `items` の生徒の表示名（生徒の ID ごと）。表示名が引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
