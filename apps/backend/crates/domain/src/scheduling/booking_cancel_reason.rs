/// 予約（[`super::Booking`]）をキャンセルした理由。外部での文字列表現は境界のアダプタが決める。
///
/// 理由を残すのは本部のキャンセルだけで、生徒が自分でしたキャンセルは理由を持たない（[`super::Booking::cancel_reason`] が
/// `None`）。どのキャンセルにどの理由を付けるかは [`super::Booking`] の 1 か所で決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookingCancelReason {
    /// コーチのお休み（[`super::CoachLeave`]）で本部がキャンセルした
    CoachLeave,
}
