use std::collections::BTreeSet;

use domain::scheduling::{BookingCanceller, CancellationDecision, CoachLeave, CoachLeaveRequest};
use uuid::Uuid;

use super::{
    CoachLeaveError, CoachLeaveRepository, SlotCoachAccountLookup, StartCoachLeaveCommand,
    calendar_day,
};
use crate::{
    Unavailable,
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
};

/// 本部がコーチをお休みにするユースケース（admin-web の `startCoachLeave`）。誰が呼べるか（本部だけ）は境界（interface）が
/// 確かめる。
///
/// コーチがいるかを profile に問い合わせ（[`SlotCoachAccountLookup`]。いなければ [`CoachLeaveError::CoachNotFound`]）、
/// 期間（[`calendar_day::days`]）の枠と予約とそのコーチのお休みの記録を読んで、domain の [`CoachLeave::start`] で期間の予約を
/// すべて本部のキャンセルにし、枠をすべて取り下げ、お休みの記録とともに 1 つにまとめて保存する（全部か何もしないか。
/// ここでは判定しない）。保存したら、キャンセルした予約ごとに境界間イベント `BookingCancelled`（`cancelled_by` は本部。回数の
/// 判定はキャンセル規定のとおり消費しない）を発行する。entitlement は確定で使った枠を戻し、notification は予約した生徒に
/// 知らせる。拒否・保存の失敗では何も発行しない。
///
/// 行動イベント（`booking.cancelled`・`slot.withdrawn`）は発火しない（actor が生徒・コーチに決まっているため。本部の運営の
/// 操作で、`suspendCoach` と同じ扱い）。
pub struct StartCoachLeave<B, A, P, C> {
    bookings: B,
    coaches: A,
    events: P,
    clock: C,
}

impl<B, A, P, C> StartCoachLeave<B, A, P, C>
where
    B: CoachLeaveRepository,
    A: SlotCoachAccountLookup,
    P: DomainEventPublisher,
    C: Clock,
{
    pub fn new(bookings: B, coaches: A, events: P, clock: C) -> Self {
        Self {
            bookings,
            coaches,
            events,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: StartCoachLeaveCommand,
    ) -> Result<CoachLeave, CoachLeaveError> {
        let coaches = self
            .coaches
            .find_slot_coach_accounts(&BTreeSet::from([command.coach_id]))
            .await?;
        if !coaches.contains_key(&command.coach_id) {
            return Err(CoachLeaveError::CoachNotFound);
        }
        let period = calendar_day::days(command.from, command.to)?;
        let now = self.clock.now();
        let request = CoachLeaveRequest {
            coach_id: command.coach_id,
            period,
            note: command.note,
            requested_by: command.requested_by,
            expected_booking_ids: command.expected_booking_ids,
        };
        let id = self.bookings.next_coach_leave_id();
        let mut decisions: Vec<CancellationDecision> = Vec::new();
        let leave = self
            .bookings
            .start_coach_leave(command.coach_id, period, |slots, bookings, existing| {
                let (leave, made) =
                    CoachLeave::start(id, &request, now, slots, bookings, existing)?;
                decisions = made;
                Ok(leave)
            })
            .await?;
        if decisions.len() != leave.cancelled_bookings().len() {
            return Err(CoachLeaveError::Unavailable(Unavailable::new(
                "保存先がお休みを適用せずに成功を返した",
            )));
        }

        for (booking, decision) in leave.cancelled_bookings().iter().zip(decisions) {
            self.events
                .publish(DomainEvent::BookingCancelled {
                    event_id: Uuid::new_v4(),
                    occurred_at: now,
                    student_id: booking.student_id(),
                    coach_id: leave.coach_id(),
                    booking_id: booking.booking_id(),
                    consumes_quota: decision.consumes_quota,
                    cancelled_at: now,
                    cancelled_by: BookingCanceller::Headquarters,
                })
                .await;
        }
        Ok(leave)
    }

    pub fn bookings(&self) -> &B {
        &self.bookings
    }

    pub fn events(&self) -> &P {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, AccountId, Email, Role},
        profile::DisplayName,
        scheduling::{
            BookingId, BookingStatus, CoachLeaveNote, SchedulingError, Slot, SlotCapacity, SlotId,
            SlotStatus,
        },
    };
    use time::{Date, Month};

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        entitlement::EntitlementQuotaGate,
        identity::AccountRepository,
        in_memory::InMemoryEntitlementRepository,
        in_memory::{
            FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
            InMemoryLessonMenuRepository, InMemorySlotRepository, RecordingAnalyticsSink,
            RecordingDomainEventPublisher,
        },
        profile::ProfileCoachAccountLookup,
        scheduling::{
            BookingRepository, ConfirmBooking, ConfirmBookingCommand, CreateBooking,
            CreateBookingCommand,
        },
    };

    type UseCase = StartCoachLeave<
        InMemoryBookingRepository,
        ProfileCoachAccountLookup<InMemoryAccountRepository>,
        RecordingDomainEventPublisher,
        FixedClock,
    >;

    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const HEADQUARTERS: AccountId = AccountId::from_u128(9);
    const HOUR: Duration = Duration::from_secs(3600);

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    /// 2026-11-05 12:00（日本時間）。今日は 11/5
    fn now() -> SystemTime {
        calendar_day::start(nov(5)) + 12 * HOUR
    }

    fn account(id: AccountId, role: Role) -> Account {
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new("山田 健").unwrap(),
            role,
            [Sport::Tennis],
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn slot(id: u128, day: u8, hour: u32) -> Slot {
        let starts_at = calendar_day::start(nov(day)) + hour * HOUR;
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

    /// 11/10 19 時（確定済みの予約）・11/12 20 時（確定待ちの予約）・11/13 19 時（予約なし）・11/20 10 時（期間の外の確定済み）の
    /// 枠を置き、予約を作った保存先と、予約の ID（11/10・11/12・11/20 の順）。
    async fn bookings() -> (InMemoryBookingRepository, Vec<BookingId>) {
        let slots = InMemorySlotRepository::with_slots([
            slot(10, 10, 19),
            slot(11, 12, 20),
            slot(12, 13, 19),
            slot(13, 20, 10),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        let mut ids = Vec::new();
        for (slot, student, confirm) in [
            (10, STUDENT, true),
            (11, OTHER_STUDENT, false),
            (13, STUDENT, true),
        ] {
            let booking = CreateBooking::new(
                bookings.clone(),
                CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
                RecordingAnalyticsSink::default(),
                FixedClock(SystemTime::UNIX_EPOCH),
            )
            .execute(CreateBookingCommand {
                student_id: student,
                slot_id: SlotId::from_u128(slot),
                is_trial: false,
            })
            .await
            .unwrap();
            if confirm {
                ConfirmBooking::new(
                    bookings.clone(),
                    EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
                    RecordingAnalyticsSink::default(),
                    RecordingDomainEventPublisher::default(),
                    FixedClock(SystemTime::UNIX_EPOCH),
                )
                .execute(ConfirmBookingCommand {
                    student_id: student,
                    booking_id: booking.id(),
                })
                .await
                .unwrap();
            }
            ids.push(booking.id());
        }
        (bookings, ids)
    }

    async fn use_case(bookings: InMemoryBookingRepository) -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for (id, role) in [(COACH, Role::Coach), (STUDENT, Role::Student)] {
            accounts.insert(account(id, role)).await.unwrap();
        }
        StartCoachLeave::new(
            bookings,
            ProfileCoachAccountLookup::new(accounts),
            RecordingDomainEventPublisher::default(),
            FixedClock(now()),
        )
    }

    fn command(
        coach_id: AccountId,
        from: Date,
        to: Date,
        expected: &[BookingId],
    ) -> StartCoachLeaveCommand {
        StartCoachLeaveCommand {
            coach_id,
            from,
            to,
            note: CoachLeaveNote::new("ご迷惑をおかけします。").unwrap(),
            requested_by: HEADQUARTERS,
            expected_booking_ids: expected.iter().copied().collect(),
        }
    }

    async fn status_of(bookings: &InMemoryBookingRepository, id: BookingId) -> BookingStatus {
        bookings.find(id).await.unwrap().unwrap().status()
    }

    #[tokio::test]
    async fn cancels_the_bookings_withdraws_the_slots_and_publishes_one_event_per_booking() {
        let (bookings, ids) = bookings().await;
        let use_case = use_case(bookings.clone()).await;

        let leave = use_case
            .execute(command(COACH, nov(10), nov(16), &ids[..2]))
            .await
            .unwrap();

        assert_eq!(leave.cancelled_bookings().len(), 2);
        assert_eq!(leave.withdrawn_slot_count(), 3);
        assert_eq!(leave.notified_student_count(), 2);
        assert_eq!(leave.created_by(), HEADQUARTERS);
        assert_eq!(calendar_day::dates_of(leave.period()), (nov(10), nov(16)));
        assert_eq!(bookings.coach_leaves(), vec![leave.clone()]);
        assert_eq!(status_of(&bookings, ids[0]).await, BookingStatus::Cancelled);
        assert_eq!(status_of(&bookings, ids[1]).await, BookingStatus::Cancelled);
        assert_eq!(
            status_of(&bookings, ids[2]).await,
            BookingStatus::Confirmed,
            "期間の外"
        );
        let statuses: Vec<_> = bookings.slots().all().iter().map(Slot::status).collect();
        assert_eq!(
            statuses,
            vec![
                SlotStatus::Closed,
                SlotStatus::Closed,
                SlotStatus::Closed,
                SlotStatus::Booked
            ]
        );

        let events = use_case.events().events();
        let published: Vec<_> = events
            .iter()
            .map(|event| match event {
                DomainEvent::BookingCancelled {
                    student_id,
                    coach_id,
                    booking_id,
                    consumes_quota,
                    cancelled_at,
                    cancelled_by,
                    ..
                } => (
                    *student_id,
                    *coach_id,
                    *booking_id,
                    *consumes_quota,
                    *cancelled_at,
                    *cancelled_by,
                ),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            published,
            vec![
                (
                    STUDENT,
                    COACH,
                    ids[0],
                    false,
                    now(),
                    BookingCanceller::Headquarters
                ),
                (
                    OTHER_STUDENT,
                    COACH,
                    ids[1],
                    false,
                    now(),
                    BookingCanceller::Headquarters
                ),
            ]
        );
    }

    #[tokio::test]
    async fn the_second_leave_for_the_same_period_is_rejected() {
        let (bookings, ids) = bookings().await;
        let use_case = use_case(bookings.clone()).await;
        use_case
            .execute(command(COACH, nov(10), nov(16), &ids[..2]))
            .await
            .unwrap();

        let again = use_case
            .execute(command(COACH, nov(10), nov(16), &[]))
            .await;

        assert!(matches!(
            again,
            Err(CoachLeaveError::Invalid(
                SchedulingError::CoachLeaveOverlaps
            ))
        ));
        assert_eq!(bookings.coach_leaves().len(), 1);
        assert_eq!(
            use_case.events().events().len(),
            2,
            "2 回目は何も発行しない"
        );
    }

    #[tokio::test]
    async fn rejections_change_nothing_and_publish_nothing() {
        for (command, expected) in [
            (
                command(COACH, nov(10), nov(16), &[]),
                "CoachLeavePreviewOutdated",
            ),
            (
                command(COACH, nov(5), nov(6), &[]),
                "CoachLeaveStartsTooSoon",
            ),
            (
                command(COACH, nov(10), nov(9), &[]),
                "BookingPeriodEndsBeforeItStarts",
            ),
            (command(STUDENT, nov(10), nov(16), &[]), "CoachNotFound"),
        ] {
            let (bookings, ids) = bookings().await;
            let use_case = use_case(bookings.clone()).await;
            let error = use_case.execute(command).await.unwrap_err();
            let name = match error {
                CoachLeaveError::CoachNotFound => "CoachNotFound".to_string(),
                CoachLeaveError::Invalid(error) => format!("{error:?}"),
                CoachLeaveError::Unavailable(error) => panic!("{error}"),
            };
            assert_eq!(name, expected);
            assert!(bookings.coach_leaves().is_empty());
            assert_eq!(status_of(&bookings, ids[0]).await, BookingStatus::Confirmed);
            assert_eq!(status_of(&bookings, ids[1]).await, BookingStatus::Pending);
            assert!(use_case.events().events().is_empty());
        }
    }
}
