/// 予約の状態。外部での文字列表現は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookingStatus {
    /// 作成した直後。スロットを確保しているが、まだ確定していない
    Pending,
    /// 確定した。スロットは予約で埋まる（[`super::SlotStatus::Booked`]）
    Confirmed,
    /// キャンセルした（[`super::Booking::cancel`]）。スロットを確保しない
    Cancelled,
}

impl BookingStatus {
    pub const ALL: [BookingStatus; 3] = [
        BookingStatus::Pending,
        BookingStatus::Confirmed,
        BookingStatus::Cancelled,
    ];

    /// スロットを確保している（有効な）予約か。1 つのスロットに有効な予約は 1 件まで。
    pub const fn is_active(self) -> bool {
        !matches!(self, BookingStatus::Cancelled)
    }
}
