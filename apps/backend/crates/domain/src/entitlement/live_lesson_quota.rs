use std::collections::BTreeSet;

use super::YearMonth;
use crate::{DomainError, scheduling::BookingId};

/// ビデオ通話の暦月の枠（月額のみ）。月額契約の生徒のビデオ通話は暦月 2 回まで（不変条件 1）。
///
/// 上限（[`Self::LIMIT`]）はここ 1 か所だけに置く。月をまたいだら使った回数は 0 から数える（[`Self::in_month`]）。
/// 枠はその月に枠を使った予約の集合で持ち、使った回数はその数。同じ予約で 2 回は使わない（[`Self::consume`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveLessonQuota {
    month: YearMonth,
    bookings: BTreeSet<BookingId>,
}

impl LiveLessonQuota {
    /// 暦月あたりのビデオ通話の上限。
    const LIMIT: u8 = 2;

    /// `month` の、まだ使っていない枠。
    pub const fn fresh(month: YearMonth) -> Self {
        Self {
            month,
            bookings: BTreeSet::new(),
        }
    }

    /// 保存してある枠（`month` に枠を使った予約）を戻す。予約の数が上限を超えるなら
    /// [`DomainError::LiveLessonsUsedOverLimit`]。
    pub fn restore(
        month: YearMonth,
        bookings: impl IntoIterator<Item = BookingId>,
    ) -> Result<Self, DomainError> {
        let bookings: BTreeSet<BookingId> = bookings.into_iter().collect();
        if bookings.len() > usize::from(Self::LIMIT) {
            return Err(DomainError::LiveLessonsUsedOverLimit {
                used: u8::try_from(bookings.len()).unwrap_or(u8::MAX),
                limit: Self::LIMIT,
            });
        }
        Ok(Self { month, bookings })
    }

    /// `month` の枠。同じ月ならそのまま、別の月（月をまたいだ）なら使った回数を 0 から数える。
    pub fn in_month(self, month: YearMonth) -> Self {
        if self.month == month {
            self
        } else {
            Self::fresh(month)
        }
    }

    /// `month` に、まだ枠を使っていない予約でビデオ通話を 1 回予約してよいか（`month` の枠が残っているか）。
    pub fn has_remaining_in(&self, month: YearMonth) -> bool {
        self.month != month || self.used() < Self::LIMIT
    }

    /// `booking` の予約で枠を 1 回使う。すでにその予約で使っていれば何も変えない（同じ予約で 2 回は使わない）。
    /// 枠が残っていなければ [`DomainError::LiveLessonQuotaExceeded`]。
    ///
    /// 月をまたぐなら先に [`Self::in_month`] を通す。
    pub fn consume(mut self, booking: BookingId) -> Result<Self, DomainError> {
        if self.bookings.contains(&booking) {
            return Ok(self);
        }
        if self.used() >= Self::LIMIT {
            return Err(DomainError::LiveLessonQuotaExceeded);
        }
        self.bookings.insert(booking);
        Ok(self)
    }

    /// `booking` の予約で使った枠を戻す。その予約で使っていなければ何も変えない。
    pub fn release(mut self, booking: BookingId) -> Self {
        self.bookings.remove(&booking);
        self
    }

    pub const fn month(&self) -> YearMonth {
        self.month
    }

    /// この月に枠を使った予約（ID の昇順）。
    pub fn bookings(&self) -> impl Iterator<Item = BookingId> + '_ {
        self.bookings.iter().copied()
    }

    /// この月に使った回数。
    pub fn used(&self) -> u8 {
        // restore と consume が上限（2）までに抑えている
        u8::try_from(self.bookings.len()).expect("使った回数は上限まで")
    }

    /// 暦月あたりの上限（2）。
    pub const fn limit(&self) -> u8 {
        Self::LIMIT
    }

    /// この月の残り回数。
    pub fn remaining(&self) -> u8 {
        Self::LIMIT - self.used()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ym(year: i32, month: u8) -> YearMonth {
        YearMonth::new(year, month).unwrap()
    }

    fn booking(id: u128) -> BookingId {
        BookingId::from_u128(id)
    }

    /// `month` に予約 1〜`used` で枠を使った状態。
    fn used(month: YearMonth, used: u128) -> LiveLessonQuota {
        LiveLessonQuota::restore(month, (1..=used).map(booking)).unwrap()
    }

    #[test]
    fn limit_is_2_per_calendar_month() {
        assert_eq!(LiveLessonQuota::LIMIT, 2);
        let quota = LiveLessonQuota::fresh(ym(2026, 10));
        assert_eq!((quota.used(), quota.limit(), quota.remaining()), (0, 2, 2));
    }

    #[test]
    fn restores_up_to_the_limit() {
        for n in 0..=2 {
            let quota = used(ym(2026, 10), n);
            assert_eq!(
                (quota.used(), quota.remaining()),
                (n as u8, 2 - n as u8),
                "{n}"
            );
        }
        assert_eq!(
            LiveLessonQuota::restore(ym(2026, 10), (1..=3).map(booking)),
            Err(DomainError::LiveLessonsUsedOverLimit { used: 3, limit: 2 })
        );
    }

    #[test]
    fn restoring_the_same_booking_twice_counts_once() {
        let quota = LiveLessonQuota::restore(ym(2026, 10), [booking(1), booking(1)]).unwrap();
        assert_eq!(quota.used(), 1);
    }

    #[test]
    fn consumes_up_to_2_and_the_third_is_exceeded() {
        let quota = LiveLessonQuota::fresh(ym(2026, 10))
            .consume(booking(1))
            .unwrap()
            .consume(booking(2))
            .unwrap();
        assert_eq!((quota.used(), quota.remaining()), (2, 0));
        assert_eq!(
            quota.bookings().collect::<Vec<_>>(),
            [booking(1), booking(2)]
        );
        assert_eq!(
            quota.consume(booking(3)),
            Err(DomainError::LiveLessonQuotaExceeded),
            "3 回目は使えない"
        );
    }

    #[test]
    fn consuming_with_the_same_booking_twice_counts_once() {
        let quota = used(ym(2026, 10), 2);
        assert_eq!(
            quota.clone().consume(booking(2)),
            Ok(quota),
            "上限でも同じ予約は通る"
        );
        let once = LiveLessonQuota::fresh(ym(2026, 10))
            .consume(booking(1))
            .unwrap();
        assert_eq!(once.clone().consume(booking(1)), Ok(once));
    }

    #[test]
    fn releasing_returns_the_booking_and_ignores_unknown_ones() {
        let quota = used(ym(2026, 10), 2).release(booking(1));
        assert_eq!((quota.used(), quota.remaining()), (1, 1));
        assert_eq!(quota.clone().release(booking(9)), quota);
    }

    #[test]
    fn has_remaining_only_until_the_limit_in_the_same_month() {
        assert!(used(ym(2026, 10), 1).has_remaining_in(ym(2026, 10)));
        assert!(!used(ym(2026, 10), 2).has_remaining_in(ym(2026, 10)));
        assert!(
            used(ym(2026, 10), 2).has_remaining_in(ym(2026, 11)),
            "翌月は残っている"
        );
    }

    #[test]
    fn same_month_keeps_used() {
        let quota = used(ym(2026, 10), 2);
        assert_eq!(quota.clone().in_month(ym(2026, 10)), quota);
    }

    #[test]
    fn crossing_into_the_next_month_resets_used_to_0() {
        let october = used(ym(2026, 10), 2);
        assert_eq!(october.remaining(), 0, "10 月は使い切っている");

        let november = october.in_month(ym(2026, 11));
        assert_eq!(november.month(), ym(2026, 11));
        assert_eq!(
            (november.used(), november.remaining()),
            (0, 2),
            "月をまたいだら 0 から"
        );
        assert!(november.consume(booking(3)).is_ok(), "翌月は再び使える");
    }

    #[test]
    fn crossing_the_year_resets_used_to_0() {
        let december = used(ym(2026, 12), 1);
        let january = december.in_month(ym(2027, 1));
        assert_eq!(january, LiveLessonQuota::fresh(ym(2027, 1)));
    }
}
