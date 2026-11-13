use std::collections::BTreeSet;

use domain::scheduling::{BookingId, BookingStatus};

use super::{
    BookingCoachDisplayNameLookup, BookingDetail, BookingRepository, CoachLeaveRepository,
    StudentDisplayNameLookup,
};
use crate::Unavailable;

/// 本部が見る予約 1 件の詳細のユースケース（admin-web の `getBooking`。本部だけが呼ぶ）。
///
/// 予約（状態を問わない）と予約したスロットを読み、コーチと生徒の表示名を profile に問い合わせて添える。キャンセル済みの
/// 予約は、それをキャンセルしたコーチのお休みの記録（[`CoachLeaveRepository::coach_leaves_of_bookings`]）も読んで添え、
/// 予約に戻す（[`domain::scheduling::Booking::with_restored_coach_leave`]。理由を残し始める前にお休みでキャンセルした予約も
/// 理由が分かる）。予約がなければ [`GetBookingError::NotFound`]。読むだけで何も変えない。
pub struct GetBooking<B, K, D> {
    bookings: B,
    coaches: K,
    students: D,
}

#[derive(Debug)]
pub enum GetBookingError {
    /// 予約がない
    NotFound,
    /// 保存先の障害（予約のスロットが保存されていないときも）
    Unavailable(Unavailable),
}

impl From<Unavailable> for GetBookingError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

impl<B, K, D> GetBooking<B, K, D>
where
    B: BookingRepository + CoachLeaveRepository,
    K: BookingCoachDisplayNameLookup,
    D: StudentDisplayNameLookup,
{
    pub fn new(bookings: B, coaches: K, students: D) -> Self {
        Self {
            bookings,
            coaches,
            students,
        }
    }

    pub async fn execute(&self, booking_id: BookingId) -> Result<BookingDetail, GetBookingError> {
        let booking = self
            .bookings
            .find(booking_id)
            .await?
            .ok_or(GetBookingError::NotFound)?;
        let coach_leave = if booking.status() == BookingStatus::Cancelled {
            self.bookings
                .coach_leaves_of_bookings(&BTreeSet::from([booking_id]))
                .await?
                .into_iter()
                .next()
        } else {
            None
        };
        let booking = match &coach_leave {
            Some(leave) => booking.with_restored_coach_leave(leave),
            None => booking,
        };
        let slot = self
            .bookings
            .find_slot(booking.slot_id())
            .await?
            .ok_or_else(|| Unavailable::new("予約のスロットが保存されていない"))?;
        let coach_display_name = self
            .coaches
            .find_coach_display_names(&BTreeSet::from([booking.coach_id()]))
            .await?
            .remove(&booking.coach_id());
        let student_display_name = self
            .students
            .find_student_display_names(&BTreeSet::from([booking.student_id()]))
            .await?
            .remove(&booking.student_id());
        Ok(BookingDetail {
            booking,
            slot,
            coach_leave,
            coach_display_name,
            student_display_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    };

    use domain::{
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuId, LessonMenuName, Sport,
        },
        identity::AccountId,
        profile::DisplayName,
        scheduling::{
            Booking, BookingCancelReason, BookingCanceller, BookingStatus, CancellationDecision,
            CoachLeave, CoachLeaveId, CoachLeaveNote, CoachLeaveRequest, Slot, SlotId, SlotSeats,
            SlotStatus,
        },
    };

    use super::*;
    use crate::{
        in_memory::{InMemoryBookingRepository, InMemorySlotRepository},
        scheduling::calendar_day,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const START: SystemTime = SystemTime::UNIX_EPOCH;
    const HOUR: Duration = Duration::from_secs(3600);

    fn name(value: &str) -> DisplayName {
        DisplayName::new(value).unwrap()
    }

    fn slot(id: u128) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            START + 48 * HOUR,
            START + 49 * HOUR,
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }

    fn menu() -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(9),
            COACH,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new("フォアの基本").unwrap(),
                description: LessonMenuDescription::default(),
            },
            true,
            START,
        )
    }

    /// `known` のアカウントの表示名だけを知っている。
    #[derive(Clone, Default)]
    struct Names {
        known: BTreeMap<AccountId, DisplayName>,
    }

    impl BookingCoachDisplayNameLookup for Names {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(coach_ids
                .iter()
                .filter_map(|id| Some((*id, self.known.get(id)?.clone())))
                .collect())
        }
    }

    impl StudentDisplayNameLookup for Names {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            Ok(student_ids
                .iter()
                .filter_map(|id| Some((*id, self.known.get(id)?.clone())))
                .collect())
        }
    }

    async fn book(bookings: &InMemoryBookingRepository, slot: u128, is_trial: bool) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), STUDENT, |slot, existing| {
                Booking::reserve(bookings.next_id(), STUDENT, is_trial, slot, existing, START)
                    .map(|booking| booking.with_lesson_menu(slot, Some(&menu())))
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn returns_the_booking_its_slot_and_display_names() {
        let with_menu = slot(1)
            .with_lesson_menu(&menu())
            .expect("枠に結びつけられるメニュー");
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([with_menu]));
        let booked = book(&bookings, 1, true).await;
        bookings
            .update(booked.id(), |booking, slot| {
                booking.confirm(STUDENT, slot, START + HOUR)
            })
            .await
            .unwrap();
        bookings
            .update(booked.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, START + 2 * HOUR).map(|_| ())
            })
            .await
            .unwrap();
        let names = Names {
            known: BTreeMap::from([(COACH, name("たなか")), (STUDENT, name("あおき"))]),
        };

        let detail = GetBooking::new(bookings, names.clone(), names)
            .execute(booked.id())
            .await
            .unwrap();
        assert_eq!(detail.booking.id(), booked.id());
        assert!(detail.booking.is_trial());
        assert_eq!(detail.booking.status(), BookingStatus::Cancelled);
        assert_eq!(
            detail
                .booking
                .lesson_menu()
                .map(|menu| menu.name().as_str()),
            Some("フォアの基本")
        );
        assert_eq!(detail.slot.id(), SlotId::from_u128(1));
        assert_eq!(
            (detail.slot.starts_at(), detail.slot.ends_at()),
            (START + 48 * HOUR, START + 49 * HOUR)
        );
        assert_eq!(detail.coach_display_name, Some(name("たなか")));
        assert_eq!(detail.student_display_name, Some(name("あおき")));
        assert_eq!(
            detail
                .booking
                .status_changes()
                .iter()
                .map(|change| (change.status(), change.changed_at()))
                .collect::<Vec<_>>(),
            [
                (BookingStatus::Confirmed, START + HOUR),
                (BookingStatus::Cancelled, START + 2 * HOUR),
            ]
        );
        let cancelled = detail.booking.status_changes()[1];
        assert_eq!(cancelled.cancelled_by(), Some(BookingCanceller::Student));
        assert_eq!(
            cancelled.cancel_reason(),
            None,
            "生徒のキャンセルは理由を残さない"
        );
        assert_eq!(detail.coach_leave, None);
    }

    #[tokio::test]
    async fn a_booking_cancelled_by_a_coach_leave_comes_with_the_leave() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1)]));
        let booked = book(&bookings, 1, false).await;
        let headquarters = AccountId::from_u128(9);
        // 枠（3 日目）の日だけのお休み。今は 1 日目
        let day = calendar_day::of(START + 48 * HOUR);
        let period = calendar_day::days(day, day).unwrap();
        let request = CoachLeaveRequest {
            coach_id: COACH,
            period,
            note: CoachLeaveNote::new("ご迷惑をおかけします。").unwrap(),
            requested_by: headquarters,
            expected_booking_ids: BTreeSet::from([booked.id()]),
        };
        let leave = bookings
            .start_coach_leave(COACH, period, |slots, existing, leaves| {
                CoachLeave::start(
                    CoachLeaveId::from_u128(5),
                    &request,
                    START,
                    slots,
                    existing,
                    leaves,
                )
                .map(|(leave, _)| leave)
            })
            .await
            .unwrap();

        let detail = GetBooking::new(bookings, Names::default(), Names::default())
            .execute(booked.id())
            .await
            .unwrap();
        assert_eq!(detail.coach_leave, Some(leave));
        let changes = detail.booking.status_changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].changed_at(), START);
        assert_eq!(
            changes[0].cancelled_by(),
            Some(BookingCanceller::Headquarters)
        );
        assert_eq!(
            changes[0].cancel_reason(),
            Some(BookingCancelReason::CoachLeave)
        );
        assert_eq!(
            changes[0].cancellation_decision(),
            Some(CancellationDecision {
                consumes_quota: false
            })
        );
    }

    #[tokio::test]
    async fn display_names_that_cannot_be_found_are_left_out() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1)]));
        let booked = book(&bookings, 1, false).await;

        let detail = GetBooking::new(bookings, Names::default(), Names::default())
            .execute(booked.id())
            .await
            .unwrap();
        assert_eq!(detail.booking.status(), BookingStatus::Pending);
        assert!(detail.booking.lesson_menu().is_none(), "枠にメニューがない");
        assert!(detail.booking.status_changes().is_empty());
        assert_eq!(detail.coach_display_name, None);
        assert_eq!(detail.student_display_name, None);
    }

    #[tokio::test]
    async fn unknown_booking_is_not_found() {
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(1)]));
        book(&bookings, 1, false).await;

        let error = GetBooking::new(bookings, Names::default(), Names::default())
            .execute(BookingId::from_u128(99))
            .await
            .unwrap_err();
        assert!(matches!(error, GetBookingError::NotFound), "{error:?}");
    }
}
