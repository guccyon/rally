use domain::{
    catalog::Sport,
    entitlement::{
        Entitlement, EntitlementError, LiveLessonQuota, Subscription, SubscriptionPlan,
        SubscriptionStatus,
    },
    identity::AccountId,
};
use uuid::Uuid;

use super::{EntitlementRepository, InsertEntitlementError, calendar_month};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
};

/// 月額契約の開始の入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。`plan` は契約プラン
/// （境界がプランの指定を省いたら既定の [`SubscriptionPlan::Standard`]）。
///
/// 決済（billing）は Gen1 の範囲外なので、決済手段もプランの料金も受け取らない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartSubscriptionCommand {
    pub student_id: AccountId,
    pub sport: Sport,
    pub plan: SubscriptionPlan,
}

#[derive(Debug)]
pub enum StartSubscriptionError {
    /// 開始が受講権と契約の不変条件を満たさない（月額はゴルフ以外 [`EntitlementError::SportNotCoveredByContract`]）
    Invalid(EntitlementError),
    /// 生徒の受講権がすでにある（Gen1 の受講権は生徒ごとに 1 つ）
    AlreadySubscribed,
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 月額契約の開始のユースケース（student-app の `startSubscription`）。月額だけで、ワンショットは開始しない。
///
/// 契約中（`active`）の月額契約と、それによる受講権（今の暦月のまだ使っていない、プランの上限の通話の枠）を作って一緒に
/// 保存する。保存したら境界間イベント `SubscriptionStarted` を発行し、行動イベント `subscription.started` を
/// 1 回発火する。拒否・保存の失敗ではどちらも出さない。
pub struct StartSubscription<E, A, P, C> {
    entitlements: E,
    analytics: A,
    events: P,
    clock: C,
}

impl<E, A, P, C> StartSubscription<E, A, P, C>
where
    E: EntitlementRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    pub fn new(entitlements: E, analytics: A, events: P, clock: C) -> Self {
        Self {
            entitlements,
            analytics,
            events,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: StartSubscriptionCommand,
    ) -> Result<(Subscription, Entitlement), StartSubscriptionError> {
        let now = self.clock.now();
        let StartSubscriptionCommand {
            student_id,
            sport,
            plan,
        } = command;
        let subscription = Subscription::new(
            self.entitlements.next_subscription_id(),
            student_id,
            sport,
            plan,
            SubscriptionStatus::Active,
            now,
        )
        .map_err(StartSubscriptionError::Invalid)?;
        let entitlement = Entitlement::monthly(
            self.entitlements.next_entitlement_id(),
            student_id,
            sport,
            subscription.id(),
            LiveLessonQuota::fresh(plan, calendar_month::of(now)),
        )
        .map_err(StartSubscriptionError::Invalid)?;

        self.entitlements
            .insert_monthly(subscription.clone(), entitlement.clone())
            .await
            .map_err(|error| match error {
                InsertEntitlementError::AlreadyExists => StartSubscriptionError::AlreadySubscribed,
                InsertEntitlementError::Unavailable(error) => {
                    StartSubscriptionError::Unavailable(error)
                }
            })?;

        self.events
            .publish(DomainEvent::SubscriptionStarted {
                event_id: Uuid::new_v4(),
                occurred_at: now,
                student_id,
                subscription_id: subscription.id(),
            })
            .await;
        self.analytics
            .emit(AnalyticsEvent::SubscriptionStarted {
                student_id,
                subscription_id: subscription.id(),
                sport,
                occurred_at: now,
            })
            .await;

        Ok((subscription, entitlement))
    }

    pub fn entitlements(&self) -> &E {
        &self.entitlements
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }

    pub fn events(&self) -> &P {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::entitlement::{ContractType, YearMonth};

    use super::*;
    use crate::in_memory::{
        FixedClock, InMemoryEntitlementRepository, RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
    };

    type UseCase = StartSubscription<
        InMemoryEntitlementRepository,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    >;

    const STUDENT: AccountId = AccountId::from_u128(1);

    /// 2026-10-26T00:00:00Z
    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn use_case() -> UseCase {
        StartSubscription::new(
            InMemoryEntitlementRepository::default(),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(now()),
        )
    }

    fn command(sport: Sport) -> StartSubscriptionCommand {
        StartSubscriptionCommand {
            student_id: STUDENT,
            sport,
            plan: SubscriptionPlan::Standard,
        }
    }

    #[tokio::test]
    async fn starts_a_monthly_subscription_and_publishes_subscription_started() {
        let use_case = use_case();

        let (subscription, entitlement) = use_case.execute(command(Sport::Tennis)).await.unwrap();

        assert_eq!(
            (
                subscription.student_id(),
                subscription.sport(),
                subscription.plan(),
                subscription.status(),
                subscription.started_at()
            ),
            (
                STUDENT,
                Sport::Tennis,
                SubscriptionPlan::Standard,
                SubscriptionStatus::Active,
                now()
            )
        );
        assert_eq!(entitlement.contract_type(), ContractType::Monthly);
        assert_eq!(entitlement.subscription_id(), Some(subscription.id()));
        assert_eq!(
            entitlement.live_lesson_quota(),
            Some(&LiveLessonQuota::fresh(
                SubscriptionPlan::Standard,
                YearMonth::new(2026, 10).unwrap()
            )),
            "今の暦月のまだ使っていない、standard の上限の枠"
        );
        assert_eq!(use_case.entitlements().all(), vec![entitlement.clone()]);
        assert_eq!(
            use_case.entitlements().subscriptions(),
            vec![subscription.clone()]
        );

        let events = use_case.events().events();
        let [
            DomainEvent::SubscriptionStarted {
                event_id,
                occurred_at,
                student_id,
                subscription_id,
            },
        ] = events.as_slice()
        else {
            panic!("SubscriptionStarted が 1 件でない: {events:?}");
        };
        assert!(!event_id.is_nil());
        assert_eq!(
            (*occurred_at, *student_id, *subscription_id),
            (now(), STUDENT, subscription.id())
        );
        assert_eq!(events[0].name(), "SubscriptionStarted");

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::SubscriptionStarted {
                student_id: STUDENT,
                subscription_id: subscription.id(),
                sport: Sport::Tennis,
                occurred_at: now(),
            }]
        );
        assert_eq!(analytics[0].name(), "subscription.started");
    }

    #[tokio::test]
    async fn plus_starts_with_a_quota_of_4_live_lessons() {
        let use_case = use_case();

        let (subscription, entitlement) = use_case
            .execute(StartSubscriptionCommand {
                plan: SubscriptionPlan::Plus,
                ..command(Sport::Swimming)
            })
            .await
            .unwrap();

        assert_eq!(subscription.plan(), SubscriptionPlan::Plus);
        assert_eq!(entitlement.plan(), Some(SubscriptionPlan::Plus));
        let quota = entitlement.live_lesson_quota().unwrap();
        assert_eq!((quota.limit(), quota.remaining()), (4, 4));
        assert_eq!(use_case.entitlements().all(), vec![entitlement.clone()]);
        assert_eq!(use_case.entitlements().subscriptions(), vec![subscription]);
    }

    #[tokio::test]
    async fn golf_is_rejected_without_saving_or_emitting() {
        let use_case = use_case();
        let result = use_case.execute(command(Sport::Golf)).await;
        assert!(
            matches!(
                result,
                Err(StartSubscriptionError::Invalid(
                    EntitlementError::SportNotCoveredByContract { .. }
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.entitlements().all().is_empty());
        assert!(use_case.events().events().is_empty());
        assert!(use_case.analytics().events().is_empty());
    }

    #[tokio::test]
    async fn second_subscription_of_the_same_student_is_rejected() {
        let use_case = use_case();
        use_case.execute(command(Sport::Tennis)).await.unwrap();

        let result = use_case.execute(command(Sport::Soccer)).await;
        assert!(
            matches!(result, Err(StartSubscriptionError::AlreadySubscribed)),
            "{result:?}"
        );
        assert_eq!(use_case.entitlements().all().len(), 1);
        assert_eq!(use_case.entitlements().subscriptions().len(), 1);
        assert_eq!(use_case.events().events().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
    }
}
