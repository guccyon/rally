use std::time::SystemTime;

use domain::identity::AccountId;

use super::{EntitlementRepository, calendar_month};
use crate::{Unavailable, scheduling::QuotaGate};

/// scheduling の [`QuotaGate`] の entitlement による実装。生徒の受講権を読み、domain の
/// [`domain::entitlement::Entitlement::may_book_live_lesson`] で `at` の暦月に枠が残っているかを答える。
///
/// 受講権のない生徒（月額契約を開始する前の体験予約など）は、暦月の回数の制約を受けないので `true`。
#[derive(Debug, Clone)]
pub struct EntitlementQuotaGate<E> {
    entitlements: E,
}

impl<E> EntitlementQuotaGate<E> {
    pub fn new(entitlements: E) -> Self {
        Self { entitlements }
    }
}

impl<E> QuotaGate for EntitlementQuotaGate<E>
where
    E: EntitlementRepository,
{
    async fn may_book_live_lesson(
        &self,
        student_id: AccountId,
        at: SystemTime,
    ) -> Result<bool, Unavailable> {
        let month = calendar_month::of(at);
        Ok(self
            .entitlements
            .find_by_student(student_id)
            .await?
            .is_none_or(|entitlement| entitlement.may_book_live_lesson(month)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        entitlement::{Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
        identity::Sport,
        scheduling::BookingId,
    };

    use super::*;
    use crate::in_memory::InMemoryEntitlementRepository;

    /// 2026-10-26T00:00:00Z
    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn monthly(student: u128, used: u128) -> Entitlement {
        Entitlement::monthly(
            EntitlementId::from_u128(student + 100),
            AccountId::from_u128(student),
            Sport::Tennis,
            SubscriptionId::from_u128(student + 200),
            LiveLessonQuota::restore(
                YearMonth::new(2026, 10).unwrap(),
                (1..=used).map(BookingId::from_u128),
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn answers_by_the_quota_of_the_month() {
        let gate = EntitlementQuotaGate::new(InMemoryEntitlementRepository::with_entitlements([
            monthly(1, 1),
            monthly(2, 2),
        ]));
        let may = |student: u128, at: SystemTime| {
            gate.may_book_live_lesson(AccountId::from_u128(student), at)
        };
        assert!(
            may(1, oct26()).await.unwrap(),
            "1 回使っただけなら残っている"
        );
        assert!(!may(2, oct26()).await.unwrap(), "2 回使ったら 3 回目は不可");
        assert!(
            may(2, oct26() + Duration::from_secs(7 * 86_400))
                .await
                .unwrap(),
            "翌月は再び予約できる"
        );
        assert!(
            may(3, oct26()).await.unwrap(),
            "受講権のない生徒は暦月の回数の制約を受けない"
        );
    }
}
