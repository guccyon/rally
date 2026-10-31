use std::time::SystemTime;

use domain::{entitlement::Entitlement, identity::AccountId, scheduling::BookingId};

use super::{EntitlementRepository, UpdateEntitlementError, calendar_month};
use crate::Unavailable;

/// ビデオ通話の暦月の枠を戻す入力。`at` は戻す元になった出来事（予約のキャンセル）の時刻で、
/// どの暦月の枠に戻すかはこれで決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseLiveLessonQuotaCommand {
    pub student_id: AccountId,
    pub booking_id: BookingId,
    pub at: SystemTime,
}

#[derive(Debug)]
pub enum ReleaseLiveLessonQuotaError {
    /// 生徒の受講権がない（月額契約を開始していない）
    NotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 予約で使ったビデオ通話の暦月の枠を戻すユースケース（entitlement）。
///
/// 回数を消費しないキャンセル（`BookingCancelled` の `consumes_quota: false`。開始の 24 時間より前）のときに
/// 境界間イベントの購読（[`super::LiveLessonQuotaSubscriber`]）から呼ぶ。確定で使った枠が戻り、確定前の予約
/// （枠を使っていない）なら何も変えない。規則は domain の [`Entitlement::release_live_lesson`]。
pub struct ReleaseLiveLessonQuota<E> {
    entitlements: E,
}

impl<E> ReleaseLiveLessonQuota<E>
where
    E: EntitlementRepository,
{
    pub fn new(entitlements: E) -> Self {
        Self { entitlements }
    }

    pub async fn execute(
        &self,
        command: ReleaseLiveLessonQuotaCommand,
    ) -> Result<Entitlement, ReleaseLiveLessonQuotaError> {
        let month = calendar_month::of(command.at);
        self.entitlements
            .update_by_student(command.student_id, |entitlement| {
                entitlement.release_live_lesson(command.booking_id, month);
                Ok(())
            })
            .await
            .map_err(|error| match error {
                UpdateEntitlementError::NotFound => ReleaseLiveLessonQuotaError::NotFound,
                UpdateEntitlementError::Unavailable(error) => {
                    ReleaseLiveLessonQuotaError::Unavailable(error)
                }
                UpdateEntitlementError::Rejected(error) => {
                    ReleaseLiveLessonQuotaError::Unavailable(Unavailable::new(error))
                }
            })
    }

    pub fn entitlements(&self) -> &E {
        &self.entitlements
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{
        catalog::Sport,
        entitlement::{
            EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan, YearMonth,
        },
    };

    use super::*;
    use crate::in_memory::InMemoryEntitlementRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);

    /// 2026-10-26T00:00:00Z
    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    /// 10 月の枠を予約 1・2 で使い切った月額の受講権
    fn use_case() -> ReleaseLiveLessonQuota<InMemoryEntitlementRepository> {
        let monthly = Entitlement::monthly(
            EntitlementId::from_u128(10),
            STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(20),
            LiveLessonQuota::restore(
                SubscriptionPlan::Standard,
                YearMonth::new(2026, 10).unwrap(),
                [BookingId::from_u128(1), BookingId::from_u128(2)],
            )
            .unwrap(),
        )
        .unwrap();
        ReleaseLiveLessonQuota::new(InMemoryEntitlementRepository::with_entitlements([monthly]))
    }

    fn command(booking: u128) -> ReleaseLiveLessonQuotaCommand {
        ReleaseLiveLessonQuotaCommand {
            student_id: STUDENT,
            booking_id: BookingId::from_u128(booking),
            at: oct26(),
        }
    }

    fn used(use_case: &ReleaseLiveLessonQuota<InMemoryEntitlementRepository>) -> u8 {
        use_case.entitlements().all()[0]
            .live_lesson_quota()
            .unwrap()
            .used()
    }

    #[tokio::test]
    async fn releases_the_quota_used_by_the_booking_once() {
        let use_case = use_case();
        use_case.execute(command(1)).await.unwrap();
        assert_eq!(used(&use_case), 1);
        use_case.execute(command(1)).await.unwrap();
        assert_eq!(used(&use_case), 1, "同じ予約を 2 回戻しても 1 回分");
        use_case.execute(command(9)).await.unwrap();
        assert_eq!(used(&use_case), 1, "枠を使っていない予約は何も変えない");
    }

    #[tokio::test]
    async fn student_without_an_entitlement_is_not_found() {
        let result = use_case()
            .execute(ReleaseLiveLessonQuotaCommand {
                student_id: AccountId::from_u128(2),
                ..command(1)
            })
            .await;
        assert!(
            matches!(result, Err(ReleaseLiveLessonQuotaError::NotFound)),
            "{result:?}"
        );
    }
}
