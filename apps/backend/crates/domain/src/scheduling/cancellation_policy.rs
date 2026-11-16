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
    /// `has_live_lesson_quota` は生徒がビデオ通話の暦月の枠を持つか（[`Self::preview`] と同じ）。持たない生徒は
    /// entitlement が回数を数えない（docs/adr/0005 の 6）ので、いつキャンセルしても `consumes_quota: false`。
    /// 体験予約（[`Booking::is_trial`]）も、月額の受講権のない間に作った無料の予約で月額の回数を使って取ったものでは
    /// ないので、あとで月額契約してもいつキャンセルしても `consumes_quota: false`（#1798）。
    /// それ以外で枠を持つ生徒は、開始時刻の 24 時間前ちょうど、またはそれより後（開始後を含む）なら
    /// `consumes_quota: true`。24 時間前より前なら `false`。予約の状態（確定前か確定済みか）は見ない。
    pub fn decide(
        booking: &Booking,
        cancelled_at: SystemTime,
        has_live_lesson_quota: bool,
    ) -> CancellationDecision {
        let consumes_quota = Self::counts_quota(booking, has_live_lesson_quota)
            && Self::free_cancel_until(booking).is_none_or(|until| cancelled_at >= until);
        CancellationDecision { consumes_quota }
    }

    /// 予約 `booking` のキャンセルが暦月の回数に関わるか。生徒が枠を持ち、体験予約でないときだけ。体験予約は
    /// 受講権のない間にしか作れない（`entitlement::TrialUsage`。docs/adr/0014）ので、作った時点で枠を持たない。
    fn counts_quota(booking: &Booking, has_live_lesson_quota: bool) -> bool {
        has_live_lesson_quota && !booking.is_trial()
    }

    /// 本部が予約 `booking` をキャンセルしたとき（コーチのお休み。[`super::Booking::cancel_by_headquarters`]）の判定。
    ///
    /// 生徒の都合でないので、いつキャンセルしても `consumes_quota: false`（entitlement は確定で使った枠を戻す）。
    /// 開始までの時間・予約の状態は見ない。
    pub fn decide_for_headquarters(_booking: &Booking) -> CancellationDecision {
        CancellationDecision {
            consumes_quota: false,
        }
    }

    /// 予約 `booking` を回数を使わずにキャンセルできる境目（開始時刻の 24 時間前）。この時刻ちょうど以降の
    /// キャンセルは回数を消費する。開始時刻から 24 時間を引けない（時刻の範囲の外）ときは `None` で、いつ
    /// キャンセルしても消費する。
    pub fn free_cancel_until(booking: &Booking) -> Option<SystemTime> {
        booking
            .starts_at()
            .checked_sub(Self::QUOTA_CONSUMING_NOTICE)
    }

    /// 見込みに載せる境目。[`Self::free_cancel_until`] が引けないとき（時刻の範囲の外）は `UNIX_EPOCH` に
    /// 丸める（実質いつキャンセルしても消費する。契約は境目の無いのを「いつでも回数を使わない」ときに限るため）。
    fn preview_boundary(booking: &Booking) -> SystemTime {
        Self::free_cancel_until(booking).unwrap_or(SystemTime::UNIX_EPOCH)
    }

    /// 予約 `booking` を `now` にキャンセルしたらどうなるかの見込み（キャンセルする前に生徒に見せる）。
    ///
    /// キャンセルできない予約（キャンセル済み）は `None`。`has_live_lesson_quota` は生徒がビデオ通話の暦月の枠を
    /// 持つか（entitlement が答える。月額の受講権があるときだけ `true`）で、持たない生徒（受講権がない・ワンショット）は
    /// キャンセルしても entitlement が回数を数えない（docs/adr/0005 の 6）ので、回数を使わず境目もない。体験予約も
    /// （[`Self::decide`] と同じく）回数を使わず境目もない。それ以外で枠を持つ生徒は [`Self::decide`] と同じ判定と、
    /// [`Self::free_cancel_until`] の境目。
    pub fn preview(
        booking: &Booking,
        now: SystemTime,
        has_live_lesson_quota: bool,
    ) -> Option<CancellationPreview> {
        if !booking.status().is_active() {
            return None;
        }
        Some(if Self::counts_quota(booking, has_live_lesson_quota) {
            CancellationPreview::with_boundary(
                Self::decide(booking, now, true).consumes_quota,
                Self::preview_boundary(booking),
            )
        } else {
            CancellationPreview::without_quota()
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
        reserve(false)
    }

    /// 体験予約（月額契約の前に作った無料の予約）
    fn trial_booking() -> Booking {
        reserve(true)
    }

    fn reserve(is_trial: bool) -> Booking {
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
            is_trial,
            &mut slot,
            &[],
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn consumes_quota(before_start: Duration) -> bool {
        CancellationPolicy::decide(&booking(), starts_at() - before_start, true).consumes_quota
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
    fn a_student_without_the_live_lesson_quota_never_consumes_quota() {
        // 受講権のない生徒は entitlement が回数を数えないので、開始の直前・開始後でも消費しない（見込みと同じ）
        for cancelled_at in [
            starts_at() - 25 * HOUR,
            starts_at() - 24 * HOUR,
            starts_at() - HOUR,
            starts_at() + HOUR,
        ] {
            assert!(!CancellationPolicy::decide(&booking(), cancelled_at, false).consumes_quota);
            assert_eq!(
                CancellationPolicy::decide(&booking(), cancelled_at, false).consumes_quota,
                CancellationPolicy::preview(&booking(), cancelled_at, false)
                    .unwrap()
                    .consumes_quota()
            );
        }
    }

    #[test]
    fn a_trial_booking_never_consumes_quota_even_after_subscribing() {
        // 体験予約は月額の回数を使って取った予約でないので、あとで月額契約して枠を持っても、開始の直前・開始後の
        // キャンセルで回数を使わない。見込みも同じで境目はない（#1798）
        for has_live_lesson_quota in [true, false] {
            for cancelled_at in [
                starts_at() - 25 * HOUR,
                starts_at() - 24 * HOUR,
                starts_at() - HOUR,
                starts_at() + HOUR,
            ] {
                assert!(
                    !CancellationPolicy::decide(
                        &trial_booking(),
                        cancelled_at,
                        has_live_lesson_quota
                    )
                    .consumes_quota
                );
                assert_eq!(
                    CancellationPolicy::preview(
                        &trial_booking(),
                        cancelled_at,
                        has_live_lesson_quota
                    ),
                    Some(CancellationPreview::without_quota())
                );
            }
        }
    }

    #[test]
    fn a_headquarters_cancel_never_consumes_quota() {
        // 生徒の都合でないので、開始の直前でも消費しない
        assert!(!CancellationPolicy::decide_for_headquarters(&booking()).consumes_quota);
    }

    #[test]
    fn cancelling_at_or_after_start_consumes_quota() {
        assert!(consumes_quota(Duration::ZERO));
        assert!(
            CancellationPolicy::decide(&booking(), starts_at() + HOUR, true).consumes_quota,
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
        let until = starts_at() - 24 * HOUR;
        for (before_start, consumes_quota) in [
            (25 * HOUR, false),
            (24 * HOUR + Duration::from_nanos(1), false),
            (24 * HOUR, true),
            (23 * HOUR, true),
            (Duration::ZERO, true),
        ] {
            assert_eq!(
                preview(before_start),
                Some(CancellationPreview::with_boundary(consumes_quota, until)),
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
                Some(CancellationPreview::without_quota())
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
            .cancel(
                AccountId::from_u128(1),
                &mut slot,
                SystemTime::UNIX_EPOCH,
                true,
            )
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
