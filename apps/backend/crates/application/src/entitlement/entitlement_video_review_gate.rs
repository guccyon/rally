use domain::identity::AccountId;

use super::TrialUsageLookup;
use crate::{
    Unavailable,
    analytics::AnalyticsSink,
    async_lesson::VideoReviewGate,
    clock::Clock,
    domain_event::DomainEventPublisher,
    scheduling::{BookingRepository, CancelBooking, CancelBookingCommand, CancelBookingError},
};

/// async-lesson の [`VideoReviewGate`] の entitlement による実装。生徒の体験の使用状況を照会し
/// （[`TrialUsageLookup`]）、domain の [`domain::entitlement::TrialUsage::may_request_video_review`] に答えさせる。
///
/// 月額の生徒（動画とチャットの送り放題）は常に通し、何も変えない。月額契約の前の生徒は、完了したレッスンも送った
/// 動画もないときだけ体験として通す。このとき、まだ受けていない体験予約（キャンセルされていない体験予約）を
/// scheduling の [`CancelBooking`] で生徒のキャンセルとして解放する（スロットの席が戻り、境界間イベント
/// `BookingCancelled` と行動イベント `booking.cancelled` も生徒がキャンセルしたときと同じく出る）。並行にキャンセル
/// された予約は飛ばす。断るときは何も変えない。
pub struct EntitlementVideoReviewGate<T, B, A, P, C> {
    usage: T,
    cancel: CancelBooking<B, A, P, C>,
}

impl<T, B, A, P, C> EntitlementVideoReviewGate<T, B, A, P, C> {
    pub fn new(usage: T, cancel: CancelBooking<B, A, P, C>) -> Self {
        Self { usage, cancel }
    }
}

impl<T, B, A, P, C> EntitlementVideoReviewGate<T, B, A, P, C>
where
    B: BookingRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    /// 生徒 `student_id` のまだ受けていない体験予約をキャンセル扱いで解放する。
    async fn release_trial_bookings(&self, student_id: AccountId) -> Result<(), Unavailable> {
        let bookings = self
            .cancel
            .bookings()
            .bookings_of_student(student_id)
            .await?;
        let untaken_trials = bookings
            .iter()
            .map(|(booking, _)| booking)
            .filter(|booking| booking.is_trial() && booking.status().is_active());
        for booking in untaken_trials {
            let command = CancelBookingCommand {
                student_id,
                booking_id: booking.id(),
            };
            match self.cancel.execute(command).await {
                // 読んだ後に並行にキャンセルされた予約は、もう解放されている
                Ok(_)
                | Err(CancelBookingError::BookingNotFound | CancelBookingError::Invalid(_)) => {}
                Err(CancelBookingError::Unavailable(error)) => return Err(error),
            }
        }
        Ok(())
    }
}

impl<T, B, A, P, C> VideoReviewGate for EntitlementVideoReviewGate<T, B, A, P, C>
where
    T: TrialUsageLookup,
    B: BookingRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    async fn admit_video_review(&self, student_id: AccountId) -> Result<bool, Unavailable> {
        let usage = self.usage.find_trial_usage(student_id).await?;
        if !usage.may_request_video_review() {
            return Ok(false);
        }
        if usage.releases_trial_booking_for_video_review() {
            self.release_trial_bookings(student_id).await?;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{Video, VideoId},
        catalog::Sport,
        entitlement::{
            Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan,
            YearMonth,
        },
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::SessionId,
        scheduling::{Booking, BookingCanceller, BookingStatus, Slot, SlotCapacity, SlotId},
    };

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        domain_event::DomainEvent,
        entitlement::EntitlementTrialUsageLookup,
        in_memory::{
            FixedClock, FixedTrialUsage, InMemoryBookingRepository, InMemoryEntitlementRepository,
            InMemoryLessonCompletionRepository, InMemoryLessonMenuRepository,
            InMemorySlotRepository, InMemoryVideoRepository, RecordingAnalyticsSink,
            RecordingDomainEventPublisher,
        },
        scheduling::{CreateBooking, CreateBookingCommand},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;
    const HOUR: Duration = Duration::from_secs(3600);

    type Gate = EntitlementVideoReviewGate<
        EntitlementTrialUsageLookup<
            InMemoryEntitlementRepository,
            InMemoryLessonCompletionRepository,
            InMemoryVideoRepository,
        >,
        InMemoryBookingRepository,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    >;

    /// 生徒の材料。
    #[derive(Default)]
    struct Materials {
        monthly: bool,
        completed: bool,
        video: bool,
    }

    fn slot(id: u128) -> Slot {
        let starts_at = NOW + (id as u32) * 24 * HOUR;
        Slot::publish(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            starts_at,
            starts_at + HOUR,
            SlotCapacity::ONE,
        )
        .unwrap()
    }

    /// 生徒 `STUDENT` の体験予約（枠 10）と通常の予約（枠 11）、別の生徒の体験予約（枠 12）を置いた門と、置いた予約。
    async fn gate(materials: Materials) -> (Gate, [Booking; 3]) {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(10),
            slot(11),
            slot(12),
        ]));
        let create = CreateBooking::new(
            bookings.clone(),
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            FixedTrialUsage::default(),
            RecordingAnalyticsSink::default(),
            FixedClock(NOW),
        );
        let mut booked = Vec::new();
        for (student_id, slot, is_trial) in [
            (STUDENT, 10, true),
            (STUDENT, 11, false),
            (OTHER_STUDENT, 12, true),
        ] {
            let command = CreateBookingCommand {
                student_id,
                slot_id: SlotId::from_u128(slot),
                is_trial,
            };
            booked.push(create.execute(command).await.unwrap());
        }
        let entitlements =
            InMemoryEntitlementRepository::with_entitlements(materials.monthly.then(|| {
                Entitlement::monthly(
                    EntitlementId::from_u128(1),
                    STUDENT,
                    Sport::Tennis,
                    SubscriptionId::from_u128(1),
                    LiveLessonQuota::fresh(
                        SubscriptionPlan::Standard,
                        YearMonth::new(1970, 1).unwrap(),
                    ),
                )
                .unwrap()
            }));
        let completions =
            InMemoryLessonCompletionRepository::with_completions(materials.completed.then(|| {
                LessonCompletion::restore(
                    LessonCompletionId::from_u128(1),
                    STUDENT,
                    COACH,
                    LessonSource::Live(SessionId::from_u128(1)),
                    NOW,
                    true,
                )
            }));
        let videos = InMemoryVideoRepository::with_videos(
            materials
                .video
                .then(|| Video::submit(VideoId::from_u128(1), STUDENT, COACH, Sport::Tennis, NOW)),
        );
        let gate = EntitlementVideoReviewGate::new(
            EntitlementTrialUsageLookup::new(entitlements, completions, videos),
            CancelBooking::new(
                bookings,
                RecordingAnalyticsSink::default(),
                RecordingDomainEventPublisher::default(),
                FixedClock(NOW + HOUR),
            ),
        );
        (gate, booked.try_into().unwrap())
    }

    fn bookings(gate: &Gate) -> Vec<Booking> {
        let mut all = gate.cancel.bookings().all();
        all.sort_by_key(Booking::slot_id);
        all
    }

    #[tokio::test]
    async fn first_video_review_releases_the_trial_booking_not_taken() {
        let (gate, [trial, regular, other]) = gate(Materials::default()).await;

        assert!(gate.admit_video_review(STUDENT).await.unwrap());

        let [released, kept, other_kept] = bookings(&gate).try_into().unwrap();
        assert_eq!(released.id(), trial.id());
        assert_eq!(released.status(), BookingStatus::Cancelled);
        assert_eq!(released.cancelled_by(), Some(BookingCanceller::Student));
        assert_eq!(released.cancelled_at(), Some(NOW + HOUR));
        assert_eq!(kept, regular, "通常の予約は解放しない");
        assert_eq!(other_kept, other, "別の生徒の体験予約は関係ない");
        // 生徒がキャンセルしたときと同じく、境界間イベントと行動イベントを出す
        assert!(
            matches!(
                gate.cancel.events().events().as_slice(),
                [DomainEvent::BookingCancelled { booking_id, cancelled_by: BookingCanceller::Student, .. }]
                    if *booking_id == trial.id()
            ),
            "{:?}",
            gate.cancel.events().events()
        );
        assert_eq!(gate.cancel.analytics().events().len(), 1);
        // 2 回目の問い合わせでも、もう解放する予約はない
        assert!(gate.admit_video_review(STUDENT).await.unwrap());
        assert_eq!(gate.cancel.events().events().len(), 1);
    }

    #[tokio::test]
    async fn used_trial_is_refused_without_changing_anything() {
        for materials in [
            Materials {
                completed: true,
                ..Materials::default()
            },
            Materials {
                video: true,
                ..Materials::default()
            },
        ] {
            let (gate, booked) = gate(materials).await;
            assert!(!gate.admit_video_review(STUDENT).await.unwrap());
            assert_eq!(bookings(&gate), booked.to_vec(), "断るときは予約を変えない");
            assert!(gate.cancel.events().events().is_empty());
            assert!(gate.cancel.analytics().events().is_empty());
        }
    }

    #[tokio::test]
    async fn monthly_students_are_admitted_without_releasing_bookings() {
        let (gate, booked) = gate(Materials {
            monthly: true,
            completed: true,
            video: true,
        })
        .await;
        for _ in 0..3 {
            assert!(gate.admit_video_review(STUDENT).await.unwrap());
        }
        assert_eq!(bookings(&gate), booked.to_vec());
        assert!(gate.cancel.events().events().is_empty());
    }
}
