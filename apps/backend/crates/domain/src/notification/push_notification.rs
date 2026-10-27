use crate::{identity::AccountId, scheduling::BookingId};

/// 1 件のプッシュ通知。変種は通知のきっかけ（境界間イベント）ごとで、受け取る人は [`PushNotification::recipient`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushNotification {
    /// 予約が確定した（`BookingConfirmed`）ことを生徒に知らせる
    BookingConfirmed {
        student_id: AccountId,
        coach_id: AccountId,
        booking_id: BookingId,
    },
}

impl PushNotification {
    /// 通知を受け取るアカウント。
    pub const fn recipient(&self) -> AccountId {
        match *self {
            PushNotification::BookingConfirmed { student_id, .. } => student_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_confirmed_booking_goes_to_the_student() {
        let notification = PushNotification::BookingConfirmed {
            student_id: AccountId::from_u128(1),
            coach_id: AccountId::from_u128(9),
            booking_id: BookingId::from_u128(3),
        };
        assert_eq!(notification.recipient(), AccountId::from_u128(1));
    }
}
