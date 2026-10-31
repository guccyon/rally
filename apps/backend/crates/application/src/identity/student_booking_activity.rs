use domain::scheduling::Booking;

/// 生徒 1 人の予約の様子（[`super::StudentBookingActivityLookup`] の結果。本部が見る生徒 1 人の利用の様子に添える）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StudentBookingActivity {
    /// これからの予約（有効な予約（未確定か確定）のうち、予約したスロットの開始が問い合わせた時刻より後のもの）。
    /// 開始時刻の古い順で、同じ開始時刻は予約の作成の古い順、それも同じなら予約 ID の順
    pub upcoming: Vec<Booking>,
    /// キャンセルした予約の件数（期間で絞らない。これまでのすべて）
    pub cancelled_count: u32,
}
