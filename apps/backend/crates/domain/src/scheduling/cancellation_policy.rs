use std::time::{Duration, SystemTime};

use super::{Booking, CancellationDecision};

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
        let consumes_quota = cancelled_at
            .checked_add(Self::QUOTA_CONSUMING_NOTICE)
            .is_none_or(|deadline| deadline >= booking.starts_at());
        CancellationDecision { consumes_quota }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{AccountId, Sport},
        scheduling::{BookingId, Slot, SlotId},
    };

    const HOUR: Duration = Duration::from_secs(3600);

    /// 予約したスロットの開始時刻
    fn starts_at() -> SystemTime {
        SystemTime::UNIX_EPOCH + 1000 * HOUR
    }

    fn booking() -> Booking {
        let slot = Slot::publish(
            SlotId::from_u128(10),
            AccountId::from_u128(3),
            Sport::Tennis,
            starts_at(),
            starts_at() + HOUR,
        )
        .unwrap();
        Booking::reserve(
            BookingId::from_u128(1),
            AccountId::from_u128(1),
            false,
            &slot,
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
    fn threshold_is_24_hours() {
        assert_eq!(CancellationPolicy::QUOTA_CONSUMING_NOTICE, 24 * HOUR);
    }
}
