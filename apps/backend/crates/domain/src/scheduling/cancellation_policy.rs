use std::time::{Duration, SystemTime};

use super::{Booking, CancellationDecision, CancellationPreview};

/// キャンセル規定（glossary の cancellation_policy）。開始 24 時間前以降のキャンセルは回数を消費する。
///
/// 判定は scheduling のここ 1 か所だけに置く（不変条件 3。docs/adr/0002 Q2）。消費の実行は entitlement が
/// 境界間イベント `BookingCancelled` の `consumes_quota` を購読して行うので、閾値を他の層・コンテキストに
/// 写さない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationPolicy;

impl CancellationPolicy {
    /// この時間より開始に近い（ちょうどを含む）キャンセルは回数を消費する。
    const QUOTA_CONSUMING_NOTICE: Duration = Duration::from_secs(24 * 60 * 60);

    /// 予約 `booking` を `cancelled_at` にキャンセルしたときの判定。
    ///
    /// 開始時刻の 24 時間前ちょうど、またはそれより後（開始後を含む）なら `consumes_quota: true`。
    /// 24 時間前より前なら `false`。予約の状態（確定前か確定済みか）・体験予約かは見ない。
    pub fn decide(booking: &Booking, cancelled_at: SystemTime) -> CancellationDecision {
        let consumes_quota =
            Self::free_cancel_until(booking).is_none_or(|until| cancelled_at >= until);
        CancellationDecision { consumes_quota }
    }

    /// 予約 `booking` を回数を使わずにキャンセルできる境目（開始時刻の 24 時間前）。この時刻ちょうど以降の
    /// キャンセルは回数を消費する。開始時刻から 24 時間を引けない（時刻の範囲の外）ときは `None` で、いつ
    /// キャンセルしても消費する。
    pub fn free_cancel_until(booking: &Booking) -> Option<SystemTime> {
        booking
            .starts_at()
            .checked_sub(Self::QUOTA_CONSUMING_NOTICE)
    }

    /// 予約 `booking` を `now` にキャンセルしたらどうなるかの見込み（キャンセルする前に生徒に見せる）。
    ///
    /// キャンセルできない予約（キャンセル済み）は `None`。`has_live_lesson_quota` は生徒がビデオ通話の暦月の枠を
    /// 持つか（entitlement が答える。月額の受講権があるときだけ `true`）で、持たない生徒（受講権がない・ワンショット）は
    /// キャンセルしても entitlement が回数を数えない（docs/adr/0005 の 6）ので、回数を使わず境目もない。持つ生徒は
    /// [`Self::decide`] と同じ判定と、[`Self::free_cancel_until`] の境目。
    pub fn preview(
        booking: &Booking,
        now: SystemTime,
        has_live_lesson_quota: bool,
    ) -> Option<CancellationPreview> {
        if !booking.status().is_active() {
            return None;
        }
        Some(if has_live_lesson_quota {
            CancellationPreview {
                consumes_quota: Self::decide(booking, now).consumes_quota,
                free_cancel_until: Self::free_cancel_until(booking),
            }
        } else {
            CancellationPreview {
                consumes_quota: false,
                free_cancel_until: None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::Sport,
        identity::AccountId,
        scheduling::{BookingId, Slot, SlotId},
    };

    const HOUR: Duration = Duration::from_secs(3600);

    /// 予約したスロットの開始時刻
    fn starts_at() -> SystemTime {
        SystemTime::UNIX_EPOCH + 1000 * HOUR
    }

    fn booking() -> Booking {
        let mut slot = Slot::publish(
            SlotId::from_u128(10),
            AccountId::from_u128(3),
            Sport::Tennis,
            starts_at(),
            starts_at() + HOUR,
            crate::scheduling::SlotCapacity::ONE,
        )
        .unwrap();
        Booking::reserve(
            BookingId::from_u128(1),
            AccountId::from_u128(1),
            false,
            &mut slot,
            &[],
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn consumes_quota(before_start: Duration) -> bool {
        CancellationPolicy::decide(&booking(), starts_at() - before_start).consumes_quota
    }

    #[test]
    fn cancelling_23_hours_before_start_consumes_quota() {
        assert!(consumes_quota(23 * HOUR));
    }

    #[test]
    fn cancelling_25_hours_before_start_does_not_consume_quota() {
        assert!(!consumes_quota(25 * HOUR));
    }

    #[test]
    fn cancelling_exactly_24_hours_before_start_consumes_quota() {
        // 「開始 24 時間前以降」はちょうど 24 時間前を含む
        assert!(consumes_quota(24 * HOUR));
        assert!(consumes_quota(24 * HOUR - Duration::from_nanos(1)));
        assert!(!consumes_quota(24 * HOUR + Duration::from_nanos(1)));
    }

    #[test]
    fn cancelling_at_or_after_start_consumes_quota() {
        assert!(consumes_quota(Duration::ZERO));
        assert!(
            CancellationPolicy::decide(&booking(), starts_at() + HOUR).consumes_quota,
            "開始後のキャンセルも消費する"
        );
    }

    #[test]
    fn free_cancel_until_is_24_hours_before_start() {
        assert_eq!(
            CancellationPolicy::free_cancel_until(&booking()),
            Some(starts_at() - 24 * HOUR)
        );
    }

    fn preview(before_start: Duration) -> Option<CancellationPreview> {
        CancellationPolicy::preview(&booking(), starts_at() - before_start, true)
    }

    #[test]
    fn preview_follows_the_decision_around_the_boundary() {
        let until = Some(starts_at() - 24 * HOUR);
        for (before_start, consumes_quota) in [
            (25 * HOUR, false),
            (24 * HOUR + Duration::from_nanos(1), false),
            (24 * HOUR, true),
            (23 * HOUR, true),
            (Duration::ZERO, true),
        ] {
            assert_eq!(
                preview(before_start),
                Some(CancellationPreview {
                    consumes_quota,
                    free_cancel_until: until,
                }),
                "{before_start:?}"
            );
            assert_eq!(consumes_quota, self::consumes_quota(before_start));
        }
    }

    #[test]
    fn preview_without_live_lesson_quota_never_consumes() {
        // 受講権がない（月額契約の前の体験予約など）・ワンショットの生徒は、24 時間前を過ぎても回数を使わない
        for now in [
            starts_at() - 25 * HOUR,
            starts_at() - HOUR,
            starts_at() + HOUR,
        ] {
            assert_eq!(
                CancellationPolicy::preview(&booking(), now, false),
                Some(CancellationPreview {
                    consumes_quota: false,
                    free_cancel_until: None,
                })
            );
        }
    }

    #[test]
    fn cancelled_booking_has_no_preview() {
        let mut slot = Slot::publish(
            SlotId::from_u128(10),
            AccountId::from_u128(3),
            Sport::Tennis,
            starts_at(),
            starts_at() + HOUR,
            crate::scheduling::SlotCapacity::ONE,
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(1),
            AccountId::from_u128(1),
            false,
            &mut slot,
            &[],
            SystemTime::UNIX_EPOCH,
        )
        .unwrap();
        booking
            .cancel(AccountId::from_u128(1), &mut slot, SystemTime::UNIX_EPOCH)
            .unwrap();
        for has_live_lesson_quota in [true, false] {
            assert_eq!(
                CancellationPolicy::preview(
                    &booking,
                    starts_at() - 25 * HOUR,
                    has_live_lesson_quota
                ),
                None
            );
        }
    }

    #[test]
    fn threshold_is_24_hours() {
        assert_eq!(CancellationPolicy::QUOTA_CONSUMING_NOTICE, 24 * HOUR);
    }
}
