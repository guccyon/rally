use std::collections::BTreeSet;

use domain::identity::AccountId;

use super::{
    AccountRepository, StudentActivity, StudentActivityCoachDisplayNameLookup,
    StudentBookingActivityLookup, StudentLessonHistoryLookup,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る生徒 1 人の利用の様子のユースケース（admin-web の `getStudentActivity`。本部だけが呼ぶ）。照会だけで、
/// 何も保存しない。
///
/// 生徒のアカウントの表示名に、これからの予約とキャンセルした予約の件数を scheduling に
/// （[`StudentBookingActivityLookup`]。今は [`Clock`] から読む）、直近の受けたレッスンを完了記録に
/// （[`StudentLessonHistoryLookup`]。最大 [`Self::RECENT_LESSON_LIMIT`] 件）問い合わせて添え、予約とレッスンの
/// コーチの表示名を profile に問い合わせて（[`StudentActivityCoachDisplayNameLookup`]）添える。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
pub struct GetStudentActivity<R, B, L, D, C> {
    accounts: R,
    bookings: B,
    lessons: L,
    coaches: D,
    clock: C,
}

#[derive(Debug)]
pub enum GetStudentActivityError {
    /// 生徒のアカウントがない（ID のアカウントがないか、生徒でない）
    StudentNotFound,
    /// 保存先の障害
    Unavailable(Unavailable),
}

impl From<Unavailable> for GetStudentActivityError {
    fn from(error: Unavailable) -> Self {
        Self::Unavailable(error)
    }
}

impl<R, B, L, D, C> GetStudentActivity<R, B, L, D, C>
where
    R: AccountRepository,
    B: StudentBookingActivityLookup,
    L: StudentLessonHistoryLookup,
    D: StudentActivityCoachDisplayNameLookup,
    C: Clock,
{
    /// 直近の受けたレッスンの件数の上限（admin-web の契約の `StudentActivity.recent_lessons` の `maxItems`）。
    pub const RECENT_LESSON_LIMIT: usize = 20;

    pub fn new(accounts: R, bookings: B, lessons: L, coaches: D, clock: C) -> Self {
        Self {
            accounts,
            bookings,
            lessons,
            coaches,
            clock,
        }
    }

    pub async fn execute(
        &self,
        student_id: AccountId,
    ) -> Result<StudentActivity, GetStudentActivityError> {
        let account = self
            .accounts
            .find(student_id)
            .await?
            .filter(|account| account.as_student().is_some())
            .ok_or(GetStudentActivityError::StudentNotFound)?;
        let bookings = self
            .bookings
            .find_booking_activity(student_id, self.clock.now())
            .await?;
        let recent_lessons = self
            .lessons
            .find_recent_lessons(student_id, Self::RECENT_LESSON_LIMIT)
            .await?;

        let coach_ids: BTreeSet<AccountId> = bookings
            .upcoming
            .iter()
            .map(|booking| booking.coach_id())
            .chain(recent_lessons.iter().map(|lesson| lesson.coach_id()))
            .collect();
        let coaches = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.coaches.find_coach_display_names(&coach_ids).await?
        };
        Ok(StudentActivity {
            student_id,
            display_name: account.display_name().clone(),
            upcoming_bookings: bookings.upcoming,
            recent_lessons,
            cancelled_booking_count: bookings.cancelled_count,
            coaches,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        async_lesson::VideoId,
        catalog::Sport,
        identity::{Account, Email, Role},
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        profile::DisplayName,
        scheduling::{Booking, BookingId, BookingStatus, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::{
        identity::StudentBookingActivity,
        in_memory::{InMemoryAccountRepository, ManualClock},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    /// 表示名が引けないコーチ
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + hour * 3600)
    }

    fn account(id: AccountId, role: Role) -> Account {
        let sports: &[Sport] = match role {
            Role::Headquarters => &[],
            _ => &[Sport::Tennis],
        };
        Account::new(
            id,
            Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            DisplayName::new(format!("なまえ{}", id.as_u128())).unwrap(),
            role,
            sports.iter().copied(),
            at(0),
        )
        .unwrap()
    }

    fn booking(id: u128, coach: AccountId, hour: u64) -> Booking {
        let slot = Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap();
        Booking::restore(
            BookingId::from_u128(id),
            STUDENT,
            &slot,
            false,
            BookingStatus::Pending,
            at(1),
            None,
            None,
        )
    }

    fn lesson(id: u128, coach: AccountId, hour: u64) -> LessonCompletion {
        LessonCompletion::restore(
            LessonCompletionId::from_u128(id),
            STUDENT,
            coach,
            LessonSource::Async(VideoId::from_u128(id)),
            at(hour),
            false,
        )
    }

    /// 問い合わせ（生徒と時刻）を記録し、決まった予約の様子を返す。
    #[derive(Clone, Default)]
    struct FakeBookings {
        activity: StudentBookingActivity,
        asked: Arc<Mutex<Vec<(AccountId, SystemTime)>>>,
    }

    impl StudentBookingActivityLookup for FakeBookings {
        async fn find_booking_activity(
            &self,
            student_id: AccountId,
            now: SystemTime,
        ) -> Result<StudentBookingActivity, Unavailable> {
            self.asked.lock().unwrap().push((student_id, now));
            Ok(self.activity.clone())
        }
    }

    /// 問い合わせ（生徒と件数の上限）を記録し、決まったレッスンを返す。
    #[derive(Clone, Default)]
    struct FakeLessons {
        lessons: Vec<LessonCompletion>,
        asked: Arc<Mutex<Vec<(AccountId, usize)>>>,
    }

    impl StudentLessonHistoryLookup for FakeLessons {
        async fn find_recent_lessons(
            &self,
            student_id: AccountId,
            limit: usize,
        ) -> Result<Vec<LessonCompletion>, Unavailable> {
            self.asked.lock().unwrap().push((student_id, limit));
            Ok(self.lessons.clone())
        }
    }

    /// `COACH` の表示名だけを知っていて（`OTHER_COACH` は引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingCoaches {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl StudentActivityCoachDisplayNameLookup for RecordingCoaches {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, DisplayName::new("たなか").unwrap()))
                .collect())
        }
    }

    /// 取り出せない保存先の偽物。
    struct Broken;

    impl StudentLessonHistoryLookup for Broken {
        async fn find_recent_lessons(
            &self,
            _student_id: AccountId,
            _limit: usize,
        ) -> Result<Vec<LessonCompletion>, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }
    }

    async fn accounts() -> InMemoryAccountRepository {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(STUDENT, Role::Student),
            account(AccountId::from_u128(2), Role::Headquarters),
            account(COACH, Role::Coach),
        ] {
            accounts.insert(account).await.unwrap();
        }
        accounts
    }

    #[tokio::test]
    async fn returns_display_name_bookings_lessons_and_coach_names() {
        let bookings = FakeBookings {
            activity: StudentBookingActivity {
                upcoming: vec![booking(20, COACH, 100)],
                cancelled_count: 3,
            },
            ..Default::default()
        };
        let lessons = FakeLessons {
            lessons: vec![lesson(30, OTHER_COACH, 50), lesson(31, COACH, 40)],
            ..Default::default()
        };
        let coaches = RecordingCoaches::default();
        let use_case = GetStudentActivity::new(
            accounts().await,
            bookings.clone(),
            lessons.clone(),
            coaches.clone(),
            ManualClock::new(at(60)),
        );

        let activity = use_case.execute(STUDENT).await.unwrap();
        assert_eq!(
            activity,
            StudentActivity {
                student_id: STUDENT,
                display_name: DisplayName::new("なまえ1").unwrap(),
                upcoming_bookings: vec![booking(20, COACH, 100)],
                recent_lessons: vec![lesson(30, OTHER_COACH, 50), lesson(31, COACH, 40)],
                cancelled_booking_count: 3,
                coaches: BTreeMap::from([(COACH, DisplayName::new("たなか").unwrap())]),
            }
        );
        assert_eq!(
            *bookings.asked.lock().unwrap(),
            [(STUDENT, at(60))],
            "これからの予約は今の時刻で問い合わせる"
        );
        assert_eq!(
            *lessons.asked.lock().unwrap(),
            [(STUDENT, 20)],
            "直近の受けたレッスンは上限の件数で問い合わせる"
        );
        assert_eq!(
            *coaches.asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "予約とレッスンのコーチをまとめて 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn student_without_bookings_or_lessons_does_not_ask_coach_names() {
        let coaches = RecordingCoaches::default();
        let use_case = GetStudentActivity::new(
            accounts().await,
            FakeBookings::default(),
            FakeLessons::default(),
            coaches.clone(),
            ManualClock::new(at(60)),
        );

        let activity = use_case.execute(STUDENT).await.unwrap();
        assert!(activity.upcoming_bookings.is_empty());
        assert!(activity.recent_lessons.is_empty());
        assert_eq!(activity.cancelled_booking_count, 0);
        assert!(activity.coaches.is_empty());
        assert!(coaches.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn missing_account_and_non_student_are_not_found_without_asking_other_contexts() {
        let bookings = FakeBookings::default();
        let lessons = FakeLessons::default();
        let use_case = GetStudentActivity::new(
            accounts().await,
            bookings.clone(),
            lessons.clone(),
            RecordingCoaches::default(),
            ManualClock::new(at(60)),
        );

        // 9 はない ID、2 は本部、7 はコーチ
        for id in [9, 2, 7] {
            assert!(matches!(
                use_case.execute(AccountId::from_u128(id)).await,
                Err(GetStudentActivityError::StudentNotFound)
            ));
        }
        assert!(bookings.asked.lock().unwrap().is_empty());
        assert!(lessons.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unavailable_lookup_is_an_error() {
        let use_case = GetStudentActivity::new(
            accounts().await,
            FakeBookings::default(),
            Broken,
            RecordingCoaches::default(),
            ManualClock::new(at(60)),
        );
        assert!(matches!(
            use_case.execute(STUDENT).await,
            Err(GetStudentActivityError::Unavailable(_))
        ));
    }
}
