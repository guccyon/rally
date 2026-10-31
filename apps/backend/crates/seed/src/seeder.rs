use std::fmt::Debug;

use application::{
    analytics::AnalyticsSink,
    async_lesson::{ReturnVideoReview, ReturnVideoReviewCommand, SubmitVideo, SubmitVideoCommand},
    catalog::CatalogLessonMenuLookup,
    entitlement::{
        EntitlementQuotaGate, EntitlementReviewGate, LiveLessonQuotaSubscriber, StartSubscription,
        StartSubscriptionCommand,
    },
    identity::{RegisterAccount, RegisterAccountCommand},
    in_memory::ManualClock,
    lesson::{RecordLessonCompletion, RecordLessonCompletionCommand},
    live_lesson::{
        EndLiveLessonSession, EndLiveLessonSessionCommand, StartLiveLessonSession,
        StartLiveLessonSessionCommand,
    },
    scheduling::{
        CancelBooking, CancelBookingCommand, ConfirmBooking, ConfirmBookingCommand, CreateBooking,
        CreateBookingCommand, PublishSlot, PublishSlotCommand, SchedulingBookingLookup,
    },
};
use domain::{
    async_lesson::{ReviewComment, VideoContentType, VideoId},
    entitlement::SubscriptionPlan,
    identity::{AccountId, Role},
    lesson::LessonSource,
    live_lesson::SessionId,
    scheduling::{BookingId, SlotCapacity, SlotId},
};
use infra::{
    InMemoryDomainEventBus, MockAiFeedbackGenerator, MockVideoCallProvider, MockVideoStorage,
    PgAccountRepository, PgBookingRepository, PgEntitlementRepository,
    PgLessonCompletionRepository, PgLessonMenuRepository, PgLiveLessonSessionRepository,
    PgSlotRepository, PgVideoRepository,
};
use sqlx::PgPool;

use crate::{SeedError, SeedPlan, Step};

/// コーチが返す添削のコメント（返した順に繰り返す）。
const REVIEW_COMMENTS: [&str; 4] = [
    "テイクバックが早いので、ボールを引きつけてから振り始めましょう",
    "軸足の位置がよくなりました。次は体重移動を意識してみてください",
    "フォロースルーで顔が早く上がっています。打点を見たまま振り切りましょう",
    "フォームは安定しています。同じリズムで 10 本続けて撮ってみてください",
];

/// [`SeedPlan`] を application のユースケースに流し、Postgres に書く。
///
/// 保存先は infra の Postgres の実装、境界間イベントは infra のインメモリのイベントバス（entitlement の
/// 通話の枠の消費を同じプロセスで購読する）、通話基盤はモック（計画の `seed` で乱数を固定する）、時計は
/// 出来事ごとに計画の時刻に合わせる。行動イベントは `analytics` に発火する（不変条件も発火もユースケースの
/// とおりで、迂回しない）。
pub struct Seeder<A> {
    pool: PgPool,
    analytics: A,
}

/// 生徒の直近の枠・予約・通話・動画（[`Step`] が指す相手）。
#[derive(Default)]
struct Current {
    slot: Option<SlotId>,
    booking: Option<BookingId>,
    session: Option<SessionId>,
    video: Option<VideoId>,
}

impl<A> Seeder<A>
where
    A: AnalyticsSink + Clone,
{
    pub fn new(pool: PgPool, analytics: A) -> Self {
        Self { pool, analytics }
    }

    /// 計画のすべての出来事を書く。空の DB（マイグレーションを当てたもの）を前提にし、どれか 1 つでも
    /// ユースケースが失敗すれば止める。
    pub async fn run(&self, plan: &SeedPlan) -> Result<(), SeedError> {
        let pool = &self.pool;
        let analytics = &self.analytics;
        let clock = ManualClock::new(plan.now);
        let bookings = PgBookingRepository::new(pool.clone());
        let entitlements = PgEntitlementRepository::new(pool.clone());
        let sessions = PgLiveLessonSessionRepository::new(pool.clone());
        let videos = PgVideoRepository::new(pool.clone());
        let events =
            InMemoryDomainEventBus::new(LiveLessonQuotaSubscriber::new(entitlements.clone()));
        let video_calls = MockVideoCallProvider::with_seed(plan.seed);

        let register = RegisterAccount::new(
            PgAccountRepository::new(pool.clone()),
            analytics.clone(),
            clock.clone(),
        );
        let publish = PublishSlot::new(
            PgSlotRepository::new(pool.clone()),
            CatalogLessonMenuLookup::new(PgLessonMenuRepository::new(pool.clone())),
            analytics.clone(),
            clock.clone(),
        );
        let create = CreateBooking::new(
            bookings.clone(),
            CatalogLessonMenuLookup::new(PgLessonMenuRepository::new(pool.clone())),
            analytics.clone(),
            clock.clone(),
        );
        let confirm = ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(entitlements.clone()),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        );
        let cancel = CancelBooking::new(
            bookings.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        );
        let subscribe = StartSubscription::new(
            entitlements.clone(),
            analytics.clone(),
            events.clone(),
            clock.clone(),
        );
        let start_call = StartLiveLessonSession::new(
            sessions.clone(),
            SchedulingBookingLookup::new(bookings.clone()),
            video_calls.clone(),
            analytics.clone(),
            clock.clone(),
        );
        let end_call = EndLiveLessonSession::new(
            sessions.clone(),
            video_calls,
            analytics.clone(),
            clock.clone(),
        );
        let submit = SubmitVideo::new(
            videos.clone(),
            CatalogLessonMenuLookup::new(PgLessonMenuRepository::new(pool.clone())),
            MockVideoStorage,
            MockAiFeedbackGenerator,
            analytics.clone(),
            events.clone(),
            clock.clone(),
        );
        let review = ReturnVideoReview::new(
            videos.clone(),
            EntitlementReviewGate,
            analytics.clone(),
            events.clone(),
            clock.clone(),
        );
        let complete = RecordLessonCompletion::new(
            sessions,
            videos,
            PgLessonCompletionRepository::new(pool.clone()),
            analytics.clone(),
            events,
            clock.clone(),
        );

        clock.set(plan.coaches_registered_at());
        let mut coach_ids: Vec<AccountId> = Vec::with_capacity(plan.coaches.len());
        for (i, coach) in plan.coaches.iter().enumerate() {
            let account = register
                .execute(RegisterAccountCommand {
                    email: format!("coach{:02}.seed{}@example.com", i + 1, plan.seed),
                    display_name: format!("コーチ {:02}", i + 1),
                    role: Role::Coach,
                    sports: vec![coach.sport],
                })
                .await
                .map_err(|e| failed(&format!("コーチ {}", i + 1), "登録", e))?;
            coach_ids.push(account.id());
        }

        let publish_slot = async |step: &Step| -> Result<SlotId, SeedError> {
            let Step::PublishSlot { slot, .. } = step else {
                unreachable!("枠の公開だけを渡す");
            };
            let published = publish
                .execute(PublishSlotCommand {
                    coach_id: coach_ids[slot.coach],
                    sport: plan.coaches[slot.coach].sport,
                    starts_at: slot.starts_at,
                    ends_at: slot.ends_at,
                    // 合成シードの枠はマンツーマン（定員 1）
                    capacity: SlotCapacity::default(),
                    // 合成シードの枠はレッスンメニューに結びつけない
                    lesson_menu_id: None,
                })
                .await
                .map_err(|e| failed(&format!("コーチ {}", slot.coach + 1), "枠の公開", e))?;
            Ok(published.id())
        };

        let mut returned_reviews = 0usize;
        for (i, student) in plan.students.iter().enumerate() {
            let who = format!("生徒 {:04}", i + 1);
            let coach_id = coach_ids[student.coach];
            let sport = plan.coaches[student.coach].sport;
            let mut student_id: Option<AccountId> = None;
            let mut current = Current::default();
            for step in &student.steps {
                clock.set(step.at());
                // 枠の公開はコーチの出来事なので、生徒の登録より前でもよい
                if let Step::PublishSlot { .. } = step {
                    current.slot = Some(publish_slot(step).await?);
                    continue;
                }
                if let Step::Register { .. } = step {
                    let account = register
                        .execute(RegisterAccountCommand {
                            email: format!("student{:04}.seed{}@example.com", i + 1, plan.seed),
                            display_name: who.clone(),
                            role: Role::Student,
                            sports: vec![sport],
                        })
                        .await
                        .map_err(|e| failed(&who, "登録", e))?;
                    student_id = Some(account.id());
                    continue;
                }
                let student_id = student_id
                    .ok_or_else(|| SeedError::new(format!("{who}: 登録より前の出来事")))?;
                let missing =
                    |what: &str| SeedError::new(format!("{who}: {what}がないのに {step:?}"));
                match *step {
                    Step::Register { .. } | Step::PublishSlot { .. } => unreachable!("上で扱った"),
                    Step::Book { is_trial, .. } => {
                        let booking = create
                            .execute(CreateBookingCommand {
                                student_id,
                                slot_id: current.slot.ok_or_else(|| missing("枠"))?,
                                is_trial,
                            })
                            .await
                            .map_err(|e| failed(&who, "予約", e))?;
                        current.booking = Some(booking.id());
                    }
                    Step::Confirm { .. } => {
                        confirm
                            .execute(ConfirmBookingCommand {
                                student_id,
                                booking_id: current.booking.ok_or_else(|| missing("予約"))?,
                            })
                            .await
                            .map_err(|e| failed(&who, "予約の確定", e))?;
                    }
                    Step::Cancel { .. } => {
                        cancel
                            .execute(CancelBookingCommand {
                                student_id,
                                booking_id: current.booking.ok_or_else(|| missing("予約"))?,
                            })
                            .await
                            .map_err(|e| failed(&who, "予約のキャンセル", e))?;
                    }
                    Step::StartCall { .. } => {
                        let started = start_call
                            .execute(StartLiveLessonSessionCommand {
                                account_id: student_id,
                                booking_id: current.booking.ok_or_else(|| missing("予約"))?,
                            })
                            .await
                            .map_err(|e| failed(&who, "通話の開始", e))?;
                        current.session = Some(started.session.id());
                    }
                    Step::EndCall { .. } => {
                        end_call
                            .execute(EndLiveLessonSessionCommand {
                                account_id: coach_id,
                                session_id: current.session.ok_or_else(|| missing("通話"))?,
                            })
                            .await
                            .map_err(|e| failed(&who, "通話の終了", e))?;
                    }
                    Step::CompleteCall { by_coach, .. } => {
                        complete
                            .execute(RecordLessonCompletionCommand {
                                account_id: if by_coach { coach_id } else { student_id },
                                source: LessonSource::Live(
                                    current.session.ok_or_else(|| missing("通話"))?,
                                ),
                            })
                            .await
                            .map_err(|e| failed(&who, "通話の完了の記録", e))?;
                    }
                    Step::Subscribe { .. } => {
                        subscribe
                            .execute(StartSubscriptionCommand {
                                student_id,
                                sport,
                                // 合成シードの契約は既定のプラン（暦月 2 回）
                                plan: SubscriptionPlan::default(),
                            })
                            .await
                            .map_err(|e| failed(&who, "月額契約の開始", e))?;
                    }
                    Step::SubmitVideo { .. } => {
                        let submitted = submit
                            .execute(SubmitVideoCommand {
                                student_id,
                                coach_id,
                                sport,
                                content_type: VideoContentType::Mp4,
                                note: None,
                                // 合成シードの動画はメニューを選ばない（今までどおり）
                                lesson_menu_id: None,
                            })
                            .await
                            .map_err(|e| failed(&who, "動画の送付", e))?;
                        current.video = Some(submitted.video.id());
                    }
                    Step::ReturnReview { .. } => {
                        let comment = REVIEW_COMMENTS[returned_reviews % REVIEW_COMMENTS.len()];
                        returned_reviews += 1;
                        review
                            .execute(ReturnVideoReviewCommand {
                                coach_id,
                                video_id: current.video.ok_or_else(|| missing("動画"))?,
                                comment: ReviewComment::new(comment)
                                    .map_err(|e| failed(&who, "添削のコメント", e))?,
                            })
                            .await
                            .map_err(|e| failed(&who, "添削の返却", e))?;
                    }
                    Step::CompleteVideo { by_coach, .. } => {
                        complete
                            .execute(RecordLessonCompletionCommand {
                                account_id: if by_coach { coach_id } else { student_id },
                                source: LessonSource::Async(
                                    current.video.ok_or_else(|| missing("動画"))?,
                                ),
                            })
                            .await
                            .map_err(|e| failed(&who, "動画の完了の記録", e))?;
                    }
                }
            }
        }

        for step in &plan.open_slots {
            clock.set(step.at());
            publish_slot(step).await?;
        }
        Ok(())
    }
}

fn failed(who: &str, what: &str, error: impl Debug) -> SeedError {
    SeedError::new(format!("{who} の{what}に失敗: {error:?}"))
}
