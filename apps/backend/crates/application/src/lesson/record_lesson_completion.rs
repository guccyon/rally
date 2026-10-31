use std::time::SystemTime;

use domain::{
    DomainError,
    identity::AccountId,
    lesson::{LessonCompletion, LessonSource},
    live_lesson::LiveLessonSession,
};
use uuid::Uuid;

use super::{LessonCompletionRepository, RecordCompletionError};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    async_lesson::VideoRepository,
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
    live_lesson::LiveLessonSessionRepository,
};

/// レッスンの完了記録の入力。`account_id` は認証したアカウント（元の集約の当事者かは domain が確かめる）、
/// `source` は完了したレッスンの元（同期は通話セッション、非同期は動画）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLessonCompletionCommand {
    pub account_id: AccountId,
    pub source: LessonSource,
}

#[derive(Debug)]
pub enum RecordLessonCompletionError {
    /// 元の通話セッション・動画がない
    SourceNotFound,
    /// 記録がドメインの不変条件を満たさない（当事者でない、通話が終わっていない・添削が返されていない、
    /// 記録済み、記録が終わりより前）
    Invalid(DomainError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// レッスンの完了記録のユースケース（student-app の `recordLessonCompletion`）。
///
/// 元の集約（live-lesson の通話セッション・async-lesson の動画）を読み、その集約の `complete` で完了記録を
/// 発行させて保存する。`is_first_lesson`（その生徒の完了記録が 0 件のときだけ true）は domain の
/// [`LessonCompletion`] が、保存先が渡すその生徒の既存の記録から判定する。保存したら行動イベント
/// `lesson.completed` を 1 回発火し、境界間イベント `LessonCompleted`（billing・review・analytics 向け）を
/// 1 回発行する。拒否・失敗では何も出さない。
///
/// 通話セッションの完了は、セッションに結び付いた予約（生徒）ごとに 1 つ（グループレッスン）。生徒が記録すると
/// 本人の完了だけを、コーチが記録すると結び付いた予約のうちまだ記録していないすべての生徒の完了を、予約の順に
/// 記録する（マンツーマンでは 1 人分で、今までと同じ）。イベントは記録した完了ごとに出し、返すのは最初に記録した
/// 完了。どの生徒も記録済みなら [`DomainError::LessonAlreadyCompleted`]。
pub struct RecordLessonCompletion<L, V, R, A, P, C> {
    sessions: L,
    videos: V,
    completions: R,
    analytics: A,
    events: P,
    clock: C,
}

impl<L, V, R, A, P, C> RecordLessonCompletion<L, V, R, A, P, C>
where
    L: LiveLessonSessionRepository,
    V: VideoRepository,
    R: LessonCompletionRepository,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    pub fn new(sessions: L, videos: V, completions: R, analytics: A, events: P, clock: C) -> Self {
        Self {
            sessions,
            videos,
            completions,
            analytics,
            events,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: RecordLessonCompletionCommand,
    ) -> Result<LessonCompletion, RecordLessonCompletionError> {
        let now = self.clock.now();
        let account = command.account_id;
        let recorded = match command.source {
            LessonSource::Live(session_id) => {
                let session = self
                    .sessions
                    .find(session_id)
                    .await
                    .map_err(RecordLessonCompletionError::Unavailable)?
                    .ok_or(RecordLessonCompletionError::SourceNotFound)?;
                return self.record_live(&session, account, now).await;
            }
            LessonSource::Async(video_id) => {
                let lesson_id = self.completions.next_id();
                let video = self
                    .videos
                    .find(video_id)
                    .await
                    .map_err(RecordLessonCompletionError::Unavailable)?
                    .ok_or(RecordLessonCompletionError::SourceNotFound)?;
                self.completions
                    .record(video.student_id(), |history| {
                        video.complete(lesson_id, account, history, now)
                    })
                    .await
            }
        };
        let completion = recorded.map_err(record_error)?;
        self.announce(&completion, now).await;
        Ok(completion)
    }

    /// 通話セッション `session` の完了を、`account` が生徒なら本人の分、そうでなければ（コーチ）結び付いた予約の
    /// 生徒のうち記録していない全員の分だけ記録する。
    async fn record_live(
        &self,
        session: &LiveLessonSession,
        account: AccountId,
        now: SystemTime,
    ) -> Result<LessonCompletion, RecordLessonCompletionError> {
        let students: Vec<AccountId> = session
            .bookings()
            .iter()
            .map(|booking| booking.student_id())
            .filter(|student| *student == account || account == session.coach_id())
            .collect();
        // 生徒でもコーチでもなければ、開始した予約の生徒の分を domain に判定させる（参加者でないと断る）
        let students = if students.is_empty() {
            vec![session.student_id()]
        } else {
            students
        };
        let mut first: Option<LessonCompletion> = None;
        for student in students {
            let lesson_id = self.completions.next_id();
            let recorded = self
                .completions
                .record(student, |history| {
                    session.complete_for(lesson_id, account, student, history, now)
                })
                .await;
            match recorded {
                Ok(completion) => {
                    self.announce(&completion, now).await;
                    first.get_or_insert(completion);
                }
                // 記録済みの生徒は飛ばす（コーチの記録で、まだの生徒の分だけ記録する）
                Err(RecordCompletionError::Rejected(DomainError::LessonAlreadyCompleted)) => {}
                Err(error) => return Err(record_error(error)),
            }
        }
        first.ok_or(RecordLessonCompletionError::Invalid(
            DomainError::LessonAlreadyCompleted,
        ))
    }

    /// 記録した完了の行動イベント `lesson.completed` と境界間イベント `LessonCompleted` を 1 回ずつ出す。
    async fn announce(&self, completion: &LessonCompletion, now: SystemTime) {
        self.analytics
            .emit(AnalyticsEvent::LessonCompleted {
                student_id: completion.student_id(),
                coach_id: completion.coach_id(),
                lesson_id: completion.id(),
                lesson_type: completion.lesson_type(),
                is_first_lesson: completion.is_first_lesson(),
                occurred_at: now,
            })
            .await;
        self.events
            .publish(DomainEvent::LessonCompleted {
                event_id: Uuid::new_v4(),
                occurred_at: now,
                student_id: completion.student_id(),
                coach_id: completion.coach_id(),
                lesson_id: completion.id(),
            })
            .await;
    }

    pub fn completions(&self) -> &R {
        &self.completions
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }

    pub fn events(&self) -> &P {
        &self.events
    }
}

fn record_error(error: RecordCompletionError) -> RecordLessonCompletionError {
    match error {
        RecordCompletionError::Rejected(error) => RecordLessonCompletionError::Invalid(error),
        RecordCompletionError::Unavailable(error) => {
            RecordLessonCompletionError::Unavailable(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
        lesson::LessonType,
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
        scheduling::{Booking, BookingId, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::in_memory::{
        FixedClock, InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemoryVideoRepository, RecordingAnalyticsSink, RecordingDomainEventPublisher,
    };

    type UseCase = RecordLessonCompletion<
        InMemoryLiveLessonSessionRepository,
        InMemoryVideoRepository,
        InMemoryLessonCompletionRepository,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    >;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    /// 生徒 1 の終了した通話セッション
    const ENDED_SESSION: SessionId = SessionId::from_u128(30);
    /// 生徒 1 の別の終了した通話セッション
    const SECOND_SESSION: SessionId = SessionId::from_u128(31);
    /// 生徒 1 の終了していない通話セッション
    const OPEN_SESSION: SessionId = SessionId::from_u128(32);
    /// 生徒 1 の添削を返した動画
    const REVIEWED_VIDEO: VideoId = VideoId::from_u128(40);
    /// 生徒 1 の添削をまだ返していない動画
    const UNREVIEWED_VIDEO: VideoId = VideoId::from_u128(41);
    /// 生徒 2 の添削を返した動画
    const OTHER_STUDENTS_VIDEO: VideoId = VideoId::from_u128(42);

    /// 2026-10-26T00:00:00Z（通話の開始・動画の送付）
    fn start() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    /// 完了を記録する時刻（開始の 2 時間後）
    fn now() -> SystemTime {
        start() + Duration::from_secs(2 * 3600)
    }

    fn session(id: SessionId, booking: u128, ended: bool) -> LiveLessonSession {
        let mut slot = Slot::restore(
            SlotId::from_u128(booking),
            COACH,
            Sport::Tennis,
            start(),
            start() + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(booking),
            STUDENT,
            false,
            &mut slot,
            &[],
            start(),
        )
        .unwrap();
        booking.confirm(STUDENT, &mut slot, start()).unwrap();
        let mut session =
            LiveLessonSession::start(id, STUDENT, &booking, &[], None, start()).unwrap();
        if ended {
            let quality = ConnectionQuality::new(0.0, 10, 50, 0).unwrap();
            session
                .end(COACH, start() + Duration::from_secs(3600), quality)
                .unwrap();
        }
        session
    }

    fn video(id: VideoId, student: AccountId, reviewed: bool) -> Video {
        let mut video = Video::submit(id, student, COACH, Sport::Tennis, start());
        if reviewed {
            video
                .return_review(
                    VideoReviewId::from_u128(id.as_u128()),
                    COACH,
                    ReviewComment::new("テイクバックが早い").unwrap(),
                    true,
                    start() + Duration::from_secs(3600),
                )
                .unwrap();
        }
        video
    }

    fn use_case() -> UseCase {
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::with_sessions([
                session(ENDED_SESSION, 10, true),
                session(SECOND_SESSION, 11, true),
                session(OPEN_SESSION, 12, false),
            ]),
            InMemoryVideoRepository::with_videos([
                video(REVIEWED_VIDEO, STUDENT, true),
                video(UNREVIEWED_VIDEO, STUDENT, false),
                video(OTHER_STUDENTS_VIDEO, OTHER_STUDENT, true),
            ]),
            InMemoryLessonCompletionRepository::default(),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(now()),
        )
    }

    fn command(account_id: AccountId, source: LessonSource) -> RecordLessonCompletionCommand {
        RecordLessonCompletionCommand { account_id, source }
    }

    #[tokio::test]
    async fn completing_a_live_lesson_emits_lesson_completed_and_publishes_it() {
        let use_case = use_case();

        let completion = use_case
            .execute(command(COACH, LessonSource::Live(ENDED_SESSION)))
            .await
            .unwrap();

        assert_eq!(completion.source(), LessonSource::Live(ENDED_SESSION));
        assert_eq!(
            (completion.student_id(), completion.coach_id()),
            (STUDENT, COACH)
        );
        assert_eq!(completion.completed_at(), now());
        assert!(completion.is_first_lesson());
        assert_eq!(use_case.completions().all(), vec![completion.clone()]);

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::LessonCompleted {
                student_id: STUDENT,
                coach_id: COACH,
                lesson_id: completion.id(),
                lesson_type: LessonType::Live,
                is_first_lesson: true,
                occurred_at: now(),
            }]
        );
        assert_eq!(analytics[0].name(), "lesson.completed");

        let events = use_case.events().events();
        assert_eq!(events.len(), 1);
        let DomainEvent::LessonCompleted {
            event_id,
            occurred_at,
            student_id,
            coach_id,
            lesson_id,
        } = events[0]
        else {
            panic!("LessonCompleted でない: {:?}", events[0]);
        };
        assert!(!event_id.is_nil());
        assert_eq!(
            (occurred_at, student_id, coach_id, lesson_id),
            (now(), STUDENT, COACH, completion.id())
        );
        assert_eq!(events[0].name(), "LessonCompleted");
    }

    #[tokio::test]
    async fn an_async_lesson_can_also_be_completed() {
        let use_case = use_case();

        let completion = use_case
            .execute(command(STUDENT, LessonSource::Async(REVIEWED_VIDEO)))
            .await
            .unwrap();

        assert_eq!(completion.lesson_type(), LessonType::Async);
        assert_eq!(completion.source(), LessonSource::Async(REVIEWED_VIDEO));
        assert!(completion.is_first_lesson());
        assert!(matches!(
            use_case.analytics().events()[..],
            [AnalyticsEvent::LessonCompleted {
                lesson_type: LessonType::Async,
                is_first_lesson: true,
                ..
            }]
        ));
        assert_eq!(use_case.events().events().len(), 1);
    }

    #[tokio::test]
    async fn only_the_first_completion_of_a_student_is_the_first_lesson() {
        let use_case = use_case();

        let first = use_case
            .execute(command(STUDENT, LessonSource::Async(REVIEWED_VIDEO)))
            .await
            .unwrap();
        let second = use_case
            .execute(command(STUDENT, LessonSource::Live(ENDED_SESSION)))
            .await
            .unwrap();
        let third = use_case
            .execute(command(COACH, LessonSource::Live(SECOND_SESSION)))
            .await
            .unwrap();
        // 別の生徒の初回は、生徒 1 の記録に左右されない
        let others = use_case
            .execute(command(
                OTHER_STUDENT,
                LessonSource::Async(OTHER_STUDENTS_VIDEO),
            ))
            .await
            .unwrap();

        assert_eq!(
            [&first, &second, &third, &others].map(LessonCompletion::is_first_lesson),
            [true, false, false, true]
        );
        let firsts: Vec<bool> = use_case
            .analytics()
            .events()
            .into_iter()
            .map(|event| match event {
                AnalyticsEvent::LessonCompleted {
                    is_first_lesson, ..
                } => is_first_lesson,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(firsts, [true, false, false, true]);
    }

    #[tokio::test]
    async fn rejected_completions_record_nothing_and_emit_nothing() {
        let use_case = use_case();

        for (command, expected) in [
            (
                command(OTHER_STUDENT, LessonSource::Live(ENDED_SESSION)),
                DomainError::NotASessionParticipant,
            ),
            (
                command(STUDENT, LessonSource::Live(OPEN_SESSION)),
                DomainError::LiveLessonSessionNotEnded,
            ),
            (
                command(OTHER_STUDENT, LessonSource::Async(REVIEWED_VIDEO)),
                DomainError::NotAVideoParty,
            ),
            (
                command(STUDENT, LessonSource::Async(UNREVIEWED_VIDEO)),
                DomainError::VideoReviewNotReturned,
            ),
        ] {
            let result = use_case.execute(command).await;
            assert!(
                matches!(&result, Err(RecordLessonCompletionError::Invalid(error)) if *error == expected),
                "{result:?}"
            );
        }
        for source in [
            LessonSource::Live(SessionId::from_u128(99)),
            LessonSource::Async(VideoId::from_u128(99)),
        ] {
            let result = use_case.execute(command(STUDENT, source)).await;
            assert!(
                matches!(result, Err(RecordLessonCompletionError::SourceNotFound)),
                "{result:?}"
            );
        }
        assert!(use_case.completions().all().is_empty());
        assert!(use_case.analytics().events().is_empty());
        assert!(use_case.events().events().is_empty());
    }

    #[tokio::test]
    async fn a_lesson_is_completed_only_once() {
        let use_case = use_case();
        use_case
            .execute(command(STUDENT, LessonSource::Live(ENDED_SESSION)))
            .await
            .unwrap();

        let result = use_case
            .execute(command(COACH, LessonSource::Live(ENDED_SESSION)))
            .await;
        assert!(
            matches!(
                result,
                Err(RecordLessonCompletionError::Invalid(
                    DomainError::LessonAlreadyCompleted
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.completions().all().len(), 1);
        assert_eq!(use_case.analytics().events().len(), 1);
        assert_eq!(use_case.events().events().len(), 1);
    }

    /// 定員 3 の枠の生徒 1・2・3 の確定した予約を結び付けた、終了した通話セッション（コーチが開始）。
    fn group_session(id: SessionId) -> LiveLessonSession {
        let mut slot = Slot::restore(
            SlotId::from_u128(20),
            COACH,
            Sport::Tennis,
            start(),
            start() + Duration::from_secs(3600),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::vacant(
                domain::scheduling::SlotCapacity::new(3).unwrap(),
            ),
        )
        .unwrap();
        let mut bookings: Vec<Booking> = Vec::new();
        for (i, student) in GROUP.into_iter().enumerate() {
            let mut booking = Booking::reserve(
                BookingId::from_u128(20 + i as u128),
                student,
                false,
                &mut slot,
                &bookings,
                start(),
            )
            .unwrap();
            booking.confirm(student, &mut slot, start()).unwrap();
            bookings.push(booking);
        }
        let mut session =
            LiveLessonSession::start(id, COACH, &bookings[0], &bookings, None, start()).unwrap();
        let quality = ConnectionQuality::new(0.0, 10, 50, 0).unwrap();
        session
            .end(COACH, start() + Duration::from_secs(3600), quality)
            .unwrap();
        session
    }

    const GROUP: [AccountId; 3] = [STUDENT, OTHER_STUDENT, AccountId::from_u128(3)];
    const GROUP_SESSION: SessionId = SessionId::from_u128(33);

    fn group_use_case() -> UseCase {
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::with_sessions([group_session(GROUP_SESSION)]),
            InMemoryVideoRepository::default(),
            InMemoryLessonCompletionRepository::default(),
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(now()),
        )
    }

    fn completed_students(use_case: &UseCase) -> Vec<AccountId> {
        use_case
            .completions()
            .all()
            .iter()
            .map(LessonCompletion::student_id)
            .collect()
    }

    #[tokio::test]
    async fn the_coach_completes_a_group_lesson_for_every_linked_student() {
        let use_case = group_use_case();

        let completion = use_case
            .execute(command(COACH, LessonSource::Live(GROUP_SESSION)))
            .await
            .unwrap();

        // 返すのは最初の 1 件、記録とイベントは 3 人分
        assert_eq!(completion.student_id(), GROUP[0]);
        assert_eq!(completed_students(&use_case), GROUP.to_vec());
        assert!(
            use_case
                .completions()
                .all()
                .iter()
                .all(|c| c.is_first_lesson() && c.source() == LessonSource::Live(GROUP_SESSION))
        );
        assert_eq!(use_case.analytics().events().len(), 3);
        assert_eq!(use_case.events().events().len(), 3);

        // 全員の分が記録済みなら 409（何も出さない）
        let result = use_case
            .execute(command(COACH, LessonSource::Live(GROUP_SESSION)))
            .await;
        assert!(
            matches!(
                result,
                Err(RecordLessonCompletionError::Invalid(
                    DomainError::LessonAlreadyCompleted
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.analytics().events().len(), 3);
    }

    #[tokio::test]
    async fn a_student_of_a_group_lesson_completes_only_their_own_and_the_coach_the_rest() {
        let use_case = group_use_case();

        let own = use_case
            .execute(command(GROUP[1], LessonSource::Live(GROUP_SESSION)))
            .await
            .unwrap();
        assert_eq!(own.student_id(), GROUP[1]);
        assert_eq!(completed_students(&use_case), vec![GROUP[1]]);

        // コーチはまだの 2 人の分だけを記録する
        let rest = use_case
            .execute(command(COACH, LessonSource::Live(GROUP_SESSION)))
            .await
            .unwrap();
        assert_eq!(rest.student_id(), GROUP[0]);
        assert_eq!(
            completed_students(&use_case),
            vec![GROUP[1], GROUP[0], GROUP[2]]
        );

        // 結び付いていない生徒は記録できない
        let result = use_case
            .execute(command(
                AccountId::from_u128(99),
                LessonSource::Live(GROUP_SESSION),
            ))
            .await;
        assert!(
            matches!(
                result,
                Err(RecordLessonCompletionError::Invalid(
                    DomainError::NotASessionParticipant
                ))
            ),
            "{result:?}"
        );
    }
}
