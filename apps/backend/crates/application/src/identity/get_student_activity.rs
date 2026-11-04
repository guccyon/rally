use std::collections::BTreeSet;

use domain::identity::AccountId;

use super::{
    AccountRepository, StudentActivity, StudentActivityCoachDisplayNameLookup,
    StudentBookingActivityLookup, StudentLessonHistoryLookup, StudentLowRatedReviewCountLookup,
    StudentUnansweredMessageThreadCountLookup, StudentVideoActivityLookup,
};
use crate::{Unavailable, clock::Clock};

/// 本部が見る生徒 1 人の利用の様子のユースケース（admin-web の `getStudentActivity`。本部だけが呼ぶ）。照会だけで、
/// 何も保存しない。
///
/// 生徒のアカウントの表示名に、これからの予約とキャンセルした予約の件数を scheduling に
/// （[`StudentBookingActivityLookup`]。今は [`Clock`] から読む）、直近の受けたレッスンを完了記録に
/// （[`StudentLessonHistoryLookup`]。最大 [`Self::RECENT_LESSON_LIMIT`] 件）、送った動画の数と今の返却待ちの数を
/// async-lesson に（[`StudentVideoActivityLookup`]。今は [`Clock`] から読む）、返信のないまま既定の時間の過ぎたスレッドの数を
/// messaging に（[`StudentUnansweredMessageThreadCountLookup`]。`listUnansweredMessageThreads` の既定の条件と同じで、数え方は
/// domain の [`domain::messaging::StudentUnansweredMessageThreadCount`]。今は [`Clock`] から読む）、その生徒がつけた直近の低い評価の
/// 件数と、そのうち本部が未対応の件数を review に（[`StudentLowRatedReviewCountLookup`]。`listLowRatedLessonReviews` に
/// `student_id` だけを付けたときと同じで、数え方は domain の [`domain::review::StudentLowRatedReviewCount`]。今は [`Clock`]
/// から読む）問い合わせて添え、予約とレッスンの
/// コーチの表示名を profile に問い合わせて（[`StudentActivityCoachDisplayNameLookup`]）添える。誰が呼べるか（本部だけ）は
/// 境界（interface）が確かめる。
pub struct GetStudentActivity<R, B, L, V, U, W, D, C> {
    accounts: R,
    bookings: B,
    lessons: L,
    videos: V,
    messages: U,
    reviews: W,
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

impl<R, B, L, V, U, W, D, C> GetStudentActivity<R, B, L, V, U, W, D, C>
where
    R: AccountRepository,
    B: StudentBookingActivityLookup,
    L: StudentLessonHistoryLookup,
    V: StudentVideoActivityLookup,
    U: StudentUnansweredMessageThreadCountLookup,
    W: StudentLowRatedReviewCountLookup,
    D: StudentActivityCoachDisplayNameLookup,
    C: Clock,
{
    /// 直近の受けたレッスンの件数の上限（admin-web の契約の `StudentActivity.recent_lessons` の `maxItems`）。
    pub const RECENT_LESSON_LIMIT: usize = 20;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        accounts: R,
        bookings: B,
        lessons: L,
        videos: V,
        messages: U,
        reviews: W,
        coaches: D,
        clock: C,
    ) -> Self {
        Self {
            accounts,
            bookings,
            lessons,
            videos,
            messages,
            reviews,
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
        let now = self.clock.now();
        let bookings = self.bookings.find_booking_activity(student_id, now).await?;
        let recent_lessons = self
            .lessons
            .find_recent_lessons(student_id, Self::RECENT_LESSON_LIMIT)
            .await?;
        let videos = self.videos.find_video_activity(student_id, now).await?;
        let unanswered_message_threads = self
            .messages
            .find_unanswered_message_thread_count(student_id, now)
            .await?;
        let low_rated_reviews = self
            .reviews
            .find_low_rated_review_count(student_id, now)
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
            submitted_video_count: videos.submitted_count,
            pending_video_review_count: videos.pending_review_count,
            unanswered_message_thread_count: unanswered_message_threads.unanswered_count(),
            low_rated_review_count: low_rated_reviews.low_rated_count(),
            unhandled_low_rated_review_count: low_rated_reviews.unhandled_count(),
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
        messaging::{
            MessageBody, MessageContent, MessageId, MessageThread, MessageThreadId,
            StudentUnansweredMessageThreadCount,
        },
        profile::DisplayName,
        review::{
            LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating,
            StudentLowRatedReviewCount,
        },
        scheduling::{Booking, BookingId, BookingStatus, Slot, SlotId, SlotSeats, SlotStatus},
    };

    use super::*;
    use crate::{
        identity::{StudentBookingActivity, StudentVideoActivity},
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

    /// 問い合わせ（生徒と時刻）を記録し、決まった動画添削の様子を返す。
    #[derive(Clone, Default)]
    struct FakeVideos {
        activity: StudentVideoActivity,
        asked: Arc<Mutex<Vec<(AccountId, SystemTime)>>>,
    }

    impl StudentVideoActivityLookup for FakeVideos {
        async fn find_video_activity(
            &self,
            student_id: AccountId,
            now: SystemTime,
        ) -> Result<StudentVideoActivity, Unavailable> {
            self.asked.lock().unwrap().push((student_id, now));
            Ok(self.activity)
        }
    }

    /// 問い合わせ（生徒と時刻）を記録し、決まったスレッドを domain の
    /// [`StudentUnansweredMessageThreadCount::tally`] で数える。
    #[derive(Clone, Default)]
    struct FakeMessages {
        threads: Vec<MessageThread>,
        asked: Arc<Mutex<Vec<(AccountId, SystemTime)>>>,
    }

    impl StudentUnansweredMessageThreadCountLookup for FakeMessages {
        async fn find_unanswered_message_thread_count(
            &self,
            student_id: AccountId,
            now: SystemTime,
        ) -> Result<StudentUnansweredMessageThreadCount, Unavailable> {
            self.asked.lock().unwrap().push((student_id, now));
            Ok(StudentUnansweredMessageThreadCount::tally(
                student_id,
                &self.threads,
                now,
            ))
        }
    }

    /// 問い合わせ（生徒と時刻）を記録し、決まった評価を domain の [`StudentLowRatedReviewCount::tally`] で数える。
    #[derive(Clone, Default)]
    struct FakeReviews {
        reviews: Vec<LessonReview>,
        asked: Arc<Mutex<Vec<(AccountId, SystemTime)>>>,
    }

    impl StudentLowRatedReviewCountLookup for FakeReviews {
        async fn find_low_rated_review_count(
            &self,
            student_id: AccountId,
            now: SystemTime,
        ) -> Result<StudentLowRatedReviewCount, Unavailable> {
            self.asked.lock().unwrap().push((student_id, now));
            Ok(StudentLowRatedReviewCount::tally(
                student_id,
                &self.reviews,
                now,
            ))
        }
    }

    /// 生徒 `STUDENT` が `coach` に `submitted_at` に点 `rating` で付けた評価 `id`。
    fn review(id: u128, coach: AccountId, rating: i64, submitted_at: SystemTime) -> LessonReview {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            STUDENT,
            coach,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("本部には見せない本文").unwrap(),
            submitted_at,
        )
    }

    /// 生徒 `STUDENT` と `coach` のスレッド `id` を開き、生徒が `hour` 時に本文を送ったまま返信がない。
    fn waiting_thread(id: u128, coach: AccountId, hour: u64) -> MessageThread {
        let mut thread =
            MessageThread::open(MessageThreadId::from_u128(id), STUDENT, coach, at(0)).unwrap();
        thread
            .send(
                MessageId::from_u128(id),
                STUDENT,
                MessageContent::Text(MessageBody::new("返事をください").unwrap()),
                true,
                at(hour),
            )
            .unwrap();
        thread
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

    impl StudentVideoActivityLookup for Broken {
        async fn find_video_activity(
            &self,
            _student_id: AccountId,
            _now: SystemTime,
        ) -> Result<StudentVideoActivity, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }
    }

    impl StudentUnansweredMessageThreadCountLookup for Broken {
        async fn find_unanswered_message_thread_count(
            &self,
            _student_id: AccountId,
            _now: SystemTime,
        ) -> Result<StudentUnansweredMessageThreadCount, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }
    }

    impl StudentLowRatedReviewCountLookup for Broken {
        async fn find_low_rated_review_count(
            &self,
            _student_id: AccountId,
            _now: SystemTime,
        ) -> Result<StudentLowRatedReviewCount, Unavailable> {
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
    async fn returns_display_name_bookings_lessons_videos_and_coach_names() {
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
        let videos = FakeVideos {
            activity: StudentVideoActivity {
                submitted_count: 4,
                pending_review_count: 2,
            },
            ..Default::default()
        };
        // 今（60 時）から 36 時間前に送った 1 件は数え、24 時間に満たない 50 時の 1 件は数えない
        let messages = FakeMessages {
            threads: vec![
                waiting_thread(50, COACH, 24),
                waiting_thread(51, OTHER_COACH, 50),
            ],
            ..Default::default()
        };
        // 今（60 時）から 30 日の中の ★1 と ★2（対応済み）を数え、★3 と 30 日より前に付けた ★1 は数えない
        let reviews = FakeReviews {
            reviews: vec![
                review(70, COACH, 1, at(40)),
                review(71, OTHER_COACH, 2, at(50)).with_handling(Some(
                    LowRatedReviewHandling::new(AccountId::from_u128(2), at(55)),
                )),
                review(72, COACH, 3, at(50)),
                review(
                    73,
                    COACH,
                    1,
                    at(60) - Duration::from_secs(30 * 24 * 3600 + 1),
                ),
            ],
            ..Default::default()
        };
        let coaches = RecordingCoaches::default();
        let use_case = GetStudentActivity::new(
            accounts().await,
            bookings.clone(),
            lessons.clone(),
            videos.clone(),
            messages.clone(),
            reviews.clone(),
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
                submitted_video_count: 4,
                pending_video_review_count: 2,
                unanswered_message_thread_count: 1,
                low_rated_review_count: 2,
                unhandled_low_rated_review_count: 1,
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
            *videos.asked.lock().unwrap(),
            [(STUDENT, at(60))],
            "動画添削の様子は予約と同じ今の時刻で問い合わせる"
        );
        assert_eq!(
            *messages.asked.lock().unwrap(),
            [(STUDENT, at(60))],
            "返信のないスレッドの数は予約と同じ今の時刻で問い合わせる"
        );
        assert_eq!(
            *reviews.asked.lock().unwrap(),
            [(STUDENT, at(60))],
            "低い評価の件数は予約と同じ今の時刻で問い合わせる"
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
            FakeVideos::default(),
            FakeMessages::default(),
            FakeReviews::default(),
            coaches.clone(),
            ManualClock::new(at(60)),
        );

        let activity = use_case.execute(STUDENT).await.unwrap();
        assert!(activity.upcoming_bookings.is_empty());
        assert!(activity.recent_lessons.is_empty());
        assert_eq!(activity.cancelled_booking_count, 0);
        assert_eq!(activity.submitted_video_count, 0);
        assert_eq!(activity.pending_video_review_count, 0);
        assert_eq!(activity.unanswered_message_thread_count, 0);
        assert_eq!(activity.low_rated_review_count, 0);
        assert_eq!(activity.unhandled_low_rated_review_count, 0);
        assert!(activity.coaches.is_empty());
        assert!(coaches.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn missing_account_and_non_student_are_not_found_without_asking_other_contexts() {
        let bookings = FakeBookings::default();
        let lessons = FakeLessons::default();
        let videos = FakeVideos::default();
        let messages = FakeMessages::default();
        let reviews = FakeReviews::default();
        let use_case = GetStudentActivity::new(
            accounts().await,
            bookings.clone(),
            lessons.clone(),
            videos.clone(),
            messages.clone(),
            reviews.clone(),
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
        assert!(videos.asked.lock().unwrap().is_empty());
        assert!(messages.asked.lock().unwrap().is_empty());
        assert!(reviews.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unavailable_lookup_is_an_error() {
        let use_case = GetStudentActivity::new(
            accounts().await,
            FakeBookings::default(),
            Broken,
            FakeVideos::default(),
            FakeMessages::default(),
            FakeReviews::default(),
            RecordingCoaches::default(),
            ManualClock::new(at(60)),
        );
        assert!(matches!(
            use_case.execute(STUDENT).await,
            Err(GetStudentActivityError::Unavailable(_))
        ));
        let use_case = GetStudentActivity::new(
            accounts().await,
            FakeBookings::default(),
            FakeLessons::default(),
            Broken,
            FakeMessages::default(),
            FakeReviews::default(),
            RecordingCoaches::default(),
            ManualClock::new(at(60)),
        );
        assert!(matches!(
            use_case.execute(STUDENT).await,
            Err(GetStudentActivityError::Unavailable(_))
        ));
        let use_case = GetStudentActivity::new(
            accounts().await,
            FakeBookings::default(),
            FakeLessons::default(),
            FakeVideos::default(),
            Broken,
            FakeReviews::default(),
            RecordingCoaches::default(),
            ManualClock::new(at(60)),
        );
        assert!(matches!(
            use_case.execute(STUDENT).await,
            Err(GetStudentActivityError::Unavailable(_))
        ));
        let use_case = GetStudentActivity::new(
            accounts().await,
            FakeBookings::default(),
            FakeLessons::default(),
            FakeVideos::default(),
            FakeMessages::default(),
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
