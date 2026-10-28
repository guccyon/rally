use std::time::SystemTime;

use domain::{DomainError, entitlement::Entitlement, identity::AccountId, scheduling::BookingId};

use super::{EntitlementRepository, UpdateEntitlementError, calendar_month};
use crate::Unavailable;

/// ビデオ通話の暦月の枠の消費の入力。`at` は消費の元になった出来事（予約の確定・キャンセル）の時刻で、
/// どの暦月の枠を使うかはこれで決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumeLiveLessonQuotaCommand {
    pub student_id: AccountId,
    pub booking_id: BookingId,
    pub at: SystemTime,
}

#[derive(Debug)]
pub enum ConsumeLiveLessonQuotaError {
    /// 生徒の受講権がない（月額契約を開始していない）
    NotFound,
    /// 消費がドメインの不変条件を満たさない（枠が残っていない）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// ビデオ通話の暦月の枠を 1 回使うユースケース（entitlement。不変条件 1）。
///
/// 境界間イベントの購読（[`super::LiveLessonQuotaSubscriber`]）から、ビデオ通話の予約が確定したとき
/// （`BookingConfirmed`）と、回数を消費するキャンセル（`BookingCancelled` の `consumes_quota: true`）のときに呼ぶ。
/// 規則（上限 2・同じ予約で 2 回は使わない・月をまたいだら 0 から）は domain の [`Entitlement::consume_live_lesson`]。
pub struct ConsumeLiveLessonQuota<E> {
    entitlements: E,
}

impl<E> ConsumeLiveLessonQuota<E>
where
    E: EntitlementRepository,
{
    pub fn new(entitlements: E) -> Self {
        Self { entitlements }
    }

    pub async fn execute(
        &self,
        command: ConsumeLiveLessonQuotaCommand,
    ) -> Result<Entitlement, ConsumeLiveLessonQuotaError> {
        let month = calendar_month::of(command.at);
        self.entitlements
            .update_by_student(command.student_id, |entitlement| {
                entitlement.consume_live_lesson(command.booking_id, month)
            })
            .await
            .map_err(|error| match error {
                UpdateEntitlementError::NotFound => ConsumeLiveLessonQuotaError::NotFound,
                UpdateEntitlementError::Rejected(error) => {
                    ConsumeLiveLessonQuotaError::Invalid(error)
                }
                UpdateEntitlementError::Unavailable(error) => {
                    ConsumeLiveLessonQuotaError::Unavailable(error)
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
        entitlement::{
            EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan, YearMonth,
        },
        identity::Sport,
    };

    use super::*;
    use crate::in_memory::InMemoryEntitlementRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);

    /// 2026-10-26T00:00:00Z
    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn use_case() -> ConsumeLiveLessonQuota<InMemoryEntitlementRepository> {
        let monthly = Entitlement::monthly(
            EntitlementId::from_u128(10),
            STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(20),
            LiveLessonQuota::fresh(
                SubscriptionPlan::Standard,
                YearMonth::new(2026, 10).unwrap(),
            ),
        )
        .unwrap();
        ConsumeLiveLessonQuota::new(InMemoryEntitlementRepository::with_entitlements([monthly]))
    }

    fn command(booking: u128, at: SystemTime) -> ConsumeLiveLessonQuotaCommand {
        ConsumeLiveLessonQuotaCommand {
            student_id: STUDENT,
            booking_id: BookingId::from_u128(booking),
            at,
        }
    }

    fn used(use_case: &ConsumeLiveLessonQuota<InMemoryEntitlementRepository>) -> (YearMonth, u8) {
        let entitlement = &use_case.entitlements().all()[0];
        let quota = entitlement.live_lesson_quota().unwrap();
        (quota.month(), quota.used())
    }

    #[tokio::test]
    async fn consumes_twice_a_month_and_rejects_the_third_without_saving() {
        let use_case = use_case();
        let october = YearMonth::new(2026, 10).unwrap();
        use_case.execute(command(1, oct26())).await.unwrap();
        use_case.execute(command(2, oct26())).await.unwrap();
        assert_eq!(used(&use_case), (october, 2));

        let result = use_case.execute(command(3, oct26())).await;
        assert!(
            matches!(
                result,
                Err(ConsumeLiveLessonQuotaError::Invalid(
                    DomainError::LiveLessonQuotaExceeded
                ))
            ),
            "{result:?}"
        );
        assert_eq!(used(&use_case), (october, 2), "拒否したら保存しない");

        // 同じ予約の 2 回目（重複して届いたイベント）は数えない
        use_case.execute(command(2, oct26())).await.unwrap();
        assert_eq!(used(&use_case), (october, 2));
    }

    #[tokio::test]
    async fn the_next_month_is_counted_from_0() {
        let use_case = use_case();
        for booking in [1, 2] {
            use_case.execute(command(booking, oct26())).await.unwrap();
        }
        let november = oct26() + Duration::from_secs(7 * 86_400);
        use_case.execute(command(3, november)).await.unwrap();
        assert_eq!(used(&use_case), (YearMonth::new(2026, 11).unwrap(), 1));
    }

    #[tokio::test]
    async fn student_without_an_entitlement_is_not_found() {
        let use_case = use_case();
        let result = use_case
            .execute(ConsumeLiveLessonQuotaCommand {
                student_id: AccountId::from_u128(2),
                ..command(1, oct26())
            })
            .await;
        assert!(
            matches!(result, Err(ConsumeLiveLessonQuotaError::NotFound)),
            "{result:?}"
        );
    }
}
