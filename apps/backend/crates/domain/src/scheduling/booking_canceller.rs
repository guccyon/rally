/// 予約（[`super::Booking`]）をキャンセルした人の役割。外部での文字列表現は境界のアダプタが決める。
///
/// 今キャンセルできるのは予約した生徒（[`super::Booking::cancel`]）と、コーチのお休みで本部
/// （[`super::Booking::cancel_by_headquarters`]）。コーチは契約の値として先に置く。予約の保存先はキャンセルした人を持つ
/// （[`super::Booking::with_restored_canceller`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookingCanceller {
    /// 予約した生徒
    Student,
    /// 予約された枠のコーチ
    Coach,
    /// 本部
    Headquarters,
}
