use std::collections::BTreeSet;

use domain::{
    identity::AccountId,
    scheduling::{CoachLeave, CoachLeaveTargets},
};
use time::Date;

use super::{
    BookingRepository, CoachLeaveError, CoachLeavePreview, CoachLeaveRepository, LessonMenuLookup,
    SlotCoachAccountLookup, StudentDisplayNameLookup, calendar_day,
};
use crate::clock::Clock;

/// 本部が見る、コーチのお休みの期間の予約と枠のユースケース（admin-web の `getCoachLeavePreview`。照会だけで、何も保存しない）。
/// 誰が呼べるか（本部だけ）は境界（interface）が確かめる。
///
/// 期間は日付（日本時間）の `from` から `to` まで（両端を含む。[`calendar_day::days`]。31 日まで）で、始まりは明日以降
/// （[`CoachLeave::ensure_starts_after`]）。コーチがいるかとその表示名・提供スポーツは profile に問い合わせ
/// （[`SlotCoachAccountLookup`]。コーチでないアカウントとないアカウントは [`CoachLeaveError::CoachNotFound`]）、期間の枠と予約を
/// 読んで、片づけるもの（[`CoachLeaveTargets`]。お休みにする操作と同じ選び方）を返す。予約した生徒の表示名と、予約のない枠に
/// 結びついたメニューを添える。行動イベントは発火しない。
pub struct PreviewCoachLeave<B, A, D, M, C> {
    bookings: B,
    coaches: A,
    students: D,
    lesson_menus: M,
    clock: C,
}

impl<B, A, D, M, C> PreviewCoachLeave<B, A, D, M, C>
where
    B: BookingRepository + CoachLeaveRepository,
    A: SlotCoachAccountLookup,
    D: StudentDisplayNameLookup,
    M: LessonMenuLookup,
    C: Clock,
{
    pub fn new(bookings: B, coaches: A, students: D, lesson_menus: M, clock: C) -> Self {
        Self {
            bookings,
            coaches,
            students,
            lesson_menus,
            clock,
        }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
        from: Date,
        to: Date,
    ) -> Result<CoachLeavePreview, CoachLeaveError> {
        let coach = self
            .coaches
            .find_slot_coach_accounts(&BTreeSet::from([coach_id]))
            .await?
            .remove(&coach_id)
            .ok_or(CoachLeaveError::CoachNotFound)?;
        let period = calendar_day::days(from, to)?;
        CoachLeave::ensure_starts_after(period, self.clock.now())?;
        let acceptance = self.bookings.find_coach_acceptance(coach_id).await?;
        let (slots, bookings) = self
            .bookings
            .coach_slots_starting_in(coach_id, period)
            .await?;
        let targets = CoachLeaveTargets::of(coach_id, period, &slots, &bookings);

        let student_ids: BTreeSet<_> = targets
            .bookings()
            .iter()
            .map(|(booking, _)| booking.student_id())
            .collect();
        let students = if student_ids.is_empty() {
            Default::default()
        } else {
            self.students
                .find_student_display_names(&student_ids)
                .await?
        };
        let menu_ids: BTreeSet<_> = targets
            .open_slots()
            .iter()
            .filter_map(|slot| slot.lesson_menu_id())
            .collect();
        let lesson_menus = if menu_ids.is_empty() {
            Default::default()
        } else {
            self.lesson_menus.find_lesson_menus(&menu_ids).await?
        };
        Ok(CoachLeavePreview {
            coach,
            acceptance_status: acceptance.status(),
            from,
            to,
            targets,
            students,
            lesson_menus,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::{Account, Email, Role},
        profile::DisplayName,
        scheduling::{CoachAcceptanceStatus, SchedulingError, Slot, SlotCapacity, SlotId},
    };
    use time::Month;

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        identity::AccountRepository,
        in_memory::{
            FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
            InMemoryLessonMenuRepository, InMemorySlotRepository, RecordingAnalyticsSink,
        },
        profile::{ProfileCoachAccountLookup, ProfileStudentDisplayNameLookup},
        scheduling::{CreateBooking, CreateBookingCommand},
    };

    type UseCase = PreviewCoachLeave<
        InMemoryBookingRepository,
        ProfileCoachAccountLookup<InMemoryAccountRepository>,
        ProfileStudentDisplayNameLookup<InMemoryAccountRepository>,
        CatalogLessonMenuLookup<InMemoryLessonMenuRepository>,
        FixedClock,
    >;

    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const HOUR: Duration = Duration::from_secs(3600);

    fn nov(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
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

    /// 11/10 19 時（佐藤 花子の予約）・11/13 19 時（予約なし）・11/20 10 時（期間の外）の枠。今は 11/5 12 時。
    async fn use_case() -> UseCase {
        let accounts = InMemoryAccountRepository::default();
        for (id, role, name) in [
            (COACH, Role::Coach, "山田 健"),
            (STUDENT, Role::Student, "佐藤 花子"),
        ] {
            accounts
                .insert(
                    Account::new(
                        id,
                        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
                        DisplayName::new(name).unwrap(),
                        role,
                        [Sport::Tennis],
                        SystemTime::UNIX_EPOCH,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
        }
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(10, 10, 19),
            slot(11, 13, 19),
            slot(12, 20, 10),
        ]));
        CreateBooking::new(
            bookings.clone(),
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            RecordingAnalyticsSink::default(),
            FixedClock(SystemTime::UNIX_EPOCH),
        )
        .execute(CreateBookingCommand {
            student_id: STUDENT,
            slot_id: SlotId::from_u128(10),
            is_trial: true,
        })
        .await
        .unwrap();
        PreviewCoachLeave::new(
            bookings,
            ProfileCoachAccountLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
            CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::default()),
            FixedClock(calendar_day::start(nov(5)) + 12 * HOUR),
        )
    }

    #[tokio::test]
    async fn lists_the_bookings_and_open_slots_of_the_period() {
        let preview = use_case()
            .await
            .execute(COACH, nov(10), nov(16))
            .await
            .unwrap();

        assert_eq!(preview.coach.display_name().as_str(), "山田 健");
        assert_eq!(preview.acceptance_status, CoachAcceptanceStatus::Accepting);
        assert_eq!((preview.from, preview.to), (nov(10), nov(16)));
        let booked: Vec<_> = preview
            .targets
            .bookings()
            .iter()
            .map(|(booking, slot)| (booking.student_id(), slot.id().as_u128()))
            .collect();
        assert_eq!(booked, vec![(STUDENT, 10)]);
        let open: Vec<_> = preview
            .targets
            .open_slots()
            .iter()
            .map(|s| s.id().as_u128())
            .collect();
        assert_eq!(open, vec![11]);
        assert_eq!(preview.targets.slot_count(), 2);
        assert_eq!(preview.students[&STUDENT].as_str(), "佐藤 花子");
        assert!(preview.lesson_menus.is_empty());
    }

    #[tokio::test]
    async fn rejects_a_period_from_today_and_unknown_coaches() {
        let use_case = use_case().await;
        assert!(matches!(
            use_case.execute(COACH, nov(5), nov(6)).await,
            Err(CoachLeaveError::Invalid(
                SchedulingError::CoachLeaveStartsTooSoon
            ))
        ));
        assert!(
            use_case.execute(COACH, nov(6), nov(6)).await.is_ok(),
            "明日からは確かめられる"
        );
        assert!(matches!(
            use_case
                .execute(
                    COACH,
                    nov(10),
                    Date::from_calendar_date(2026, Month::December, 11).unwrap()
                )
                .await,
            Err(CoachLeaveError::Invalid(
                SchedulingError::BookingPeriodTooLong { max_days: 31 }
            ))
        ));
        assert!(matches!(
            use_case.execute(STUDENT, nov(10), nov(16)).await,
            Err(CoachLeaveError::CoachNotFound)
        ));
    }
}
