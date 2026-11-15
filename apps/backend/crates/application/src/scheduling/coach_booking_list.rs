use std::collections::BTreeMap;

use domain::{
    identity::AccountId,
    live_lesson::LiveLessonSession,
    profile::DisplayName,
    scheduling::{Booking, BookingId, Slot},
};
use time::Date;

/// コーチの自分の予約の一覧（[`super::ListCoachBookings`] の結果）。
#[derive(Debug, Clone, PartialEq)]
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
    /// `items` の予約が結び付いた通話中のセッション（予約 ID ごと。グループレッスンでは結び付いた予約のどれにも同じ
    /// セッションが入る）。終了したセッション（本部が終わらせたものを含む）と、セッションのない予約は含まない
    pub sessions: BTreeMap<BookingId, LiveLessonSession>,
}
