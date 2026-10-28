use std::collections::BTreeSet;

use super::{SubscriptionPlan, YearMonth};
use crate::{DomainError, scheduling::BookingId};

/// ビデオ通話の暦月の枠（月額のみ）。月額契約の生徒のビデオ通話は、契約プランで決まる暦月の上限まで（不変条件 1）。
///
/// 上限は契約プラン（[`SubscriptionPlan::live_lessons_per_month`]）から受け取り、ここには数を持たない。月をまたいだら
/// 使った回数は 0 から数える（[`Self::in_month`]）。枠はその月に枠を使った予約の集合で持ち、使った回数はその数。
/// 同じ予約で 2 回は使わない（[`Self::consume`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveLessonQuota {
    plan: SubscriptionPlan,
    month: YearMonth,
    bookings: BTreeSet<BookingId>,
}

impl LiveLessonQuota {
    /// `plan` の契約の、`month` のまだ使っていない枠。
    pub const fn fresh(plan: SubscriptionPlan, month: YearMonth) -> Self {
        Self {
            plan,
            month,
            bookings: BTreeSet::new(),
        }
    }

    /// 保存してある `plan` の契約の枠（`month` に枠を使った予約）を戻す。予約の数がプランの上限を超えるなら
    /// [`DomainError::LiveLessonsUsedOverLimit`]。
    pub fn restore(
        plan: SubscriptionPlan,
        month: YearMonth,
        bookings: impl IntoIterator<Item = BookingId>,
    ) -> Result<Self, DomainError> {
        let bookings: BTreeSet<BookingId> = bookings.into_iter().collect();
        let limit = plan.live_lessons_per_month();
        if bookings.len() > usize::from(limit) {
            return Err(DomainError::LiveLessonsUsedOverLimit {
                used: u8::try_from(bookings.len()).unwrap_or(u8::MAX),
                limit,
            });
        }
        Ok(Self {
            plan,
            month,
            bookings,
        })
    }

    /// `month` の枠。同じ月ならそのまま、別の月（月をまたいだ）なら使った回数を 0 から数える。
    pub fn in_month(self, month: YearMonth) -> Self {
        if self.month == month {
            self
        } else {
            Self::fresh(self.plan, month)
        }
    }

    /// `month` に、まだ枠を使っていない予約でビデオ通話を 1 回予約してよいか（`month` の枠が残っているか）。
    pub fn has_remaining_in(&self, month: YearMonth) -> bool {
        self.month != month || self.used() < self.limit()
    }

    /// `booking` の予約で枠を 1 回使う。すでにその予約で使っていれば何も変えない（同じ予約で 2 回は使わない）。
    /// 枠が残っていなければ [`DomainError::LiveLessonQuotaExceeded`]。
    ///
    /// 月をまたぐなら先に [`Self::in_month`] を通す。
    pub fn consume(mut self, booking: BookingId) -> Result<Self, DomainError> {
        if self.bookings.contains(&booking) {
            return Ok(self);
        }
        if self.used() >= self.limit() {
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

    /// 上限を決める契約プラン。
    pub const fn plan(&self) -> SubscriptionPlan {
        self.plan
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
        // restore と consume がプランの上限までに抑えている
        u8::try_from(self.bookings.len()).expect("使った回数は上限まで")
    }

    /// 暦月あたりの上限（契約プランで決まる）。
    pub const fn limit(&self) -> u8 {
        self.plan.live_lessons_per_month()
    }

    /// この月の残り回数。
    pub fn remaining(&self) -> u8 {
        self.limit() - self.used()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD: SubscriptionPlan = SubscriptionPlan::Standard;
    const PLUS: SubscriptionPlan = SubscriptionPlan::Plus;

    fn ym(year: i32, month: u8) -> YearMonth {
        YearMonth::new(year, month).unwrap()
    }

    fn booking(id: u128) -> BookingId {
        BookingId::from_u128(id)
    }

    /// `plan` の契約で、`month` に予約 1〜`used` で枠を使った状態。
    fn used(plan: SubscriptionPlan, month: YearMonth, used: u128) -> LiveLessonQuota {
        LiveLessonQuota::restore(plan, month, (1..=used).map(booking)).unwrap()
    }

    /// 予約 1, 2, … で枠を使い切るまで使い、使えた回数を返す。
    fn consume_until_exceeded(mut quota: LiveLessonQuota) -> (LiveLessonQuota, u8) {
        let mut consumed = 0;
        for id in 1.. {
            match quota.clone().consume(booking(id)) {
                Ok(next) => {
                    quota = next;
                    consumed += 1;
                }
                Err(error) => {
                    assert_eq!(error, DomainError::LiveLessonQuotaExceeded);
                    break;
                }
            }
        }
        (quota, consumed)
    }

    #[test]
    fn limit_comes_from_the_plan() {
        let quota = LiveLessonQuota::fresh(STANDARD, ym(2026, 10));
        assert_eq!(quota.plan(), STANDARD);
        assert_eq!((quota.used(), quota.limit(), quota.remaining()), (0, 2, 2));
        let quota = LiveLessonQuota::fresh(PLUS, ym(2026, 10));
        assert_eq!(quota.plan(), PLUS);
        assert_eq!((quota.used(), quota.limit(), quota.remaining()), (0, 4, 4));
    }

    #[test]
    fn restores_up_to_the_limit_of_the_plan() {
        for (plan, limit) in [(STANDARD, 2u8), (PLUS, 4)] {
            for n in 0..=u128::from(limit) {
                let quota = used(plan, ym(2026, 10), n);
                assert_eq!(
                    (quota.used(), quota.remaining()),
                    (n as u8, limit - n as u8),
                    "{plan:?} {n}"
                );
            }
            assert_eq!(
                LiveLessonQuota::restore(
                    plan,
                    ym(2026, 10),
                    (1..=u128::from(limit) + 1).map(booking)
                ),
                Err(DomainError::LiveLessonsUsedOverLimit {
                    used: limit + 1,
                    limit
                }),
                "{plan:?}"
            );
        }
    }

    #[test]
    fn restoring_the_same_booking_twice_counts_once() {
        let quota =
            LiveLessonQuota::restore(STANDARD, ym(2026, 10), [booking(1), booking(1)]).unwrap();
        assert_eq!(quota.used(), 1);
    }

    #[test]
    fn standard_consumes_up_to_2_and_the_third_is_exceeded() {
        let (quota, consumed) =
            consume_until_exceeded(LiveLessonQuota::fresh(STANDARD, ym(2026, 10)));
        assert_eq!(consumed, 2, "3 回目は使えない");
        assert_eq!((quota.used(), quota.remaining()), (2, 0));
        assert_eq!(
            quota.bookings().collect::<Vec<_>>(),
            [booking(1), booking(2)]
        );
    }

    #[test]
    fn plus_consumes_up_to_4_and_the_fifth_is_exceeded() {
        let (quota, consumed) = consume_until_exceeded(LiveLessonQuota::fresh(PLUS, ym(2026, 10)));
        assert_eq!(consumed, 4, "5 回目は使えない");
        assert_eq!((quota.used(), quota.remaining()), (4, 0));
        assert!(!quota.has_remaining_in(ym(2026, 10)));
        assert_eq!(
            quota.consume(booking(5)),
            Err(DomainError::LiveLessonQuotaExceeded)
        );
    }

    #[test]
    fn consuming_with_the_same_booking_twice_counts_once() {
        let quota = used(STANDARD, ym(2026, 10), 2);
        assert_eq!(
            quota.clone().consume(booking(2)),
            Ok(quota),
            "上限でも同じ予約は通る"
        );
        let once = LiveLessonQuota::fresh(STANDARD, ym(2026, 10))
            .consume(booking(1))
            .unwrap();
        assert_eq!(once.clone().consume(booking(1)), Ok(once));
    }

    #[test]
    fn releasing_returns_the_booking_and_ignores_unknown_ones() {
        let quota = used(STANDARD, ym(2026, 10), 2).release(booking(1));
        assert_eq!((quota.used(), quota.remaining()), (1, 1));
        assert_eq!(quota.clone().release(booking(9)), quota);
    }

    #[test]
    fn has_remaining_only_until_the_limit_in_the_same_month() {
        assert!(used(STANDARD, ym(2026, 10), 1).has_remaining_in(ym(2026, 10)));
        assert!(!used(STANDARD, ym(2026, 10), 2).has_remaining_in(ym(2026, 10)));
        assert!(
            used(PLUS, ym(2026, 10), 2).has_remaining_in(ym(2026, 10)),
            "plus は 3 回目も予約できる"
        );
        assert!(used(PLUS, ym(2026, 10), 3).has_remaining_in(ym(2026, 10)));
        assert!(!used(PLUS, ym(2026, 10), 4).has_remaining_in(ym(2026, 10)));
        assert!(
            used(STANDARD, ym(2026, 10), 2).has_remaining_in(ym(2026, 11)),
            "翌月は残っている"
        );
    }

    #[test]
    fn same_month_keeps_used() {
        let quota = used(STANDARD, ym(2026, 10), 2);
        assert_eq!(quota.clone().in_month(ym(2026, 10)), quota);
    }

    #[test]
    fn crossing_into_the_next_month_resets_used_to_0_and_keeps_the_plan() {
        for (plan, limit) in [(STANDARD, 2u8), (PLUS, 4)] {
            let october = used(plan, ym(2026, 10), u128::from(limit));
            assert_eq!(october.remaining(), 0, "10 月は使い切っている");

            let november = october.in_month(ym(2026, 11));
            assert_eq!((november.plan(), november.month()), (plan, ym(2026, 11)));
            assert_eq!(
                (november.used(), november.remaining()),
                (0, limit),
                "月をまたいだら 0 から"
            );
            let (_, consumed) = consume_until_exceeded(november);
            assert_eq!(consumed, limit, "翌月は再び上限まで使える");
        }
    }

    #[test]
    fn crossing_the_year_resets_used_to_0() {
        let december = used(PLUS, ym(2026, 12), 3);
        let january = december.in_month(ym(2027, 1));
        assert_eq!(january, LiveLessonQuota::fresh(PLUS, ym(2027, 1)));
    }
}
