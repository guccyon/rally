use domain::{entitlement::Entitlement, identity::AccountId};

use super::{EntitlementRepository, calendar_month};
use crate::{Unavailable, clock::Clock};

/// 受講権の照会の入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEntitlementQuery {
    pub student_id: AccountId,
}

#[derive(Debug)]
pub enum GetEntitlementError {
    /// 生徒の受講権がまだない
    NotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 受講権の照会のユースケース（student-app の `getMyEntitlement`）。照会だけで、何も保存しない。
///
/// 月額の通話の枠は今の暦月のものにして返す（月をまたいでいれば使った回数は 0 から。domain の
/// [`Entitlement::in_month`]）。
pub struct GetEntitlement<E, C> {
    entitlements: E,
    clock: C,
}

impl<E, C> GetEntitlement<E, C>
where
    E: EntitlementRepository,
    C: Clock,
{
    pub fn new(entitlements: E, clock: C) -> Self {
        Self {
            entitlements,
            clock,
        }
    }

    pub async fn execute(
        &self,
        query: GetEntitlementQuery,
    ) -> Result<Entitlement, GetEntitlementError> {
        let month = calendar_month::of(self.clock.now());
        self.entitlements
            .find_by_student(query.student_id)
            .await
            .map_err(GetEntitlementError::Unavailable)?
            .map(|entitlement| entitlement.in_month(month))
            .ok_or(GetEntitlementError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        entitlement::{EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth},
        identity::Sport,
    };

    use super::*;
    use crate::in_memory::{FixedClock, InMemoryEntitlementRepository};

    const STUDENT: AccountId = AccountId::from_u128(1);

    /// 2026-11-01T00:00:00Z
    fn nov1() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn october() -> YearMonth {
        YearMonth::new(2026, 10).unwrap()
    }

    /// 10 月の枠を使い切った月額の受講権
    fn monthly() -> Entitlement {
        Entitlement::monthly(
            EntitlementId::from_u128(10),
            STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(20),
            LiveLessonQuota::restore(october(), 2).unwrap(),
        )
        .unwrap()
    }

    fn use_case(now: SystemTime) -> GetEntitlement<InMemoryEntitlementRepository, FixedClock> {
        GetEntitlement::new(
            InMemoryEntitlementRepository::with_entitlements([monthly()]),
            FixedClock(now),
        )
    }

    #[tokio::test]
    async fn returns_the_quota_of_the_current_month() {
        let entitlement = use_case(nov1() - Duration::from_secs(1))
            .execute(GetEntitlementQuery {
                student_id: STUDENT,
            })
            .await
            .unwrap();
        assert_eq!(entitlement, monthly(), "同じ月は使った回数のまま");
    }

    #[tokio::test]
    async fn resets_the_quota_after_crossing_the_month() {
        let use_case = use_case(nov1());
        let entitlement = use_case
            .execute(GetEntitlementQuery {
                student_id: STUDENT,
            })
            .await
            .unwrap();
        assert_eq!(
            entitlement.live_lesson_quota(),
            Some(LiveLessonQuota::fresh(YearMonth::new(2026, 11).unwrap())),
            "11 月は 0 から"
        );
        assert_eq!(
            use_case.entitlements.all(),
            vec![monthly()],
            "照会では保存しない"
        );
    }

    #[tokio::test]
    async fn student_without_an_entitlement_is_not_found() {
        let result = use_case(nov1())
            .execute(GetEntitlementQuery {
                student_id: AccountId::from_u128(2),
            })
            .await;
        assert!(
            matches!(result, Err(GetEntitlementError::NotFound)),
            "{result:?}"
        );
    }
}
