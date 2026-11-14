use std::collections::BTreeSet;

use domain::{
    identity::AccountId,
    scheduling::{BookingStatus, NextAction},
};

use super::{BookingCoachDisplayNameLookup, BookingRepository, MyNextActions};
use crate::{
    Unavailable, async_lesson::VideoRepository, clock::Clock, entitlement::EntitlementRepository,
    lesson::LessonCompletionRepository, live_lesson::LiveLessonSessionRepository,
};

/// 生徒の次の操作のユースケース（student-app の `getMyNextActions`。生徒だけが呼ぶ。ADR 0010）。
///
/// 生徒 `student_id` の予約と予約した枠・予約が結び付いた通話セッション・送った動画・完了記録・受講権の有無を読み、
/// 今（[`Clock`]）の時点の次の操作を domain の [`NextAction::decide`] で決める（決め方はそこだけ）。通話セッションは、
/// 確定済みで枠の開始時刻を過ぎた予約のものだけを読む（セッションは確定した予約の枠の開始からしか始められない）。
/// 完了したかは完了記録の保存先だけで決める（ADR 0006）。コーチの表示名は profile に問い合わせて
/// （[`BookingCoachDisplayNameLookup`]）添える。読むのは本人の分だけで、何も変えない（ビデオ通話の枠も消費しない）。
pub struct GetMyNextActions<B, L, W, R, E, D, C> {
    bookings: B,
    sessions: L,
    videos: W,
    completions: R,
    entitlements: E,
    coaches: D,
    clock: C,
}

impl<B, L, W, R, E, D, C> GetMyNextActions<B, L, W, R, E, D, C>
where
    B: BookingRepository,
    L: LiveLessonSessionRepository,
    W: VideoRepository,
    R: LessonCompletionRepository,
    E: EntitlementRepository,
    D: BookingCoachDisplayNameLookup,
    C: Clock,
{
    pub fn new(
        bookings: B,
        sessions: L,
        videos: W,
        completions: R,
        entitlements: E,
        coaches: D,
        clock: C,
    ) -> Self {
        Self {
            bookings,
            sessions,
            videos,
            completions,
            entitlements,
            coaches,
            clock,
        }
    }

    pub async fn execute(&self, student_id: AccountId) -> Result<MyNextActions, Unavailable> {
        let now = self.clock.now();
        let bookings = self.bookings.bookings_of_student(student_id).await?;
        let mut sessions = Vec::new();
        for (booking, slot) in &bookings {
            if booking.status() != BookingStatus::Confirmed || slot.starts_at() > now {
                continue;
            }
            if let Some(session) = self.sessions.find_by_booking(booking.id()).await? {
                sessions.push(session);
            }
        }
        let videos = self.videos.videos_of_student(student_id).await?;
        let completions = self.completions.completions_of_student(student_id).await?;
        let has_entitlement = self
            .entitlements
            .find_by_student(student_id)
            .await?
            .is_some();

        let actions = NextAction::decide(
            student_id,
            &bookings,
            &sessions,
            &videos,
            &completions,
            has_entitlement,
            now,
        );
        let coach_ids: BTreeSet<AccountId> =
            actions.iter().filter_map(NextAction::coach_id).collect();
        let coaches = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.coaches.find_coach_display_names(&coach_ids).await?
        };
        Ok(MyNextActions { actions, coaches })
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
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
        entitlement::{
            Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan,
            YearMonth,
        },
        lesson::{LessonCompletion, LessonCompletionId, LessonSource},
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
        profile::DisplayName,
        scheduling::{Booking, Slot, SlotCapacity, SlotId},
    };

    use super::*;
    use crate::in_memory::{
        FixedClock, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemorySlotRepository, InMemoryVideoRepository,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    /// 表示名が引けないコーチ
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hour * 3600)
    }

    fn slot(id: u128, coach: AccountId, hour: u64) -> Slot {
        Slot::publish(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            SlotCapacity::new(1).unwrap(),
        )
        .unwrap()
    }

    /// `COACH` の表示名だけを知っていて、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingNames {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl BookingCoachDisplayNameLookup for RecordingNames {
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

    struct Records {
        bookings: InMemoryBookingRepository,
        sessions: InMemoryLiveLessonSessionRepository,
        videos: InMemoryVideoRepository,
        completions: InMemoryLessonCompletionRepository,
        entitlements: InMemoryEntitlementRepository,
    }

    impl Records {
        fn new(slots: impl IntoIterator<Item = Slot>) -> Self {
            Self {
                bookings: InMemoryBookingRepository::new(InMemorySlotRepository::with_slots(slots)),
                sessions: InMemoryLiveLessonSessionRepository::default(),
                videos: InMemoryVideoRepository::default(),
                completions: InMemoryLessonCompletionRepository::default(),
                entitlements: InMemoryEntitlementRepository::default(),
            }
        }

        async fn book(&self, student: AccountId, slot: u128, confirm: bool) -> Booking {
            let booking = self
                .bookings
                .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                    Booking::reserve(
                        self.bookings.next_id(),
                        student,
                        false,
                        slot,
                        existing,
                        at(0),
                    )
                })
                .await
                .unwrap();
            if !confirm {
                return booking;
            }
            self.bookings
                .update(booking.id(), |booking, slot| {
                    booking.confirm(student, slot, at(0))
                })
                .await
                .unwrap()
        }

        async fn use_case(
            self,
            names: RecordingNames,
            hour: u64,
            student: AccountId,
        ) -> MyNextActions {
            GetMyNextActions::new(
                self.bookings,
                self.sessions,
                self.videos,
                self.completions,
                self.entitlements,
                names,
                FixedClock(at(hour)),
            )
            .execute(student)
            .await
            .unwrap()
        }
    }

    fn ended_session(id: u128, booking: &Booking, hour: u64) -> LiveLessonSession {
        let mut session = LiveLessonSession::start(
            SessionId::from_u128(id),
            booking.coach_id(),
            booking,
            &[],
            None,
            at(hour),
        )
        .unwrap();
        session
            .end(
                booking.coach_id(),
                at(hour + 1),
                ConnectionQuality::new(0.0, 0, 50, 0).unwrap(),
            )
            .unwrap();
        session
    }

    fn video(id: u128, student: AccountId, coach: AccountId, returned: bool) -> Video {
        let mut video = Video::submit(VideoId::from_u128(id), student, coach, Sport::Tennis, at(0));
        if returned {
            video
                .return_review(
                    VideoReviewId::from_u128(id),
                    coach,
                    ReviewComment::new("よい").unwrap(),
                    true,
                    at(1),
                )
                .unwrap();
        }
        video
    }

    fn monthly(student: AccountId) -> Entitlement {
        Entitlement::monthly(
            EntitlementId::from_u128(100),
            student,
            Sport::Tennis,
            SubscriptionId::from_u128(200),
            LiveLessonQuota::restore(
                SubscriptionPlan::Standard,
                YearMonth::new(2026, 10).unwrap(),
                [],
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn kinds(actions: &MyNextActions) -> Vec<&'static str> {
        actions
            .actions
            .iter()
            .map(|action| match action {
                NextAction::ConfirmBooking { .. } => "confirm_booking",
                NextAction::JoinLiveLesson { .. } => "join_live_lesson",
                NextAction::RecordLiveLessonCompletion { .. }
                | NextAction::RecordAsyncLessonCompletion { .. } => "record_lesson_completion",
                NextAction::VideoReviewInProgress { .. } => "video_review_in_progress",
                NextAction::StartSubscription => "start_subscription",
            })
            .collect()
    }

    #[tokio::test]
    async fn reads_the_students_records_and_decides_in_the_contract_order() {
        let records = Records::new([
            slot(1, COACH, 100),
            slot(2, OTHER_COACH, 10),
            slot(3, COACH, 200),
        ]);
        records.book(STUDENT, 1, false).await;
        let finished = records.book(STUDENT, 2, true).await;
        records
            .sessions
            .insert(ended_session(50, &finished, 10))
            .await
            .unwrap();
        records.book(STUDENT, 3, true).await;
        records
            .videos
            .insert(video(60, STUDENT, COACH, false))
            .await
            .unwrap();

        let names = RecordingNames::default();
        let actions = records.use_case(names.clone(), 20, STUDENT).await;
        assert_eq!(
            kinds(&actions),
            [
                "confirm_booking",
                "record_lesson_completion",
                "join_live_lesson"
            ],
            "3 件まで（添削待ちは 4 件目で出ない）"
        );
        assert_eq!(
            actions.coaches,
            BTreeMap::from([(COACH, DisplayName::new("たなか").unwrap())]),
            "表示名が引けないコーチ（OTHER_COACH）は含まない"
        );
        assert_eq!(
            *names.asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "出す操作のコーチを重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn completion_records_and_entitlement_come_from_the_server() {
        let records = Records::new([]);
        records
            .videos
            .insert(video(60, STUDENT, COACH, true))
            .await
            .unwrap();
        let first = records
            .use_case(RecordingNames::default(), 2, STUDENT)
            .await;
        assert_eq!(kinds(&first), ["record_lesson_completion"]);

        let records = Records::new([]);
        records
            .videos
            .insert(video(60, STUDENT, COACH, true))
            .await
            .unwrap();
        records
            .completions
            .record(STUDENT, |history| {
                Ok(LessonCompletion::restore(
                    LessonCompletionId::from_u128(1),
                    STUDENT,
                    COACH,
                    LessonSource::Async(VideoId::from_u128(60)),
                    at(2),
                    history.is_empty(),
                ))
            })
            .await
            .unwrap();
        let recorded = records
            .use_case(RecordingNames::default(), 3, STUDENT)
            .await;
        assert_eq!(kinds(&recorded), ["start_subscription"]);
        assert!(recorded.coaches.is_empty());

        let mut records = Records::new([]);
        records.entitlements = InMemoryEntitlementRepository::with_entitlements([monthly(STUDENT)]);
        records
            .completions
            .record(STUDENT, |_| {
                Ok(LessonCompletion::restore(
                    LessonCompletionId::from_u128(1),
                    STUDENT,
                    COACH,
                    LessonSource::Async(VideoId::from_u128(60)),
                    at(2),
                    true,
                ))
            })
            .await
            .unwrap();
        assert!(
            kinds(
                &records
                    .use_case(RecordingNames::default(), 3, STUDENT)
                    .await
            )
            .is_empty(),
            "受講権があれば月額契約を勧めない"
        );
    }

    #[tokio::test]
    async fn other_students_records_are_not_returned() {
        let records = Records::new([slot(1, COACH, 100), slot(2, COACH, 10)]);
        records.book(OTHER_STUDENT, 1, false).await;
        let finished = records.book(OTHER_STUDENT, 2, true).await;
        records
            .sessions
            .insert(ended_session(50, &finished, 10))
            .await
            .unwrap();
        records
            .videos
            .insert(video(60, OTHER_STUDENT, COACH, false))
            .await
            .unwrap();
        let names = RecordingNames::default();
        let actions = records.use_case(names.clone(), 20, STUDENT).await;
        assert!(actions.actions.is_empty());
        assert!(
            names.asked.lock().unwrap().is_empty(),
            "操作がなければ表示名を問い合わせない"
        );
    }
}
